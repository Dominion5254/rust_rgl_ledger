use std::path::PathBuf;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::models::{Acquisition, NewAcquisition};
use crate::schema::{acquisitions, dispositions};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BucketRecord {
    wallet: String,
    #[serde(rename = "BTC")]
    btc: String,
}

pub fn allocate(file: &PathBuf, conn: &mut SqliteConnection) -> Result<(), String> {
    let mut rdr = csv::Reader::from_path(file)
        .map_err(|e| format!("Error reading file {:?}: {}", file, e))?;

    let mut buckets: Vec<(String, i64)> = Vec::new();
    for result in rdr.deserialize::<BucketRecord>() {
        let record = result.map_err(|e| format!("Error parsing bucket CSV: {}", e))?;
        let btc = Decimal::from_str_exact(&record.btc)
            .map_err(|e| format!("Invalid BTC amount '{}': {}", record.btc, e))?;
        let sats = (btc * Decimal::from(100_000_000i64)).round()
            .to_string().parse::<i64>()
            .map_err(|e| format!("Error converting BTC to sats: {}", e))?;
        buckets.push((record.wallet, sats));
    }

    // Validate that GAAP and tax trackers are equal on all undisposed lots
    // Allocate should only be run before wallet-scoped imports; if trackers have
    // diverged, splitting lots would corrupt the tax state.
    let all_lots: Vec<Acquisition> = acquisitions::table
        .filter(acquisitions::undisposed_satoshis.gt(0))
        .select(Acquisition::as_select())
        .load(conn)
        .map_err(|e| format!("Error querying lots: {}", e))?;

    for lot in &all_lots {
        if lot.undisposed_satoshis != lot.tax_undisposed_satoshis {
            return Err(format!(
                "Lot {} (acquired {}) has divergent GAAP ({}) and tax ({}) undisposed satoshis. \
                 Allocate can only be run before wallet-scoped imports have created divergence. \
                 Re-import with wallet assignments in the CSV instead.",
                lot.id,
                lot.acquisition_date.format("%Y-%m-%d"),
                lot.undisposed_satoshis,
                lot.tax_undisposed_satoshis
            ));
        }
    }

    // Validate total allocation against total undisposed
    let total_bucket_sats: i64 = buckets.iter().map(|(_, s)| *s).sum();
    let total_undisposed: i64 = all_lots.iter().map(|l| l.undisposed_satoshis).sum();

    let diff = (total_bucket_sats - total_undisposed).abs();
    if diff > buckets.len() as i64 {
        return Err(format!(
            "Bucket total ({} sats) differs from total undisposed ({} sats) by {} sats (exceeds tolerance of {})",
            total_bucket_sats, total_undisposed, diff, buckets.len()
        ));
    }

    // Get all undisposed lots in FIFO order
    let mut lots: Vec<Acquisition> = acquisitions::table
        .filter(acquisitions::undisposed_satoshis.gt(0))
        .order(acquisitions::acquisition_date.asc())
        .select(Acquisition::as_select())
        .load(conn)
        .map_err(|e| format!("Error fetching undisposed lots: {}", e))?;

    let mut lot_idx = 0;

    for (wallet_name, bucket_sats) in &buckets {
        let mut remaining_capacity = *bucket_sats;

        while remaining_capacity > 0 && lot_idx < lots.len() {
            let lot = &lots[lot_idx];
            let lot_undisposed = lot.undisposed_satoshis;

            if lot_undisposed <= remaining_capacity {
                // Entire lot fits in this bucket
                diesel::update(acquisitions::table.find(lot.id))
                    .set(acquisitions::wallet.eq(wallet_name))
                    .execute(conn)
                    .map_err(|e| format!("Error updating lot wallet: {}", e))?;

                remaining_capacity -= lot_undisposed;
                lot_idx += 1;
            } else {
                // Lot must be split — undisposed portion exceeds bucket capacity
                let excess = lot_undisposed - remaining_capacity;

                // Update original lot: assign to this wallet, reduce both trackers equally
                diesel::update(acquisitions::table.find(lot.id))
                    .set((
                        acquisitions::wallet.eq(wallet_name),
                        acquisitions::undisposed_satoshis.eq(remaining_capacity),
                        acquisitions::tax_undisposed_satoshis.eq(remaining_capacity),
                    ))
                    .execute(conn)
                    .map_err(|e| format!("Error updating split lot: {}", e))?;

                // Insert new lot for the excess
                let new_lot = NewAcquisition {
                    acquisition_date: lot.acquisition_date,
                    satoshis: excess,
                    undisposed_satoshis: excess,
                    usd_cents_btc_basis: lot.usd_cents_btc_basis,
                    usd_cents_btc_fair_value: lot.usd_cents_btc_fair_value,
                    wallet: "unallocated".to_string(),
                    tax_undisposed_satoshis: excess,
                };

                diesel::insert_into(acquisitions::table)
                    .values(&new_lot)
                    .execute(conn)
                    .map_err(|e| format!("Error inserting split lot: {}", e))?;

                // Reload the newly inserted lot so it can be assigned in subsequent iterations
                let new_acq: Acquisition = acquisitions::table
                    .order(acquisitions::id.desc())
                    .select(Acquisition::as_select())
                    .first(conn)
                    .map_err(|e| format!("Error fetching new split lot: {}", e))?;

                // Insert the new lot into our working list at the next position
                lot_idx += 1;
                lots.insert(lot_idx, new_acq);

                remaining_capacity = 0;
            }
        }
    }

    // Set existing dispositions to 'legacy' wallet
    diesel::update(dispositions::table)
        .set(dispositions::wallet.eq("legacy"))
        .execute(conn)
        .map_err(|e| format!("Error updating disposition wallets: {}", e))?;

    Ok(())
}
