mod common;

use common::{setup_test_db, create_test_csv};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::report::report;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Mutex;

static CWD_LOCK: Mutex<()> = Mutex::new(());

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

fn parse_report_rows(content: &str) -> Vec<csv::StringRecord> {
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    rdr.records().filter_map(|r| r.ok()).collect()
}

#[test]
fn test_report_short_term_only() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("03/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_report_in_dir("01/01/2024", "12/31/2024", &mut conn);
    let rows = parse_report_rows(&content);

    let mut found_short = false;
    let mut found_long = false;
    for row in &rows {
        if let Some(term) = row.get(9) {
            if term == "short" { found_short = true; }
            if term == "long" { found_long = true; }
        }
    }
    assert!(found_short, "Should have short-term entries in report");
    assert!(!found_long, "Should not have long-term entries");
}

#[test]
fn test_report_long_term_only() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2023", "1.00000000", "$30,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_report_in_dir("01/01/2024", "12/31/2024", &mut conn);
    let rows = parse_report_rows(&content);

    let mut found_short = false;
    let mut found_long = false;
    for row in &rows {
        if let Some(term) = row.get(9) {
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
    let csv = create_test_csv(&[
        ("01/01/2023", "1.00000000", "$30,000.00"),
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.50000000", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_report_in_dir("01/01/2024", "12/31/2024", &mut conn);
    let rows = parse_report_rows(&content);

    let mut found_short = false;
    let mut found_long = false;
    for row in &rows {
        if let Some(term) = row.get(9) {
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
    let csv = create_test_csv(&[
        ("01/01/2024", "2.00000000", "$40,000.00"),
        ("03/01/2024", "-0.50000000", "$45,000.00"),
        ("07/01/2024", "-0.50000000", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_report_in_dir("01/01/2024", "03/31/2024", &mut conn);

    let rows = parse_report_rows(&content);
    let detail_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(9).map_or(false, |s| !s.is_empty())
    }).collect();

    assert_eq!(detail_rows.len(), 1, "Should have exactly 1 detail row for Q1 report");
}

#[test]
fn test_report_totals_match_sum() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "2.00000000", "$40,000.00"),
        ("03/01/2024", "-0.30000000", "$45,000.00"),
        ("04/01/2024", "-0.70000000", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_report_in_dir("01/01/2024", "12/31/2024", &mut conn);
    let rows = parse_report_rows(&content);

    let detail_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(9).map_or(false, |s| s == "short" || s == "long")
    }).collect();

    let total_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| s.is_empty()) &&
        r.get(2).map_or(false, |s| !s.is_empty()) &&
        r.get(9).map_or(false, |s| s == "short" || s == "long")
    }).collect();

    assert!(!detail_rows.is_empty(), "Should have detail rows");
    assert!(!total_rows.is_empty(), "Should have totals row");

    let detail_sum: Decimal = detail_rows.iter()
        .map(|r| Decimal::from_str(r.get(5).unwrap()).unwrap())
        .sum();
    let total_tax_rgl = Decimal::from_str(total_rows[0].get(5).unwrap()).unwrap();

    assert_eq!(detail_sum, total_tax_rgl, "Sum of detail tax_rgl should equal total tax_rgl");
}

// --- Bug-exposing test ---

#[test]
fn test_report_disposal_fmv_minus_basis_equals_rgl() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-0.33333333", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_report_in_dir("01/01/2024", "12/31/2024", &mut conn);
    let rows = parse_report_rows(&content);

    let detail_rows: Vec<_> = rows.iter().filter(|r| {
        r.get(0).map_or(false, |s| !s.is_empty()) &&
        r.get(9).map_or(false, |s| s == "short" || s == "long")
    }).collect();

    for row in &detail_rows {
        let disposal_fmv = Decimal::from_str(row.get(3).unwrap()).unwrap();
        let tax_basis = Decimal::from_str(row.get(4).unwrap()).unwrap();
        let tax_rgl = Decimal::from_str(row.get(5).unwrap()).unwrap();

        assert_eq!(
            disposal_fmv - tax_basis, tax_rgl,
            "disposal_fmv ({}) - tax_basis ({}) should equal tax_rgl ({}), but got {}",
            disposal_fmv, tax_basis, tax_rgl, disposal_fmv - tax_basis
        );
    }
}
