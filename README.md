# Rust RGL Ledger
This project provides a CLI tool for tracking Bitcoin acquisitions/dispositions and associated realized gains and losses for both GAAP and Tax purposes using the FIFO methodology. GAAP RGL are calculated using fair value (or cost if `mark-to-market` has not been run), while Tax RGL are calculated using the original cost basis.

## Dependencies
* [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)
* SQLite

## Installation
* Clone rust_rgl_ledger and cd into the directory `cd rust_rgl_ledger`
* Create a `.env` file (see [Configuration](#configuration) below)
* Build using `cargo build --release`
* Install Diesel CLI with only the sqlite feature using `cargo install diesel_cli --no-default-features --features sqlite`
* Create sqlite DB and run migrations `diesel setup`
* Install systemwide with `cargo install --path .`
* Run `rust_rgl_ledger --help` to see the available commands.

## Configuration
The `.env` file in the project root controls the database connection and lot matching behavior.

### Required
| Variable | Description | Example |
|---|---|---|
| `DATABASE_URL` | Path to the SQLite database file | `database/rgl_db.sqlite3` |

### Optional — Lot Matching
These variables control how acquisition lots are matched to dispositions. If omitted, the defaults shown below are used.

| Variable | Default | Options | Description |
|---|---|---|---|
| `TAX_LOT_SCOPE` | `wallet` | `wallet`, `universal` | Whether tax lot matching is scoped to the same wallet or across all wallets |

Both GAAP and tax use FIFO lot matching. GAAP always uses universal scope — lots are matched in FIFO order regardless of wallet assignment.

**Example `.env`:**
```
DATABASE_URL=database/rgl_db.sqlite3
TAX_LOT_SCOPE=wallet
```

When `TAX_LOT_SCOPE` is set to `wallet`, tax dispositions will only consume acquisition lots that share the same wallet. When set to `universal`, dispositions consume the oldest lots regardless of wallet assignment.

## Import File
When importing a CSV file using `rust_rgl_ledger import -f <file>` the file should be specified with the path from the current working directory i.e. `./import_files/transactions.csv`.

The file to be imported should have the below columns with headers exactly as they appear below, and the corresponding data:
* **Date** - Import supports a variety of formats including DateTime i.e. `10/31/23 14:32:17`. If no time is specified a time of 00:00:00 will be used.
* **Bitcoin** - Expressed as a decimal i.e. `.21` BTC rather than `21,000,000` Satoshis. Bitcoin acquisitions should be expressed as a positive number while Bitcoin dispositions should be expressed as a negative number.
* **Price** - The USD exchange price of One BTC i.e. `$46,145.26`. **Note** Entering the USD value of the acquisition/disposition for this field will result in the WRONG calculations.
* **Wallet** *(optional)* - The wallet or account name for this transaction. Defaults to `default` if omitted. Used for wallet-scoped lot matching when `*_LOT_SCOPE=wallet`.

## Allocate Command
The `allocate` command assigns existing unallocated lots to wallets using a bucket CSV file. This is useful when migrating from a single-wallet setup to wallet-scoped tracking.

```
rust_rgl_ledger allocate -f <bucket_file.csv>
```

The bucket CSV should have two columns:
* **Wallet** - The wallet name to assign lots to
* **BTC** - The amount of BTC to allocate to that wallet

Lots are assigned in FIFO order. If a lot must be split across wallets, the tool will automatically split it. The total BTC across all buckets must match the total undisposed BTC in the database.

**Note:** `allocate` can only be run before wallet-scoped imports have caused the GAAP and tax undisposed trackers to diverge.

## Transfer Command
The `transfer` command records BTC moving between wallets (e.g., cold storage to exchange). A transfer is not an economic event — no realized gain/loss is recognized, and lots retain their original acquisition date and basis.

```
rust_rgl_ledger transfer -f <transfer_file.csv>
```

The transfer CSV should have four columns:
* **Date** - Used for ordering rows; lots retain their original acquisition date
* **From** - The source wallet name
* **To** - The destination wallet name
* **BTC** - The amount of BTC to transfer

Lots are consumed from the source wallet in FIFO order. If a transfer amount falls mid-lot, the lot is split proportionally. All rows are processed within a single transaction — if any row fails, all changes are rolled back.

## Limitations
* At this time, rust_rgl_ledger is only configured to work using a sqlite database.
* Only the FIFO lot matching method is currently supported.