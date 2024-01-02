use std::path::PathBuf;
use diesel::RunQueryDsl;

use crate::establish_connection;
use crate::schema::acquisitions::dsl::acquisitions;
use crate::schema::dispositions::dsl::dispositions;

use crate::models::{NewRecord, NewDisposition, NewAcquisition};

pub fn import_transactions(file: PathBuf) {
    let conn = &mut establish_connection();
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
                    usd_cents_btc_impaired_value: record.price,
                };
                diesel::insert_into(acquisitions)
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
                diesel::insert_into(dispositions)
                    .values(&new_disposition)
                    .execute(conn)
                    .expect(format!("Error saving acquisition: {:?}", new_disposition).as_str());
            }
        }
    }
}