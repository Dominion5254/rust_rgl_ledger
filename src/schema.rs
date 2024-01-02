// @generated automatically by Diesel CLI.

diesel::table! {
    acquisitions (id) {
        id -> Integer,
        acquisition_date -> Timestamp,
        satoshis -> BigInt,
        undisposed_satoshis -> BigInt,
        usd_cents_btc_basis -> BigInt,
        usd_cents_btc_fair_value -> BigInt,
        usd_cents_btc_impaired_value -> BigInt,
    }
}

diesel::table! {
    dispositions (id) {
        id -> Integer,
        disposition_date -> Timestamp,
        satoshis -> BigInt,
        undisposed_satoshis -> BigInt,
        usd_cents_btc_basis -> BigInt,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    acquisitions,
    dispositions,
);
