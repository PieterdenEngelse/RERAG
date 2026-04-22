use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::RwLock;

const STORE_RECORD_CAP: usize = 50;

/// Per-file normalization record covering all three levels.
#[derive(Debug, Clone, Serialize, Default)]
pub struct StoreRecord {
    pub file: String,
    pub chars_in: u64,
    pub chars_out: u64,
    pub embed_chars_in: u64,
    pub embed_chars_out: u64,
    pub index_chars_in: u64,
    pub index_chars_out: u64,
}

/// Per-call-site counters for normalize() and to_index().
#[derive(Debug, Clone, Default, Serialize)]
pub struct CallSiteStats {
    /// Number of times normalize() / to_index() was called at this site.
    pub calls: u64,
    /// Total input characters processed.
    pub chars_in: u64,
    /// Total output characters (after normalization).
    pub chars_out: u64,
}

/// Aggregated canonicalization statistics across all call sites.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CanonStats {
    /// normalize(Store) — called once per file immediately after extraction.
    pub store_ingestion: CallSiteStats,
    /// Per-file records for normalize(Store), newest first, capped at 50.
    pub store_records: Vec<StoreRecord>,
    /// normalize(Embed) — called once per chunk at index time.
    pub embed_ingestion: CallSiteStats,
    /// to_index() — called once per chunk at index time (upgrades Embed → Index).
    pub index_ingestion: CallSiteStats,
    /// normalize(Embed) — called on the query string at search time.
    pub embed_query: CallSiteStats,
    /// to_index() — called on the query string at search time.
    pub index_query: CallSiteStats,
}

struct State {
    stats: CanonStats,
    store_records: VecDeque<StoreRecord>,
}

impl Default for State {
    fn default() -> Self {
        Self { stats: CanonStats::default(), store_records: VecDeque::new() }
    }
}

static STATE: Lazy<RwLock<State>> = Lazy::new(|| RwLock::new(State::default()));

/// Increment Store-ingestion counters (one call per extracted document).
pub fn record_store(file: &str, chars_in: usize, chars_out: usize) {
    if let Ok(mut s) = STATE.write() {
        s.stats.store_ingestion.calls += 1;
        s.stats.store_ingestion.chars_in += chars_in as u64;
        s.stats.store_ingestion.chars_out += chars_out as u64;
        if s.store_records.len() == STORE_RECORD_CAP {
            s.store_records.pop_back();
        }
        s.store_records.push_front(StoreRecord {
            file: file.to_string(),
            chars_in: chars_in as u64,
            chars_out: chars_out as u64,
            ..Default::default()
        });
    }
}

/// Increment Embed-ingestion counters (one call per chunk).
pub fn record_embed_ingestion(chars_in: usize, chars_out: usize) {
    if let Ok(mut s) = STATE.write() {
        s.stats.embed_ingestion.calls += 1;
        s.stats.embed_ingestion.chars_in += chars_in as u64;
        s.stats.embed_ingestion.chars_out += chars_out as u64;
    }
}

/// Increment Index-ingestion counters (one call per chunk).
pub fn record_index_ingestion(chars_in: usize, chars_out: usize) {
    if let Ok(mut s) = STATE.write() {
        s.stats.index_ingestion.calls += 1;
        s.stats.index_ingestion.chars_in += chars_in as u64;
        s.stats.index_ingestion.chars_out += chars_out as u64;
    }
}

/// Update the embed-level totals for a file already in the record list.
/// Called once per file after all its chunks have been processed.
pub fn record_file_embed(file: &str, chars_in: usize, chars_out: usize) {
    if let Ok(mut s) = STATE.write() {
        if let Some(rec) = s.store_records.iter_mut().find(|r| r.file == file) {
            rec.embed_chars_in = chars_in as u64;
            rec.embed_chars_out = chars_out as u64;
        }
    }
}

/// Update the index-level totals for a file already in the record list.
/// Called once per file after all its chunks have been processed.
pub fn record_file_index(file: &str, chars_in: usize, chars_out: usize) {
    if let Ok(mut s) = STATE.write() {
        if let Some(rec) = s.store_records.iter_mut().find(|r| r.file == file) {
            rec.index_chars_in = chars_in as u64;
            rec.index_chars_out = chars_out as u64;
        }
    }
}

/// Increment Embed-query counters (one call per search request).
pub fn record_embed_query(chars_in: usize, chars_out: usize) {
    if let Ok(mut s) = STATE.write() {
        s.stats.embed_query.calls += 1;
        s.stats.embed_query.chars_in += chars_in as u64;
        s.stats.embed_query.chars_out += chars_out as u64;
    }
}

/// Increment Index-query counters (one call per search request).
pub fn record_index_query(chars_in: usize, chars_out: usize) {
    if let Ok(mut s) = STATE.write() {
        s.stats.index_query.calls += 1;
        s.stats.index_query.chars_in += chars_in as u64;
        s.stats.index_query.chars_out += chars_out as u64;
    }
}

pub fn get_stats() -> CanonStats {
    STATE.read().map(|s| {
        let mut stats = s.stats.clone();
        stats.store_records = s.store_records.iter().cloned().collect();
        stats
    }).unwrap_or_default()
}
