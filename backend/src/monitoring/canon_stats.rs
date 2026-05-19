use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

const STORE_RECORD_CAP: usize = 50;

/// Per-file normalization record covering all three levels.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoreRecord {
    pub file: String,
    pub corpus: String,
    pub chars_in: u64,
    pub chars_out: u64,
    pub embed_chars_in: u64,
    pub embed_chars_out: u64,
    pub index_chars_in: u64,
    pub index_chars_out: u64,
}

/// Per-call-site counters for normalize() and to_index().
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Default)]
struct State {
    stats: CanonStats,
    store_records: VecDeque<StoreRecord>,
}

static STATE: Lazy<RwLock<State>> = Lazy::new(|| RwLock::new(State::default()));
static SNAPSHOT_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Serialize, Deserialize, Default)]
struct Snapshot {
    store_ingestion: CallSiteStats,
    embed_ingestion: CallSiteStats,
    index_ingestion: CallSiteStats,
    embed_query: CallSiteStats,
    index_query: CallSiteStats,
    store_records: Vec<StoreRecord>,
}

pub fn init(path: PathBuf) {
    let _ = SNAPSHOT_PATH.set(path.clone());
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(snap) = serde_json::from_str::<Snapshot>(&data) {
            if let Ok(mut s) = STATE.write() {
                s.stats.store_ingestion = snap.store_ingestion;
                s.stats.embed_ingestion = snap.embed_ingestion;
                s.stats.index_ingestion = snap.index_ingestion;
                s.stats.embed_query = snap.embed_query;
                s.stats.index_query = snap.index_query;
                s.store_records = snap.store_records.into_iter().collect();
            }
        }
    }
}

pub fn flush() {
    let Some(path) = SNAPSHOT_PATH.get() else {
        return;
    };
    let snap = STATE
        .read()
        .map(|s| Snapshot {
            store_ingestion: s.stats.store_ingestion.clone(),
            embed_ingestion: s.stats.embed_ingestion.clone(),
            index_ingestion: s.stats.index_ingestion.clone(),
            embed_query: s.stats.embed_query.clone(),
            index_query: s.stats.index_query.clone(),
            store_records: s.store_records.iter().cloned().collect(),
        })
        .unwrap_or_default();
    if let Ok(json) = serde_json::to_string(&snap) {
        let _ = std::fs::write(path, json);
    }
}

/// Increment Store-ingestion counters (one call per extracted document).
pub fn record_store(file: &str, chars_in: usize, chars_out: usize, corpus: &str) {
    if let Ok(mut s) = STATE.write() {
        s.stats.store_ingestion.calls += 1;
        s.stats.store_ingestion.chars_in += chars_in as u64;
        s.stats.store_ingestion.chars_out += chars_out as u64;
        // Remove any existing entry for this file so re-indexes don't duplicate rows.
        s.store_records
            .retain(|r| !(r.file == file && r.corpus == corpus));
        if s.store_records.len() == STORE_RECORD_CAP {
            s.store_records.pop_back();
        }
        s.store_records.push_front(StoreRecord {
            file: file.to_string(),
            corpus: corpus.to_string(),
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
pub fn record_file_embed(file: &str, chars_in: usize, chars_out: usize, corpus: &str) {
    if let Ok(mut s) = STATE.write() {
        if let Some(rec) = s
            .store_records
            .iter_mut()
            .find(|r| r.file == file && r.corpus == corpus)
        {
            rec.embed_chars_in = chars_in as u64;
            rec.embed_chars_out = chars_out as u64;
        }
    }
}

/// Update the index-level totals for a file already in the record list.
/// Called once per file after all its chunks have been processed.
pub fn record_file_index(file: &str, chars_in: usize, chars_out: usize, corpus: &str) {
    if let Ok(mut s) = STATE.write() {
        if let Some(rec) = s
            .store_records
            .iter_mut()
            .find(|r| r.file == file && r.corpus == corpus)
        {
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

pub fn forget_file(filename: &str, corpus: &str) {
    if let Ok(mut s) = STATE.write() {
        s.store_records
            .retain(|r| !(r.file == filename && r.corpus == corpus));
        s.stats.store_ingestion = CallSiteStats {
            calls: s.store_records.len() as u64,
            chars_in: s.store_records.iter().map(|r| r.chars_in).sum(),
            chars_out: s.store_records.iter().map(|r| r.chars_out).sum(),
        };
    }
    flush();
}

pub fn get_stats() -> CanonStats {
    get_stats_for(None)
}

pub fn get_stats_for(corpus: Option<&str>) -> CanonStats {
    let corpus = corpus.filter(|s| !s.is_empty());
    STATE
        .read()
        .map(|s| {
            let store_records: Vec<StoreRecord> = s
                .store_records
                .iter()
                .filter(|r| corpus.is_none_or(|c| r.corpus == c))
                .cloned()
                .collect();
            let store_ingestion = if corpus.is_some() {
                CallSiteStats {
                    calls: store_records.len() as u64,
                    chars_in: store_records.iter().map(|r| r.chars_in).sum(),
                    chars_out: store_records.iter().map(|r| r.chars_out).sum(),
                }
            } else {
                s.stats.store_ingestion.clone()
            };
            CanonStats {
                store_ingestion,
                store_records,
                embed_ingestion: s.stats.embed_ingestion.clone(),
                index_ingestion: s.stats.index_ingestion.clone(),
                embed_query: s.stats.embed_query.clone(),
                index_query: s.stats.index_query.clone(),
            }
        })
        .unwrap_or_default()
}
