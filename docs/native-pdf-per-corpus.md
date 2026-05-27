# Native PDF Extraction vs non-native (TIP path)

What changes downstream — in the Text Ingestion Pipeline (`/monitor/tip`) — when
the native PDF extraction pipeline is on, vs. the plain pdftotext fallback.
Grounded in the actual code paths, not abstract claims.

## The two paths

**Non-native** (`backend/src/index.rs:1945`):
`pdftotext` (poppler) → flat `String` → `flat_text_ir` (`index.rs:1234-1242`)
wraps it in **one `DocBlock::text`** with `BlockType::Text`. Page numbers,
table boundaries, headings — gone.

**Native** (`backend/src/pdf/native_extractor.rs`): produces a `DocIR` with
typed `DocBlock`s — `Text`, `Header{level}`, `Table{rows,cols}`, `Code`,
`Caption`, `List`, `Image`, `Formula`, `PageBreak` — each carrying `page`
and (when available) `bbox`.

## Where the typed blocks actually get read

Three TIP stages dispatch on `BlockType`:

| TIP stage | Without native (flat Text) | With native (typed DocIR) |
|-----------|----------------------------|---------------------------|
| **Chunker** (`memory/chunker_factory.rs:85-124`) | Plain N-char windows. Tables get sliced mid-row, headers don't reset chunk boundaries, code blocks can split between `}` lines. | `is_atomic()` → tables / code / formulas / images become **one chunk, never split** (line 85). `is_strong_boundary()` → headers / page breaks **flush the buffer** (line 103) so a chunk never spans two sections. Each chunk carries `ChunkMeta { block_type, page, extractor }`. |
| **Retriever** (`retriever.rs:1221-1230`) | Every chunk's `block_type` is `"Text"`. No structural signal at query time. | Block-type tally per corpus; `upload_search.rs:666` returns `block_type` in search hits so the UI / reranker can boost-by-role (e.g. weight a Header match higher than body Text). |
| **Monitor** (`monitor_routes.rs:1700-1721`, `/monitor/datastores`) | Block-type distribution is uniformly `"Text"` — uninformative. | Real distribution: "this corpus is 60% Text / 12% Table / 8% Header / …" — visible structural insight per corpus. |

## Net retrieval-quality impact

1. **Tables stay coherent** → table cell ↔ row context survives into one
   vector. A search for "GPU utilisation 84%" doesn't return a torn fragment.
2. **Section-bounded chunks** → no chunk that's half "Methods" and half
   "Results", which is a classic source of confused hits.
3. **The reranker / agent loop can see *what kind* of block produced a hit**
   instead of treating everything as faceless text.

## Costs

- **Ingest latency** — pdfium render + DETR inference per page (or
  heuristic, which is cheap). Query latency unchanged.
- **Memory** — rendered page bitmaps held during classify.
- **Graceful degradation** — every stage falls through (Tier 0 → 1 → 2 →
  heuristic → text-mode tables → flat text), so worst case you land on
  something equivalent to the non-native path. The pipeline never refuses
  to run.

## Bottom line

Native PDF extraction is what turns the chunker, retriever, and monitor from
"text in, text out" into structure-aware components. Without it those features
exist but have nothing to dispatch on.
