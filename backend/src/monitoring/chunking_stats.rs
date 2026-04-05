use crate::config::ChunkerMode;
use crate::memory::chunker_factory::ChunkingStats;
use chrono::Utc;
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const DEFAULT_HISTORY_SIZE: usize = 50;
const MIN_HISTORY_SIZE: usize = 1;
const MAX_HISTORY_SIZE: usize = 1000;

static HISTORY_CAPACITY: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(DEFAULT_HISTORY_SIZE));
static SNAPSHOTS: Lazy<Mutex<VecDeque<ChunkingStatsSnapshot>>> =
    Lazy::new(|| Mutex::new(VecDeque::with_capacity(DEFAULT_HISTORY_SIZE)));
static LOGGING_ENABLED: Lazy<std::sync::atomic::AtomicBool> =
    Lazy::new(|| std::sync::atomic::AtomicBool::new(true));

/// Detection info for observability - tracks raw inputs vs derived conclusions
#[derive(Clone, Serialize, Debug, Default)]
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

#[derive(Clone, Serialize, Debug)]
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
        }
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

pub fn record_chunking_snapshot(snapshot: ChunkingStatsSnapshot) {
    if LOGGING_ENABLED.load(Ordering::Relaxed) {
        // Persist to logs for long-term retention
        tracing::info!(
            target = "chunking_snapshot",
            snapshot = %serde_json::to_string(&snapshot).unwrap_or_default()
        );
    }

    if let Ok(mut guard) = SNAPSHOTS.lock() {
        let cap = current_capacity();
        if guard.len() == cap {
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

pub fn chunking_snapshot_history(limit: usize) -> Vec<ChunkingStatsSnapshot> {
    let limit = limit
        .clamp(MIN_HISTORY_SIZE, MAX_HISTORY_SIZE)
        .min(current_capacity());
    SNAPSHOTS
        .lock()
        .map(|guard| guard.iter().rev().take(limit).cloned().collect())
        .unwrap_or_default()
}
