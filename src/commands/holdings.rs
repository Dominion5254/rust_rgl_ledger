use std::path::PathBuf;

use anyhow::Ok;
use diesel::prelude::*;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use rust_decimal_macros::dec;
use crate::{models::{HoldingsDate, Acquisition, Holding}, schema::acquisitions, establish_connection};

pub fn holdings(date: &String) -> Result<(), anyhow::Error> {
    let mut holdings_date: HoldingsDate = serde_json::from_str(&format!(r#"{{ "date": "{}" }}"#, date)).expect("Failed to deserialize holdings date");
    holdings_date.date = holdings_date.date.date().and_hms_opt(23, 59, 59).unwrap();

    let conn = &mut establish_connection();
    let file_path: PathBuf = PathBuf::from(format!("./reports/holdings_{}.csv", holdings_date.date.date()));
    let mut wtr = csv::Writer::from_path(file_path).unwrap();

    let holdings: Vec<Acquisition> = acquisitions::table
                            .filter(acquisitions::acquisition_date.le(holdings_date.date))
                            .filter(acquisitions::undisposed_satoshis.gt(0))
                            .select(Acquisition::as_select())
                            .load(conn)
                            .unwrap();

    let mut total_btc = dec!(0);
    let mut total_undisposed_btc = dec!(0);
    let mut total_basis = dec!(0);
    let mut total_impaired_basis = dec!(0);

    for lot in holdings {
        let btc = Decimal::from_i64(lot.satoshis).unwrap() / dec!(100_000_000);
        let undisposed_btc = Decimal::from_i64(lot.undisposed_satoshis).unwrap() / dec!(100_000_000);

        let holding = Holding {
            acquisition_date: lot.acquisition_date,
            btc,
            undisposed_btc,
            usd_basis: (Decimal::from_i64(lot.usd_cents_btc_basis).unwrap() / dec!(100) * undisposed_btc).round_dp(2),
            usd_impaired_value: (Decimal::from_i64(lot.usd_cents_btc_impaired_value).unwrap() / dec!(100) * undisposed_btc).round_dp(2),
        };
        total_btc += holding.btc;
        total_undisposed_btc += holding.undisposed_btc;
        total_basis += holding.usd_basis;
        total_impaired_basis += holding.usd_impaired_value;

        wtr.serialize(holding).unwrap();
    }

    wtr.write_record(&[
        String::from(""),
        total_btc.to_string(),
        total_undisposed_btc.to_string(),
        total_basis.to_string(),
        total_impaired_basis.to_string()
    ]).unwrap();
    
    Ok(())
}