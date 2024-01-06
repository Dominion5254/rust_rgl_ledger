use clap::{Parser, Subcommand};
use rust_rgl_ledger::commands::import::import_transactions;
use rust_rgl_ledger::commands::impair::impair_holdings;

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
                Err(_) => {
                    eprint!("Error impairing bitcoin holdings")
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
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Command,
}