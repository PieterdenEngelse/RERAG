//! ONNX embedding runtime monitoring counters. v1.0.0
//! Updated by embedder.rs; read by GET /monitoring/onnx.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

// ── Runtime counters ──────────────────────────────────────────────────────────
static CACHE_HITS:        AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES:      AtomicU64 = AtomicU64::new(0);
static TOTAL_EMBEDDINGS:  AtomicU64 = AtomicU64::new(0);
static TOTAL_BATCHES:     AtomicU64 = AtomicU64::new(0);
static TOTAL_BATCH_TEXTS: AtomicU64 = AtomicU64::new(0);
static TOTAL_EMBED_US:    AtomicU64 = AtomicU64::new(0);
static LAST_EMBED_US:     AtomicU64 = AtomicU64::new(0);

// ── Static model metadata ─────────────────────────────────────────────────────
struct ModelMeta {
    name:       String,
    dims:       usize,
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

// ── Snapshot ──────────────────────────────────────────────────────────────────
#[derive(serde::Serialize)]
pub struct OnnxSnapshot {
    pub status:            &'static str,
    pub model_name:        String,
    pub model_dims:        usize,
    pub batch_size:        usize,
    pub cache_hits:        u64,
    pub cache_misses:      u64,
    pub cache_hit_rate:    f64,
    pub total_embeddings:  u64,
    pub total_batches:     u64,
    pub total_batch_texts: u64,
    pub avg_embed_ms:      f64,
    pub last_embed_ms:     f64,
}

pub fn snapshot() -> OnnxSnapshot {
    let hits     = CACHE_HITS.load(Ordering::Relaxed);
    let misses   = CACHE_MISSES.load(Ordering::Relaxed);
    let total    = TOTAL_EMBEDDINGS.load(Ordering::Relaxed);
    let total_us = TOTAL_EMBED_US.load(Ordering::Relaxed);
    let last_us  = LAST_EMBED_US.load(Ordering::Relaxed);
    let batches  = TOTAL_BATCHES.load(Ordering::Relaxed);
    let b_texts  = TOTAL_BATCH_TEXTS.load(Ordering::Relaxed);

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

    let (model_name, model_dims, batch_size) = MODEL_META
        .get()
        .map(|m| (m.name.clone(), m.dims, m.batch_size))
        .unwrap_or_else(|| ("unknown".to_owned(), 0, 0));

    OnnxSnapshot {
        status: if model_dims > 0 { "loaded" } else { "unregistered" },
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
    }
}
