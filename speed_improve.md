# Latency & Throughput Improvements (2025-02-14)

## Summary
Implemented the first round of concrete speed improvements suggested earlier. Work now covers:

1. **Retrieval tunability** – configurable `SEARCH_TOP_K` knob so we can shrink the Tantivy result set without touching code.
2. **Better observability** – cache effectiveness + top‑K configuration are exported via Prometheus for dashboards/alerts.
3. **Live instrumentation** – retriever updates those gauges on every search, giving near real-time feedback when hit rates dip.
4. **Faster ingestion** – chunk embedding is now parallelized via Rayon so large documents finish indexing quicker, keeping ingestion from blocking other work.

## Code Changes

| Area | File | Notes |
| --- | --- | --- |
| Config | `backend/src/config.rs` | Added `search_top_k` field + `SEARCH_TOP_K` env parsing (defaults to 10). |
| Startup | `backend/src/main.rs` | Applies the configured value immediately after `Retriever::new_with_paths`. |
| Retriever | `backend/src/retriever.rs` | Stores `search_top_k`, exposes `set_search_top_k`, uses it for `TopDocs::with_limit(...)`, and feeds metrics gauge updates (hit rate + top‑K). |
| Monitoring | `backend/src/monitoring/metrics.rs` | Added `search_cache_hit_rate_percent` and `search_top_k_config` gauges plus helper setters so the retriever can update them per request. |
| Indexer | `backend/src/index.rs` | Embedding generation now runs in parallel (`rayon::par_iter()`), reducing wall-clock time for large files. |
| Docs | `speed_improve.md` | This file – captures what changed and why for future reference. |

## How to Use

1. **Tune retrieval depth**
   ```bash
   export SEARCH_TOP_K=5   # or set in .env
   cargo run -p backend
   ```
   Lower numbers cut model context size and improve latency; raise only if answer quality drops.

2. **Monitor cache efficiency**
   - Grafana/Prometheus metrics now include:
     - `search_cache_hit_rate_percent`
     - `search_top_k_config`
   - Alert if hit rate stays <80% or if top‑K drifts from desired defaults.

3. **Faster ingestion**
   - Large bulk uploads benefit automatically from the parallel embedding pass (no extra flags required).

## Real Embeddings (2025-01-12)

Integrated **fastembed** crate with **bge-small-en-v1.5** (384-dim) for production-quality semantic embeddings.

| Area | File | Notes |
| --- | --- | --- |
| Dependency | `backend/Cargo.toml` | Added `fastembed = "5.8.1"` with rustls TLS. |
| Embedder | `backend/src/embedder.rs` | New `EmbeddingRuntime` wraps fastembed; falls back to hash if model init fails. |
| Indexer | `backend/src/index.rs` | Uses `embed_batch()` for efficient bulk embedding (no rayon needed). |
| Config | `.env.example` | `EMBEDDING_PROVIDER`, `EMBEDDING_MODEL`, `EMBEDDING_BATCH_SIZE`, `EMBEDDING_CACHE_SIZE`. |

### Environment Variables
```bash
EMBEDDING_PROVIDER=fastembed   # or "hash" to force fallback
EMBEDDING_MODEL=bge-small-en-v1.5
EMBEDDING_BATCH_SIZE=32
EMBEDDING_CACHE_SIZE=10000
```

First run downloads model weights (~30 MB) from Hugging Face; subsequent runs use cached files.

## Embedding Instrumentation (2025-01-12)

Added comprehensive metrics and tracing for embedding operations.

### New Prometheus Metrics

| Metric | Type | Description |
| --- | --- | --- |
| `embedding_latency_ms` | Histogram | Time to generate embeddings (per batch) |
| `embedding_batch_size` | Histogram | Number of texts per embedding batch |
| `embedding_cache_hits_total` | Counter | Cache hits in EmbeddingService |
| `embedding_cache_misses_total` | Counter | Cache misses in EmbeddingService |
| `embedding_total` | Counter | Total embeddings generated |

### Tracing Spans

- `embed_text` - Single text embedding with cache lookup
- `embed_batch` - Batch embedding operation

### Environment Variables

```bash
# Customize histogram buckets (milliseconds)
EMBEDDING_HISTO_BUCKETS=1,5,10,25,50,100,250,500,1000
```

### Grafana Dashboard Queries

```promql
# Embedding latency p99
histogram_quantile(0.99, rate(embedding_latency_ms_bucket[5m]))

# Cache hit rate
rate(embedding_cache_hits_total[5m]) / (rate(embedding_cache_hits_total[5m]) + rate(embedding_cache_misses_total[5m]))

# Embeddings per second
rate(embedding_total[1m])
```

## Inference Concurrency Gateway (2025-01-12)

Added semaphore-based concurrency control to prevent resource exhaustion during inference.

### Purpose

When multiple requests try to run embeddings or LLM inference simultaneously, they can:
- Exhaust GPU/CPU memory
- Cause OOM kills
- Create unpredictable latency spikes

The gateway limits concurrent operations with configurable semaphores.

### New Module

`backend/src/inference_gateway.rs` provides:
- `acquire_embedding_permit()` - Wait for embedding slot
- `acquire_llm_permit()` - Wait for LLM slot
- `try_acquire_*()` - Non-blocking variants
- `gateway_stats()` - Current permit availability

### Environment Variables

```bash
INFERENCE_MAX_CONCURRENT_EMBEDDINGS=4  # Max concurrent embedding ops
INFERENCE_MAX_CONCURRENT_LLM=2         # Max concurrent LLM ops
INFERENCE_ACQUIRE_TIMEOUT_MS=30000     # Timeout (0 = wait forever)
```

### New Prometheus Metrics

| Metric | Type | Description |
| --- | --- | --- |
| `inference_permits_acquired_total` | Counter | Permits acquired (by type) |
| `inference_permits_rejected_total` | Counter | Permits rejected/timeout (by type) |
| `inference_permits_available` | Gauge | Currently available permits (by type) |

### API Endpoint

```bash
GET /monitor/inference_gateway
```

Returns current gateway statistics including permits available, acquired, rejected, and wait times.

### Usage Example

```rust
use crate::inference_gateway::acquire_embedding_permit;

async fn embed_with_backpressure(text: &str) -> Result<Vec<f32>, Error> {
    // Wait for permit (respects concurrency limit)
    let _permit = acquire_embedding_permit().await
        .ok_or_else(|| Error::new("Embedding service overloaded"))?;
    
    // Permit auto-released when dropped
    embedder::embed(text)
}
```

## Chunking Tuned for BGE-small (2025-01-12)

Optimized default chunk sizes for the BGE-small-en-v1.5 embedding model.

### Why This Matters

BGE-small-en-v1.5 has a **512 token maximum sequence length**. Chunks that exceed this limit get truncated, losing information. The previous defaults (384 target, 512 max) were too aggressive.

### New Defaults

| Parameter | Old Value | New Value | Rationale |
| --- | --- | --- | --- |
| `CHUNK_TARGET_SIZE` | 384 | **256** | ~50% of model max, leaves room for query |
| `CHUNK_MIN_SIZE` | 192 | **128** | Smaller minimum for better semantic boundaries |
| `CHUNK_MAX_SIZE` | 512 | **384** | Stay well under 512 limit |
| `CHUNK_OVERLAP` | 50 | **32** | ~12% overlap for context continuity |

### New API: `ChunkerConfig::for_embedding_model()`

```rust
// Get optimal config for your embedding model
let config = ChunkerConfig::for_embedding_model("bge-small-en-v1.5");

// Supported models:
// - bge-small-en-v1.5, bge-small-en-v1.5q
// - bge-base-en-v1.5
// - bge-large-en-v1.5
// - all-minilm-l6-v2
```

### Environment Variables

```bash
# Override defaults if needed
CHUNK_TARGET_SIZE=256
CHUNK_MIN_SIZE=128
CHUNK_MAX_SIZE=384
CHUNK_OVERLAP=32
```

### Impact

- **Better retrieval quality**: Chunks fit within model's attention window
- **More chunks per document**: Finer-grained retrieval
- **Slightly higher storage**: More embeddings to store
- **Faster embedding**: Smaller chunks = faster inference

## Next Steps (Optional)
- Instrument LLM inference spans once the model service is integrated.
- Explore quantized model variant (`bge-small-en-v1.5q`) for faster inference.
- Integrate gateway permits into EmbeddingService for automatic backpressure.
- Add auto-detection of embedding model to set chunk config automatically.
