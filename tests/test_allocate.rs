mod common;

use common::{setup_test_db, create_test_csv, create_bucket_csv, default_config, get_acquisitions, get_dispositions};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::allocate::allocate;

#[test]
fn test_allocate_simple() {
    let mut conn = setup_test_db();
    let config = default_config();

    // Import 2 lots: 1.0 BTC and 0.5 BTC
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("02/01/2024", "0.50000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // Allocate: 1.0 BTC to coinbase, 0.5 BTC to ledger
    let bucket_csv = create_bucket_csv(&[
        ("coinbase", "1.00000000"),
        ("ledger", "0.50000000"),
    ]);
    allocate(&bucket_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs[0].wallet, "coinbase");
    assert_eq!(acqs[1].wallet, "ledger");
}

#[test]
fn test_allocate_with_split() {
    let mut conn = setup_test_db();
    let config = default_config();

    // Import 1 lot of 2.0 BTC
    let csv = create_test_csv(&[
        ("01/01/2024", "2.00000000", "$40,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // Allocate: 1.5 BTC to coinbase, 0.5 BTC to ledger
    let bucket_csv = create_bucket_csv(&[
        ("coinbase", "1.50000000"),
        ("ledger", "0.50000000"),
    ]);
    allocate(&bucket_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs.len(), 2, "Original lot should be split into 2");

    // First lot should be coinbase with 1.5 BTC undisposed
    assert_eq!(acqs[0].wallet, "coinbase");
    assert_eq!(acqs[0].undisposed_satoshis, 150_000_000);
    assert_eq!(acqs[0].tax_undisposed_satoshis, 150_000_000);

    // Second lot should be ledger with 0.5 BTC undisposed
    assert_eq!(acqs[1].wallet, "ledger");
    assert_eq!(acqs[1].undisposed_satoshis, 50_000_000);
    assert_eq!(acqs[1].tax_undisposed_satoshis, 50_000_000);

    // Both should have the same basis price
    assert_eq!(acqs[0].usd_cents_btc_basis, acqs[1].usd_cents_btc_basis);
    assert_eq!(acqs[0].acquisition_date, acqs[1].acquisition_date);
}

#[test]
fn test_allocate_partial_lot() {
    let mut conn = setup_test_db();
    let config = default_config();

    // Import: buy 2 BTC, sell 0.5 BTC — leaves 1.5 BTC undisposed
    let csv = create_test_csv(&[
        ("01/01/2024", "2.00000000", "$40,000.00"),
        ("02/01/2024", "-0.50000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // Allocate the 1.5 BTC undisposed portion
    let bucket_csv = create_bucket_csv(&[
        ("coinbase", "1.00000000"),
        ("ledger", "0.50000000"),
    ]);
    allocate(&bucket_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    // The lot should be split: original has 1.0 BTC undisposed (coinbase), new has 0.5 BTC (ledger)
    assert_eq!(acqs.len(), 2);

    let coinbase_lot = acqs.iter().find(|a| a.wallet == "coinbase").unwrap();
    assert_eq!(coinbase_lot.undisposed_satoshis, 100_000_000);

    let ledger_lot = acqs.iter().find(|a| a.wallet == "ledger").unwrap();
    assert_eq!(ledger_lot.undisposed_satoshis, 50_000_000);
}

#[test]
fn test_allocate_sets_disposition_wallet_to_legacy() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv(&[
        ("01/01/2024", "2.00000000", "$40,000.00"),
        ("02/01/2024", "-0.50000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let bucket_csv = create_bucket_csv(&[
        ("coinbase", "1.50000000"),
    ]);
    allocate(&bucket_csv.path().to_path_buf(), &mut conn).unwrap();

    let disps = get_dispositions(&mut conn);
    for d in &disps {
        assert_eq!(d.wallet, "legacy", "Existing dispositions should be set to 'legacy'");
    }
}

#[test]
fn test_allocate_sum_mismatch_errors() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // Try to allocate 2.0 BTC when only 1.0 BTC exists
    let bucket_csv = create_bucket_csv(&[
        ("coinbase", "2.00000000"),
    ]);
    let result = allocate(&bucket_csv.path().to_path_buf(), &mut conn);
    assert!(result.is_err(), "Should error when bucket total exceeds undisposed total");
}
