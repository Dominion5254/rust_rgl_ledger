mod common;

use common::{setup_test_db, create_test_csv, default_config, get_tax_acq_disps, get_gaap_acq_disps};
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

fn run_report_in_dir(beg: &str, end: &str, conn: &mut diesel::SqliteConnection) -> (String, String) {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("reports")).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    report(&beg.to_string(), &end.to_string(), "both", conn).unwrap();

    let report_files: Vec<_> = std::fs::read_dir("./reports")
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    let tax_content = report_files.iter()
        .find(|e| e.file_name().to_string_lossy().starts_with("rgl_tax_"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .unwrap_or_default();

    let gaap_content = report_files.iter()
        .find(|e| e.file_name().to_string_lossy().starts_with("rgl_gaap_"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .unwrap_or_default();

    std::env::set_current_dir(original_dir).unwrap();
    (tax_content, gaap_content)
}

#[test]
fn test_gaap_tax_diverge_after_mtm() {
    let _lock = CWD_LOCK.lock().unwrap();

    let mut conn = setup_test_db();
    let config = default_config();

    // Step 1: Buy 1 BTC at $40k
    let csv = create_test_csv(&[("01/01/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // Step 2: Mark to market at $45k
    run_mtm_in_dir("$45,000.00", "06/30/2024", &mut conn);

    // Step 3: Sell 1 BTC at $50k (imported as second CSV)
    let csv2 = create_test_csv(&[("09/01/2024", "-1.00000000", "$50,000.00")]);
    import_transactions(&csv2.path().to_path_buf(), &mut conn, &config).unwrap();

    let tax_ads = get_tax_acq_disps(&mut conn);
    let gaap_ads = get_gaap_acq_disps(&mut conn);
    assert_eq!(tax_ads.len(), 1);
    assert_eq!(gaap_ads.len(), 1);

    // tax_basis = 1.0 BTC * $40,000.00 = 4,000,000 cents
    assert_eq!(tax_ads[0].basis, 4_000_000);
    // gaap_basis = 1.0 BTC * $45,000.00 (fair value) = 4,500,000 cents
    assert_eq!(gaap_ads[0].basis, 4_500_000);

    // fv_disposed = 1.0 BTC * $50,000.00 = 5,000,000 cents
    // tax_rgl = 5,000,000 - 4,000,000 = 1,000,000
    assert_eq!(tax_ads[0].rgl, 1_000_000);
    // gaap_rgl = 5,000,000 - 4,500,000 = 500,000
    assert_eq!(gaap_ads[0].rgl, 500_000);
}

#[test]
fn test_report_tax_gaap_separate_files_after_mtm() {
    let _lock = CWD_LOCK.lock().unwrap();

    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv(&[("01/01/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    run_mtm_in_dir("$45,000.00", "06/30/2024", &mut conn);

    let csv2 = create_test_csv(&[("09/01/2024", "-1.00000000", "$50,000.00")]);
    import_transactions(&csv2.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax_content, gaap_content) = run_report_in_dir("01/01/2024", "12/31/2024", &mut conn);

    // Tax report: columns AcquisitionDate(0), DispositionDate(1), DisposedBtc(2), CostPerBtc(3),
    //             DisposalFmvPerBtc(4), DisposalFmv(5), Basis(6), Rgl(7), Term(8)
    let mut tax_rdr = csv::Reader::from_reader(tax_content.as_bytes());
    let tax_rows: Vec<csv::StringRecord> = tax_rdr.records().filter_map(|r| r.ok()).collect();
    let tax_detail: Vec<_> = tax_rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(8).map_or(false, |s| s == "short" || s == "long")
    }).collect();
    assert_eq!(tax_detail.len(), 1, "Should have 1 tax detail row");
    let tax_basis = Decimal::from_str(tax_detail[0].get(6).unwrap()).unwrap();
    assert_eq!(tax_basis, Decimal::from(40_000), "Tax basis should be $40,000 (cost basis)");

    // GAAP report: columns AcquisitionDate(0), DispositionDate(1), DisposedBtc(2), CostPerBtc(3),
    //              DisposalFmvPerBtc(4), GaapPerBtc(5), DisposalFmv(6), CostBasis(7),
    //              Basis(8), FmvDisposed(9), Rgl(10), Term(11)
    let mut gaap_rdr = csv::Reader::from_reader(gaap_content.as_bytes());
    let gaap_rows: Vec<csv::StringRecord> = gaap_rdr.records().filter_map(|r| r.ok()).collect();
    let gaap_detail: Vec<_> = gaap_rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(11).map_or(false, |s| s == "short" || s == "long")
    }).collect();
    assert_eq!(gaap_detail.len(), 1, "Should have 1 gaap detail row");

    // CostBasis should be $40,000 (original acquisition cost)
    let cost_basis = Decimal::from_str(gaap_detail[0].get(7).unwrap()).unwrap();
    assert_eq!(cost_basis, Decimal::from(40_000), "CostBasis should be $40,000 (original cost)");

    // Basis should be $45,000 (fair value carrying amount after MTM)
    let gaap_basis = Decimal::from_str(gaap_detail[0].get(8).unwrap()).unwrap();
    assert_eq!(gaap_basis, Decimal::from(45_000), "GAAP basis should be $45,000 (fair value after MTM)");

    // FmvDisposed should be $5,000 ($45k fair value - $40k cost)
    let fmv_disposed = Decimal::from_str(gaap_detail[0].get(9).unwrap()).unwrap();
    assert_eq!(fmv_disposed, Decimal::from(5_000), "FmvDisposed should be $5,000 (MTM adjustment write-off)");
}
