use std::path::PathBuf;
use std::fs::File;
use anyhow::Error;
use chrono::NaiveDateTime;
use csv::Writer;
use diesel::SelectableHelper;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use rust_decimal_macros::dec;
use crate::models::RGL;
use crate::models::ReportDates;
use crate::models::{AcquisitionDisposition, Disposition, Acquisition};
use crate::schema::{acquisition_dispositions, dispositions, acquisitions};

pub fn report(beg: &String, end: &String, conn: &mut SqliteConnection) -> Result<(), Error> {
    let dates: ReportDates = serde_json::from_str(&format!(r#"{{ "beginning_date": "{}", "ending_date": "{}" }}"#, beg, end)).expect("Failed to deserialize provided dates");
    let beg_date_hms = dates.beginning_date.date().and_hms_opt(0, 0, 0).unwrap();
    let end_date_hms = dates.ending_date.date().and_hms_opt(23, 59, 59).unwrap();

    let file_path: PathBuf = PathBuf::from(format!("./reports/rgl_{}_{}.csv", dates.beginning_date.date(), dates.ending_date.date()));
    let mut wtr = csv::Writer::from_path(file_path).unwrap();

    report_term(&mut wtr, beg_date_hms, end_date_hms, "short".to_string(), conn);
    report_term(&mut wtr, beg_date_hms, end_date_hms, "long".to_string(), conn);

    Ok(())
}

pub fn report_term(wtr: &mut Writer<File>, beg: NaiveDateTime, end: NaiveDateTime, term: String, conn: &mut SqliteConnection) {

    let acq_disps: Vec<(Disposition, Acquisition, AcquisitionDisposition)> = dispositions::table
        .filter(dispositions::disposition_date.ge(beg))
        .filter(dispositions::disposition_date.le(end))
        .inner_join(acquisition_dispositions::table.inner_join(acquisitions::table))
        .select((Disposition::as_select(), Acquisition::as_select(), AcquisitionDisposition::as_select()))
        .filter(acquisition_dispositions::term.eq(&term))
        .load(conn)
        .unwrap();

    let mut total_disposed_btc = dec!(0);
    let mut total_disposal_fmv = dec!(0);
    let mut total_tax_basis = dec!(0);
    let mut total_tax_rgl = dec!(0);
    let mut total_gaap_basis = dec!(0);
    let mut total_gaap_rgl = dec!(0);
    let mut total_fair_value_disposed = dec!(0);

    for acq_disp in acq_disps {
        let sats_dec = Decimal::from_i64(acq_disp.2.satoshis).unwrap() / dec!(100_000_000);
        let tax_basis = (Decimal::from_i64(acq_disp.2.tax_basis).unwrap() / dec!(100)).round_dp(2);
        let gaap_basis = (Decimal::from_i64(acq_disp.2.gaap_basis).unwrap() / dec!(100)).round_dp(2);
        let fair_value_disposed: Decimal = -(tax_basis - gaap_basis);

        let rgl = RGL {
            acquisition_date: acq_disp.1.acquisition_date,
            disposition_date: acq_disp.0.disposition_date,
            disposed_btc: sats_dec,
            disposal_fmv: (sats_dec * Decimal::from_i64(acq_disp.0.usd_cents_btc_basis).unwrap() / dec!(100)).round_dp(2),
            tax_basis,
            tax_rgl:( Decimal::from_i64(acq_disp.2.tax_rgl).unwrap() / dec!(100)).round_dp(2),
            gaap_basis,
            gaap_rgl: (Decimal::from_i64(acq_disp.2.gaap_rgl).unwrap() / dec!(100)).round_dp(2),
            fair_value_disposed,
            term: term.clone(),
        };

        total_disposed_btc += rgl.disposed_btc;
        total_disposal_fmv += rgl.disposal_fmv;
        total_tax_basis += rgl.tax_basis;
        total_tax_rgl += rgl.tax_rgl;
        total_gaap_basis += rgl.gaap_basis;
        total_gaap_rgl += rgl.gaap_rgl;
        total_fair_value_disposed += rgl.fair_value_disposed;

        wtr.serialize(rgl).unwrap();
    }

    match total_disposed_btc.eq(&dec!(0)) {
        true => {},
        false => {
            wtr.write_record(&[
                String::from(""),
                String::from(""),
                total_disposed_btc.to_string(),
                total_disposal_fmv.to_string(),
                total_tax_basis.to_string(),
                total_tax_rgl.to_string(),
                total_gaap_basis.to_string(),
                total_gaap_rgl.to_string(),
                total_fair_value_disposed.to_string(),
                term.clone(),
            ]).unwrap();
        
            wtr.write_record(&[
                String::from(""),
                String::from(""),
                String::from(""),
                String::from(""),
                String::from(""),
                String::from(""),
                String::from(""),
                String::from(""),
                String::from(""),
                String::from(""),
            ]).unwrap();
        }
    }

}