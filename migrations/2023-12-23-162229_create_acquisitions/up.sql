-- Your SQL goes here
CREATE TABLE acquisitions (
  id INTEGER PRIMARY KEY NOT NULL,
  acquisition_date TIMESTAMP NOT NULL,
  satoshis BIGINT NOT NULL,
  undisposed_satoshis BIGINT NOT NULL,
  usd_cents_btc_basis BIGINT NOT NULL,
  usd_cents_btc_fair_value BIGINT NOT NULL,
  usd_cents_btc_impaired_value BIGINT NOT NULL
)