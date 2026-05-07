use crate::config::ChunkerMode;
use crate::memory::chunker_factory::ChunkingStats;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const DEFAULT_HISTORY_SIZE: usize = 50;
const MIN_HISTORY_SIZE: usize = 1;
/// In-memory cap for live session; disk retention is time-based (7 days).
const MAX_HISTORY_SIZE: usize = 10_000;
const RETENTION_DAYS: i64 = 7;

static HISTORY_CAPACITY: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(DEFAULT_HISTORY_SIZE));
static SNAPSHOTS: Lazy<Mutex<VecDeque<ChunkingStatsSnapshot>>> =
    Lazy::new(|| Mutex::new(VecDeque::with_capacity(DEFAULT_HISTORY_SIZE)));
static LOGGING_ENABLED: Lazy<std::sync::atomic::AtomicBool> =
    Lazy::new(|| std::sync::atomic::AtomicBool::new(true));
static SNAPSHOT_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Detection info for observability - tracks raw inputs vs derived conclusions
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct DetectionInfo {
    /// Raw input: MIME type from magic bytes (if detected)
    pub mime_type: Option<String>,
    /// Raw input: File extension
    pub extension: Option<String>,
    /// Derived conclusion: Detected content type
    pub detected_format: String,
    /// Derived conclusion: Chosen chunking strategy
    pub chosen_strategy: String,
    /// Detection method used (magic_bytes, extension, heuristic)
    pub detection_method: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChunkingStatsSnapshot {
    pub recorded_at: String,
    pub file: String,
    pub chunker_mode: String,
    pub chunks: usize,
    pub tokens: usize,
    pub duration_ms: u64,
    pub stats: Option<ChunkingStats>,
    /// Detection observability: raw inputs and derived conclusions
    pub detection: Option<DetectionInfo>,
    /// Which tokenizer model was active when this chunking operation ran
    pub tokenizer_model: Option<String>,
    #[serde(default)]
    pub corpus: String,
}

impl ChunkingStatsSnapshot {
    pub fn new(
        file: &str,
        chunker_mode: ChunkerMode,
        chunks: usize,
        tokens: usize,
        duration_ms: u64,
        stats: Option<ChunkingStats>,
    ) -> Self {
        Self {
            recorded_at: Utc::now().to_rfc3339(),
            file: file.to_string(),
            chunker_mode: format!("{:?}", chunker_mode),
            chunks,
            tokens,
            duration_ms,
            stats,
            detection: None,
            tokenizer_model: None,
            corpus: String::new(),
        }
    }

    /// Create snapshot with detection info for full observability
    pub fn with_detection(
        file: &str,
        chunker_mode: ChunkerMode,
        chunks: usize,
        tokens: usize,
        duration_ms: u64,
        stats: Option<ChunkingStats>,
        detection: DetectionInfo,
    ) -> Self {
        Self {
            recorded_at: Utc::now().to_rfc3339(),
            file: file.to_string(),
            chunker_mode: format!("{:?}", chunker_mode),
            chunks,
            tokens,
            duration_ms,
            stats,
            detection: Some(detection),
            tokenizer_model: None,
            corpus: String::new(),
        }
    }

    fn age_days(&self) -> i64 {
        DateTime::parse_from_rfc3339(&self.recorded_at)
            .map(|t| (Utc::now() - t.with_timezone(&Utc)).num_days())
            .unwrap_or(i64::MAX)
    }
}

fn current_capacity() -> usize {
    HISTORY_CAPACITY
        .load(Ordering::Relaxed)
        .clamp(MIN_HISTORY_SIZE, MAX_HISTORY_SIZE)
}

pub fn set_chunking_history_capacity(new_cap: usize) -> usize {
    let bounded = new_cap.clamp(MIN_HISTORY_SIZE, MAX_HISTORY_SIZE);
    HISTORY_CAPACITY.store(bounded, Ordering::Relaxed);
    bounded
}

pub fn set_chunking_logging_enabled(enabled: bool) {
    LOGGING_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn chunking_logging_enabled() -> bool {
    LOGGING_ENABLED.load(Ordering::Relaxed)
}

/// Load persisted snapshots from disk, discarding records older than 7 days.
pub fn init(path: PathBuf) {
    let _ = SNAPSHOT_PATH.set(path.clone());
    let Ok(data) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(records) = serde_json::from_str::<Vec<ChunkingStatsSnapshot>>(&data) else {
        return;
    };
    let cutoff = Utc::now() - chrono::Duration::days(RETENTION_DAYS);
    if let Ok(mut guard) = SNAPSHOTS.lock() {
        for snap in records {
            if let Ok(ts) = DateTime::parse_from_rfc3339(&snap.recorded_at) {
                if ts.with_timezone(&Utc) >= cutoff {
                    guard.push_back(snap);
                }
            }
        }
        // Update capacity to match how many we loaded so they are not immediately evicted.
        let loaded = guard.len().max(DEFAULT_HISTORY_SIZE);
        HISTORY_CAPACITY.store(loaded.min(MAX_HISTORY_SIZE), Ordering::Relaxed);
    }
}

/// Write all in-memory snapshots to disk (called by the 30 s background timer).
pub fn flush() {
    let Some(path) = SNAPSHOT_PATH.get() else {
        return;
    };
    let cutoff = Utc::now() - chrono::Duration::days(RETENTION_DAYS);
    let records: Vec<ChunkingStatsSnapshot> = SNAPSHOTS
        .lock()
        .map(|guard| {
            guard
                .iter()
                .filter(|s| {
                    DateTime::parse_from_rfc3339(&s.recorded_at)
                        .map(|t| t.with_timezone(&Utc) >= cutoff)
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if let Ok(json) = serde_json::to_string(&records) {
        let _ = std::fs::write(path, json);
    }
}

pub fn record_chunking_snapshot(snapshot: ChunkingStatsSnapshot) {
    if LOGGING_ENABLED.load(Ordering::Relaxed) {
        tracing::info!(
            target = "chunking_snapshot",
            snapshot = %serde_json::to_string(&snapshot).unwrap_or_default()
        );
    }

    if let Ok(mut guard) = SNAPSHOTS.lock() {
        // Remove any existing entry for this (file, corpus) pair so re-indexes don't duplicate rows.
        guard.retain(|s| !(s.file == snapshot.file && s.corpus == snapshot.corpus));
        let cap = current_capacity();
        if guard.len() >= cap {
            guard.pop_front();
        }
        guard.push_back(snapshot);
    }
}

pub fn latest_chunking_snapshot() -> Option<ChunkingStatsSnapshot> {
    SNAPSHOTS
        .lock()
        .ok()
        .and_then(|guard| guard.back().cloned())
}

pub fn chunking_snapshot_history(limit: usize, corpus: Option<&str>) -> Vec<ChunkingStatsSnapshot> {
    let limit = limit.clamp(MIN_HISTORY_SIZE, MAX_HISTORY_SIZE);
    let corpus = corpus.filter(|s| !s.is_empty());
    SNAPSHOTS
        .lock()
        .map(|guard| {
            guard
                .iter()
                .rev()
                .filter(|s| corpus.is_none_or(|c| s.corpus == c))
                .filter(|s| s.age_days() <= RETENTION_DAYS)
                .take(limit)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}
