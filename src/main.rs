use clap::{Parser, Subcommand};
use rust_rgl_ledger::{establish_connection, load_lot_config};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::report::report;
use rust_rgl_ledger::commands::holdings::holdings;
use rust_rgl_ledger::commands::mark_to_market::mark_to_market;
use rust_rgl_ledger::commands::allocate::allocate;

fn main() {
    let command = Cli::parse();
    let conn = &mut establish_connection();
    let config = load_lot_config();
    match command.subcommand {
        Command::Import { file } => {
            match import_transactions(&file, conn, &config) {
                Ok(_) => {
                    println!("Successfully Imported transactions from {:?}", file)
                }
                Err(e) => {
                    println!("Error importing file {:?}: {}", file, e)
                }
            };
        },
        Command::Report { beg, end, view } => {
            match report(&beg, &end, &view.unwrap_or_else(|| "both".to_string()), conn) {
                Ok(_) => {
                    println!("Realized gain/loss report run for the period {} - {}", beg, end)
                }
                Err(e) => {
                    eprint!("Error creating realized gain/loss report: {}", e)
                }
            }
        },
        Command::Holdings { date, view } => {
            match holdings(&date, &view.unwrap_or_else(|| "gaap".to_string()), conn) {
                Ok(_) => {
                    println!("Holdings report run for the period ended {}", date)
                }
                Err(e) => {
                    eprint!("Error creating holdings report: {}", e)
                }
            }
        },
        Command::MarkToMarket { price, date } => {
            match mark_to_market(&price, &date, conn) {
                Ok(_) => {
                    println!("Successfully adjusted Bitcoin holdings to {} as of {}", price, &date);
                    println!("Mark to Market report saved to ./reports/mark-to-market-{}", date);
                }
                Err(e) => {
                    eprint!("Error marking to market bitcoin holdings: {}", e)
                }
            }
        },
        Command::Allocate { file } => {
            match allocate(&file, conn) {
                Ok(_) => {
                    println!("Successfully allocated lots from {:?}", file)
                }
                Err(e) => {
                    eprint!("Error allocating lots: {}", e)
                }
            }
        },
    }
}

#[derive(Subcommand)]
enum Command {
    /// Import a specified CSV file at the provided path
    Import {
        /// The file to import including columns: Date, Bitcoin, Price, [Wallet]
        #[clap(long, short)]
        file: std::path::PathBuf,
    },
    /// Export a CSV report of Realized Gain/Loss activity for a specfied period to the 'reports' directory
    Report {
        /// The beginning date for RGL report
        #[clap(long, short)]
        beg: String,
        /// The ending date for RGL report
        #[clap(long, short)]
        end: String,
        /// View: "tax", "gaap", or "both" (default) — which report(s) to generate
        #[clap(long, short)]
        view: Option<String>,
    },
    /// Export a CSV report of Bitcoin holdings as of a specified date to the 'reports' directory
    Holdings {
        /// The ending date of the holdings report
        #[clap(long, short)]
        date: String,
        /// View: "gaap" (default) or "tax" — controls which undisposed tracker to use
        #[clap(long, short)]
        view: Option<String>,
    },
    /// Mark holdings to provided market price and export mark-to-market report to 'reports' directory
    MarkToMarket {
        /// The USD price to value Bitcoin holdings
        #[clap(long, short)]
        price: String,
        /// The Date to mark holdings to Fair Value
        #[clap(long, short)]
        date: String,
    },
    /// Allocate existing lots to wallets using a bucket CSV
    Allocate {
        /// The bucket CSV file with columns: Wallet, BTC
        #[clap(long, short)]
        file: std::path::PathBuf,
    },
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Command,
}
