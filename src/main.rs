use clap::{Parser, Subcommand};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::impair::impair_holdings;
use rust_rgl_ledger::commands::report::report;
use rust_rgl_ledger::commands::holdings::holdings;

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
        Command::Impair { price, date } => {
            match impair_holdings(&price, &date) {
                Ok(_) => {
                    println!("Successfully impaired Bitcoin holdings to {} as of {}", price, date)
                }
                Err(e) => {
                    eprint!("Error impairing bitcoin holdings: {}", e)
                }
            }
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
    }
}

#[derive(Subcommand)]
enum Command {
    Import {
        /// The file to import
        #[clap(long)]
        file: std::path::PathBuf,
    },
    Impair {
        /// The USD price to impair Bitcoin holdings
        #[clap(long)]
        price: String,
        /// The Date to impair Bitcoin holdings
        #[clap(long)]
        date: String,
    },
    Report {
        /// The beginning date for RGL report
        #[clap(long)]
        beg: String,
        /// The ending date for RGL report
        #[clap(long)]
        end: String,
    },
    Holdings {
        /// The ending date of the holdings report
        #[clap(long)]
        date: String,
    },
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Command,
}