mod common;

use common::{setup_test_db, create_test_csv_with_wallet, create_transfer_csv, default_config, get_acquisitions};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::transfer::transfer;

#[test]
fn test_transfer_whole_lot() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv_with_wallet(&[
        ("2024-01-01", "1.00000000", "$40,000.00", "cold-storage"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let transfer_csv = create_transfer_csv(&[
        ("2024-06-15", "cold-storage", "exchange", "1.00000000"),
    ]);
    transfer(&transfer_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs.len(), 1, "No new lots should be created for a whole-lot transfer");
    assert_eq!(acqs[0].wallet, "exchange");
    assert_eq!(acqs[0].satoshis, 100_000_000);
    assert_eq!(acqs[0].undisposed_satoshis, 100_000_000);
    assert_eq!(acqs[0].tax_undisposed_satoshis, 100_000_000);
    assert_eq!(acqs[0].usd_cents_btc_basis, 4_000_000);
    assert_eq!(acqs[0].usd_cents_btc_fair_value, 4_000_000);
    assert_eq!(acqs[0].acquisition_date.format("%Y-%m-%d").to_string(), "2024-01-01");
}

#[test]
fn test_transfer_requires_lot_split() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv_with_wallet(&[
        ("2024-01-01", "2.00000000", "$40,000.00", "cold-storage"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let transfer_csv = create_transfer_csv(&[
        ("2024-06-15", "cold-storage", "exchange", "0.75000000"),
    ]);
    transfer(&transfer_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs.len(), 2, "Lot should be split into 2");

    let remaining = acqs.iter().find(|a| a.wallet == "cold-storage").unwrap();
    assert_eq!(remaining.tax_undisposed_satoshis, 125_000_000);
    assert_eq!(remaining.undisposed_satoshis, 125_000_000);

    let transferred = acqs.iter().find(|a| a.wallet == "exchange").unwrap();
    assert_eq!(transferred.tax_undisposed_satoshis, 75_000_000);
    assert_eq!(transferred.undisposed_satoshis, 75_000_000);

    // Same basis and date
    assert_eq!(remaining.usd_cents_btc_basis, transferred.usd_cents_btc_basis);
    assert_eq!(remaining.acquisition_date, transferred.acquisition_date);
}

#[test]
fn test_transfer_spans_multiple_lots_fifo() {
    let mut conn = setup_test_db();
    let config = default_config();

    // 3 lots in cold-storage: 0.5, 0.3, 1.0 BTC
    let csv = create_test_csv_with_wallet(&[
        ("2024-01-01", "0.50000000", "$30,000.00", "cold-storage"),
        ("2024-02-01", "0.30000000", "$35,000.00", "cold-storage"),
        ("2024-03-01", "1.00000000", "$40,000.00", "cold-storage"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // Transfer 1.0 BTC: should consume lot 1 (0.5), lot 2 (0.3), and split lot 3 (0.2)
    let transfer_csv = create_transfer_csv(&[
        ("2024-06-15", "cold-storage", "exchange", "1.00000000"),
    ]);
    transfer(&transfer_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    // 4 lots: 3 original (2 moved whole, 1 reduced) + 1 new from split
    assert_eq!(acqs.len(), 4);

    let exchange_lots: Vec<_> = acqs.iter().filter(|a| a.wallet == "exchange").collect();
    let cold_lots: Vec<_> = acqs.iter().filter(|a| a.wallet == "cold-storage").collect();

    assert_eq!(exchange_lots.len(), 3, "3 lots moved to exchange (2 whole + 1 split)");
    assert_eq!(cold_lots.len(), 1, "1 lot remaining in cold-storage");

    let exchange_total: i64 = exchange_lots.iter().map(|a| a.tax_undisposed_satoshis).sum();
    assert_eq!(exchange_total, 100_000_000, "Total transferred should be 1.0 BTC");

    assert_eq!(cold_lots[0].tax_undisposed_satoshis, 80_000_000, "Remaining should be 0.8 BTC");
}

#[test]
fn test_transfer_with_diverged_trackers() {
    let mut conn = setup_test_db();
    let config = default_config();

    // Import 2 lots in different wallets, then sell from wallet-A.
    // With default config (GAAP=universal, tax=wallet), GAAP and tax trackers diverge.
    let csv = create_test_csv_with_wallet(&[
        ("2024-01-01", "1.00000000", "$30,000.00", "wallet-a"),
        ("2024-02-01", "1.00000000", "$40,000.00", "wallet-b"),
        ("2024-06-01", "-0.50000000", "$50,000.00", "wallet-a"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // After disposal: wallet-a lot has tax_undisposed=50M, gaap_undisposed=50M
    // wallet-b lot has tax_undisposed=100M, gaap_undisposed=100M
    // (with universal GAAP, the GAAP tracker on lot 1 is consumed first)
    let acqs_before = get_acquisitions(&mut conn);
    let lot_b = acqs_before.iter().find(|a| a.wallet == "wallet-b").unwrap();
    let gaap_before = lot_b.undisposed_satoshis;
    let tax_before = lot_b.tax_undisposed_satoshis;

    // Transfer half of wallet-b to wallet-c
    let transfer_csv = create_transfer_csv(&[
        ("2024-07-01", "wallet-b", "wallet-c", "0.50000000"),
    ]);
    transfer(&transfer_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    let lot_b_after = acqs.iter().find(|a| a.wallet == "wallet-b" && a.acquisition_date.format("%Y-%m-%d").to_string() == "2024-02-01").unwrap();
    let lot_c = acqs.iter().find(|a| a.wallet == "wallet-c").unwrap();

    // Proportional GAAP split: transferred_gaap = rounding_div(gaap_before * 50M, tax_before)
    let expected_transferred_gaap = rust_rgl_ledger::rounding_div(gaap_before as i128 * 50_000_000i128, tax_before as i128);
    assert_eq!(lot_c.undisposed_satoshis, expected_transferred_gaap);
    assert_eq!(lot_c.tax_undisposed_satoshis, 50_000_000);

    assert_eq!(lot_b_after.undisposed_satoshis, gaap_before - expected_transferred_gaap);
    assert_eq!(lot_b_after.tax_undisposed_satoshis, tax_before - 50_000_000);
}

#[test]
fn test_transfer_insufficient_sats_error() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv_with_wallet(&[
        ("2024-01-01", "0.50000000", "$40,000.00", "cold-storage"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let transfer_csv = create_transfer_csv(&[
        ("2024-06-15", "cold-storage", "exchange", "1.00000000"),
    ]);
    let result = transfer(&transfer_csv.path().to_path_buf(), &mut conn);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Insufficient"));

    // DB unchanged
    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs.len(), 1);
    assert_eq!(acqs[0].wallet, "cold-storage");
    assert_eq!(acqs[0].tax_undisposed_satoshis, 50_000_000);
}

#[test]
fn test_transfer_multiple_rows_date_ordered() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv_with_wallet(&[
        ("2024-01-01", "1.00000000", "$40,000.00", "wallet-a"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // CSV rows in reverse date order — should still process June first, then July
    let transfer_csv = create_transfer_csv(&[
        ("2024-07-01", "wallet-b", "wallet-c", "0.30000000"),
        ("2024-06-01", "wallet-a", "wallet-b", "0.60000000"),
    ]);
    transfer(&transfer_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);

    let wallet_a_sats: i64 = acqs.iter().filter(|a| a.wallet == "wallet-a").map(|a| a.tax_undisposed_satoshis).sum();
    let wallet_b_sats: i64 = acqs.iter().filter(|a| a.wallet == "wallet-b").map(|a| a.tax_undisposed_satoshis).sum();
    let wallet_c_sats: i64 = acqs.iter().filter(|a| a.wallet == "wallet-c").map(|a| a.tax_undisposed_satoshis).sum();

    assert_eq!(wallet_a_sats, 40_000_000, "wallet-a should have 0.4 BTC remaining");
    assert_eq!(wallet_b_sats, 30_000_000, "wallet-b: received 0.6, sent 0.3 = 0.3 BTC");
    assert_eq!(wallet_c_sats, 30_000_000, "wallet-c should have 0.3 BTC");
}

#[test]
fn test_transfer_split_sums_equal_original() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv_with_wallet(&[
        ("2024-01-01", "1.00000000", "$40,000.00", "cold-storage"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let acqs_before = get_acquisitions(&mut conn);
    let original_satoshis = acqs_before[0].satoshis;
    let original_gaap = acqs_before[0].undisposed_satoshis;
    let original_tax = acqs_before[0].tax_undisposed_satoshis;

    let transfer_csv = create_transfer_csv(&[
        ("2024-06-15", "cold-storage", "exchange", "0.33333333"),
    ]);
    transfer(&transfer_csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs.len(), 2);

    let total_satoshis: i64 = acqs.iter().map(|a| a.satoshis).sum();
    let total_gaap: i64 = acqs.iter().map(|a| a.undisposed_satoshis).sum();
    let total_tax: i64 = acqs.iter().map(|a| a.tax_undisposed_satoshis).sum();

    assert_eq!(total_satoshis, original_satoshis, "satoshis must sum to original");
    assert_eq!(total_gaap, original_gaap, "GAAP undisposed must sum to original");
    assert_eq!(total_tax, original_tax, "tax undisposed must sum to original");
}

#[test]
fn test_transfer_rollback_on_error() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv_with_wallet(&[
        ("2024-01-01", "1.00000000", "$40,000.00", "wallet-a"),
        ("2024-02-01", "0.20000000", "$45,000.00", "wallet-b"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    // First row succeeds, second row fails (insufficient in wallet-b)
    let transfer_csv = create_transfer_csv(&[
        ("2024-06-01", "wallet-a", "wallet-c", "0.50000000"),
        ("2024-07-01", "wallet-b", "wallet-d", "0.50000000"),
    ]);
    let result = transfer(&transfer_csv.path().to_path_buf(), &mut conn);
    assert!(result.is_err());

    // First row's changes should be rolled back
    let acqs = get_acquisitions(&mut conn);
    let wallet_a_sats: i64 = acqs.iter().filter(|a| a.wallet == "wallet-a").map(|a| a.tax_undisposed_satoshis).sum();
    let wallet_b_sats: i64 = acqs.iter().filter(|a| a.wallet == "wallet-b").map(|a| a.tax_undisposed_satoshis).sum();
    let wallet_c_lots: Vec<_> = acqs.iter().filter(|a| a.wallet == "wallet-c").collect();
    let wallet_d_lots: Vec<_> = acqs.iter().filter(|a| a.wallet == "wallet-d").collect();

    assert_eq!(wallet_a_sats, 100_000_000, "wallet-a should be unchanged after rollback");
    assert_eq!(wallet_b_sats, 20_000_000, "wallet-b should be unchanged after rollback");
    assert!(wallet_c_lots.is_empty(), "wallet-c should not exist after rollback");
    assert!(wallet_d_lots.is_empty(), "wallet-d should not exist after rollback");
}
