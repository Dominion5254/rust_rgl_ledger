use std::path::PathBuf;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::Connection;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::rounding_div;
use crate::models::{Acquisition, NewAcquisition, parse_date_str};
use crate::schema::acquisitions;

#[derive(Debug)]
enum TransferError {
    Diesel(diesel::result::Error),
    Custom(String),
}

impl From<diesel::result::Error> for TransferError {
    fn from(e: diesel::result::Error) -> Self {
        TransferError::Diesel(e)
    }
}

impl From<String> for TransferError {
    fn from(s: String) -> Self {
        TransferError::Custom(s)
    }
}

impl std::fmt::Display for TransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferError::Diesel(e) => write!(f, "{}", e),
            TransferError::Custom(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TransferRecord {
    date: String,
    from: String,
    to: String,
    #[serde(rename = "BTC")]
    btc: String,
}

pub fn transfer(file: &PathBuf, conn: &mut SqliteConnection) -> Result<(), String> {
    let mut rdr = csv::Reader::from_path(file)
        .map_err(|e| format!("Error reading file {:?}: {}", file, e))?;

    let mut records: Vec<(NaiveDateTime, String, String, i64)> = Vec::new();
    for result in rdr.deserialize::<TransferRecord>() {
        let record = result.map_err(|e| format!("Error parsing transfer CSV: {}", e))?;
        let date = parse_date_str(&record.date)?;
        let btc = Decimal::from_str_exact(&record.btc)
            .map_err(|e| format!("Invalid BTC amount '{}': {}", record.btc, e))?;
        let sats = (btc * Decimal::from(100_000_000i64)).round()
            .to_string().parse::<i64>()
            .map_err(|e| format!("Error converting BTC to sats: {}", e))?;
        if sats <= 0 {
            return Err(format!("Transfer BTC amount must be positive, got '{}'", record.btc));
        }
        records.push((date, record.from, record.to, sats));
    }

    records.sort_by_key(|(date, _, _, _)| date.and_utc().timestamp());

    conn.transaction::<(), TransferError, _>(|conn| {
        for (_, from_wallet, to_wallet, transfer_sats) in &records {
            let lots: Vec<Acquisition> = acquisitions::table
                .filter(acquisitions::wallet.eq(from_wallet))
                .filter(acquisitions::tax_undisposed_satoshis.gt(0))
                .order((acquisitions::acquisition_date.asc(), acquisitions::id.asc()))
                .select(Acquisition::as_select())
                .load(conn)
                .map_err(|e| format!("Error querying lots: {}", e))?;

            let total_available: i64 = lots.iter().map(|l| l.tax_undisposed_satoshis).sum();
            if total_available < *transfer_sats {
                return Err(TransferError::Custom(format!(
                    "Insufficient BTC in wallet '{}': available {} sats, need {} sats. All changes have been rolled back.",
                    from_wallet, total_available, transfer_sats
                )));
            }

            let mut remaining = *transfer_sats;

            for lot in &lots {
                if remaining == 0 {
                    break;
                }

                let tax_undisposed = lot.tax_undisposed_satoshis;

                if tax_undisposed <= remaining {
                    // Whole lot fits — just reassign wallet
                    diesel::update(acquisitions::table.find(lot.id))
                        .set(acquisitions::wallet.eq(to_wallet))
                        .execute(conn)
                        .map_err(|e| format!("Error updating lot wallet: {}", e))?;

                    remaining -= tax_undisposed;
                } else {
                    // Lot must split
                    let transfer_sats_from_lot = remaining;

                    let transferred_gaap = rounding_div(
                        lot.undisposed_satoshis as i128 * transfer_sats_from_lot as i128,
                        tax_undisposed as i128,
                    );
                    let transferred_satoshis = rounding_div(
                        lot.satoshis as i128 * transfer_sats_from_lot as i128,
                        tax_undisposed as i128,
                    );

                    // Update original lot (stays in from_wallet): reduce by transferred amounts
                    diesel::update(acquisitions::table.find(lot.id))
                        .set((
                            acquisitions::satoshis.eq(lot.satoshis - transferred_satoshis),
                            acquisitions::undisposed_satoshis.eq(lot.undisposed_satoshis - transferred_gaap),
                            acquisitions::tax_undisposed_satoshis.eq(tax_undisposed - transfer_sats_from_lot),
                        ))
                        .execute(conn)
                        .map_err(|e| format!("Error updating split lot: {}", e))?;

                    // Insert new lot (in to_wallet) with transferred amounts
                    let new_lot = NewAcquisition {
                        acquisition_date: lot.acquisition_date,
                        satoshis: transferred_satoshis,
                        undisposed_satoshis: transferred_gaap,
                        usd_cents_btc_basis: lot.usd_cents_btc_basis,
                        usd_cents_btc_fair_value: lot.usd_cents_btc_fair_value,
                        wallet: to_wallet.clone(),
                        tax_undisposed_satoshis: transfer_sats_from_lot,
                    };

                    diesel::insert_into(acquisitions::table)
                        .values(&new_lot)
                        .execute(conn)
                        .map_err(|e| format!("Error inserting split lot: {}", e))?;

                    remaining = 0;
                }
            }
        }

        Ok(())
    }).map_err(|e| e.to_string())
}
