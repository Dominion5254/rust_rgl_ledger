mod common;

use rust_rgl_ledger::models::NewRecord;

fn parse_record(date: &str, bitcoin: &str, price: &str) -> NewRecord {
    let csv_data = format!("Date,Bitcoin,Price\n{},\"{}\",\"{}\"\n", date, bitcoin, price);
    let mut rdr = csv::ReaderBuilder::new().from_reader(csv_data.as_bytes());
    rdr.deserialize::<NewRecord>().next().unwrap().unwrap()
}

fn parse_record_with_wallet(date: &str, bitcoin: &str, price: &str, wallet: &str) -> NewRecord {
    let csv_data = format!("Date,Bitcoin,Price,Wallet\n{},\"{}\",\"{}\",{}\n", date, bitcoin, price, wallet);
    let mut rdr = csv::ReaderBuilder::new().from_reader(csv_data.as_bytes());
    rdr.deserialize::<NewRecord>().next().unwrap().unwrap()
}

// --- Date format tests ---

#[test]
fn test_date_format_slash_mdy() {
    let record = parse_record("01/15/24", "1.00000000", "$50,000.00");
    assert_eq!(record.date.format("%Y-%m-%d").to_string(), "2024-01-15");
}

#[test]
fn test_date_format_slash_mdy_4y() {
    let record = parse_record("01/15/2024", "1.00000000", "$50,000.00");
    assert_eq!(record.date.format("%Y-%m-%d").to_string(), "2024-01-15");
}

#[test]
fn test_date_format_iso() {
    let record = parse_record("2024-01-15", "1.00000000", "$50,000.00");
    assert_eq!(record.date.format("%Y-%m-%d").to_string(), "2024-01-15");
}

#[test]
fn test_date_format_with_time() {
    let record = parse_record("01/15/24 14:30:00", "1.00000000", "$50,000.00");
    assert_eq!(record.date.format("%Y-%m-%d %H:%M:%S").to_string(), "2024-01-15 14:30:00");
}

#[test]
fn test_date_format_12h() {
    let record = parse_record("01/15/24 2:30 PM", "1.00000000", "$50,000.00");
    assert_eq!(record.date.format("%Y-%m-%d %H:%M").to_string(), "2024-01-15 14:30");
}

// --- Price deserialization tests ---

#[test]
fn test_price_whole_dollars() {
    let record = parse_record("01/01/24", "1.00000000", "$50,000.00");
    assert_eq!(record.price, 5_000_000);
}

#[test]
fn test_price_with_cents() {
    let record = parse_record("01/01/24", "1.00000000", "$46,145.26");
    assert_eq!(record.price, 4_614_526);
}

#[test]
fn test_price_no_commas() {
    let record = parse_record("01/01/24", "1.00000000", "$100.50");
    assert_eq!(record.price, 10_050);
}

// --- Bitcoin deserialization tests ---

#[test]
fn test_bitcoin_whole() {
    let record = parse_record("01/01/24", "1.00000000", "$50,000.00");
    assert_eq!(record.bitcoin, 100_000_000);
}

#[test]
fn test_bitcoin_fractional() {
    let record = parse_record("01/01/24", "0.50000000", "$50,000.00");
    assert_eq!(record.bitcoin, 50_000_000);
}

#[test]
fn test_bitcoin_negative() {
    let record = parse_record("01/01/24", "-0.50000000", "$50,000.00");
    assert_eq!(record.bitcoin, -50_000_000);
}

#[test]
fn test_bitcoin_small() {
    let record = parse_record("01/01/24", "0.00000001", "$50,000.00");
    assert_eq!(record.bitcoin, 1);
}

// --- Wallet tests ---

#[test]
fn test_csv_backward_compat_no_wallet() {
    let record = parse_record("01/01/24", "1.00000000", "$50,000.00");
    assert_eq!(record.wallet, "default", "Missing wallet column should default to 'default'");
}

#[test]
fn test_csv_with_wallet() {
    let record = parse_record_with_wallet("01/01/24", "1.00000000", "$50,000.00", "coinbase");
    assert_eq!(record.wallet, "coinbase");
}

// --- Bug-exposing tests (f64 precision) ---

#[test]
fn test_price_deserialize_uses_decimal_precision() {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let test_prices = [
        "$0.29", "$0.57", "$0.58", "$1.13", "$1.14", "$1.15",
        "$2.01", "$2.03", "$2.05",
    ];

    for price_str in &test_prices {
        let record = parse_record("01/01/24", "1.00000000", price_str);
        let cleaned = price_str.replace("$", "").replace(",", "");
        let expected = (Decimal::from_str(&cleaned).unwrap() * Decimal::from(100))
            .round()
            .to_string()
            .parse::<i64>()
            .unwrap();
        assert_eq!(
            record.price, expected,
            "Price mismatch for {}: got {} expected {}",
            price_str, record.price, expected
        );
    }
}

#[test]
fn test_bitcoin_deserialize_uses_decimal_precision() {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let test_amounts = [
        "0.00000001", "0.12345678", "1.23456789", "0.99999999",
        "0.33333333", "21000000.00000000", "0.00010001",
    ];

    for btc_str in &test_amounts {
        let record = parse_record("01/01/24", btc_str, "$50,000.00");
        let expected = (Decimal::from_str(btc_str).unwrap() * Decimal::from(100_000_000i64))
            .round()
            .to_string()
            .parse::<i64>()
            .unwrap();
        assert_eq!(
            record.bitcoin, expected,
            "Bitcoin mismatch for {}: got {} expected {}",
            btc_str, record.bitcoin, expected
        );
    }
}
