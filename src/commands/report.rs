use std::path::PathBuf;
use std::fs::File;
use anyhow::Error;
use chrono::NaiveDateTime;
use csv::Writer;
use diesel::SelectableHelper;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rust_decimal::{Decimal, prelude::FromPrimitive, RoundingStrategy};
use rust_decimal_macros::dec;
use crate::rounding_div;
use crate::models::{TaxRGL, GaapRGL};
use crate::models::ReportDates;
use crate::models::{AcquisitionDisposition, Disposition, Acquisition};
use crate::schema::{acquisition_dispositions, dispositions, acquisitions};

pub fn report(beg: &String, end: &String, view: &str, conn: &mut SqliteConnection) -> Result<(), Error> {
    if !["tax", "gaap", "both"].contains(&view) {
        return Err(anyhow::anyhow!("Invalid view '{}'. Must be 'tax', 'gaap', or 'both'.", view));
    }

    let dates: ReportDates = serde_json::from_str(&format!(r#"{{ "beginning_date": "{}", "ending_date": "{}" }}"#, beg, end)).expect("Failed to deserialize provided dates");
    let beg_date_hms = dates.beginning_date.date().and_hms_opt(0, 0, 0).unwrap();
    let end_date_hms = dates.ending_date.date().and_hms_opt(23, 59, 59).unwrap();

    if view == "tax" || view == "both" {
        let file_path: PathBuf = PathBuf::from(format!("./reports/rgl_tax_{}_{}.csv", dates.beginning_date.date(), dates.ending_date.date()));
        let mut wtr = csv::Writer::from_path(file_path).unwrap();
        report_tax_term(&mut wtr, beg_date_hms, end_date_hms, "short".to_string(), conn);
        report_tax_term(&mut wtr, beg_date_hms, end_date_hms, "long".to_string(), conn);
    }

    if view == "gaap" || view == "both" {
        let file_path: PathBuf = PathBuf::from(format!("./reports/rgl_gaap_{}_{}.csv", dates.beginning_date.date(), dates.ending_date.date()));
        let mut wtr = csv::Writer::from_path(file_path).unwrap();
        report_gaap_term(&mut wtr, beg_date_hms, end_date_hms, "short".to_string(), conn);
        report_gaap_term(&mut wtr, beg_date_hms, end_date_hms, "long".to_string(), conn);
    }

    Ok(())
}

fn query_acq_disps(
    beg: NaiveDateTime,
    end: NaiveDateTime,
    term: &str,
    match_type: &str,
    conn: &mut SqliteConnection,
) -> Vec<(Disposition, Acquisition, AcquisitionDisposition)> {
    dispositions::table
        .filter(dispositions::disposition_date.ge(beg))
        .filter(dispositions::disposition_date.le(end))
        .inner_join(acquisition_dispositions::table.inner_join(acquisitions::table))
        .select((Disposition::as_select(), Acquisition::as_select(), AcquisitionDisposition::as_select()))
        .filter(acquisition_dispositions::term.eq(term))
        .filter(acquisition_dispositions::match_type.eq(match_type))
        .load(conn)
        .unwrap()
}

pub fn report_tax_term(wtr: &mut Writer<File>, beg: NaiveDateTime, end: NaiveDateTime, term: String, conn: &mut SqliteConnection) {
    let acq_disps = query_acq_disps(beg, end, &term, "tax", conn);

    let mut total_disposed_btc = dec!(0);
    let mut total_disposal_fmv = dec!(0);
    let mut total_basis = dec!(0);
    let mut total_rgl = dec!(0);

    for acq_disp in acq_disps {
        let sats_dec = Decimal::from_i64(acq_disp.2.satoshis).unwrap() / dec!(100_000_000);
        let basis = (Decimal::from_i64(acq_disp.2.basis).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        let cost_per_btc = (Decimal::from_i64(acq_disp.1.usd_cents_btc_basis).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        let disposal_fmv_per_btc = (Decimal::from_i64(acq_disp.0.usd_cents_btc_basis).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        let rgl = TaxRGL {
            acquisition_date: acq_disp.1.acquisition_date,
            disposition_date: acq_disp.0.disposition_date,
            disposed_btc: sats_dec,
            cost_per_btc,
            disposal_fmv_per_btc,
            disposal_fmv: (sats_dec * Decimal::from_i64(acq_disp.0.usd_cents_btc_basis).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
            basis,
            rgl: (Decimal::from_i64(acq_disp.2.rgl).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
            term: term.clone(),
        };

        total_disposed_btc += rgl.disposed_btc;
        total_disposal_fmv += rgl.disposal_fmv;
        total_basis += rgl.basis;
        total_rgl += rgl.rgl;

        wtr.serialize(rgl).unwrap();
    }

    match total_disposed_btc.eq(&dec!(0)) {
        true => {},
        false => {
            wtr.write_record(&[
                String::from(""),
                String::from(""),
                total_disposed_btc.to_string(),
                String::from(""),
                String::from(""),
                total_disposal_fmv.to_string(),
                total_basis.to_string(),
                total_rgl.to_string(),
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
            ]).unwrap();
        }
    }
}

pub fn report_gaap_term(wtr: &mut Writer<File>, beg: NaiveDateTime, end: NaiveDateTime, term: String, conn: &mut SqliteConnection) {
    let acq_disps = query_acq_disps(beg, end, &term, "gaap", conn);

    let mut total_disposed_btc = dec!(0);
    let mut total_disposal_fmv = dec!(0);
    let mut total_cost_basis = dec!(0);
    let mut total_basis = dec!(0);
    let mut total_fmv_disposed = dec!(0);
    let mut total_rgl = dec!(0);

    for acq_disp in acq_disps {
        let sats_dec = Decimal::from_i64(acq_disp.2.satoshis).unwrap() / dec!(100_000_000);
        let basis = (Decimal::from_i64(acq_disp.2.basis).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        // Cost basis for the disposed sats (original acquisition price)
        let cost_basis_cents = rounding_div(acq_disp.2.satoshis as i128 * acq_disp.1.usd_cents_btc_basis as i128, 100_000_000);
        let cost_basis = (Decimal::from_i64(cost_basis_cents).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        // FMV Disposed = fair value basis - cost basis for same sats
        // This is the MTM adjustment being written off
        let fmv_disposed = (Decimal::from_i64(acq_disp.2.basis - cost_basis_cents).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        let cost_per_btc = (Decimal::from_i64(acq_disp.1.usd_cents_btc_basis).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        let disposal_fmv_per_btc = (Decimal::from_i64(acq_disp.0.usd_cents_btc_basis).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        let gaap_per_btc = (Decimal::from_i64(acq_disp.1.usd_cents_btc_fair_value).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        let rgl = GaapRGL {
            acquisition_date: acq_disp.1.acquisition_date,
            disposition_date: acq_disp.0.disposition_date,
            disposed_btc: sats_dec,
            cost_per_btc,
            disposal_fmv_per_btc,
            gaap_per_btc,
            disposal_fmv: (sats_dec * Decimal::from_i64(acq_disp.0.usd_cents_btc_basis).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
            cost_basis,
            basis,
            fmv_disposed,
            rgl: (Decimal::from_i64(acq_disp.2.rgl).unwrap() / dec!(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
            term: term.clone(),
        };

        total_disposed_btc += rgl.disposed_btc;
        total_disposal_fmv += rgl.disposal_fmv;
        total_cost_basis += rgl.cost_basis;
        total_basis += rgl.basis;
        total_fmv_disposed += rgl.fmv_disposed;
        total_rgl += rgl.rgl;

        wtr.serialize(rgl).unwrap();
    }

    match total_disposed_btc.eq(&dec!(0)) {
        true => {},
        false => {
            wtr.write_record(&[
                String::from(""),
                String::from(""),
                total_disposed_btc.to_string(),
                String::from(""),
                String::from(""),
                String::from(""),
                total_disposal_fmv.to_string(),
                total_cost_basis.to_string(),
                total_basis.to_string(),
                total_fmv_disposed.to_string(),
                total_rgl.to_string(),
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
                String::from(""),
                String::from(""),
            ]).unwrap();
        }
    }
}
