// @generated automatically by Diesel CLI.

diesel::table! {
    acquisition_dispositions (acquisition_id, disposition_id) {
        acquisition_id -> Integer,
        disposition_id -> Integer,
        satoshis -> BigInt,
        gaap_basis -> BigInt,
        gaap_rgl -> BigInt,
        tax_basis -> BigInt,
        tax_rgl -> BigInt,
        term -> Text,
    }
}

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

diesel::table! {
    impairments (id) {
        id -> Integer,
        impairment_cents -> BigInt,
        date -> Timestamp,
    }
}

diesel::joinable!(acquisition_dispositions -> acquisitions (acquisition_id));
diesel::joinable!(acquisition_dispositions -> dispositions (disposition_id));

diesel::allow_tables_to_appear_in_same_query!(
    acquisition_dispositions,
    acquisitions,
    dispositions,
    impairments,
);
