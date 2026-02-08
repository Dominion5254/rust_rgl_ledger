pub mod commands;
pub mod models;
pub mod schema;

use diesel::sqlite::SqliteConnection;
use diesel::prelude::*;
use dotenvy::dotenv;
use std::env;

pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

#[derive(Debug, Clone)]
pub struct LotConfig {
    pub tax_lot_method: String,
    pub tax_lot_scope: String,
    pub gaap_lot_method: String,
    pub gaap_lot_scope: String,
}

impl Default for LotConfig {
    fn default() -> Self {
        Self {
            tax_lot_method: "fifo".to_string(),
            tax_lot_scope: "wallet".to_string(),
            gaap_lot_method: "fifo".to_string(),
            gaap_lot_scope: "universal".to_string(),
        }
    }
}

pub fn rounding_div(numerator: i64, denominator: i64) -> i64 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder * 2 >= denominator {
        quotient + 1
    } else {
        quotient
    }
}

pub fn load_lot_config() -> LotConfig {
    dotenv().ok();
    let config = LotConfig {
        tax_lot_method: env::var("TAX_LOT_METHOD").unwrap_or_else(|_| "fifo".to_string()),
        tax_lot_scope: env::var("TAX_LOT_SCOPE").unwrap_or_else(|_| "wallet".to_string()),
        gaap_lot_method: env::var("GAAP_LOT_METHOD").unwrap_or_else(|_| "fifo".to_string()),
        gaap_lot_scope: env::var("GAAP_LOT_SCOPE").unwrap_or_else(|_| "universal".to_string()),
    };

    if config.tax_lot_method != "fifo" {
        panic!("Unsupported TAX_LOT_METHOD '{}'. Only 'fifo' is currently supported.", config.tax_lot_method);
    }
    if config.gaap_lot_method != "fifo" {
        panic!("Unsupported GAAP_LOT_METHOD '{}'. Only 'fifo' is currently supported.", config.gaap_lot_method);
    }
    if !["wallet", "universal"].contains(&config.tax_lot_scope.as_str()) {
        panic!("Unsupported TAX_LOT_SCOPE '{}'. Must be 'wallet' or 'universal'.", config.tax_lot_scope);
    }
    if !["wallet", "universal"].contains(&config.gaap_lot_scope.as_str()) {
        panic!("Unsupported GAAP_LOT_SCOPE '{}'. Must be 'wallet' or 'universal'.", config.gaap_lot_scope);
    }

    config
}