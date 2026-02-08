use std::path::PathBuf;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use core::cmp::min;

use crate::models::AcquisitionDisposition;
use crate::models::{NewRecord, Acquisition, NewDisposition, NewAcquisition, Disposition};
use crate::schema::{acquisitions, dispositions, acquisition_dispositions};
use crate::schema::acquisitions::dsl::*;

pub fn import_transactions(file: &PathBuf, conn: &mut SqliteConnection) -> Result<(), String> {
    let mut rdr = csv::Reader::from_path(&file).expect(format!("Error reading file {:?}", file).as_str());

    let mut records: Vec<NewRecord> = rdr.deserialize::<NewRecord>().map(|r| r.unwrap()).collect();
    records.sort_by(|a, b| a.date.timestamp().cmp(&b.date.timestamp()));

    for record in records {
        match record.bitcoin.gt(&0) {
            true => {
                let new_acquisition = NewAcquisition {
                    acquisition_date: record.date,
                    satoshis: record.bitcoin,
                    undisposed_satoshis: record.bitcoin,
                    usd_cents_btc_basis: record.price,
                    usd_cents_btc_fair_value: record.price,
                };
                diesel::insert_into(crate::schema::acquisitions::table)
                    .values(&new_acquisition)
                    .execute(conn)
                    .expect(format!("Error saving acquisition: {:?}", new_acquisition).as_str());
            },
            false => {
                let new_disposition = NewDisposition {
                    disposition_date: record.date,
                    satoshis: record.bitcoin,
                    undisposed_satoshis: record.bitcoin,
                    usd_cents_btc_basis: record.price,
                };
                diesel::insert_into(dispositions::table)
                    .values(&new_disposition)
                    .execute(conn)
                    .expect(format!("Error saving acquisition: {:?}", new_disposition).as_str());
            }
        }
    }

    let undisposed_lots: Vec<Disposition> = dispositions::table
                                                .filter(dispositions::undisposed_satoshis.lt(0))
                                                .select(Disposition::as_select())
                                                .load(conn)
                                                .expect("Error fetching Dispositions");
    
    for disp_lot in undisposed_lots {
        let mut remaining_sat_disposition = disp_lot.undisposed_satoshis;

        while remaining_sat_disposition != 0 {
            let acq_lot: Acquisition = acquisitions::table
                                        .filter(acquisitions::undisposed_satoshis.gt(0))
                                        .select(Acquisition::as_select())
                                        .limit(1)
                                        .get_result(conn)
                                        .expect("Error fetching first undisposed acquisition lot");

            let sats_disposed = min(-remaining_sat_disposition, acq_lot.undisposed_satoshis);
            let gaap_basis: i64 = sats_disposed * acq_lot.usd_cents_btc_fair_value / 100_000_000;
            let tax_basis:i64 = sats_disposed * acq_lot.usd_cents_btc_basis / 100_000_000;
            let fv_disposed_cents = sats_disposed * disp_lot.usd_cents_btc_basis / 100_000_000;
            let gaap_rgl = fv_disposed_cents - gaap_basis;
            let tax_rgl = fv_disposed_cents - tax_basis;
            let term = disp_lot.disposition_date - acq_lot.acquisition_date;

            if term.num_seconds() < 0 {
                return Err(String::from("Disposition Date is before earliest Acquisition Date"))
            }

            let new_acq_disp = AcquisitionDisposition {
                acquisition_id: acq_lot.id,
                disposition_id: disp_lot.id,
                satoshis: sats_disposed,
                gaap_basis,
                gaap_rgl,
                tax_basis,
                tax_rgl,
                term: if term.num_days().ge(&365) { String::from("long") } else { String::from("short") }
            };

            diesel::update(acquisitions::table.find(acq_lot.id))
                .set(undisposed_satoshis.eq(undisposed_satoshis - sats_disposed))
                .execute(conn)
                .expect("Error updating Acquisition Lot Undisposed Sats");

            diesel::update(dispositions::table.find(disp_lot.id))
                .set(crate::schema::dispositions::undisposed_satoshis.eq(crate::schema::dispositions::undisposed_satoshis + sats_disposed))
                .execute(conn)
                .expect("Error updating Disposition Lot Undisposed Sats");

            diesel::insert_into(acquisition_dispositions::table)
                .values(new_acq_disp)
                .execute(conn)
                .expect("Error Inserting Acquisition Disposition");

            remaining_sat_disposition += sats_disposed;
        }
    }
    Ok(())
}