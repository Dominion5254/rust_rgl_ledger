mod common;

use common::{setup_test_db, create_test_csv, default_config};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::report::report;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Mutex;

static CWD_LOCK: Mutex<()> = Mutex::new(());

fn run_report_in_dir(beg: &str, end: &str, view: &str, conn: &mut diesel::SqliteConnection) -> (String, String) {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("reports")).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    report(&beg.to_string(), &end.to_string(), view, conn).unwrap();

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

fn parse_report_rows(content: &str) -> Vec<csv::StringRecord> {
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    rdr.records().filter_map(|r| r.ok()).collect()
}

#[test]
fn test_report_generates_separate_files() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "both", &mut conn);
    assert!(!tax.is_empty(), "Tax report should be generated");
    assert!(!gaap.is_empty(), "GAAP report should be generated");
}

#[test]
fn test_report_view_tax_only() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "tax", &mut conn);
    assert!(!tax.is_empty(), "Tax report should be generated");
    assert!(gaap.is_empty(), "GAAP report should NOT be generated when view=tax");
}

#[test]
fn test_report_view_gaap_only() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "gaap", &mut conn);
    assert!(tax.is_empty(), "Tax report should NOT be generated when view=gaap");
    assert!(!gaap.is_empty(), "GAAP report should be generated");
}

// Tax report columns: AcquisitionDate(0), DispositionDate(1), DisposedBtc(2), CostPerBtc(3), DisposalFmvPerBtc(4), DisposalFmv(5), Basis(6), Rgl(7), Term(8)
// GAAP report columns: AcquisitionDate(0), DispositionDate(1), DisposedBtc(2), CostPerBtc(3), DisposalFmvPerBtc(4), GaapPerBtc(5), DisposalFmv(6), CostBasis(7), Basis(8), FmvDisposed(9), Rgl(10), Term(11)

#[test]
fn test_report_short_term_only() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("03/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, _gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "both", &mut conn);
    let rows = parse_report_rows(&tax);

    let mut found_short = false;
    let mut found_long = false;
    for row in &rows {
        if let Some(term) = row.get(8) {
            if term == "short" { found_short = true; }
            if term == "long" { found_long = true; }
        }
    }
    assert!(found_short, "Should have short-term entries in tax report");
    assert!(!found_long, "Should not have long-term entries in tax report");
}

#[test]
fn test_report_long_term_only() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2023", "1.00000000", "$30,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, _gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "both", &mut conn);
    let rows = parse_report_rows(&tax);

    let mut found_short = false;
    let mut found_long = false;
    for row in &rows {
        if let Some(term) = row.get(8) {
            if term == "short" { found_short = true; }
            if term == "long" { found_long = true; }
        }
    }
    assert!(!found_short, "Should not have short-term entries");
    assert!(found_long, "Should have long-term entries in report");
}

#[test]
fn test_report_mixed_terms() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2023", "1.00000000", "$30,000.00"),
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.50000000", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, _gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "both", &mut conn);
    let rows = parse_report_rows(&tax);

    let mut found_short = false;
    let mut found_long = false;
    for row in &rows {
        if let Some(term) = row.get(8) {
            if term == "short" { found_short = true; }
            if term == "long" { found_long = true; }
        }
    }
    assert!(found_short, "Should have short-term entries");
    assert!(found_long, "Should have long-term entries");
}

#[test]
fn test_report_date_filtering() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "2.00000000", "$40,000.00"),
        ("03/01/2024", "-0.50000000", "$45,000.00"),
        ("07/01/2024", "-0.50000000", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, _gaap) = run_report_in_dir("01/01/2024", "03/31/2024", "both", &mut conn);
    let rows = parse_report_rows(&tax);

    // Detail rows: non-empty first column and term in column 8
    let detail_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(8).map_or(false, |s| s == "short" || s == "long")
    }).collect();

    assert_eq!(detail_rows.len(), 1, "Tax report should have 1 detail row for Q1");
}

#[test]
fn test_report_totals_match_sum() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "2.00000000", "$40,000.00"),
        ("03/01/2024", "-0.30000000", "$45,000.00"),
        ("04/01/2024", "-0.70000000", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, _gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "both", &mut conn);
    let rows = parse_report_rows(&tax);

    // Tax: columns are AcquisitionDate(0), DispositionDate(1), DisposedBtc(2), CostPerBtc(3), DisposalFmvPerBtc(4), DisposalFmv(5), Basis(6), Rgl(7), Term(8)
    let detail_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(8).map_or(false, |s| s == "short")
    }).collect();

    let total_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| s.is_empty()) &&
        r.get(2).map_or(false, |s| !s.is_empty()) &&
        r.get(8).map_or(false, |s| s == "short")
    }).collect();

    assert!(!detail_rows.is_empty(), "Should have detail rows");
    assert!(!total_rows.is_empty(), "Should have totals row");

    // rgl is column 7 in tax report
    let detail_sum: Decimal = detail_rows.iter()
        .map(|r| Decimal::from_str(r.get(7).unwrap()).unwrap())
        .sum();
    let total_rgl = Decimal::from_str(total_rows[0].get(7).unwrap()).unwrap();

    assert_eq!(detail_sum, total_rgl, "Sum of detail rgl should equal total rgl");
}

#[test]
fn test_report_disposal_fmv_minus_basis_equals_rgl() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-0.33333333", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, _gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "both", &mut conn);
    let rows = parse_report_rows(&tax);

    let detail_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(8).map_or(false, |s| s == "short" || s == "long")
    }).collect();

    for row in &detail_rows {
        let disposal_fmv = Decimal::from_str(row.get(5).unwrap()).unwrap();
        let basis = Decimal::from_str(row.get(6).unwrap()).unwrap();
        let rgl = Decimal::from_str(row.get(7).unwrap()).unwrap();

        assert_eq!(
            disposal_fmv - basis, rgl,
            "disposal_fmv ({}) - basis ({}) should equal rgl ({}), but got {}",
            disposal_fmv, basis, rgl, disposal_fmv - basis
        );
    }
}

#[test]
fn test_gaap_report_has_fmv_disposed_column() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (_tax, gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "both", &mut conn);

    // Parse header
    let mut rdr = csv::Reader::from_reader(gaap.as_bytes());
    let headers = rdr.headers().unwrap().clone();
    // GAAP columns: AcquisitionDate(0), DispositionDate(1), DisposedBtc(2), CostPerBtc(3),
    //               DisposalFmvPerBtc(4), GaapPerBtc(5), DisposalFmv(6), CostBasis(7),
    //               Basis(8), FmvDisposed(9), Rgl(10), Term(11)
    assert_eq!(headers.get(7).unwrap(), "CostBasis", "GAAP report should have CostBasis at column 7");
    assert_eq!(headers.get(9).unwrap(), "FmvDisposed", "GAAP report should have FmvDisposed at column 9");

    // Without MTM, fmv_disposed should be 0 and cost_basis should equal basis
    let rows: Vec<csv::StringRecord> = rdr.records().filter_map(|r| r.ok()).collect();
    let detail_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(11).map_or(false, |s| s == "short" || s == "long")
    }).collect();

    for row in &detail_rows {
        let cost_basis = Decimal::from_str(row.get(7).unwrap()).unwrap();
        let basis = Decimal::from_str(row.get(8).unwrap()).unwrap();
        let fmv_disposed = Decimal::from_str(row.get(9).unwrap()).unwrap();
        assert_eq!(fmv_disposed, Decimal::from(0), "FmvDisposed should be 0 when no MTM has been run");
        assert_eq!(cost_basis, basis, "CostBasis should equal Basis when no MTM has been run");
    }
}

#[test]
fn test_tax_report_has_no_fmv_disposed_column() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let config = default_config();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let (tax, _gaap) = run_report_in_dir("01/01/2024", "12/31/2024", "both", &mut conn);

    let mut rdr = csv::Reader::from_reader(tax.as_bytes());
    let headers = rdr.headers().unwrap().clone();

    // Tax report: 9 columns, no FmvDisposed
    assert_eq!(headers.len(), 9, "Tax report should have 9 columns");
    let header_names: Vec<&str> = (0..headers.len()).map(|i| headers.get(i).unwrap()).collect();
    assert!(!header_names.contains(&"FmvDisposed"), "Tax report should NOT have FmvDisposed column");
}
