use std::path::PathBuf;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::Connection;
use core::cmp::min;

use crate::LotConfig;
use crate::rounding_div;
use crate::models::AcquisitionDisposition;
use crate::models::{NewRecord, Acquisition, NewDisposition, NewAcquisition, Disposition};
use crate::schema::{acquisitions, dispositions, acquisition_dispositions};

#[derive(Debug)]
enum ImportError {
    Diesel(diesel::result::Error),
    Custom(String),
}

impl From<diesel::result::Error> for ImportError {
    fn from(e: diesel::result::Error) -> Self {
        ImportError::Diesel(e)
    }
}

impl From<String> for ImportError {
    fn from(s: String) -> Self {
        ImportError::Custom(s)
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Diesel(e) => write!(f, "{}", e),
            ImportError::Custom(s) => write!(f, "{}", s),
        }
    }
}

pub fn import_transactions(file: &PathBuf, conn: &mut SqliteConnection, config: &LotConfig) -> Result<(), String> {
    let mut rdr = csv::Reader::from_path(&file)
        .map_err(|e| format!("Error reading file {:?}: {}", file, e))?;

    let mut records: Vec<NewRecord> = rdr.deserialize::<NewRecord>()
        .map(|r| r.map_err(|e| format!("Error parsing CSV record: {}", e)))
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_by(|a, b| a.date.timestamp().cmp(&b.date.timestamp()));

    conn.transaction::<(), ImportError, _>(|conn| {
        for record in records {
            match record.bitcoin.gt(&0) {
                true => {
                    let new_acquisition = NewAcquisition {
                        acquisition_date: record.date,
                        satoshis: record.bitcoin,
                        undisposed_satoshis: record.bitcoin,
                        usd_cents_btc_basis: record.price,
                        usd_cents_btc_fair_value: record.price,
                        wallet: record.wallet,
                        tax_undisposed_satoshis: record.bitcoin,
                    };
                    diesel::insert_into(acquisitions::table)
                        .values(&new_acquisition)
                        .execute(conn)
                        .map_err(|e| format!("Error saving acquisition: {}", e))?;
                },
                false => {
                    let new_disposition = NewDisposition {
                        disposition_date: record.date,
                        satoshis: record.bitcoin,
                        undisposed_satoshis: record.bitcoin,
                        usd_cents_btc_basis: record.price,
                        wallet: record.wallet,
                        tax_undisposed_satoshis: record.bitcoin,
                    };
                    diesel::insert_into(dispositions::table)
                        .values(&new_disposition)
                        .execute(conn)
                        .map_err(|e| format!("Error saving disposition: {}", e))?;
                }
            }
        }

        // GAAP matching pass
        fifo_match(conn, "gaap", "universal", true)?;

        // Tax matching pass
        fifo_match(conn, "tax", &config.tax_lot_scope, false)?;

        Ok(())
    }).map_err(|e| e.to_string())
}

fn fifo_match(
    conn: &mut SqliteConnection,
    match_type: &str,
    scope: &str,
    use_fair_value: bool,
) -> Result<(), String> {
    let undisposed_disps: Vec<Disposition> = if match_type == "gaap" {
        dispositions::table
            .filter(dispositions::undisposed_satoshis.lt(0))
            .order(dispositions::disposition_date.asc())
            .select(Disposition::as_select())
            .load(conn)
            .map_err(|e| format!("Error fetching dispositions: {}", e))?
    } else {
        dispositions::table
            .filter(dispositions::tax_undisposed_satoshis.lt(0))
            .order(dispositions::disposition_date.asc())
            .select(Disposition::as_select())
            .load(conn)
            .map_err(|e| format!("Error fetching dispositions: {}", e))?
    };

    for disp_lot in undisposed_disps {
        let mut remaining = if match_type == "gaap" {
            disp_lot.undisposed_satoshis
        } else {
            disp_lot.tax_undisposed_satoshis
        };

        while remaining != 0 {
            // Build acquisition query based on scope and match_type
            let acq_lot: Acquisition = if match_type == "gaap" {
                let mut query = acquisitions::table
                    .filter(acquisitions::undisposed_satoshis.gt(0))
                    .order((acquisitions::acquisition_date.asc(), acquisitions::id.asc()))
                    .into_boxed();
                if scope == "wallet" {
                    query = query.filter(acquisitions::wallet.eq(&disp_lot.wallet));
                }
                query
                    .select(Acquisition::as_select())
                    .first(conn)
                    .optional()
                    .map_err(|e| format!("Error querying acquisition lots: {}", e))?
                    .ok_or_else(|| {
                        let scope_msg = if scope == "wallet" {
                            format!(" in wallet '{}'", disp_lot.wallet)
                        } else {
                            String::new()
                        };
                        format!(
                            "No undisposed acquisition lots available{} for {} FIFO matching. \
                             Disposition on {} for {} sats cannot be matched. \
                             All changes have been rolled back.",
                            scope_msg, match_type,
                            disp_lot.disposition_date.format("%Y-%m-%d"),
                            -disp_lot.satoshis
                        )
                    })?
            } else {
                let mut query = acquisitions::table
                    .filter(acquisitions::tax_undisposed_satoshis.gt(0))
                    .order((acquisitions::acquisition_date.asc(), acquisitions::id.asc()))
                    .into_boxed();
                if scope == "wallet" {
                    query = query.filter(acquisitions::wallet.eq(&disp_lot.wallet));
                }
                query
                    .select(Acquisition::as_select())
                    .first(conn)
                    .optional()
                    .map_err(|e| format!("Error querying acquisition lots: {}", e))?
                    .ok_or_else(|| {
                        let scope_msg = if scope == "wallet" {
                            format!(" in wallet '{}'", disp_lot.wallet)
                        } else {
                            String::new()
                        };
                        format!(
                            "No undisposed acquisition lots available{} for {} FIFO matching. \
                             Disposition on {} for {} sats cannot be matched. \
                             All changes have been rolled back.",
                            scope_msg, match_type,
                            disp_lot.disposition_date.format("%Y-%m-%d"),
                            -disp_lot.satoshis
                        )
                    })?
            };

            let acq_undisposed = if match_type == "gaap" {
                acq_lot.undisposed_satoshis
            } else {
                acq_lot.tax_undisposed_satoshis
            };

            let sats_disposed = min(-remaining, acq_undisposed);

            let price_per_btc = if use_fair_value {
                acq_lot.usd_cents_btc_fair_value
            } else {
                acq_lot.usd_cents_btc_basis
            };

            let basis: i64 = rounding_div(sats_disposed as i128 * price_per_btc as i128, 100_000_000);
            let fv_disposed_cents = rounding_div(sats_disposed as i128 * disp_lot.usd_cents_btc_basis as i128, 100_000_000);
            let rgl = fv_disposed_cents - basis;
            let term = disp_lot.disposition_date - acq_lot.acquisition_date;

            if term.num_seconds() < 0 {
                return Err(format!(
                    "Disposition on {} is before acquisition on {}. All changes have been rolled back.",
                    disp_lot.disposition_date.format("%Y-%m-%d"),
                    acq_lot.acquisition_date.format("%Y-%m-%d")
                ));
            }

            let new_acq_disp = AcquisitionDisposition {
                acquisition_id: acq_lot.id,
                disposition_id: disp_lot.id,
                match_type: match_type.to_string(),
                satoshis: sats_disposed,
                basis,
                rgl,
                term: if term.num_days() > 365 { String::from("long") } else { String::from("short") },
            };

            // Update the appropriate undisposed tracker
            if match_type == "gaap" {
                diesel::update(acquisitions::table.find(acq_lot.id))
                    .set(acquisitions::undisposed_satoshis.eq(acquisitions::undisposed_satoshis - sats_disposed))
                    .execute(conn)
                    .map_err(|e| format!("Error updating acquisition undisposed sats: {}", e))?;

                diesel::update(dispositions::table.find(disp_lot.id))
                    .set(dispositions::undisposed_satoshis.eq(dispositions::undisposed_satoshis + sats_disposed))
                    .execute(conn)
                    .map_err(|e| format!("Error updating disposition undisposed sats: {}", e))?;
            } else {
                diesel::update(acquisitions::table.find(acq_lot.id))
                    .set(acquisitions::tax_undisposed_satoshis.eq(acquisitions::tax_undisposed_satoshis - sats_disposed))
                    .execute(conn)
                    .map_err(|e| format!("Error updating acquisition tax undisposed sats: {}", e))?;

                diesel::update(dispositions::table.find(disp_lot.id))
                    .set(dispositions::tax_undisposed_satoshis.eq(dispositions::tax_undisposed_satoshis + sats_disposed))
                    .execute(conn)
                    .map_err(|e| format!("Error updating disposition tax undisposed sats: {}", e))?;
            }

            diesel::insert_into(acquisition_dispositions::table)
                .values(new_acq_disp)
                .execute(conn)
                .map_err(|e| format!("Error inserting acquisition_disposition: {}", e))?;

            remaining += sats_disposed;
        }
    }
    Ok(())
}
