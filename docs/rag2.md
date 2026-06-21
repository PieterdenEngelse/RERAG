# Relational PDF Extraction for RERAG — Planning Notes

Inspired by *"Stop returning flat text from a PDF — the relational shape RAG needs"*
(https://towardsdatascience.com/stop-returning-flat-text-from-a-pdf-the-relational-shape-rag-needs/).

This document captures the design conversation, the decisions taken, the
existing scaffolding the plan rides on, and the Phase 1 shape ready to
implement.

---

## 1. The article in one paragraph

Naive `text = extract_text(pdf)` flattens tables, columns, captions, and
cross-references into one long string. A Schedule-of-Charges table becomes
meaningless concatenated text — "EUR 200 one-time, Late payment 75" — and
RAG can't answer "what's the renewal fee?" because the column relationship
that pairs labels with numbers is gone.

The article's fix: parse each PDF **once** into ~8 linked tables and query
that relational shape instead of re-parsing or chunking flat text:

- `toc_df` — native document outline
- `line_df` — every line with position, typography, column assignment
- `page_df` — per-page aggregations (page type, OCR quality flags)
- `image_df` — figures with bounding boxes and content hashes
- `object_registry` — figure/table/annex captions (the targets)
- `cross_ref_df` — body-text mentions like "see Figure 2" (the sources)
- `span_df` — sub-line typography (bold, italic, colour, font size)
- `parsing_summary` — document-level metadata (scanned vs native, OCR
  quality, document type)

Coordinate-anchored joins via `(page_num, y-coordinate)`. The article uses
PyMuPDF (fitz); production systems persist to SQLite.

---

## 2. Why RERAG should implement this — plain version

Today RERAG reads a PDF the way a blender reads a fruit salad. Everything
goes in, gets pulped into one smooth stream of text, and then you try to
fish out the apple. Tables, columns, captions, "see Figure 2" — that
structure was *information*, and the blender threw it away before search
ever ran.

The article says: stop blending. Read the PDF the way a human reads it —
noticing "this is a table", "this is the left column", "this caption
belongs to that figure", "this line is in the table of contents and points
to page 14." Then write all of that down in a few connected lists before
doing anything else.

Why RERAG specifically:

1. **Most "bad RAG answers" aren't a model problem — they're a shape
   problem.** The model never had a fair chance because the input was
   already mangled. Fixing extraction is higher-leverage than fancier
   retrieval.
2. **The right tools are already in the stack.** SQLite for rows, FalkorDB
   for relationships, Tantivy for search. Most projects would have to add
   a graph database to do this.
3. **It fits what RERAG is *for*.** Educational mission — make the
   invisible visible. The extraction step is the most invisible part of
   every RAG system. A dashboard that shows "here's the table we found,
   here's the left column tagged, here's the cross-reference we resolved"
   actually *teaches* how RAG works instead of just running a chat box.
4. **It's the difference between "demo that impresses" and "demo that's
   honest."** Today, ask the app a question about a two-column invoice
   and it confidently gives a wrong answer. That's the failure mode every
   real RAG user hits in week one.

One-line version: structure is information; throwing it away during
ingestion is the original sin of RAG, and RERAG happens to already have
the tools to stop committing it.

---

## 3. Decisions taken

Each of these started as an open design question and got resolved before
writing code.

### 3.1 Cargo feature placement → extend `layout_ml`

**Decision: no new feature. Fold relational extraction into the existing
`layout_ml` feature.**

Why: `layout_ml` already pulls in `pdfium-render`, `lopdf`, `extractous`,
`ort`, `image`, `hf-hub`. A new `relational_pdf` feature would either
duplicate those deps or depend on `layout_ml` anyway, so the user effectively
picks "both or neither." One feature is one less thing on `/config/onnx`
to explain.

### 3.2 Tantivy granularity → single index + facet (not two indexes)

**Decision: single Tantivy index. Add `column_position` as a multi-valued
facet on chunks. Defer line-level indexing past Phase 1.**

Why two indexes is wrong: two writers, two warmers, two snapshot paths, two
cache layers in `retriever.rs`. The facet approach is additive — existing
chunk docs simply lack the facet, every old query keeps working.

Tradeoff: if we later add line-level indexing, the line corpus is ~10–50×
larger than the chunk corpus, so facet-less queries become slow if a
handler forgets to filter. Mitigation: default every search handler to
`granularity:chunk` and require any new PDF tool to opt into
`granularity:line` explicitly.

### 3.3 Activation → per-corpus opt-in

**Decision: `PDF_RELATIONAL_ENABLED` global default, per-corpus override
on `/config/corpus`. Same shape as `LAYOUT_ML_ENABLED`.**

Why: always-on is tempting for the educational angle, but parse cost on
large PDFs is not trivial (pdfium loads the whole doc; column k-means
runs per page). Per-corpus matches the Native PDF pattern users already
understand. For maximum visibility, ship with the default corpus
pre-enabled in a migration — gives "on by default for new installs"
without being unconditional.

### 3.4 Line embeddings → no

**Decision: BM25 + facets only. No line-level embeddings.**

Why this is the firmest of the four: embedding every line means roughly
10–50× the embedding workload per PDF, balloons the `embeddings` table,
and forces a `granularity` column on a table that's currently keyed on
`chunk_id`. The article's use case ("give me lines in column 1 of page 3
that mention 'tax'") is keyword + structural, not semantic; BM25 + facets
answers it directly. Chunks stay as the semantic-retrieval unit. If
semantic line search becomes a need later, the line rows in SQLite are
already there — no rewrite.

### 3.5 Risk: k-means k=2 column detection is brittle

Fine for the demo invoice; breaks on 3-column layouts, mixed
single/multi-column pages, and figures floating across columns.

**Mitigation in Phase 1:** keep it dumb on purpose (always k=2), but
**log the silhouette score per page** so Phase 2 can promote to adaptive-k
without a schema or UI change. Pages with silhouette below threshold
(say 0.3) get tagged `column_position = 'multi'` — don't pretend to know.

---

## 4. The big design point that nearly got missed

**Chunk-level column facets alone do NOT answer the article's renewal-fee
question.** A chunk with `column_position_set = {left, right}` containing
`"EUR 200 one-time Late payment 75"` still confuses the LLM — the facet
filters which chunks come back, but doesn't disambiguate within a chunk.

The fix: **make the chunker column-aware.** Two same-page blocks in
different columns become a strong boundary, exactly like `PageBreak` is
today. After that, every chunk has at most one non-`multi` column_position,
and the facet does real work.

This is a pure `chunk_ir` change. No Tantivy schema change beyond adding
the facet field. No line-level Tantivy indexing needed in Phase 1.

---

## 5. Existing scaffolding the plan rides on

The codebase has more of this already built than initially assumed. The
relevant existing pieces (verified against current `main`):

| Already there | Where | What it does |
|---|---|---|
| `WordSpan { text, page, bbox: Option<[i64;4]> }` | `backend/src/pdf/word_extractor.rs` | lopdf parses content streams, emits words with bboxes normalised to 0–1000. extractous is the text-only fallback. |
| 4-stage native PDF pipeline | `backend/src/pdf/native_extractor.rs` | words → region classification (DETR/heuristic) → table model → `build_ir` |
| `DocIR` + `DocBlock` with `bbox: Option<BoundingBox>` and `page: Option<u32>` | `backend/src/doc_ir.rs` | The chunker consumes DocIR, not flat text. Bbox is already a first-class field on every block. |
| `PageBreak` boundary blocks | `backend/src/doc_ir.rs:129` | Chunker resets `section_id` per page; that pattern (boundary block → chunker behavior change) is the template for column awareness. |
| `ChunkMeta { page, extractor, heading_path, section_id }` | `backend/src/doc_ir.rs:236` | Already survives the chunking boundary into search results — the place to add `column_position_set`. |
| Per-corpus opt-in scaffolding | `backend/src/db/corpora.rs:161`, `backend/src/index.rs:990` | `effective_native_pdf_enabled` already exists. Reusable verbatim. |
| `LAYOUT_ML_ENABLED` global + `/config/onnx` "Feature compiled" tile | `backend/src/settings/registry.rs:206`, `backend/src/api/config_routes.rs:1488` | Same shape as `PDF_RELATIONAL_ENABLED` will need. |
| `chunks.metadata_json` column | `backend/src/db/schema.sql:21` | No schema change needed to attach `column_position_set` to existing chunks. |

What's missing is narrower than originally planned:

1. **No line grouping** — `WordSpan` exists, but there's no `LineSpan`
   stage that y-clusters words into lines
2. **No column classifier** — bboxes exist, nothing reads them spatially
3. **No `pdf_lines` / `pdf_pages` persistence** — words are in-memory only
4. **No TOC / cross-ref extraction** — pdfium-render's `get_toc` /
   `get_links` not called anywhere
5. **No `column_position` facet on Tantivy chunks**

---

## 6. How the eight tables map onto RERAG's existing stack

No fourth vector store. Reuses what's there.

| Article's table | Natural home in RERAG |
|---|---|
| `line_df`, `page_df`, `span_df` | **SQLite** (`agent.db`) — row-shaped, queried by `(page, y)` joins, not by similarity |
| `toc_df`, `cross_ref_df`, `object_registry` | **FalkorDB** — TOC→section→line and "see Figure 2"→caption are edges. This is what GraphRAG is for. |
| `line_df` text content (later) | **Tantivy** stays, but per-line with `(doc_id, page, line, column_position)` as facets so queries can filter `column_position == "right"` instead of regex-hunting |
| `image_df` | SQLite row + Tantivy text field for the vision caption |
| `parsing_summary` | New `documents` column or extend existing ingestion metadata |

---

## 7. Phased plan

### Phase 1 — smallest useful slice

Goal: a two-column invoice answers "what's the renewal fee?" correctly.

**New files (two):**
- `backend/src/pdf/line_grouper.rs` — `Vec<WordSpan> → Vec<LineSpan>` by
  y-clustering per page
- `backend/src/pdf/column_detect.rs` — k=2 k-means on line `x0` per page
  → `column_position` + silhouette score

**Modified files:**
- `backend/src/pdf/ir_builder.rs` — annotate each `DocBlock` with
  `column_position` derived from its source lines (single value when all
  source lines agree, `multi` otherwise). Stash in
  `block.metadata["column_position"]`.
- `backend/src/pdf/native_extractor.rs` — wire `line_grouper` +
  `column_detect` between Stage 1 (word extraction) and Stage 2 (region
  classification); persist `pdf_lines` + `pdf_pages` rows via a callback
  or returned struct.
- `backend/src/memory/chunker.rs` (the IR-aware chunker) — treat
  *cross-column same-page* transitions as strong boundaries, same as
  PageBreak. Propagate `column_position` into `ChunkMeta`.
- `backend/src/doc_ir.rs` — add `column_position_set: BTreeSet<ColumnPosition>`
  to `ChunkMeta`.
- `backend/src/db/schema.sql` — add `pdf_lines` and `pdf_pages` tables.
  Bump migration version.
- `backend/src/retriever.rs` (Tantivy schema) — add `column_position` as
  a multi-valued facet field on chunks. Existing chunks lack it → no-op
  for non-PDF corpora.
- `backend/src/settings/registry.rs` — add `PDF_RELATIONAL_ENABLED`
  (corpus default, no restart required since it's a per-extract decision).
- `backend/src/db/corpora.rs` — add `relational_pdf_enabled` per-corpus
  override beside the existing `native_pdf_enabled`.
- `frontend/fro/src/pages/...` — extraction view shows line bboxes
  coloured by column on a per-page canvas, with silhouette score as a
  confidence badge. Locate existing PDF extraction page first.

**SQLite schema:**

```sql
CREATE TABLE IF NOT EXISTS pdf_lines (
  document_id TEXT NOT NULL,
  page INTEGER NOT NULL,
  line_idx INTEGER NOT NULL,
  text TEXT NOT NULL,
  x0 INTEGER, y0 INTEGER, x1 INTEGER, y1 INTEGER,  -- 0..1000 normalised
  column_position TEXT NOT NULL
    CHECK(column_position IN ('single','left','right','multi')),
  PRIMARY KEY (document_id, page, line_idx),
  FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_pdf_lines_doc_page
  ON pdf_lines(document_id, page);
CREATE INDEX IF NOT EXISTS idx_pdf_lines_column
  ON pdf_lines(document_id, column_position);

CREATE TABLE IF NOT EXISTS pdf_pages (
  document_id TEXT NOT NULL,
  page INTEGER NOT NULL,
  line_count INTEGER NOT NULL,
  column_k_used INTEGER NOT NULL DEFAULT 2,
  column_silhouette REAL,
  is_scanned INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (document_id, page),
  FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
);
```

**Done when:**
- Upload `two_column_invoice.pdf`, ask "what's the renewal fee?", get the
  right number from the right column
- `/docs/{id}/extraction` shows the invoice with left and right columns
  visibly tagged + silhouette badge
- `cargo fmt && cargo clippy --all-targets -- -D warnings` clean
- Fixture tests green

### Phase 2 — full table set

Add the remaining tables to SQLite. All still single-store, no graph yet.

- `pdf_pages` page-type classifier (cover / TOC / body / appendix)
  heuristic over `pdf_lines`
- `pdf_images` — image extraction with content hash; vision captions
  disabled by default behind `PDF_VISION_CAPTIONS` toggle
- `pdf_spans` — sub-line typography (`bold`, `italic`, colour,
  `font_size` deltas) built lazily on first access, not eagerly
- `pdf_objects` — captions linked to figures via spatial proximity +
  "Figure N" regex
- `pdf_parsing_summary` — document-level metadata table
- Promote column detection from k=2 to adaptive-k using collected
  silhouette scores
- Scanned-PDF detection sets `is_scanned=1`; those docs fall back to the
  current flat-text path with a banner saying so
- Dashboard expansion: page-type badges, per-page metrics row, image
  previews

### Phase 3 — graph + agent tools

This is where FalkorDB earns its keep.

- FalkorDB writers persist `TOC` (Section→Section, Section→Line),
  `CROSS_REF` (Line→Caption), `OBJECT_REGISTRY` (Caption→Figure/Table)
  edges
- Agentic mode (`backend/src/agent.rs`) gets new Rig tools:
  - `get_toc(doc_id) -> tree`
  - `get_lines_in_column(doc_id, page, column) -> [line]`
  - `get_cross_references_for(doc_id, page, line) -> [target]`
  - `get_table_near(doc_id, page, y) -> table_rows`
- Dashboard: a FalkorDB Browser link pre-loaded with
  `MATCH (s:Section)-[:CONTAINS*]->(l:Line) WHERE l.doc='...'` so users
  *see* the structure as a graph — the "make the invisible visible"
  payoff
- Info modal on the extraction page walks through: "this is what a
  flat-text chunker would have given you. This is what we have instead.
  Watch the agent ask `get_lines_in_column('right')` instead of
  regexing."

---

## 8. Risks & mitigations

1. **PDFium native dep complicates installer** — feature-gated, installer
   already handles runtime layout, document the .so location in
   `installer/`.
2. **k=2 k-means brittle on 3-column / asymmetric layouts** — Phase 1
   supports 1–2 columns explicitly; everything else returns `multi` and
   we don't pretend to be smart. Silhouette score persisted from day one
   enables Phase 2 promotion without UI changes.
3. **Scanned PDFs need OCR** — Phase 1 punts;
   `parsing_summary.is_scanned=true` falls back to existing flat-text
   chunkers. OCR is its own phase or never.
4. **Dashboard becomes a developer debug tool, not a learning tool** —
   every new viz gets an info modal answering "what is this teaching the
   user about how RAG works." If the answer is empty, the viz doesn't
   ship.
5. **Adds yet another vector store by accident** — hard rule, restated
   in the PR: SQLite for rows, FalkorDB for edges, Tantivy for search.
   Nothing else.
6. **Re-indexing all PDFs on first upgrade is a footgun** — opt-in via
   existing reindex flow, not automatic.

---

## 9. Test strategy

- **Unit**: column classifier given synthetic x-coordinate inputs;
  cache-invalidation on file SHA change; line-grouper edge cases (single
  word per line, overlapping y-ranges, words missing bboxes)
- **Integration** (`backend/tests/pdf_extraction.rs`): ship three fixture
  PDFs in `backend/tests/fixtures/pdf/`:
  - `two_column_invoice.pdf`
  - `single_column_article.pdf`
  - `toc_with_links.pdf`
  Assert row counts + column distribution + a small set of golden
  `(page, line) → column_position` pairs
- **API**: `/upload` fixture → `/search` for known answer returns right
  chunk with correct `column_position`
- **Quality gate**: existing
  `cargo fmt && cargo clippy --all-targets -- -D warnings`

---

## 10. Open question before coding starts

**Should the LLM tool layer (Agentic mode) get a
`get_lines(doc_id, page, column)` tool in Phase 1, or wait until Phase 3
with the rest of the relational tools?**

Adding it in Phase 1 makes the educational value concrete on day one —
users can watch the agent ask for "right-column lines on page 3" instead
of regexing. But it widens Phase 1's scope into `backend/src/agent.rs`
and the Rig tool registry.

Leaning **yes, ship it in Phase 1** — the column-aware chunker already
gets most of the way there, and one tool isn't expensive. User's call.

---

## 11. Smallest path to "I can see it work"

1. Land Phase 1 behind the existing `layout_ml` feature flag, with
   `PDF_RELATIONAL_ENABLED` default off
2. Upload `two_column_invoice.pdf`, query "renewal fee" — get the right
   number from the right column
3. Open `/docs/{id}/extraction`, see the left/right column tagging +
   silhouette badge
4. Ship Phase 2 + 3 only after the Phase 1 dashboard page is something
   you'd *show someone* to teach them what RAG ingestion is doing
