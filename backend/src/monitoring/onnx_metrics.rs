//! ONNX embedding runtime monitoring counters. v1.0.0
//! Updated by embedder.rs; read by GET /monitoring/onnx.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

// ── Runtime counters ──────────────────────────────────────────────────────────
static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static TOTAL_EMBEDDINGS: AtomicU64 = AtomicU64::new(0);
static TOTAL_BATCHES: AtomicU64 = AtomicU64::new(0);
static TOTAL_BATCH_TEXTS: AtomicU64 = AtomicU64::new(0);
static TOTAL_EMBED_US: AtomicU64 = AtomicU64::new(0);
static LAST_EMBED_US: AtomicU64 = AtomicU64::new(0);

/// Time spent waiting to acquire the embedder mutex. Visible because ort
/// 2.0.0-rc.12 forces `&mut self` on `Session::run`, so concurrent embed
/// requests serialize on this lock; this counter shows how much that costs.
static TOTAL_LOCK_WAIT_US: AtomicU64 = AtomicU64::new(0);
static LAST_LOCK_WAIT_US: AtomicU64 = AtomicU64::new(0);
static LOCK_WAIT_SAMPLES: AtomicU64 = AtomicU64::new(0);

/// Set once at startup if the embedder fell back to SimpleTokenizer because
/// no usable `tokenizer.json` was found. Surfaces in the Monitor dashboard
/// so operators can see they're running degraded embeddings.
static SIMPLE_TOKENIZER_FALLBACK: AtomicU64 = AtomicU64::new(0);

// ── Static model metadata ─────────────────────────────────────────────────────
struct ModelMeta {
    name: String,
    dims: usize,
    batch_size: usize,
}

static MODEL_META: OnceLock<ModelMeta> = OnceLock::new();

pub fn register_model(name: &str, dims: usize, batch_size: usize) {
    let _ = MODEL_META.set(ModelMeta {
        name: name.to_owned(),
        dims,
        batch_size,
    });
}

// ── Recording helpers ─────────────────────────────────────────────────────────
pub fn record_cache_hit() {
    CACHE_HITS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_cache_miss() {
    CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
}

pub fn record_single_embed(duration_ms: f64) {
    TOTAL_EMBEDDINGS.fetch_add(1, Ordering::Relaxed);
    let us = (duration_ms * 1_000.0) as u64;
    TOTAL_EMBED_US.fetch_add(us, Ordering::Relaxed);
    LAST_EMBED_US.store(us, Ordering::Relaxed);
}

pub fn record_batch(text_count: usize) {
    TOTAL_BATCHES.fetch_add(1, Ordering::Relaxed);
    TOTAL_BATCH_TEXTS.fetch_add(text_count as u64, Ordering::Relaxed);
}

pub fn record_simple_tokenizer_fallback() {
    SIMPLE_TOKENIZER_FALLBACK.store(1, Ordering::Relaxed);
}

pub fn record_lock_wait(duration_ms: f64) {
    let us = (duration_ms * 1_000.0) as u64;
    TOTAL_LOCK_WAIT_US.fetch_add(us, Ordering::Relaxed);
    LAST_LOCK_WAIT_US.store(us, Ordering::Relaxed);
    LOCK_WAIT_SAMPLES.fetch_add(1, Ordering::Relaxed);
}

// ── Snapshot ──────────────────────────────────────────────────────────────────
#[derive(serde::Serialize)]
pub struct OnnxSnapshot {
    pub status: &'static str,
    pub model_name: String,
    pub model_dims: usize,
    pub batch_size: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,
    pub total_embeddings: u64,
    pub total_batches: u64,
    pub total_batch_texts: u64,
    pub avg_embed_ms: f64,
    pub last_embed_ms: f64,
    pub avg_lock_wait_ms: f64,
    pub last_lock_wait_ms: f64,
    pub lock_wait_samples: u64,
    pub simple_tokenizer_fallback: bool,
}

pub fn snapshot() -> OnnxSnapshot {
    let hits = CACHE_HITS.load(Ordering::Relaxed);
    let misses = CACHE_MISSES.load(Ordering::Relaxed);
    let total = TOTAL_EMBEDDINGS.load(Ordering::Relaxed);
    let total_us = TOTAL_EMBED_US.load(Ordering::Relaxed);
    let last_us = LAST_EMBED_US.load(Ordering::Relaxed);
    let batches = TOTAL_BATCHES.load(Ordering::Relaxed);
    let b_texts = TOTAL_BATCH_TEXTS.load(Ordering::Relaxed);

    let cache_total = hits + misses;
    let cache_hit_rate = if cache_total > 0 {
        (hits as f64 / cache_total as f64) * 100.0
    } else {
        0.0
    };
    let avg_embed_ms = if total > 0 {
        (total_us as f64 / 1_000.0) / total as f64
    } else {
        0.0
    };

    let lock_wait_samples = LOCK_WAIT_SAMPLES.load(Ordering::Relaxed);
    let total_lock_wait_us = TOTAL_LOCK_WAIT_US.load(Ordering::Relaxed);
    let last_lock_wait_us = LAST_LOCK_WAIT_US.load(Ordering::Relaxed);
    let avg_lock_wait_ms = if lock_wait_samples > 0 {
        (total_lock_wait_us as f64 / 1_000.0) / lock_wait_samples as f64
    } else {
        0.0
    };

    let (model_name, model_dims, batch_size) = MODEL_META
        .get()
        .map(|m| (m.name.clone(), m.dims, m.batch_size))
        .unwrap_or_else(|| ("unknown".to_owned(), 0, 0));

    OnnxSnapshot {
        status: if model_dims > 0 {
            "loaded"
        } else {
            "unregistered"
        },
        model_name,
        model_dims,
        batch_size,
        cache_hits: hits,
        cache_misses: misses,
        cache_hit_rate,
        total_embeddings: total,
        total_batches: batches,
        total_batch_texts: b_texts,
        avg_embed_ms,
        last_embed_ms: last_us as f64 / 1_000.0,
        avg_lock_wait_ms,
        last_lock_wait_ms: last_lock_wait_us as f64 / 1_000.0,
        lock_wait_samples,
        simple_tokenizer_fallback: SIMPLE_TOKENIZER_FALLBACK.load(Ordering::Relaxed) > 0,
    }
}
