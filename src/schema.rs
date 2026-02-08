// @generated automatically by Diesel CLI.

diesel::table! {
    acquisition_dispositions (acquisition_id, disposition_id, match_type) {
        acquisition_id -> Integer,
        disposition_id -> Integer,
        match_type -> Text,
        satoshis -> BigInt,
        basis -> BigInt,
        rgl -> BigInt,
        term -> Text,
    }
}

diesel::table! {
    acquisition_fair_values (acquisition_id, fair_value_id) {
        acquisition_id -> Integer,
        fair_value_id -> Integer,
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
        wallet -> Text,
        tax_undisposed_satoshis -> BigInt,
    }
}

diesel::table! {
    dispositions (id) {
        id -> Integer,
        disposition_date -> Timestamp,
        satoshis -> BigInt,
        undisposed_satoshis -> BigInt,
        usd_cents_btc_basis -> BigInt,
        wallet -> Text,
        tax_undisposed_satoshis -> BigInt,
    }
}

diesel::table! {
    fair_values (id) {
        id -> Integer,
        fair_value_cents -> BigInt,
        date -> Timestamp,
    }
}

diesel::joinable!(acquisition_dispositions -> acquisitions (acquisition_id));
diesel::joinable!(acquisition_dispositions -> dispositions (disposition_id));
diesel::joinable!(acquisition_fair_values -> acquisitions (acquisition_id));
diesel::joinable!(acquisition_fair_values -> fair_values (fair_value_id));

diesel::allow_tables_to_appear_in_same_query!(
    acquisition_dispositions,
    acquisition_fair_values,
    acquisitions,
    dispositions,
    fair_values,
);
