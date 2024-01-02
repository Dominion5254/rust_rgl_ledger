-- Your SQL goes here
CREATE TABLE dispositions (
  id INTEGER PRIMARY KEY NOT NULL,
  disposition_date DATETIME NOT NULL,
  satoshis BIGINT NOT NULL,
  undisposed_satoshis BIGINT NOT NULL,
  usd_cents_btc_basis BIGINT NOT NULL
)