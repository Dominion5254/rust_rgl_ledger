use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use serde::{de, Deserialize, Deserializer, Serialize};
use diesel::prelude::*;
use crate::schema::{acquisitions, dispositions, acquisition_dispositions, fair_values};

#[derive(Queryable, Selectable, Debug, PartialEq, Eq, Serialize, Identifiable)]
#[diesel(table_name = acquisitions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Acquisition {
    pub id: i32,
    pub acquisition_date: NaiveDateTime,
    pub satoshis: i64,
    pub undisposed_satoshis: i64,
    pub usd_cents_btc_basis: i64,
    pub usd_cents_btc_fair_value: i64,
    pub wallet: String,
    pub tax_undisposed_satoshis: i64,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = acquisitions)]
pub struct NewAcquisition {
    pub acquisition_date: NaiveDateTime,
    pub satoshis: i64,
    pub undisposed_satoshis: i64,
    pub usd_cents_btc_basis: i64,
    pub usd_cents_btc_fair_value: i64,
    pub wallet: String,
    pub tax_undisposed_satoshis: i64,
}

fn default_wallet() -> String {
    "default".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NewRecord {
    #[serde(deserialize_with = "deserialize_date")]
    pub date: NaiveDateTime,
    #[serde(deserialize_with = "deserialize_bitcoin")]
    pub bitcoin: i64,
    #[serde(deserialize_with = "deserialize_price")]
    pub price: i64,
    #[serde(default = "default_wallet")]
    pub wallet: String,
}

pub fn parse_date_str(s: &str) -> Result<NaiveDateTime, String> {
    let date_formats = [
        "%m/%d/%y %H:%M:%S",
        "%m/%d/%Y %H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%m/%d/%y %I:%M %p",
        "%m/%d/%y",
        "%m/%d/%Y",
        "%y-%m-%d",
        "%Y-%m-%d",
    ];

    for format in &date_formats {
        if let Ok(parsed_date) = NaiveDateTime::parse_from_str(s, format) {
            return Ok(parsed_date);
        }
        if let Ok(parsed_date) = NaiveDate::parse_from_str(s, format) {
            return Ok(parsed_date.and_hms_opt(0, 0, 0).expect("Error adding time 00:00:00 to Date"));
        }
    }

    Err(format!("Invalid date format: {}", s))
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let date_str = String::deserialize(deserializer)?;
    parse_date_str(&date_str).map_err(de::Error::custom)
}

pub fn deserialize_price<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let price_str = String::deserialize(deserializer)?;
    let cleaned = price_str.replace("$", "").replace(",", "");
    match Decimal::from_str_exact(&cleaned) {
        Ok(price) => {
            let cents = (price * Decimal::from(100)).round();
            Ok(cents.to_string().parse::<i64>().unwrap())
        }
        Err(e) => {
            Err(de::Error::custom(format!("Invalid Price format: {}\nError: {}", price_str, e)))
        }
    }
}

pub fn deserialize_bitcoin<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let bitcoin_str = String::deserialize(deserializer)?;
    let btc = Decimal::from_str_exact(&bitcoin_str)
        .map_err(|e| de::Error::custom(format!("Invalid Bitcoin format: {}\nError: {}", bitcoin_str, e)))?;
    let sats = (btc * Decimal::from(100_000_000i64)).round();
    Ok(sats.to_string().parse::<i64>().unwrap())
}

#[derive(Queryable, Selectable, Debug, Identifiable)]
#[diesel(table_name = dispositions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Disposition {
    pub id: i32,
    pub disposition_date: NaiveDateTime,
    pub satoshis: i64,
    pub undisposed_satoshis: i64,
    pub usd_cents_btc_basis: i64,
    pub wallet: String,
    pub tax_undisposed_satoshis: i64,
}

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = dispositions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewDisposition {
    pub disposition_date: NaiveDateTime,
    pub satoshis: i64,
    pub undisposed_satoshis: i64,
    pub usd_cents_btc_basis: i64,
    pub wallet: String,
    pub tax_undisposed_satoshis: i64,
}

#[derive(Queryable, Selectable, Identifiable, Insertable, PartialEq, Debug, Associations)]
#[diesel(belongs_to(Acquisition))]
#[diesel(belongs_to(Disposition))]
#[diesel(table_name = acquisition_dispositions)]
#[diesel(primary_key(acquisition_id, disposition_id, match_type))]
pub struct AcquisitionDisposition {
    pub acquisition_id: i32,
    pub disposition_id: i32,
    pub match_type: String,
    pub satoshis: i64,
    pub basis: i64,
    pub rgl: i64,
    pub term: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReportDates {
    #[serde(deserialize_with = "deserialize_date")]
    pub beginning_date: NaiveDateTime,
    #[serde(deserialize_with = "deserialize_date")]
    pub ending_date: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HoldingsDate {
    #[serde(deserialize_with = "deserialize_date")]
    pub date: NaiveDateTime,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TaxRGL {
    pub acquisition_date: NaiveDateTime,
    pub disposition_date: NaiveDateTime,
    pub disposed_btc: Decimal,
    pub cost_per_btc: Decimal,
    pub disposal_fmv_per_btc: Decimal,
    pub disposal_fmv: Decimal,
    pub basis: Decimal,
    pub rgl: Decimal,
    pub term: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GaapRGL {
    pub acquisition_date: NaiveDateTime,
    pub disposition_date: NaiveDateTime,
    pub disposed_btc: Decimal,
    pub cost_per_btc: Decimal,
    pub disposal_fmv_per_btc: Decimal,
    pub gaap_per_btc: Decimal,
    pub disposal_fmv: Decimal,
    pub cost_basis: Decimal,
    pub basis: Decimal,
    pub fmv_disposed: Decimal,
    pub rgl: Decimal,
    pub term: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Holding {
    pub wallet: String,
    pub acquisition_date: NaiveDateTime,
    pub btc: Decimal,
    pub undisposed_btc: Decimal,
    pub usd_basis: Decimal,
    pub usd_fair_value: Decimal,
}

#[derive(Queryable, Selectable, Debug, Deserialize)]
#[diesel(table_name = fair_values)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct FairValue {
    pub id: i32,
    pub fair_value_cents: i64,
    pub date: NaiveDateTime,
}

#[derive(Queryable, Insertable, Debug, Deserialize)]
#[diesel(table_name = fair_values)]
pub struct NewFairValue {
    #[serde(deserialize_with = "deserialize_price")]
    pub fair_value_cents: i64,
    #[serde(deserialize_with = "deserialize_date")]
    pub date: NaiveDateTime,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FairValueHolding {
    pub wallet: String,
    pub acquisition_date: NaiveDateTime,
    pub btc: Decimal,
    pub undisposed_btc: Decimal,
    pub usd_basis: Decimal,
    pub previous_usd_fair_value: Decimal,
    pub current_usd_fair_value: Decimal,
    pub fair_value_adjustment: Decimal,
}
