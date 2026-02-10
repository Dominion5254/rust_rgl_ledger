mod common;

use common::{setup_test_db, create_test_csv_with_wallet, default_config, universal_config,
             get_acquisitions, get_tax_acq_disps, get_gaap_acq_disps};
use rust_rgl_ledger::commands::import::import_transactions;

#[test]
fn test_wallet_scoped_tax_fifo() {
    // Two wallets with lots at different prices.
    // Tax FIFO (wallet-scoped) should match within each wallet.
    // GAAP FIFO (universal) should match the globally-earliest lot.
    let mut conn = setup_test_db();
    let config = default_config(); // tax=wallet, gaap=universal

    let csv = create_test_csv_with_wallet(&[
        ("01/01/2025", "1.00000000", "$40,000.00", "ledger"),     // lot 1: ledger, earliest
        ("02/01/2025", "1.00000000", "$50,000.00", "coinbase"),   // lot 2: coinbase
        ("06/01/2025", "-0.50000000", "$60,000.00", "coinbase"),  // sell from coinbase
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let tax_ads = get_tax_acq_disps(&mut conn);
    assert_eq!(tax_ads.len(), 1, "Tax should have 1 match");
    // Tax should match against coinbase lot (wallet-scoped)
    // Coinbase lot is lot 2, acquisition_id depends on insert order
    let acqs = get_acquisitions(&mut conn);
    let coinbase_lot = acqs.iter().find(|a| a.wallet == "coinbase").unwrap();
    assert_eq!(tax_ads[0].acquisition_id, coinbase_lot.id,
        "Tax FIFO should match the coinbase lot (wallet-scoped)");
    assert_eq!(tax_ads[0].basis, 50_000_000i64 * 5_000_000 / 100_000_000,
        "Tax basis should use coinbase lot's cost basis ($50k)");

    let gaap_ads = get_gaap_acq_disps(&mut conn);
    assert_eq!(gaap_ads.len(), 1, "GAAP should have 1 match");
    // GAAP should match against the globally-earliest lot (ledger)
    let ledger_lot = acqs.iter().find(|a| a.wallet == "ledger").unwrap();
    assert_eq!(gaap_ads[0].acquisition_id, ledger_lot.id,
        "GAAP FIFO should match the ledger lot (universal — it's earliest)");
    assert_eq!(gaap_ads[0].basis, 50_000_000i64 * 4_000_000 / 100_000_000,
        "GAAP basis should use ledger lot's fair value ($40k, same as cost since no MTM)");
}

#[test]
fn test_universal_gaap_fifo_with_wallets() {
    // GAAP uses universal FIFO so it should pick the earliest lot regardless of wallet.
    // Tax is wallet-scoped so selling 0.5 from coinbase matches coinbase lots only.
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv_with_wallet(&[
        ("01/01/2025", "1.00000000", "$30,000.00", "ledger"),
        ("02/01/2025", "1.00000000", "$40,000.00", "coinbase"),
        ("03/01/2025", "-0.50000000", "$50,000.00", "coinbase"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let gaap_ads = get_gaap_acq_disps(&mut conn);
    // GAAP universal FIFO: matches the ledger lot (globally earliest)
    assert_eq!(gaap_ads.len(), 1, "GAAP should have 1 match");

    let acqs = get_acquisitions(&mut conn);
    let ledger_lot = acqs.iter().find(|a| a.wallet == "ledger").unwrap();
    assert_eq!(gaap_ads[0].acquisition_id, ledger_lot.id,
        "GAAP should match the ledger lot (earliest globally)");
    assert_eq!(gaap_ads[0].satoshis, 50_000_000);

    let tax_ads = get_tax_acq_disps(&mut conn);
    // Tax wallet-scoped: matches within coinbase only
    assert_eq!(tax_ads.len(), 1, "Tax should have 1 match");
    let coinbase_lot = acqs.iter().find(|a| a.wallet == "coinbase").unwrap();
    assert_eq!(tax_ads[0].acquisition_id, coinbase_lot.id,
        "Tax should match the coinbase lot (wallet-scoped)");
}

#[test]
fn test_dual_matching_different_lots() {
    // Tax and GAAP match DIFFERENT acquisition lots for the same disposition
    let mut conn = setup_test_db();
    let config = default_config(); // tax=wallet, gaap=universal

    let csv = create_test_csv_with_wallet(&[
        ("01/01/2025", "1.00000000", "$30,000.00", "ledger"),
        ("02/01/2025", "1.00000000", "$40,000.00", "coinbase"),
        ("06/01/2025", "-0.50000000", "$50,000.00", "coinbase"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let tax_ads = get_tax_acq_disps(&mut conn);
    let gaap_ads = get_gaap_acq_disps(&mut conn);

    // Tax: wallet-scoped → matches coinbase lot
    // GAAP: universal → matches ledger lot (earliest globally)
    assert_ne!(tax_ads[0].acquisition_id, gaap_ads[0].acquisition_id,
        "Tax and GAAP should match DIFFERENT lots when scopes differ");
}

#[test]
fn test_universal_tax_and_gaap_match_same_lots() {
    // When both tax and GAAP are universal, they should match the same lots
    let mut conn = setup_test_db();
    let config = universal_config(); // both universal

    let csv = create_test_csv_with_wallet(&[
        ("01/01/2025", "1.00000000", "$30,000.00", "ledger"),
        ("02/01/2025", "1.00000000", "$40,000.00", "coinbase"),
        ("06/01/2025", "-0.50000000", "$50,000.00", "coinbase"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let tax_ads = get_tax_acq_disps(&mut conn);
    let gaap_ads = get_gaap_acq_disps(&mut conn);

    assert_eq!(tax_ads[0].acquisition_id, gaap_ads[0].acquisition_id,
        "Both should match the same lot when both are universal");
}

#[test]
fn test_wallet_assigned_on_import() {
    let mut conn = setup_test_db();
    let config = default_config();

    let csv = create_test_csv_with_wallet(&[
        ("01/01/2025", "1.00000000", "$40,000.00", "coinbase"),
        ("02/01/2025", "0.50000000", "$45,000.00", "ledger"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn, &config).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs.len(), 2);
    assert_eq!(acqs[0].wallet, "coinbase");
    assert_eq!(acqs[1].wallet, "ledger");
}
