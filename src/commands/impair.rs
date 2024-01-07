use std::path::PathBuf;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use rust_decimal_macros::dec;
use diesel::prelude::*;
use crate::{models::{Impairment, Acquisition, ImpairmentLoss}, schema::acquisitions, schema::impairments, establish_connection};

pub fn impair_holdings(price: &String, date: &String) -> Result<(), anyhow::Error> {
    let mut impairment: Impairment = serde_json::from_str(&format!(r#"{{ "impairment_cents": "{}", "date": "{}" }}"#, price, date)).expect("Failed to deserialize provided date/price");
    impairment.date = impairment.date.date().and_hms_opt(23, 59, 59).unwrap();

    let conn = &mut establish_connection();

    diesel::insert_into(impairments::table)
        .values(&impairment)
        .execute(conn)
        .expect("Error inserting {:?} into the Imapirments table");

    let impaired_lots: Vec<Acquisition> = acquisitions::table
                            .filter(acquisitions::acquisition_date.le(&impairment.date))
                            .filter(acquisitions::usd_cents_btc_impaired_value.gt(&impairment.impairment_cents))
                            .select(Acquisition::as_select())
                            .load(conn)
                            .expect("Error fetching impaired lots");

    let file_path: PathBuf = PathBuf::from(format!("./reports/impairment_{}.csv", impairment.date.date()));
    let mut wtr = csv::Writer::from_path(file_path).unwrap();
    
    let mut total_undisposed_btc = dec!(0);
    let mut total_pre_impair_usd_value = dec!(0);
    let mut total_post_impair_usd_value = dec!(0);
    let mut total_impairment_loss = dec!(0);

    for lot in impaired_lots {
        let lot_undisposed_sats_decimal = Decimal::from_i64(lot.undisposed_satoshis).unwrap();
        let lot_impaired_value_decimal = Decimal::from_i64(lot.usd_cents_btc_impaired_value).unwrap();
        let impaired_cents_decimal = Decimal::from_i64(impairment.impairment_cents).unwrap();

        let impairment_loss = ImpairmentLoss {
            undisposed_btc: (lot_undisposed_sats_decimal / dec!(100_000_000)),
            pre_impairment_btc_price: (lot_impaired_value_decimal / dec!(100)).round_dp(2),
            post_impairment_btc_price: (impaired_cents_decimal / dec!(100)).round_dp(2),
            pre_impairment_usd_value: (lot_undisposed_sats_decimal * lot_impaired_value_decimal / dec!(100_000_000) / dec!(100)).round_dp(2),
            post_impairment_usd_value: (lot_undisposed_sats_decimal * impaired_cents_decimal / dec!(100_000_000) / dec!(100)).round_dp(2),
            impairment_loss: ((lot_undisposed_sats_decimal * lot_impaired_value_decimal - lot_undisposed_sats_decimal * impaired_cents_decimal) / dec!(100_000_000) / dec!(100)).round_dp(2)
        };

        total_undisposed_btc += impairment_loss.undisposed_btc;
        total_pre_impair_usd_value += impairment_loss.pre_impairment_usd_value;
        total_post_impair_usd_value += impairment_loss.post_impairment_usd_value;
        total_impairment_loss += impairment_loss.impairment_loss;

        wtr.serialize(impairment_loss)?;
    }

    wtr.write_record(&[
        total_undisposed_btc.to_string(),
        String::from(""),
        String::from(""),
        total_pre_impair_usd_value.to_string(),
        total_post_impair_usd_value.to_string(),
        total_impairment_loss.to_string()
    ])?;

    diesel::update(acquisitions::table)
        .filter(acquisitions::usd_cents_btc_impaired_value.gt(impairment.impairment_cents))
        .filter(acquisitions::acquisition_date.le(impairment.date))
        .set(acquisitions::usd_cents_btc_impaired_value.eq(impairment.impairment_cents))
        .execute(conn)
        .expect("Error updating Acquisition Lots Impaired Value");

    Ok(())
}