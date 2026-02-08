mod common;

use common::{setup_test_db, create_test_csv, get_acquisitions, get_dispositions, get_acq_disps};
use rust_rgl_ledger::commands::import::import_transactions;

#[test]
fn test_single_acquisition() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[("01/15/2024", "1.00000000", "$40,000.00")]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs.len(), 1);
    assert_eq!(acqs[0].satoshis, 100_000_000);
    assert_eq!(acqs[0].undisposed_satoshis, 100_000_000);
    assert_eq!(acqs[0].usd_cents_btc_basis, 4_000_000);
    assert_eq!(acqs[0].usd_cents_btc_fair_value, 4_000_000);
}

#[test]
fn test_single_disposition() {
    let mut conn = setup_test_db();
    // Need an acquisition first so FIFO matching doesn't panic
    let csv = create_test_csv(&[
        ("01/15/2024", "1.00000000", "$40,000.00"),
        ("02/15/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let disps = get_dispositions(&mut conn);
    assert_eq!(disps.len(), 1);
    assert_eq!(disps[0].satoshis, -100_000_000);
    assert_eq!(disps[0].usd_cents_btc_basis, 4_500_000);
}

#[test]
fn test_fifo_single_buy_sell() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps.len(), 1);
    assert_eq!(acq_disps[0].satoshis, 100_000_000);
    assert_eq!(acq_disps[0].tax_basis, 4_000_000);
    assert_eq!(acq_disps[0].tax_rgl, 500_000);
    assert_eq!(acq_disps[0].gaap_basis, 4_000_000);
    assert_eq!(acq_disps[0].gaap_rgl, 500_000);
}

#[test]
fn test_fifo_partial_lot() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-0.50000000", "$45,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs[0].undisposed_satoshis, 50_000_000);

    let disps = get_dispositions(&mut conn);
    assert_eq!(disps[0].undisposed_satoshis, 0);

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps.len(), 1);
    assert_eq!(acq_disps[0].satoshis, 50_000_000);
}

#[test]
fn test_fifo_multiple_acquisitions() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "0.50000000", "$30,000.00"),
        ("02/01/2024", "0.50000000", "$40,000.00"),
        ("06/01/2024", "-0.75000000", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps.len(), 2);

    // First match: all 0.5 BTC from lot 1 at $30k
    assert_eq!(acq_disps[0].satoshis, 50_000_000);
    assert_eq!(acq_disps[0].tax_basis, 50_000_000i64 * 3_000_000 / 100_000_000); // 1_500_000
    // Second match: 0.25 BTC from lot 2 at $40k
    assert_eq!(acq_disps[1].satoshis, 25_000_000);
    assert_eq!(acq_disps[1].tax_basis, 25_000_000i64 * 4_000_000 / 100_000_000); // 1_000_000

    // Verify undisposed satoshis
    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs[0].undisposed_satoshis, 0);
    assert_eq!(acqs[1].undisposed_satoshis, 25_000_000);
}

#[test]
fn test_fifo_multiple_dispositions() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("03/01/2024", "-0.30000000", "$45,000.00"),
        ("04/01/2024", "-0.30000000", "$46,000.00"),
        ("05/01/2024", "-0.40000000", "$47,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps.len(), 3);

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs[0].undisposed_satoshis, 0);

    // Verify all dispositions are fully matched
    let disps = get_dispositions(&mut conn);
    for d in &disps {
        assert_eq!(d.undisposed_satoshis, 0);
    }
}

#[test]
fn test_term_short() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-1.00000000", "$45,000.00"), // 152 days
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps[0].term, "short");
}

#[test]
fn test_term_long() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2023", "1.00000000", "$40,000.00"),
        ("01/02/2024", "-1.00000000", "$45,000.00"), // 366 days
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps[0].term, "long");
}

#[test]
fn test_term_boundary() {
    // 364 days = short
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("12/31/2024", "-1.00000000", "$45,000.00"), // 365 days (2024 is leap year: Jan1->Dec31 = 365 days)
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();
    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps[0].term, "long");

    // Exactly 364 days = short
    let mut conn2 = setup_test_db();
    let csv2 = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("12/30/2024", "-1.00000000", "$45,000.00"), // 364 days
    ]);
    import_transactions(&csv2.path().to_path_buf(), &mut conn2).unwrap();
    let acq_disps2 = get_acq_disps(&mut conn2);
    assert_eq!(acq_disps2[0].term, "short");
}

#[test]
fn test_disposition_before_acquisition_errors() {
    // Sell without any buy — code panics (uses .expect() internally)
    let result = std::panic::catch_unwind(|| {
        let mut conn2 = setup_test_db();
        let csv2 = create_test_csv(&[
            ("06/01/2024", "-1.00000000", "$45,000.00"),
        ]);
        import_transactions(&csv2.path().to_path_buf(), &mut conn2)
    });
    assert!(result.is_err(), "Selling without a prior acquisition should panic");

    // Sell date before buy date — returns Err (hits term.num_seconds() < 0 check)
    let mut conn3 = setup_test_db();
    let csv3 = create_test_csv(&[
        ("01/01/2025", "1.00000000", "$40,000.00"),
        ("01/01/2024", "-1.00000000", "$45,000.00"),
    ]);
    let result3 = import_transactions(&csv3.path().to_path_buf(), &mut conn3);
    assert!(result3.is_err());
}

#[test]
fn test_gaap_equals_tax_before_mark_to_market() {
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.00"),
        ("06/01/2024", "-0.50000000", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    for ad in &acq_disps {
        assert_eq!(ad.gaap_basis, ad.tax_basis);
        assert_eq!(ad.gaap_rgl, ad.tax_rgl);
    }
}

#[test]
fn test_chronological_sorting() {
    let mut conn = setup_test_db();
    // CSV has dates out of order — import should sort them chronologically
    let csv = create_test_csv(&[
        ("06/01/2024", "-0.50000000", "$50,000.00"),
        ("01/01/2024", "1.00000000", "$40,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acqs = get_acquisitions(&mut conn);
    assert_eq!(acqs.len(), 1);
    assert_eq!(acqs[0].satoshis, 100_000_000);

    let disps = get_dispositions(&mut conn);
    assert_eq!(disps.len(), 1);
    assert_eq!(disps[0].satoshis, -50_000_000);

    // FIFO should have worked because the buy was processed first
    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps.len(), 1);
}

// --- Bug-exposing tests ---

#[test]
fn test_split_lot_basis_sums_to_total() {
    // Buy 1 BTC at $46,145.27, sell in 3 roughly equal parts.
    // With truncation, each partial basis was systematically low, summing to 4,614,525 (off by -2).
    // With rounding, each partial basis rounds to nearest cent. The sum may differ from the
    // exact total by at most 1 cent per split due to independent rounding, but should not
    // have the systematic downward bias that truncation causes.
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$46,145.27"),
        ("03/01/2024", "-0.33333333", "$50,000.00"),
        ("04/01/2024", "-0.33333333", "$50,000.00"),
        ("05/01/2024", "-0.33333334", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps.len(), 3);

    let total_tax_basis: i64 = acq_disps.iter().map(|ad| ad.tax_basis).sum();
    let expected = 4_614_527i64;
    let diff = (total_tax_basis - expected).abs();
    assert!(
        diff <= acq_disps.len() as i64,
        "Sum of split lot bases ({}) should be within {} cents of total basis ({}), but diff is {}",
        total_tax_basis, acq_disps.len(), expected, diff
    );
}

#[test]
fn test_basis_uses_rounding_not_truncation() {
    // Buy 1 BTC at $40,000.01, sell 0.33333333 BTC
    // Integer calc: 33_333_333 * 4_000_001 / 100_000_000 = 1_333_333 (truncated)
    // Correct: 33_333_333 * 4_000_001 = 133_333_365_333_333
    //   133_333_365_333_333 / 100_000_000 = 1_333_333.65333333 → should round to 1_333_334
    let mut conn = setup_test_db();
    let csv = create_test_csv(&[
        ("01/01/2024", "1.00000000", "$40,000.01"),
        ("06/01/2024", "-0.33333333", "$50,000.00"),
    ]);
    import_transactions(&csv.path().to_path_buf(), &mut conn).unwrap();

    let acq_disps = get_acq_disps(&mut conn);
    assert_eq!(acq_disps.len(), 1);

    assert_eq!(
        acq_disps[0].tax_basis, 1_333_334,
        "Basis should be rounded (1,333,334 cents), not truncated (1,333,333 cents), got {}",
        acq_disps[0].tax_basis
    );
}
