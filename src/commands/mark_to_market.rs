use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rust_decimal::{Decimal, prelude::FromPrimitive, RoundingStrategy};
use rust_decimal_macros::dec;
use std::path::PathBuf;
use crate::schema::acquisition_fair_values::*;
use crate::schema::{fair_values, acquisition_fair_values};
use crate::schema::acquisitions::{self, undisposed_satoshis, acquisition_date, usd_cents_btc_fair_value};
use crate::models::{FairValue, NewFairValue, Acquisition, FairValueHolding};

pub fn mark_to_market(price: &String, date: &String, conn: &mut SqliteConnection) -> Result<(), anyhow::Error> {
    let mut fair_value: NewFairValue = serde_json::from_str(&format!(r#"{{ "fair_value_cents": "{}", "date": "{}" }}"#, price, date)).expect("Failed to deserialize provided date/price");

    fair_value.date = fair_value.date.date().and_hms_opt(23, 59, 59).unwrap();

    let fair_value_inserted: FairValue = diesel::insert_into(fair_values::table)
        .values(&fair_value)
        .get_result(conn)
        .expect(format!("Error inserting {:?} into the Fair Values table", fair_value).as_str());

    // MTM is a GAAP operation — only include lots with GAAP undisposed satoshis
    let undisposed_lots: Vec<Acquisition> = acquisitions::table
                                                .filter(undisposed_satoshis.gt(0))
                                                .filter(acquisition_date.le(fair_value.date))
                                                .select(Acquisition::as_select())
                                                .load(conn)
                                                .expect("Error fetching Undisposed Lots");

    let file_path: PathBuf = PathBuf::from(format!("./reports/mark-to-market-{}.csv", fair_value.date.date()));
    let mut wtr = csv::Writer::from_path(file_path).unwrap();

    let mut total_btc = dec!(0);
    let mut total_undisposed_btc = dec!(0);
    let mut total_usd_basis = dec!(0);
    let mut total_previous_usd_fair_value = dec!(0);
    let mut total_current_usd_fair_value = dec!(0);
    let mut total_fair_value_adjustment = dec!(0);

    for lot in undisposed_lots {
        // Use GAAP tracker for the report (MTM is a GAAP operation)
        let undisposed_btc = Decimal::from_i64(lot.undisposed_satoshis).unwrap() / dec!(100_000_000);
        let current_usd_fair_value_price = Decimal::from_i64(fair_value.fair_value_cents).unwrap() / dec!(100);
        let previous_usd_fair_value = undisposed_btc * Decimal::from_i64(lot.usd_cents_btc_fair_value).unwrap() / dec!(100);
        let current_usd_fair_value = undisposed_btc * current_usd_fair_value_price;

        let fv_lot = FairValueHolding {
            wallet: lot.wallet.clone(),
            acquisition_date: lot.acquisition_date,
            btc: Decimal::from_i64(lot.satoshis).unwrap() / dec!(100_000_000),
            undisposed_btc,
            usd_basis: (Decimal::from_i64(lot.usd_cents_btc_basis).unwrap() / dec!(100) * undisposed_btc).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
            previous_usd_fair_value: previous_usd_fair_value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
            current_usd_fair_value: current_usd_fair_value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
            fair_value_adjustment: (current_usd_fair_value - previous_usd_fair_value).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
        };


        total_btc += fv_lot.btc;
        total_undisposed_btc += fv_lot.undisposed_btc;
        total_usd_basis += fv_lot.usd_basis;
        total_previous_usd_fair_value += fv_lot.previous_usd_fair_value;
        total_current_usd_fair_value += fv_lot.current_usd_fair_value;
        total_fair_value_adjustment += fv_lot.fair_value_adjustment;

        wtr.serialize(fv_lot)?;

        diesel::insert_into(acquisition_fair_values::table)
            .values((acquisition_id.eq(lot.id), fair_value_id.eq(fair_value_inserted.id)))
            .execute(conn)
            .expect("Error inserting acquisition_fair_value");
    }

    wtr.write_record(&[
        String::from(""),
        String::from(""),
        total_btc.to_string(),
        total_undisposed_btc.to_string(),
        total_usd_basis.to_string(),
        total_previous_usd_fair_value.to_string(),
        total_current_usd_fair_value.to_string(),
        total_fair_value_adjustment.to_string(),
    ]).unwrap();

    diesel::update(acquisitions::table)
        .filter(acquisition_date.le(fair_value.date))
        .filter(undisposed_satoshis.gt(0))
        .set(usd_cents_btc_fair_value.eq(fair_value.fair_value_cents))
        .execute(conn)
        .expect("Error updating Acquistion Lot Fair Value");

    Ok(())
}
