use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, MigrationHarness};
use std::io::Write;
use tempfile::NamedTempFile;

pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!("migrations");

pub fn setup_test_db() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:")
        .expect("Failed to create in-memory SQLite connection");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
    conn
}

pub fn default_config() -> rust_rgl_ledger::LotConfig {
    rust_rgl_ledger::LotConfig::default()
}

pub fn universal_config() -> rust_rgl_ledger::LotConfig {
    rust_rgl_ledger::LotConfig {
        tax_lot_method: "fifo".to_string(),
        tax_lot_scope: "universal".to_string(),
        gaap_lot_method: "fifo".to_string(),
        gaap_lot_scope: "universal".to_string(),
    }
}

pub fn create_test_csv(records: &[(&str, &str, &str)]) -> NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(".csv")
        .tempfile()
        .expect("Failed to create temp CSV file");
    writeln!(file, "Date,Bitcoin,Price").unwrap();
    for (date, bitcoin, price) in records {
        writeln!(file, "{},\"{}\",\"{}\"", date, bitcoin, price).unwrap();
    }
    file.flush().unwrap();
    file
}

pub fn create_test_csv_with_wallet(records: &[(&str, &str, &str, &str)]) -> NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(".csv")
        .tempfile()
        .expect("Failed to create temp CSV file");
    writeln!(file, "Date,Bitcoin,Price,Wallet").unwrap();
    for (date, bitcoin, price, wallet) in records {
        writeln!(file, "{},\"{}\",\"{}\",{}", date, bitcoin, price, wallet).unwrap();
    }
    file.flush().unwrap();
    file
}

pub fn create_bucket_csv(records: &[(&str, &str)]) -> NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(".csv")
        .tempfile()
        .expect("Failed to create temp CSV file");
    writeln!(file, "Wallet,BTC").unwrap();
    for (wallet, btc) in records {
        writeln!(file, "{},{}", wallet, btc).unwrap();
    }
    file.flush().unwrap();
    file
}

use rust_rgl_ledger::schema::{acquisitions, dispositions, acquisition_dispositions, fair_values, acquisition_fair_values};
use rust_rgl_ledger::models::{Acquisition, Disposition, AcquisitionDisposition};

pub fn get_acquisitions(conn: &mut SqliteConnection) -> Vec<Acquisition> {
    acquisitions::table
        .select(Acquisition::as_select())
        .load(conn)
        .expect("Failed to load acquisitions")
}

pub fn get_dispositions(conn: &mut SqliteConnection) -> Vec<Disposition> {
    dispositions::table
        .select(Disposition::as_select())
        .load(conn)
        .expect("Failed to load dispositions")
}

pub fn get_acq_disps(conn: &mut SqliteConnection) -> Vec<AcquisitionDisposition> {
    acquisition_dispositions::table
        .select(AcquisitionDisposition::as_select())
        .load(conn)
        .expect("Failed to load acquisition_dispositions")
}

pub fn get_tax_acq_disps(conn: &mut SqliteConnection) -> Vec<AcquisitionDisposition> {
    acquisition_dispositions::table
        .filter(acquisition_dispositions::match_type.eq("tax"))
        .select(AcquisitionDisposition::as_select())
        .load(conn)
        .expect("Failed to load tax acquisition_dispositions")
}

pub fn get_gaap_acq_disps(conn: &mut SqliteConnection) -> Vec<AcquisitionDisposition> {
    acquisition_dispositions::table
        .filter(acquisition_dispositions::match_type.eq("gaap"))
        .select(AcquisitionDisposition::as_select())
        .load(conn)
        .expect("Failed to load gaap acquisition_dispositions")
}

pub fn get_fair_value_count(conn: &mut SqliteConnection) -> i64 {
    fair_values::table
        .count()
        .get_result(conn)
        .expect("Failed to count fair_values")
}

pub fn get_acq_fair_value_count(conn: &mut SqliteConnection) -> i64 {
    acquisition_fair_values::table
        .count()
        .get_result(conn)
        .expect("Failed to count acquisition_fair_values")
}
