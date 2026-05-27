# Make Native PDF Extraction corpus dependent

## Context

Native PDF Extraction (the `layout_ml` pipeline: lopdf word bboxes → DETR/heuristic region classifier → TableFormer → DocIR) is currently controlled by a **single global** boolean `LAYOUT_ML_ENABLED`. The decision is made once at startup in `backend/src/main.rs:512–528`: when the flag is on, `NativePdfExtractor` is registered in the global `extractor::Registry` and every PDF upload — across every corpus — runs through it. There's no way to keep clean per-corpus segregation (e.g. use Native PDF for a `papers` corpus that needs table structure, but plain `pdftotext` for a `scratchpad` corpus where speed matters more).

The user wants this decision to be per-corpus. `CorpusSettings` already overrides several global defaults the same way (`chunker_mode`, `distance_metric`, HNSW params, …) — Native PDF should follow that pattern: `None` = inherit the global, `Some(true/false)` = override for this corpus.

## Approach at a glance

```
   /config/corpus  ──► CorpusSettings.native_pdf_enabled  ─┐
                                                            ▼
   /config/onnx    ──► LAYOUT_ML_ENABLED  (global default) ─┤
                                                            ▼
                              effective_native_pdf_enabled(slug)
                                                            │
   PDF upload (file watcher / /upload)                      │
       │                                                    │
       ▼                                                    │
   index::extract_ir_async(path, corpus) ──────────────────►│
       │                                            corpus-aware exclude list
       ▼                                                    │
   extractor::Registry  ──── if disabled, skip "native_pdf"─┘
       │                       (Docling sidecar still runs if it's registered;
       │                        otherwise falls through to built-in pdftotext)
       ▼
   DocIR  ──►  chunker  ──►  index
```

Key shift: today's "register iff global on" becomes "register whenever the Cargo feature is compiled in; the global flag is just the default value of the per-corpus override". Model **pre-warm** stays gated on the global flag so we don't pay startup cost when no corpus uses Native PDF.

## Changes

### 1. Backend — settings layer

**`backend/src/db/corpora.rs`** — extend `CorpusSettings`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub native_pdf_enabled: Option<bool>,
```

Add at module bottom:

```rust
/// Effective Native PDF Extraction setting for `slug`:
///   per-corpus override → global LAYOUT_ML_ENABLED → false.
pub fn effective_native_pdf_enabled(conn: &Connection, slug: &str) -> bool {
    if let Ok(s) = get_corpus_settings(conn, slug) {
        if let Some(v) = s.native_pdf_enabled {
            return v;
        }
    }
    crate::settings::effective_bool("LAYOUT_ML_ENABLED", false)
}
```

No new DB column needed — `CorpusSettings` is stored as JSON in `corpora.settings`, the new field round-trips automatically.

### 2. Backend — extractor registry filter

**`backend/src/extractor.rs`** — keep the public API stable, add name-filtered variants:

```rust
impl ExtractorRegistry {
    pub fn has_handler_filtered(&self, ct: &ContentType, exclude: &[&str]) -> bool {
        self.extractors.iter()
            .any(|e| !exclude.contains(&e.name()) && e.can_handle(ct))
    }

    pub fn extract_filtered(
        &self,
        bytes: Vec<u8>,
        filename: &str,
        ct: &ContentType,
        exclude: &[&str],
    ) -> Option<DocIR> {
        // same loop as `extract()` but filter `matching` by name first
    }
}
```

The existing `has_handler` / `extract` become thin wrappers that call the filtered versions with an empty slice — no churn for non-PDF call sites.

### 3. Backend — wire the per-corpus decision into extraction

**`backend/src/index.rs`** (`extract_ir` at ~989 and `extract_ir_async` at ~1014):

Both already receive `corpus: &str`. Before consulting the registry:

```rust
let exclude: &[&str] = {
    // open a short-lived connection; same pattern as file_watcher.rs:287
    let conn = crate::db::open_connection(/* …existing path manager helper… */).ok();
    let disabled = conn
        .as_ref()
        .map(|c| !crate::db::corpora::effective_native_pdf_enabled(c, corpus))
        .unwrap_or(false);
    if disabled { &["native_pdf"] } else { &[] }
};
```

Then swap `reg.has_handler(&ct)` → `reg.has_handler_filtered(&ct, exclude)` and `reg.extract(...)` → `reg.extract_filtered(..., exclude)`.

If pulling a DB connection on every extract is too heavy, cache the resolved bool on the `Retriever` / file-watcher config the same way `file_watcher.rs:287` already pulls `get_corpus_settings` per scan. Worst case is one extra SQLite query per file (cheap; settings table is tiny).

### 4. Backend — startup wiring

**`backend/src/main.rs:508–528`** — flip the gate semantics:

```rust
#[cfg(feature = "layout_ml")]
{
    // Always register so corpora can opt-in even with global off.
    let native = ag::pdf::native_extractor::NativePdfExtractor;
    ag::extractor::init_registry(vec![Box::new(native)]);

    // Pre-warm models only when the *default* is on, to avoid paying the
    // load cost when no corpus actually uses Native PDF.
    if ag::settings::effective_bool("LAYOUT_ML_ENABLED", false) {
        tokio::task::spawn_blocking(|| {
            ag::pdf::layout_model::LayoutModel::load_or_heuristic();
            ag::pdf::table_model::TableModel::load_or_text();
        });
        info!("✅ NativePdfExtractor registered + pre-warmed (global default on)");
    } else {
        info!("✓ NativePdfExtractor registered, models lazy-load on first use");
    }
}
```

Note: `init_registry` currently warns on second call — the existing Docling block at ~496 also calls it. That call already merges Docling + Native into one `init_registry(vec![…])`; keep that merged init (Docling first, Native second) so registry order = priority order. Re-read main.rs:480–528 carefully when implementing to preserve the existing fusion / waterfall configuration.

### 5. Backend — `KnownKey` description

**`backend/src/settings/registry.rs:209`** — append one sentence to the `LAYOUT_ML_ENABLED` description:

> "This is now the **default** for new corpora — each corpus can override on /config/corpus."

The key stays restart-required (model pre-warm is startup-time); per-corpus overrides do *not* require restart (they're consulted per-extract).

### 6. Frontend — API DTO

**`frontend/fro/src/api.rs:4348`** — add to `CorpusSettings`:

```rust
pub native_pdf_enabled: Option<bool>,
```

(The struct already has `#[derive(Default)]` and serde handles `Option` correctly.)

### 7. Frontend — /config/corpus UI

**`frontend/fro/src/pages/config_corpus.rs`** — add a 7th tri-state control to the per-corpus settings row at lines ~322–416. Use the same `<select>` pattern as `chunker_mode` (lines 343–356), since the field is tri-state (None = inherit, false = off, true = on):

```rust
div { class: "flex flex-col gap-1",
    div { class: "flex items-center gap-1",
        label { class: "text-xs text-gray-400 shrink-0", "Native PDF" }
        button { class: BTN_CLASS, style: BTN_STYLE,
            onclick: move |_| show_native_pdf.set(!show_native_pdf()),
            {info_icon()} }
    }
    select {
        class: "select select-sm select-bordered bg-gray-700 text-gray-200",
        value: native_pdf_str(),
        onchange: move |evt| native_pdf_str.set(evt.value()),
        option { value: "", "— global —" }
        option { value: "true",  "on" }
        option { value: "false", "off" }
    }
}
```

Wire the signal alongside the existing per-corpus signals (`chunker_mode`, `metric`, etc.) at lines 36–43, 75–86, 100–123, and the save closure at 137–145. Serialize as:

```rust
native_pdf_enabled: match native_pdf_str().as_str() {
    "true"  => Some(true),
    "false" => Some(false),
    _ => None,
},
```

Add a matching info-panel block in the `if show_native_pdf() { … }` group (lines ~419–468) explaining the trade-off (Native PDF gives block-type tags + table structure; plain pdftotext is faster and side-effect-free).

Mention in the existing "Global defaults" info block at lines 309–316 that the new field exists.

### 8. /config/onnx — clarifying caption

**`frontend/fro/src/pages/onnx.rs`** in the Native PDF Extraction tile (around line 280): add a single sentence under the toggle: "This is the default for all corpora — override per corpus on /config/corpus." Keep the existing modal text untouched; the per-corpus override is documented in the new corpus-page info panel.

## Files touched

| File | Change |
|---|---|
| `backend/src/db/corpora.rs` | new field, new `effective_native_pdf_enabled` helper |
| `backend/src/extractor.rs` | add `has_handler_filtered` / `extract_filtered` |
| `backend/src/index.rs` | compute exclude list, call filtered variants |
| `backend/src/main.rs` | always register Native; gate only pre-warm on global |
| `backend/src/settings/registry.rs` | append one sentence to `LAYOUT_ML_ENABLED` description |
| `frontend/fro/src/api.rs` | new field in `CorpusSettings` |
| `frontend/fro/src/pages/config_corpus.rs` | new tri-state select + signal + save wiring + info panel |
| `frontend/fro/src/pages/onnx.rs` | one-sentence caption pointing to corpus page |

## Verification

1. `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings` — must pass.
2. `cd backend && cargo test --all` — existing extractor / corpus tests should pass; add a unit test in `db/corpora.rs` covering the three states of `effective_native_pdf_enabled` (override true / override false / fall-through to global).
3. Manual smoke test:
   - `LAYOUT_ML_ENABLED=true` globally. Create corpora `papers` and `scratch`. Set `scratch.native_pdf_enabled = false` via `PATCH /corpora/scratch/settings`.
   - Upload the same PDF into both. Inspect `/monitor/tip` (or DocIR debug endpoint): `papers` should show block-type tags (`Header`, `Table`, …); `scratch` should show one `Text` block (the pdftotext fallback). The `extractor_tag` on the resulting DocIR is the easiest visible signal — `"native_pdf"` vs `"pdftotext"`.
   - Flip `LAYOUT_ML_ENABLED=false` globally with no corpus override; both corpora should fall back to pdftotext.
   - Set `scratch.native_pdf_enabled = true` with global off; that corpus alone should use Native PDF (models lazy-load on first PDF).
4. Frontend: visit `/config/corpus`, switch corpus, toggle the new select, hit Save, refresh, confirm the value round-trips. Re-upload a PDF and confirm the behavior change without restarting the backend.
