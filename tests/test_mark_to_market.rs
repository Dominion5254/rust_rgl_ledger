mod common;

use common::{setup_test_db, create_test_csv, get_acquisitions, get_fair_value_count, get_acq_fair_value_count};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::mark_to_market::mark_to_market;
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

#[test]
fn test_mtm_inserts_fair_value() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[("01/01/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    assert_eq!(get_fair_value_count(&mut conn), 0);
    run_mtm_in_dir("$45,000.00", "06/30/2024", &mut conn);
    assert_eq!(get_fair_value_count(&mut conn), 1);
}

#[test]
fn test_mtm_updates_acquisition_fair_value() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[("01/01/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs_before = get_acquisitions(&mut conn);
    assert_eq!(acqs_before[0].usd_cents_btc_fair_value, 4_000_000);

    run_mtm_in_dir("$45,000.00", "06/30/2024", &mut conn);

    let acqs_after = get_acquisitions(&mut conn);
    assert_eq!(acqs_after[0].usd_cents_btc_fair_value, 4_500_000);
}

#[test]
fn test_mtm_only_affects_undisposed_lots() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("02/01/2024", "1.00000000", "$42,000.00"),
        ("03/01/2024", "-1.00000000", "$44,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs[0].undisposed_satoshis, 0);
    assert!(acqs[1].undisposed_satoshis > 0);

    run_mtm_in_dir("$50,000.00", "06/30/2024", &mut conn);

    let acqs_after = get_acquisitions(&mut conn);
    assert_eq!(acqs_after[0].usd_cents_btc_fair_value, 4_000_000, "Fully disposed lot fair value should not change");
    assert_eq!(acqs_after[1].usd_cents_btc_fair_value, 5_000_000, "Undisposed lot fair value should be updated");
}

#[test]
fn test_mtm_only_affects_lots_before_date() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "0.50000000", "$40,000.00"),
        ("08/01/2024", "0.50000000", "$42,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    run_mtm_in_dir("$50,000.00", "06/30/2024", &mut conn);

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs[0].usd_cents_btc_fair_value, 5_000_000, "Jan lot should be marked to $50k");
    assert_eq!(acqs[1].usd_cents_btc_fair_value, 4_200_000, "Aug lot should still be at acquisition price $42k");
}

#[test]
fn test_mtm_creates_acquisition_fair_value_links() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "0.50000000", "$40,000.00"),
        ("02/01/2024", "0.50000000", "$42,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    assert_eq!(get_acq_fair_value_count(&mut conn), 0);
    run_mtm_in_dir("$45,000.00", "06/30/2024", &mut conn);
    assert_eq!(get_acq_fair_value_count(&mut conn), 2);
}

#[test]
fn test_mtm_successive_adjustments() {
    let _lock = CWD_LOCK.lock().unwrap();
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[("01/01/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    run_mtm_in_dir("$45,000.00", "03/31/2024", &mut conn);
    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs[0].usd_cents_btc_fair_value, 4_500_000);

    run_mtm_in_dir("$50,000.00", "06/30/2024", &mut conn);
    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs[0].usd_cents_btc_fair_value, 5_000_000);

    assert_eq!(get_fair_value_count(&mut conn), 2);
    assert_eq!(get_acq_fair_value_count(&mut conn), 2);
}
