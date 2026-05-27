# ort / ONNX embedder — review & remediation plan

Source: `docs/ort-onnx.ods` (three embedded screenshots). This file is a
text transcription so the plan is grep-able and diff-able.

Target file: `backend/src/perf/onnx_embedder.rs` (with touches in
`backend/src/embedder.rs` and `backend/src/main.rs`).

---

## What's done right

- Global `EmbeddingRuntime` behind `OnceLock` — single Session shared
  process-wide, correct.
- Blocking pool is sized to the configured intra-op thread count, so
  ONNX threads aren't fighting tokio's.
- Single-batch path is used everywhere — no accidental per-token
  Session calls.
- `SimpleTokenizer` fallback exists so a missing `tokenizer.json`
  doesn't crash the process.
- `OnnxConfig` exposes the knobs that actually matter (opt level, intra
  / inter threads, mem pattern, CPU arena).
- Errors are typed (`OnnxError`), not stringly.

## What's wrong

- **`Mutex` in the hot path.** A mutex guards every embed call even
  though `ort::Session::run` is already `Send + Sync`. Serializes the
  Session for no reason — biggest single perf win to remove.
- **`eprintln!` noise.** Debug `eprintln!` lines scattered through
  init and the embed path; should be `tracing::debug!` / `info!`.
- **`ort::init()` called per Session.** Should run once at process
  start; calling it again is a no-op but the intent is wrong and the
  log line repeats.
- **`normalize_output` missing.** Without L2 normalization the HNSW
  DotProduct metric is not cosine, which is what BGE-style models
  expect. Recall is silently degraded.
- **Pooling strategy is CLS-only.** No mean-pool option, and the mean
  path (when it does exist) ignores the attention mask, so padding
  tokens pollute the average.
- **SimpleTokenizer divergence.** The fallback tokenizer produces
  embeddings that don't match the real one — caller has no way to know
  it kicked in.
- **`hf_hub` crate pulled in but unused (?)** — verify and drop if so.

---

## Phased plan

### Phase 4 — Cleanup *(behavior-preserving, ~30 min)*

- Kill `eprintln!` lines in `embedder.rs:166`, `:177`, `:188` and across
  `onnx_embedder.rs`.
- Replace with `tracing::debug!` (init details) and `tracing::info!`
  (one-shot "embedder ready").
- Drop any dead helpers left over from the eprintln era.

### Phase 5 — L2-normalize output *(behavior change, needs verification, ~1 hr)*

- Add `normalize_output: bool` to `OnnxConfig`, default `true`.
- After pooling and before return, divide each vector by its L2 norm
  (skip zero vectors).
- Unit test: a one-line vector normalizes to a known unit vector.
- Verify recall on a small fixture set doesn't regress relative to the
  pre-change baseline.

### Phase 6 — Correctness *(behavior change, needs verification)*

- Add attention-mask-aware mean pooling (zero out padded positions,
  divide by sum of mask, not by seq length).
- Pooling strategy is configurable in Phase 7; for Phase 6 the goal is
  just "if mean pool is used, it's mathematically correct".

### Phase 7 — Pooling strategy *(small)*

- `enum PoolingStrategy { Cls, Mean }` on `OnnxConfig`.
- Dispatch in the embed path; CLS path is unchanged, Mean path uses the
  Phase 6 implementation.
- Default `Cls` to preserve current behavior; BGE-flavoured models
  should be flipped to `Mean` in config.

### Phase 8 — SimpleTokenizer fallback hardening

- Gate behind an explicit `allow_simple_tokenizer` flag (default
  `false`) — production should fail loudly if `tokenizer.json` is
  missing, not silently produce garbage embeddings.
- When the fallback is used, log `WARN` once at startup and emit a
  metric so the operator can see it in dashboards.

### Phase 9 — Surface metrics *(small)*

- Counter: embeddings produced.
- Histogram: embed latency (per-batch).
- Gauge / one-shot log: selected execution provider, intra-/inter-op
  threads, opt level — so the Datastores / Monitor pages can show what
  the runtime actually chose.

### Phase 10 — Drop the global Mutex *(biggest single win, ~1 hr)*

- `ort::Session` is `Send + Sync`; the mutex is unnecessary.
- Remove the lock; share the Session through `Arc<Session>` inside the
  runtime struct.
- Stress test with the parallel-embed benchmark afterwards to confirm
  the throughput jump and no panics.

### Phase 11 — Cache initial Session / drop `hf_hub`

- Confirm `hf_hub` is genuinely unused; if so, remove from
  `Cargo.toml` and prune any feature flags that reference it.
- Cache the first Session build so warm starts skip the
  GraphOptimization pass when the model file hash is unchanged.

---

## Risks & sequencing notes

- **L2 normalize is a silent semantic change.** Existing HNSW indexes
  built with un-normalized vectors will give different (worse) results
  after the switch. Either bump an `embedding_version` and rebuild on
  load, or document a one-time reindex.
- **Pooling change rewrites embedding semantics too** — same reindex
  consideration as L2 normalize.
- **Mutex removal is the throughput unlock**; everything else is
  correctness or hygiene. If only one thing ships, it's that.
- **SimpleTokenizer gating is a startup-failure mode change** — make
  sure the env / runtime setting is documented before flipping the
  default.

---

## Suggested order

1. Phase 4 (cleanup) — together, one commit.
2. Phase 5 (normalize) + reindex note — one commit.
3. Phase 9 (metrics) — small, lands cheaply.
4. Phase 10 (mutex) — own commit, with the benchmark result in the
   message.
5. Phase 6 + 7 (pooling correctness then strategy) — one commit each.
6. Phase 8 (tokenizer gating) — own commit, with docs touch.
7. Phase 11 (hf_hub / Session cache) — cleanup tail.

## Status against current branch (`falkordb-migration`, uncommitted)

- Phase 4 — **done** in working tree (`eprintln!` → `tracing`).
- Phase 5 — **done** (`normalize_output: bool` default `true`,
  `l2_normalize` helper, two unit tests).
- "`ort::init()` once at process start" fix — **done**
  (`init_runtime()` in `onnx_embedder.rs`, called from `main.rs`).
- Phase 9 — **done**. Startup "ONNX session ready" log now includes
  execution provider, execution mode, mem pattern, CPU arena, batch
  size, normalize flag. Lock-wait counter added (see Phase 10).
- Phase 10 — **reframed and done**. `ort 2.0.0-rc.12::Session::run`
  takes `&mut self` on its public API (even though `run_inner` only
  needs `&self`), so the `Mutex<OnnxEmbedder>` in `embedder.rs` cannot
  be dropped in safe Rust. The lock now records wait time via
  `onnx_metrics::record_lock_wait`, surfaced on `OnnxSnapshot` so the
  Datastores dashboard can show whether contention is a problem. If
  the lock-wait p95 ever becomes material, the right move is a
  Session pool (one Session per worker) rather than unsafe lock
  removal.
- Phase 6 — **done**. `mean_pool(data, mask, batch, seq, hidden)`
  helper with four unit tests covering all-unmasked, partial mask,
  fully-masked sequence, and multi-batch.
- Phase 7 — **done**. `PoolingStrategy { Cls, Mean }` added to
  `OnnxConfig`, default `Cls` (behavior-preserving). The `[batch,
  seq, hidden]` branch dispatches on the enum; `[batch, hidden]`
  outputs (pre-pooled by the model) ignore the config.
- Phase 8 — **done**. `allow_simple_tokenizer: bool` (default
  `false`) added to `OnnxConfig`. Missing or unloadable
  `tokenizer.json` now returns `OnnxError::TokenizerMissing` unless
  the flag is set; when the fallback IS used,
  `onnx_metrics::record_simple_tokenizer_fallback()` flips a flag
  visible on `OnnxSnapshot`. The flag is wired from the
  `ONNX_ALLOW_SIMPLE_TOKENIZER` runtime setting.
- Phase 11 — **partially N/A**. `hf-hub` is not a direct dependency
  of `backend/Cargo.toml`; it's only pulled transitively via
  `fastembed`. Nothing to drop. The "cache initial Session" half is
  still open and is a stand-alone optimization worth its own pass.

### Open after this round

- Phase 11b — cache the optimized Session across restarts (write
  optimized model on first build, skip the optimization pass on
  subsequent boots).
- Optional: bump an `embedding_version` tag so cached vector indexes
  built with the old un-normalized / CLS-only behavior are
  invalidated on next load. Currently a manual reindex is required
  if you flip `normalize_output` or `pooling` after data exists.
