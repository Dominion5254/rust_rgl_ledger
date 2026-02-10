-- Restore original acquisition_dispositions structure
CREATE TABLE acquisition_dispositions_old (
    acquisition_id INTEGER NOT NULL REFERENCES acquisitions(id),
    disposition_id INTEGER NOT NULL REFERENCES dispositions(id),
    satoshis BIGINT NOT NULL,
    gaap_basis BIGINT NOT NULL,
    gaap_rgl BIGINT NOT NULL,
    tax_basis BIGINT NOT NULL,
    tax_rgl BIGINT NOT NULL,
    term TEXT NOT NULL CHECK(term IN ('short', 'long')),
    PRIMARY KEY (acquisition_id, disposition_id)
);

-- Reconstruct from separate gaap/tax rows by joining them
INSERT INTO acquisition_dispositions_old
    SELECT
        g.acquisition_id,
        g.disposition_id,
        g.satoshis,
        g.basis AS gaap_basis,
        g.rgl AS gaap_rgl,
        t.basis AS tax_basis,
        t.rgl AS tax_rgl,
        g.term
    FROM acquisition_dispositions g
    INNER JOIN acquisition_dispositions t
        ON g.acquisition_id = t.acquisition_id
        AND g.disposition_id = t.disposition_id
        AND g.match_type = 'gaap'
        AND t.match_type = 'tax';

DROP TABLE acquisition_dispositions;
ALTER TABLE acquisition_dispositions_old RENAME TO acquisition_dispositions;

-- Remove added columns (SQLite doesn't support DROP COLUMN before 3.35.0,
-- but we use ALTER TABLE approach for consistency with the up migration)
-- For a proper rollback, we'd need to recreate the tables without the new columns.
-- Since this is a development migration, we'll leave the columns in place on rollback.
-- The application code won't reference them after rolling back.
