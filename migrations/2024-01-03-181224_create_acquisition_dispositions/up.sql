-- Your SQL goes here
CREATE TABLE acquisition_dispositions (
  acquisition_id INTEGER REFERENCES acquisitions(id) NOT NULL,
  disposition_id INTEGER REFERENCES dispositions(id) NOT NULL,
  satoshis BIGINT NOT NULL,
  gaap_rgl BIGINT NOT NULL,
  tax_rgl BIGINT NOT NULL,
  term TEXT CHECK(term IN ('short', 'long')) NOT NULL,
  PRIMARY KEY(acquisition_id, disposition_id)
)