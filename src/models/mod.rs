use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use serde::{de, Deserialize, Deserializer, Serialize};
use diesel::prelude::*;
use crate::schema::{acquisitions, dispositions, acquisition_dispositions, impairments, fair_values};

#[derive(Queryable, Selectable, Debug, PartialEq, Eq, Serialize)]
#[diesel(table_name = acquisitions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Acquisition {
    pub id: i32,
    pub acquisition_date: NaiveDateTime,
    pub satoshis: i64,
    pub undisposed_satoshis: i64,
    pub usd_cents_btc_basis: i64,
    pub usd_cents_btc_fair_value: i64,
    pub usd_cents_btc_impaired_value: i64,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = acquisitions)]
pub struct NewAcquisition {
    pub acquisition_date: NaiveDateTime,
    pub satoshis: i64,
    pub undisposed_satoshis: i64,
    pub usd_cents_btc_basis: i64,
    pub usd_cents_btc_fair_value: i64,
    pub usd_cents_btc_impaired_value: i64,
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
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let date_formats = [
        "%m/%d/%Y %H:%M:%S",
        "%m/%d/%y %H:%M:%S",
        "%m/%d/%y %I:%M %p",
        "%m/%d/%y",
        "%m/%d/%Y",
        "%y-%m-%d",
        "%Y-%m-%d",
    ];

    let date_str = String::deserialize(deserializer)?;

    for format in &date_formats {
        if let Ok(parsed_date) = NaiveDateTime::parse_from_str(&date_str, format) {
            return Ok(parsed_date);
        }
        if let Ok(parsed_date) = NaiveDate::parse_from_str(&date_str, format) {
            return Ok(parsed_date.and_hms_opt(0, 0, 0).expect("Error adding time 00:00:00 to Date"));
        }
    }

    Err(de::Error::custom(format!("Invalid date format: {}", date_str)))
}

fn deserialize_price<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let price_str = String::deserialize(deserializer)?;
    let price_i64 = price_str.replace("$", "").replace(",", "").parse::<f64>();
    match price_i64 {
        Ok(price) => {
            return Ok((price * 100.0) as i64);
        }
        Err(e) => {
            Err(de::Error::custom(format!("Invalid Price format: {}\nError: {}", price_str, e)))
        }
    }
}

fn deserialize_bitcoin<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let bitcoin_str = String::deserialize(deserializer)?;
    let sats: i64 = (bitcoin_str.parse::<f64>().unwrap() * (100_000_000 as f64)) as i64;
    Ok(sats)
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = dispositions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Disposition {
    pub id: i32,
    pub disposition_date: NaiveDateTime,
    pub satoshis: i64,
    pub undisposed_satoshis: i64,
    pub usd_cents_btc_basis: i64,
}

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = dispositions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewDisposition {
    pub disposition_date: NaiveDateTime,
    pub satoshis: i64,
    pub undisposed_satoshis: i64,
    pub usd_cents_btc_basis: i64,
}

#[derive(Queryable, Selectable, Identifiable, Insertable, PartialEq, Debug)]
#[diesel(belongs_to(Acquisition))]
#[diesel(belongs_to(Disposition))]
#[diesel(table_name = acquisition_dispositions)]
#[diesel(primary_key(acquisition_id, disposition_id))]
pub struct AcquisitionDisposition {
    pub acquisition_id: i32,
    pub disposition_id: i32,
    pub satoshis: i64,
    pub gaap_basis: i64,
    pub gaap_rgl: i64,
    pub tax_basis: i64,
    pub tax_rgl: i64,
    pub term: String,
}

#[derive(Queryable, Insertable, Debug, Deserialize)]
#[diesel(table_name = impairments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Impairment {
    #[serde(deserialize_with = "deserialize_price")]
    pub impairment_cents: i64,
    #[serde(deserialize_with = "deserialize_date")]
    pub date: NaiveDateTime,
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
pub struct ImpairmentLoss {
    pub acquisition_date: NaiveDateTime,
    pub undisposed_btc: Decimal,
    pub pre_impairment_btc_price: Decimal,
    pub post_impairment_btc_price: Decimal,
    pub pre_impairment_usd_value: Decimal,
    pub post_impairment_usd_value: Decimal,
    pub impairment_loss: Decimal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RGL {
    pub acquisition_date: NaiveDateTime,
    pub disposition_date: NaiveDateTime,
    pub disposed_btc: Decimal,
    pub disposal_fmv: Decimal,
    pub tax_basis: Decimal,
    pub tax_rgl: Decimal,
    pub gaap_basis: Decimal,
    pub gaap_rgl: Decimal,
    pub impairment_disposed: Decimal,
    pub term: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Holding {
    pub acquisition_date: NaiveDateTime,
    pub btc: Decimal,
    pub undisposed_btc: Decimal,
    pub usd_basis: Decimal,
    pub usd_impaired_value: Decimal,
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
    pub acquisition_date: NaiveDateTime,
    pub btc: Decimal,
    pub undisposed_btc: Decimal,
    pub usd_basis: Decimal,
    pub previous_usd_fair_value: Decimal,
    pub current_usd_fair_value: Decimal,
    pub fair_value_adjustment: Decimal,
}