use clap::{Parser, Subcommand};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::report::report;
use rust_rgl_ledger::commands::holdings::holdings;
use rust_rgl_ledger::commands::mark_to_market::mark_to_market;

fn main() {
    let command = Cli::parse();
    match command.subcommand {
        Command::Import { file } => {
            match import_transactions(&file) {
                Ok(_) => {
                    println!("Successfully Imported transactions from {:?}", file)
                }
                Err(e) => {
                    println!("Error importing file {:?}: {}", file, e)
                }
            };
        },
        Command::Report { beg, end } => {
            match report(&beg, &end) {
                Ok(_) => {
                    println!("Realized gain/loss report run for the period {} - {}", beg, end)
                }
                Err(e) => {
                    eprint!("Error creating realized gain/loss report: {}", e)
                }
            }
        },
        Command::Holdings { date } => {
            match holdings(&date) {
                Ok(_) => {
                    println!("Holdings report run for the period ended {}", date)
                }
                Err(e) => {
                    eprint!("Error creating holdings report: {}", e)
                }
            }
        },
        Command::MarkToMarket { price, date } => {
            match mark_to_market(&price, &date) {
                Ok(_) => {
                    println!("Successfully adjusted Bitcoin holdings to {} as of {}", price, &date);
                    println!("Mark to Market report saved to ./reports/mark-to-market-{}", date);
                }
                Err(e) => {
                    eprint!("Error marking to market bitcoin holdings: {}", e)
                }
            }
        },
    }
}

#[derive(Subcommand)]
enum Command {
    /// Import a specified CSV file at the provided path
    Import {
        /// The file to import including three columns: Date, Bitcoin, Price
        #[clap(long)]
        file: std::path::PathBuf,
    },
    /// Export a CSV report of GAAP and Tax Realized Gain/Loss activity for a specfied period
    Report {
        /// The beginning date for RGL report
        #[clap(long)]
        beg: String,
        /// The ending date for RGL report
        #[clap(long)]
        end: String,
    },
    /// Export a CSV report of Bitcoin holdings as of a specified date
    Holdings {
        /// The ending date of the holdings report
        #[clap(long)]
        date: String,
    },
    /// Mark holdings to provided market price
    MarkToMarket {
        /// The USD price to value Bitcoin holdings
        #[clap(long)]
        price: String,
        /// The Date to mark holdings to Fair Value
        #[clap(long)]
        date: String,
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Command,
}