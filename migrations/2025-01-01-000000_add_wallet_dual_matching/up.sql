-- Add wallet and tax_undisposed_satoshis to acquisitions
ALTER TABLE acquisitions ADD COLUMN wallet TEXT NOT NULL DEFAULT 'default';
ALTER TABLE acquisitions ADD COLUMN tax_undisposed_satoshis BIGINT NOT NULL DEFAULT 0;
UPDATE acquisitions SET tax_undisposed_satoshis = undisposed_satoshis;

-- Add wallet and tax_undisposed_satoshis to dispositions
ALTER TABLE dispositions ADD COLUMN wallet TEXT NOT NULL DEFAULT 'default';
ALTER TABLE dispositions ADD COLUMN tax_undisposed_satoshis BIGINT NOT NULL DEFAULT 0;
UPDATE dispositions SET tax_undisposed_satoshis = undisposed_satoshis;

-- Restructure acquisition_dispositions with match_type discriminator
CREATE TABLE acquisition_dispositions_new (
    acquisition_id INTEGER NOT NULL REFERENCES acquisitions(id),
    disposition_id INTEGER NOT NULL REFERENCES dispositions(id),
    match_type TEXT NOT NULL CHECK(match_type IN ('tax', 'gaap')),
    satoshis BIGINT NOT NULL,
    basis BIGINT NOT NULL,
    rgl BIGINT NOT NULL,
    term TEXT NOT NULL CHECK(term IN ('short', 'long')),
    PRIMARY KEY (acquisition_id, disposition_id, match_type)
);

-- Migrate existing data: GAAP records
INSERT INTO acquisition_dispositions_new
    SELECT acquisition_id, disposition_id, 'gaap', satoshis, gaap_basis, gaap_rgl, term
    FROM acquisition_dispositions;

-- Migrate existing data: Tax records
INSERT INTO acquisition_dispositions_new
    SELECT acquisition_id, disposition_id, 'tax', satoshis, tax_basis, tax_rgl, term
    FROM acquisition_dispositions;

DROP TABLE acquisition_dispositions;
ALTER TABLE acquisition_dispositions_new RENAME TO acquisition_dispositions;
