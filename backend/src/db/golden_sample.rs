//! Golden corpus sample: a stable, seeded random subset of the user's actual
//! chunks, captured under one tokenizer. The Step 3 diff engine runs candidate
//! tokenizers against this baseline.
//!
//! Capture strategy: opportunistic reservoir sampling. Every chunk produced by
//! the live ingest pipeline is offered via [`offer_chunk`]; the reservoir
//! decides probabilistically whether to keep it. This means the sample fills
//! as the user ingests — there is no separate capture pass over the existing
//! corpus.
//!
//! Re-capture: [`recapture`] clears the table and resets the counter; the
//! sample will repopulate on the next ingest.

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tracing::{info, warn};

const DEFAULT_CAPACITY: usize = 100;
const DEFAULT_SEED: u64 = 0xA60D5A_4_AEu64;

static DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static DB_CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
pub struct GoldenSampleStatus {
    pub capacity: usize,
    pub current_size: usize,
    pub chunks_seen: u64,
    pub seed: u64,
    pub captured_at: Option<String>,
    pub tokenizer_model: Option<String>,
}

/// Splitmix64 PRNG — small, deterministic, enough for reservoir sampling.
/// Self-contained so we don't pull a new dep into the regular build.
#[inline]
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[derive(Debug, Clone, Serialize)]
pub struct GoldenSampleEntry {
    pub id: i64,
    pub chunk_text: String,
    pub baseline_token_count: usize,
    pub baseline_token_ids: Option<Vec<u32>>,
    pub tokenizer_model: String,
    pub captured_at: String,
    pub position_in_corpus: u64,
}

pub fn init(db_path: PathBuf) {
    let _ = DB_PATH.set(db_path.clone());
    match Connection::open(&db_path) {
        Ok(conn) => {
            // Seed the meta row if missing
            let _ = conn.execute(
                "INSERT OR IGNORE INTO golden_sample_meta (id, capacity, chunks_seen, seed) VALUES (1, ?1, 0, ?2)",
                params![capacity_from_env() as i64, DEFAULT_SEED as i64],
            );
            let _ = DB_CONN.set(Mutex::new(conn));
            info!("golden_sample: initialized");
        }
        Err(e) => warn!("golden_sample: failed to open DB: {}", e),
    }
}

fn capacity_from_env() -> usize {
    std::env::var("GOLDEN_SAMPLE_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n >= 10 && *n <= 5000)
        .unwrap_or(DEFAULT_CAPACITY)
}

/// Offer a chunk to the reservoir. Cheap when the sample is full and the
/// reservoir decides not to keep this one (just a counter bump + RNG roll).
/// More expensive when we keep it, since we tokenize and write to SQLite.
pub fn offer_chunk(chunk_text: &str, corpus_slug: &str) {
    let Some(mutex) = DB_CONN.get() else { return };
    let Ok(conn) = mutex.lock() else { return };

    // Read current state from meta.
    let (capacity, chunks_seen, seed): (i64, i64, i64) = match conn.query_row(
        "SELECT capacity, chunks_seen, seed FROM golden_sample_meta WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ) {
        Ok(row) => row,
        Err(_) => return,
    };
    let capacity = capacity.max(1) as usize;
    let position = chunks_seen as u64;

    // Deterministic per-position seed — reproducible across restarts without
    // having to carry RNG state in the DB.
    let mut rng_state = (seed as u64).wrapping_add(position).wrapping_add(1);
    let _ = splitmix64(&mut rng_state); // discard first draw to avoid low-bit bias

    let current_size: i64 = conn
        .query_row("SELECT COUNT(*) FROM golden_sample", [], |row| row.get(0))
        .unwrap_or(0);
    let current_size = current_size as usize;

    let keep_decision = if current_size < capacity {
        Some(KeepDecision::Append)
    } else {
        // Uniform draw in [0, position] using splitmix64 % (position+1).
        // For position values typical here (< 10^9) the modulo bias is negligible.
        let j = splitmix64(&mut rng_state) % (position + 1);
        if (j as usize) < capacity {
            Some(KeepDecision::Replace(j as usize))
        } else {
            None
        }
    };

    if let Some(decision) = keep_decision {
        let counter = crate::api::get_token_counter();
        let baseline_token_count = counter
            .as_ref()
            .map(|h| h.count_tokens(chunk_text))
            .unwrap_or(0);
        let baseline_token_ids = counter
            .as_ref()
            .and_then(|h| h.encode_ids(chunk_text))
            .map(|ids| serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into()));
        let tokenizer_model = counter
            .as_ref()
            .map(|h| h.model_name())
            .unwrap_or_else(|| "unknown".into());

        match decision {
            KeepDecision::Append => {
                let _ = conn.execute(
                    "INSERT INTO golden_sample (chunk_text, baseline_token_count, baseline_token_ids, tokenizer_model, position_in_corpus, corpus_slug)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        chunk_text,
                        baseline_token_count as i64,
                        baseline_token_ids,
                        tokenizer_model,
                        position as i64,
                        corpus_slug,
                    ],
                );
            }
            KeepDecision::Replace(slot_index) => {
                // Find the slot's row id by ordering on insertion id.
                let row_id: Result<i64, _> = conn.query_row(
                    "SELECT id FROM golden_sample ORDER BY id LIMIT 1 OFFSET ?1",
                    params![slot_index as i64],
                    |row| row.get(0),
                );
                if let Ok(rid) = row_id {
                    let _ = conn.execute(
                        "UPDATE golden_sample
                         SET chunk_text = ?1, baseline_token_count = ?2, baseline_token_ids = ?3,
                             tokenizer_model = ?4, captured_at = CURRENT_TIMESTAMP, position_in_corpus = ?5,
                             corpus_slug = ?6
                         WHERE id = ?7",
                        params![
                            chunk_text,
                            baseline_token_count as i64,
                            baseline_token_ids,
                            tokenizer_model,
                            position as i64,
                            corpus_slug,
                            rid,
                        ],
                    );
                }
            }
        }

        let _ = conn.execute(
            "UPDATE golden_sample_meta
             SET captured_at = CURRENT_TIMESTAMP, tokenizer_model = ?1
             WHERE id = 1",
            params![tokenizer_model],
        );
    }

    let _ = conn.execute(
        "UPDATE golden_sample_meta SET chunks_seen = chunks_seen + 1 WHERE id = 1",
        [],
    );
}

enum KeepDecision {
    Append,
    Replace(usize),
}

pub fn status() -> Option<GoldenSampleStatus> {
    let mutex = DB_CONN.get()?;
    let conn = mutex.lock().ok()?;
    let (capacity, chunks_seen, seed, captured_at, tokenizer_model): (
        i64,
        i64,
        i64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT capacity, chunks_seen, seed, captured_at, tokenizer_model FROM golden_sample_meta WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .ok()?;
    let current_size: i64 = conn
        .query_row("SELECT COUNT(*) FROM golden_sample", [], |row| row.get(0))
        .ok()?;
    Some(GoldenSampleStatus {
        capacity: capacity.max(0) as usize,
        current_size: current_size.max(0) as usize,
        chunks_seen: chunks_seen.max(0) as u64,
        seed: seed as u64,
        captured_at,
        tokenizer_model,
    })
}

pub fn list(limit: usize) -> Vec<GoldenSampleEntry> {
    let Some(mutex) = DB_CONN.get() else { return vec![] };
    let Ok(conn) = mutex.lock() else { return vec![] };
    let limit = limit.clamp(1, 1000) as i64;
    let mut stmt = match conn.prepare(
        "SELECT id, chunk_text, baseline_token_count, baseline_token_ids,
                tokenizer_model, captured_at, position_in_corpus
         FROM golden_sample
         ORDER BY position_in_corpus
         LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![limit], |row| {
        let ids_json: Option<String> = row.get(3)?;
        let ids = ids_json.and_then(|j| serde_json::from_str::<Vec<u32>>(&j).ok());
        Ok(GoldenSampleEntry {
            id: row.get(0)?,
            chunk_text: row.get(1)?,
            baseline_token_count: row.get::<_, i64>(2)? as usize,
            baseline_token_ids: ids,
            tokenizer_model: row.get(4)?,
            captured_at: row.get(5)?,
            position_in_corpus: row.get::<_, i64>(6)? as u64,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Clear the sample and reset the counter. The reservoir will repopulate from
/// the next ingest. Optionally rotates the seed so re-captures don't deterministically
/// reproduce the prior selection.
pub fn recapture(rotate_seed: bool) -> bool {
    let Some(mutex) = DB_CONN.get() else { return false };
    let Ok(conn) = mutex.lock() else { return false };
    let new_seed: i64 = if rotate_seed {
        Utc::now().timestamp_nanos_opt().unwrap_or(DEFAULT_SEED as i64)
    } else {
        DEFAULT_SEED as i64
    };
    let _ = conn.execute("DELETE FROM golden_sample", []);
    let _ = conn.execute(
        "UPDATE golden_sample_meta SET chunks_seen = 0, seed = ?1, captured_at = NULL, tokenizer_model = NULL WHERE id = 1",
        params![new_seed],
    );
    info!(rotate_seed = rotate_seed, "golden_sample: recapture requested — sample cleared, will repopulate on next ingest");
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/schema.sql")).unwrap();
        conn
    }

    #[test]
    fn schema_has_golden_sample_table() {
        let conn = fresh_db();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'golden_sample'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn meta_row_is_seeded_on_init() {
        let conn = fresh_db();
        // Mimic init() insert (we can't share the OnceLock across tests).
        conn.execute(
            "INSERT OR IGNORE INTO golden_sample_meta (id, capacity, chunks_seen, seed) VALUES (1, ?1, 0, ?2)",
            params![DEFAULT_CAPACITY as i64, DEFAULT_SEED as i64],
        )
        .unwrap();
        let (cap, seen, seed): (i64, i64, i64) = conn
            .query_row(
                "SELECT capacity, chunks_seen, seed FROM golden_sample_meta WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(cap, DEFAULT_CAPACITY as i64);
        assert_eq!(seen, 0);
        assert_eq!(seed, DEFAULT_SEED as i64);
    }
}
