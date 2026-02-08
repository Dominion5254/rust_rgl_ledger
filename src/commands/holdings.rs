use std::path::PathBuf;

use crate::{
    models::{Acquisition, AcquisitionDisposition, Holding, HoldingsDate},
    schema::{acquisitions, dispositions},
};
use anyhow::Ok;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rust_decimal::{prelude::FromPrimitive, Decimal};
use rust_decimal_macros::dec;

pub fn holdings(date: &String, conn: &mut SqliteConnection) -> Result<(), anyhow::Error> {
    let mut holdings_date: HoldingsDate =
        serde_json::from_str(&format!(r#"{{ "date": "{}" }}"#, date))
            .expect("Failed to deserialize holdings date");
    holdings_date.date = holdings_date.date.date().and_hms_opt(23, 59, 59).unwrap();
    let file_path: PathBuf = PathBuf::from(format!(
        "./reports/holdings_{}.csv",
        holdings_date.date.date()
    ));
    let mut wtr = csv::Writer::from_path(file_path).unwrap();

    let holdings: Vec<Acquisition> = acquisitions::table
        .filter(acquisitions::acquisition_date.le(holdings_date.date))
        .select(Acquisition::as_select())
        .load(conn)
        .unwrap();

    let subsequent_acq_disps: Vec<AcquisitionDisposition> =
        AcquisitionDisposition::belonging_to(&holdings)
            .inner_join(dispositions::table)
            .filter(dispositions::disposition_date.gt(holdings_date.date))
            .select(AcquisitionDisposition::as_select())
            .load(conn)?;

    let holdings_with_subsequent_acq_disps: Vec<(Acquisition, i64)> = subsequent_acq_disps
        .grouped_by(&holdings)
        .into_iter()
        .zip(holdings)
        .map(|(acq_disps, holding)| {
            (
                holding,
                if acq_disps.len() > 0 {
                    acq_disps.iter().map(|l| l.satoshis).sum()
                } else {
                    0
                },
            )
        })
        .collect();

    let mut total_btc = dec!(0);
    let mut total_undisposed_btc = dec!(0);
    let mut total_basis = dec!(0);
    let mut total_fair_value = dec!(0);

    for (lot, subsequent_disposals) in holdings_with_subsequent_acq_disps {
        let btc = Decimal::from_i64(lot.satoshis).unwrap() / dec!(100_000_000);
        let undisposed_btc = Decimal::from_i64(lot.undisposed_satoshis + subsequent_disposals)
            .unwrap()
            / dec!(100_000_000);
        if undisposed_btc == dec!(0) {
            continue;
        }
        let holding = Holding {
            acquisition_date: lot.acquisition_date,
            btc,
            undisposed_btc,
            usd_basis: (Decimal::from_i64(lot.usd_cents_btc_basis).unwrap() / dec!(100)
                * undisposed_btc)
                .round_dp(2),
            usd_fair_value: (Decimal::from_i64(lot.usd_cents_btc_fair_value).unwrap() / dec!(100)
                * undisposed_btc)
                .round_dp(2),
        };
        total_btc += holding.btc;
        total_undisposed_btc += holding.undisposed_btc;
        total_basis += holding.usd_basis;
        total_fair_value += holding.usd_fair_value;

        wtr.serialize(holding).unwrap();
    }

    wtr.write_record(&[
        String::from(""),
        total_btc.to_string(),
        total_undisposed_btc.to_string(),
        total_basis.to_string(),
        total_fair_value.to_string(),
    ])
    .unwrap();

    Ok(())
}
