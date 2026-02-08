mod common;

use common::{setup_test_db, create_test_csv, get_acq_disps};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::mark_to_market::mark_to_market;
use rust_rgl_ledger::commands::report::report;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Mutex;

static CWD_LOCK: Mutex<()> = Mutex::new(());

fn run_mtm_in_dir(price: &str, date: &str, conn: &mut diesel::SqliteConnection) {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("reports")).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    mark_to_market(&price.to_string(), &date.to_string(), conn).unwrap();
    std::env::set_current_dir(original_dir).unwrap();
}

fn run_report_in_dir(beg: &str, end: &str, conn: &mut diesel::SqliteConnection) -> String {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("reports")).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    report(&beg.to_string(), &end.to_string(), conn).unwrap();

    let report_files: Vec<_> = std::fs::read_dir("./reports")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("rgl_"))
        .collect();

    let content = if report_files.is_empty() {
        String::new()
    } else {
        std::fs::read_to_string(report_files[0].path()).unwrap()
    };

    std::env::set_current_dir(original_dir).unwrap();
    content
}

#[test]
fn test_gaap_tax_diverge_after_mtm() {
    let _lock = CWD_LOCK.lock().unwrap();

    let mut conn = setup_test_db();

    // Step 1: Buy 1 BTC at $40k
    let csv = create_test_csv(&[("01/01/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    // Step 2: Mark to market at $45k
    run_mtm_in_dir("$45,000.00", "06/30/2024", &mut conn);

    // Step 3: Sell 1 BTC at $50k (imported as second CSV)
    let csv2 = create_test_csv(&[("09/01/2024", "-1.00000000", "$50,000.00")]);
    import_transactions(&csv2.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps.len(), 1);

    // tax_basis = 1.0 BTC * $40,000.00 = 4,000,000 cents
    assert_eq!(acq_disps[0].tax_basis, 4_000_000);
    // gaap_basis = 1.0 BTC * $45,000.00 (fair value) = 4,500,000 cents
    assert_eq!(acq_disps[0].gaap_basis, 4_500_000);

    // fv_disposed = 1.0 BTC * $50,000.00 = 5,000,000 cents
    // tax_rgl = 5,000,000 - 4,000,000 = 1,000,000
    assert_eq!(acq_disps[0].tax_rgl, 1_000_000);
    // gaap_rgl = 5,000,000 - 4,500,000 = 500,000
    assert_eq!(acq_disps[0].gaap_rgl, 500_000);
}

#[test]
fn test_fair_value_disposed_equals_gaap_minus_tax_basis() {
    let _lock = CWD_LOCK.lock().unwrap();

    let mut conn = setup_test_db();

    let csv = create_test_csv(&[("01/01/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    run_mtm_in_dir("$45,000.00", "06/30/2024", &mut conn);

    let csv2 = create_test_csv(&[("09/01/2024", "-1.00000000", "$50,000.00")]);
    import_transactions(&csv2.path().to_path_buf(), &mut conn).unwrap();

    let content = run_report_in_dir("01/01/2024", "12/31/2024", &mut conn);
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let rows: Vec<csv::StringRecord> = rdr.records().filter_map(|r| r.ok()).collect();

    let detail_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(9).map_or(false, |s| s == "short" || s == "long")
    }).collect();

    for row in &detail_rows {
        let tax_basis = Decimal::from_str(row.get(4).unwrap()).unwrap();
        let gaap_basis = Decimal::from_str(row.get(6).unwrap()).unwrap();
        let fair_value_disposed = Decimal::from_str(row.get(8).unwrap()).unwrap();

        assert_eq!(
            fair_value_disposed, gaap_basis - tax_basis,
            "fair_value_disposed ({}) should equal gaap_basis ({}) - tax_basis ({})",
            fair_value_disposed, gaap_basis, tax_basis
        );
    }
}
