# Rust RGL Ledger
This project provides a CLI interface for tracking Bitcoin acquisitions/dispositions and associated realized gains and losses for both GAAP and Tax purposes using the FIFO methodology. GAAP RGL are calculated using fair value, while Tax RGL are calculated the original basis.

## Installation
* Download the RGL Ledger using `git clone && cd rust_rgl_ledger`.
* Specify the `DATABASE_URL` with the command `echo DATABASE_URL=/Users/name/path/to/rust_rgl_ledger/database/rgl_db.sqlite3 > .env`
* Build using `cargo build --release`.
* Install systemwide with `cargo install --path .`
* Run `rust_rgl_ledger --help` to see the available commands.

## Input File
When importing a CSV file using `rust_rgl_ledger import <file>` the file should be specified with the path from the current working directory i.e. `./importfiles/transactions.csv`.

The file to be imported should have 3 columns with the below headers exactly as they appear below, and the corresponding data:
* Date - Import supports a variety of formats including DateTime i.e. `10/31/23 14:32:17`.
* Bitcoin - Expressed as a decimal i.e. `.21`. Bitcoin acquisitions should be expressed as a positive number while Bitcoin dispositions should be expressed as a negative number.
* Price - The USD exchange price of One BTC i.e. `$46,145.26`. **Note** Entering the USD value of the acquisition/disposition for this field will result in the WRONG calculations.

## Misc
* At this time, rust_rgl_ledger is only configured to work using a sqlite database.
* `Holdings` Report limitations: While the holdings report will include all acquisitions up to the specified date, the UndisposedBTC values *will* include any dispositions after the specified date. i.e. if a holdings report is run as of 6/30/23, the UndisposedBTC values will consider dispositions from 7/1/23 and beyond.