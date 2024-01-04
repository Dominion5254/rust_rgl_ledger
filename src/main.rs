use clap::{Parser, Subcommand};
use rust_rgl_ledger::commands::import::import_transactions;

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
        }
        _ => { println!("Invalid Command!") }
    }
}

#[derive(Subcommand)]
enum Command {
    Import {
        /// The file to import
        #[clap(long)]
        file: std::path::PathBuf,
    },
    Two,
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Command,
}