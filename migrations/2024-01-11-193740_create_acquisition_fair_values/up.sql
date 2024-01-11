-- Your SQL goes here
CREATE TABLE acquisition_fair_values(
  acquisition_id INTEGER REFERENCES acquisitions(id) NOT NULL,
  fair_value_id INTEGER REFERENCES fair_values(id) NOT NULL,
  PRIMARY KEY(acquisition_id, fair_value_id)
)