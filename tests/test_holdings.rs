mod common;

use common::{setup_test_db, create_test_csv};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::holdings::holdings;
use std::sync::Mutex;

static CWD_LOCK: Mutex<()> = Mutex::new(());

fn run_holdings_in_dir(date: &str, conn: &mut diesel::SqliteConnection) -> String {
    let _lock = CWD_LOCK.lock().unwrap();
    let report_dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(report_dir.path().join("reports")).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(report_dir.path()).unwrap();

    holdings(&date.to_string(), conn).unwrap();

    let report_files: Vec<_> = std::fs::read_dir("./reports")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("holdings_"))
        .collect();

    let content = if report_files.is_empty() {
        String::new()
    } else {
        std::fs::read_to_string(report_files[0].path()).unwrap()
    };

    std::env::set_current_dir(original_dir).unwrap();
    content
}

/// Parse CSV into rows. Returns (header_fields, data_rows).
fn parse_holdings_csv(content: &str) -> Vec<Vec<String>> {
    let mut rdr = csv::ReaderBuilder::new().has_headers(true).from_reader(content.as_bytes());
    rdr.records()
        .filter_map(|r| r.ok())
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect()
}

#[test]
fn test_holdings_single_lot() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[("01/15/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_holdings_in_dir("01/15/2024", &mut conn);
    let rows = parse_holdings_csv(&content);
    // Should have 1 detail row + 1 totals row = 2 rows
    assert_eq!(rows.len(), 2, "Expected 2 rows (detail + totals): {}", content);
    // Detail row: [acquisition_date, btc, undisposed_btc, usd_basis, usd_fair_value]
    assert_eq!(rows[0][0], "2024-01-15T00:00:00");
    // Btc = 1 (Decimal renders without trailing zeros for whole numbers)
    assert!(rows[0][1].starts_with("1"), "Btc should be 1: got {}", rows[0][1]);
    assert!(rows[0][3].contains("40000"), "Should reference $40k basis: got {}", rows[0][3]);
}

#[test]
fn test_holdings_excludes_future_acquisitions() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "0.50000000", "$30,000.00"),
        ("03/01/2024", "0.50000000", "$40,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_holdings_in_dir("02/01/2024", &mut conn);
    let rows = parse_holdings_csv(&content);
    // Only one detail row (Jan 1 lot), plus totals
    assert_eq!(rows.len(), 2, "Expected 2 rows (1 detail + 1 totals): {}", content);
    assert_eq!(rows[0][0], "2024-01-01T00:00:00");
    // Should show 0.50 BTC, not the $40k lot
    assert!(rows[0][3].contains("15000"), "Basis for 0.5 BTC at $30k = $15k: got {}", rows[0][3]);
}

#[test]
fn test_holdings_adjusts_for_dispositions() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("02/01/2024", "-0.50000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_holdings_in_dir("03/01/2024", &mut conn);
    let rows = parse_holdings_csv(&content);
    assert_eq!(rows.len(), 2, "Expected 2 rows: {}", content);
    // undisposed_btc (index 2) should be 0.50
    assert!(rows[0][2].starts_with("0.5"), "Should show 0.5 BTC undisposed: got {}", rows[0][2]);
}

#[test]
fn test_holdings_subsequent_disposals_added_back() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("03/01/2024", "-0.50000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    // Holdings as-of Feb 1 — the Mar 1 disposal hadn't happened yet
    let content = run_holdings_in_dir("02/01/2024", &mut conn);
    let rows = parse_holdings_csv(&content);
    assert_eq!(rows.len(), 2, "Expected 2 rows: {}", content);
    // undisposed_btc (index 2) should be 1 (full lot since disposal is in the future)
    assert!(rows[0][2].starts_with("1"), "Should show full 1.0 BTC before disposal date: got {}", rows[0][2]);
}

#[test]
fn test_holdings_fully_disposed_lot_excluded() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("02/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let content = run_holdings_in_dir("03/01/2024", &mut conn);
    // When fully disposed, the holdings function skips the lot (continue on undisposed_btc == 0).
    // Only the totals row remains. But the totals row is written with write_record, not serialize,
    // so it won't have a header. The CSV has a header from the first serialized row or none at all.
    // With no detail rows, the output is just the totals: ",0,0,0,0\n"
    // Parse without assuming headers
    let mut rdr = csv::ReaderBuilder::new().has_headers(false).from_reader(content.as_bytes());
    let rows: Vec<csv::StringRecord> = rdr.records().filter_map(|r| r.ok()).collect();
    // Should have exactly 1 row (the totals row with all zeros)
    assert_eq!(rows.len(), 1, "Should only have totals row: {}", content);
    // Totals row should show 0 for all values
    assert_eq!(rows[0].get(1).unwrap(), "0", "Total BTC should be 0");
    assert_eq!(rows[0].get(2).unwrap(), "0", "Total undisposed BTC should be 0");
}
