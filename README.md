# Rust RGL Ledger
This project provides a CLI tool for tracking Bitcoin acquisitions/dispositions and associated realized gains and losses for both GAAP and Tax purposes using the FIFO methodology. GAAP RGL are calculated using fair value (or cost if `mark-to-market` has not been run), while Tax RGL are calculated using the original cost basis.

## Dependencies
* [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)
* SQLite

## Installation
* Clone rust_rgl_ledger and cd into the directory `cd rust_rgl_ledger`
* Specify the `DATABASE_URL` with the command `echo DATABASE_URL=/Users/name/path/to/rust_rgl_ledger/database/rgl_db.sqlite3 > .env`
* Build using `cargo build --release`
* Install Diesel CLI with only the sqlite feature using `cargo install diesel_cli --no-default-features --features sqlite`
* Create sqlite DB and run migrations `diesel setup`
* Install systemwide with `cargo install --path .`
* Run `rust_rgl_ledger --help` to see the available commands.

## Import File
When importing a CSV file using `rust_rgl_ledger import <file>` the file should be specified with the path from the current working directory i.e. `./import_files/transactions.csv`.

The file to be imported should have 3 columns with the below headers exactly as they appear below, and the corresponding data:
* Date - Import supports a variety of formats including DateTime i.e. `10/31/23 14:32:17`. If no time is specified a time of 00:00:00 will be used.
* Bitcoin - Expressed as a decimal i.e. `.21` BTC rather than `21,000,000` Satoshis. Bitcoin acquisitions should be expressed as a positive number while Bitcoin dispositions should be expressed as a negative number.
* Price - The USD exchange price of One BTC i.e. `$46,145.26`. **Note** Entering the USD value of the acquisition/disposition for this field will result in the WRONG calculations.

## Limitations
* At this time, rust_rgl_ledger is only configured to work using a sqlite database.