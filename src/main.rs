// use diesel::prelude::*;
// use diesel::sqlite::SqliteConnection;
use clap::{Parser, Subcommand};
use rust_rgl_ledger::commands::import::import_transactions;

fn main() {
    let command = Cli::parse();
    match command.subcommand {
        Command::Import { file } => {
            println!("File: {:?}", file);
            import_transactions(file);
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