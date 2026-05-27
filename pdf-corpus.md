# Plan: Make Native PDF Extraction corpus-dependent

**Status:** Approved via `/ultraplan` — remote execution session at
`https://claude.ai/code/session_01HvKYP8QzhRcYky5wXsn3pv?from=cli`.
Result will arrive as a pull request.

> This document is a local reconstruction of the plan based on the
> conversation context and the existing codebase patterns. The PR may
> diverge in details; treat this as the working sketch, not the
> authoritative spec.

## Goal

Today the Native PDF Extraction pipeline is configured **globally**:
`LAYOUT_ML_ENABLED`, `LAYOUT_ML_MODEL_ID`, `LAYOUT_DETR_MODEL_PATH`,
`LAYOUT_ORT_MODEL_PATH`, `LAYOUT_DETR_THRESHOLD`, `LAYOUT_DETR_NUM_CLASSES`
are read from `settings::effective_*` at process startup (or first-use
via `LayoutModel::load_or_heuristic()`'s `OnceLock`).

Goal: let each corpus override these so an `engineering-papers` corpus
can use a DocLayNet-trained model while a `business-reports` corpus uses
PubLayNet — or so a fast-turnaround corpus can disable the ML pipeline
entirely and fall back to plain-text extraction.

This mirrors the existing `chunker_mode` precedence model:
**per-corpus override (DB) → runtime override (`overrides.json`) → env
var → registry default.**

## Architectural challenge

The current pipeline registers **one** `NativePdfExtractor` instance at
startup (`main.rs:512-528`), and `LayoutModel::load_or_heuristic()` is
gated by a `OnceLock<LayoutModel>` — first call wins, every subsequent
call returns the same model regardless of caller.

To make this per-corpus, one of:

1. **Lazy per-corpus model cache.** Replace the global `OnceLock` with
   a `DashMap<CorpusKey, Arc<LayoutModel>>`. The extractor receives the
   corpus slug as part of its context and looks up (or builds) the right
   model. Pros: minimal API surface change. Cons: extractor trait
   doesn't currently carry a corpus arg.
2. **Dispatcher extractor.** Replace `NativePdfExtractor` with a thin
   `CorpusAwarePdfExtractor` that holds per-corpus configs and routes
   to the appropriate downstream extractor. Pros: cleaner separation.
   Cons: more moving parts.
3. **Extract-at-upload-time configuration.** Read the per-corpus settings
   in the upload handler, instantiate a one-shot extractor with the
   right config, and call it directly — bypass the registry. Pros: most
   explicit. Cons: drops the registry abstraction; harder to extend.

Recommended: **option 1**. Smallest blast radius, keeps the registry
contract, the model cache amortises cold-start cost across uploads to
the same corpus.

The `DocExtractor` trait at `backend/src/extractor.rs:18-25` currently is:

```rust
pub trait DocExtractor: Send + Sync {
    fn name(&self) -> &str;
    fn can_handle(&self, content_type: &ContentType) -> bool;
    fn extract(&self, bytes: Vec<u8>, filename: &str, ct: &ContentType)
        -> anyhow::Result<DocIR>;
}
```

We'd add an extraction-context parameter (or a fourth arg) so the
registry caller in `index.rs:1034` can pass the corpus slug down. To
keep backwards compatibility for Docling / Unstructured extractors,
make it an `&ExtractionContext { corpus: &str }` with sensible defaults.

## Phases

### Phase 1 — DB schema

Extend the per-corpus settings table (`corpora.settings_json` or
dedicated columns; check current shape in
`backend/src/db/corpora.rs`) with optional layout-ML fields:

```rust
pub struct CorpusSettings {
    // existing fields...
    pub chunker_mode: Option<String>,
    pub target_size: Option<u32>,
    // ...

    // new — all Option so None means "inherit global"
    pub layout_ml_enabled: Option<bool>,
    pub layout_ml_model_id: Option<String>,
    pub layout_detr_model_path: Option<String>,
    pub layout_ort_model_path: Option<String>,
    pub layout_detr_threshold: Option<f64>,
    pub layout_detr_num_classes: Option<u64>,
}
```

If the table uses `settings_json`, no migration is required —
serde rolls forward. If discrete columns, add a SQL migration.

### Phase 2 — Effective-config resolution

New function alongside `effective_chunker_config()` in
`backend/src/db/corpora.rs`:

```rust
pub struct LayoutConfig {
    pub enabled: bool,
    pub model_id: String,
    pub detr_path: String,
    pub ort_path: String,
    pub threshold: f32,
    pub num_classes: usize,
}

pub fn effective_layout_config(
    global_settings: &Settings,
    corpus_settings: &CorpusSettings,
) -> LayoutConfig { ... }
```

`global_settings` reads from `crate::settings::effective_*`, which already
honours `overrides.json → env → default`. Per-corpus overrides take
precedence, falling through to the global for any `None` field.

### Phase 3 — Per-corpus model cache

Replace the `MODEL: OnceLock<LayoutModel>` in
`backend/src/pdf/layout_model.rs:78` with:

```rust
static MODEL_CACHE: OnceLock<DashMap<LayoutKey, Arc<LayoutModel>>> = OnceLock::new();

#[derive(Hash, Eq, PartialEq, Clone)]
struct LayoutKey {
    model_id: String,    // e.g. "hf:cmarkea/detr-layout-detection" or "local:/path"
    num_classes: usize,
    threshold_bits: u32, // f32::to_bits for hashing
}

impl LayoutModel {
    pub fn load_or_heuristic_for(cfg: &LayoutConfig) -> Arc<LayoutModel> {
        let key = LayoutKey::from(cfg);
        let cache = MODEL_CACHE.get_or_init(DashMap::new);
        if let Some(m) = cache.get(&key) { return Arc::clone(&m); }
        let model = Arc::new(Self::load_or_heuristic_inner(cfg));
        cache.insert(key.clone(), Arc::clone(&model));
        model
    }
}
```

The existing zero-arg `load_or_heuristic()` becomes a wrapper that
builds a `LayoutConfig` from the global settings layer — preserves
backwards compatibility for any non-corpus-aware call site.

### Phase 4 — Extractor trait + plumbing

Add `ExtractionContext` and update the trait:

```rust
pub struct ExtractionContext<'a> {
    pub corpus: &'a str,
}

pub trait DocExtractor: Send + Sync {
    fn name(&self) -> &str;
    fn can_handle(&self, ct: &ContentType) -> bool;
    fn extract(
        &self,
        bytes: Vec<u8>,
        filename: &str,
        ct: &ContentType,
        ctx: &ExtractionContext,
    ) -> anyhow::Result<DocIR>;
}
```

Update implementations:
- `NativePdfExtractor::extract` — read per-corpus settings, build
  `LayoutConfig`, call `LayoutModel::load_or_heuristic_for(&cfg)`.
- `DoclingExtractor`, `UnstructuredExtractor`, `FusionExtractor` —
  accept the ctx but ignore it (no per-corpus config today).

Update the registry caller at `index.rs:1034`:

```rust
reg.extract(bytes, &fname, &ct_clone, &ExtractionContext { corpus: &corpus })
```

### Phase 5 — API surface

Extend the corpus-settings endpoints
(`backend/src/api/corpus_routes.rs:179-203`) to accept the new
fields in PATCH requests:

```rust
// GET /corpora/{slug}/settings → returns existing + layout_* fields
// PATCH /corpora/{slug}/settings → accepts any subset, persists deltas
```

No new route; just expand the existing JSON shape.

### Phase 6 — Frontend

On `/config/corpus` (which already shows per-corpus chunker overrides):
add a **Native PDF Extraction** section mirroring `/config/onnx`:

- `LAYOUT_ML_ENABLED` toggle — with explicit `(inherit from global)`
  tri-state option (None / true / false).
- `LAYOUT_ML_MODEL_ID` input — empty string = inherit.
- `LAYOUT_DETR_MODEL_PATH` / `LAYOUT_ORT_MODEL_PATH` inputs.
- `LAYOUT_DETR_THRESHOLD` and `LAYOUT_DETR_NUM_CLASSES` numeric inputs.

Each control needs an explicit "use global" affordance so the user can
clear a per-corpus override and revert to inheritance.

Add a chip on `/config/onnx` per-corpus override **counter**: e.g.
`Corpora with overrides: 2 (engineering-papers, scratch) — view`,
linking to `/config/corpus`. Makes the inheritance picture discoverable.

### Phase 7 — Tests

- Unit tests in `db/corpora.rs` for `effective_layout_config` merge
  precedence (8 combinations: every field None vs Some, override vs
  global differing).
- Integration test: PATCH per-corpus override → upload PDF →
  `extraction_records` shows the override took effect (different
  extractor tag or chars count vs the global-only path).
- Cache-key test: two corpora with same effective config share the
  same `Arc<LayoutModel>` in the cache (no duplicate model loads).

### Phase 8 — Docs

- Update `CLAUDE.md` § Runtime settings layer to mention the per-corpus
  layer.
- Update `/docu/index/onnx` (or wherever Native PDF docs live) with
  the precedence model.
- Add to the LAYOUT_ML_MODEL_ID info modal a note: "Per-corpus override
  available on `/config/corpus`."

## Migration / compatibility

- All new corpus fields are `Option<...>` — existing rows roll forward
  unchanged.
- Existing global behaviour preserved: a corpus with no overrides reads
  from the global settings layer exactly as today.
- `LayoutModel::load_or_heuristic()` kept as a no-corpus convenience for
  any caller that doesn't yet thread the context (e.g.
  `config_routes.rs:1469` capability check — keep it as the "what's
  loaded with the default config?" probe).

## Risks

| Risk | Mitigation |
|---|---|
| Memory bloat from caching N layout models for N corpora | Cache key dedups identical configs; in practice most deployments use ≤3 distinct models. Add eviction policy later if needed. |
| Cold-start latency on first upload to a new corpus with a different model | Same as today's first-upload cold start — but multiplied by number of distinct configs. Document; consider a warm-up endpoint. |
| Trait signature change breaks downstream consumers | Only `DocExtractor` impls in-tree (3) and the registry caller (1). Search & replace before merging. |
| Per-corpus config drift / inheritance confusion | UI must show effective value + source ("from corpus" / "inherited"). Mirror the runtime-settings page treatment. |

## Out of scope

- Per-corpus chunker mode integration with per-corpus layout config —
  they're already independent; no coupling needed.
- Per-document overrides (corpus-level is enough for the foreseeable
  use case).
- GPU/CUDA execution provider selection per corpus — keep that global
  for now.
- Migrating non-PDF extractors (Docling, Unstructured) to per-corpus —
  same trait extension allows it later, but no current ask.

## Open questions for the PR review

1. Should `effective_layout_config` cache its merged result, or
   re-compute on every extract call? (Current `effective_chunker_config`
   re-computes; same precedent makes sense.)
2. Should an explicit `LAYOUT_ML_ENABLED=false` at corpus level
   completely bypass the registry, or just bypass the ML pipeline
   inside `NativePdfExtractor::extract`? (Latter is simpler — bail
   early in `can_handle` based on context, but `can_handle` doesn't
   currently take a context. Trait change adds context to
   `can_handle` too.)
3. How to expose the per-corpus chip count on `/config/onnx` without
   another DB query on every page load? (Cache or compute on the
   corpus-settings GET response.)
