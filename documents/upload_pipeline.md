# Upload Pipeline

How a document goes from HTTP request to indexed chunks.

---

## Stages

### 1. HTTP ingress (actix-web)

The request hits `POST /upload` as a multipart stream. Before any byte touches disk,
`PayloadConfig` enforces a hard size cap (default 150 MB, set via `UPLOAD_MAX_MB`).
Requests over the limit are rejected immediately with **413 Payload Too Large**.

The upload handler (`api/upload_search.rs`) streams each multipart field to disk
chunk by chunk — no full file buffering in RAM at this stage.

Allowed extensions are checked here. Unknown types get **400 Bad Request** before
any extraction work begins.

### 2. Extraction (index.rs — `extract_ir_async`)

Each saved file is read and converted to a **DocIR** — a typed intermediate
representation of the document's structure (headers, paragraphs, tables, code
blocks, etc.). This is where format-specific work happens.

**External path (Docling sidecar, if `DOCLING_ENABLED=true`)**

The file's bytes are moved (not cloned) into the `DoclingExtractor`, which POSTs
them to the sidecar at `DOCLING_URL/convert`. The sidecar returns DocIR JSON.
If the sidecar is unreachable or returns an error, the backend falls through to the
built-in path and re-reads the file from disk — one extra disk read, no double-copy
in RAM.

**Built-in path**

| Format | Strategy | Notes |
|--------|----------|-------|
| PDF (text layer) | `pdftotext` subprocess | Bounded memory; reads from path |
| PDF (image-only, ≤25 MB) | `pdftoppm` + `tesseract` | Up to 20 pages OCR'd |
| PDF (image-only, >25 MB) | none | Returns no text; use Docling sidecar |
| Markdown / HTML | in-process parser | Typed blocks (headers, code, tables) |
| DOCX / ODT / EPUB / PPTX | in-process XML parser | Typed blocks |
| XLSX / ODS / CSV | calamine | Flat text |
| Code files | in-process | Single Code block |

`pdf_extract` (the pure-Rust PDF crate) was removed because it loads the full PDF
DOM in-process and OOMs on files above ~10 MB with complex fonts or embedded images.

All extractions run concurrently across a multi-file upload (`join_all`), so
uploading 5 files at once processes them in parallel.

### 3. Chunking (`memory/chunker_factory.rs — chunk_ir`)

DocIR blocks are fed to the active chunker (set by `CHUNKER_MODE`):

| Mode | Behaviour |
|------|-----------|
| `fixed` | Split on token count; respects block boundaries |
| `lightweight` | Paragraph-aware split |
| `semantic` | Embedding-similarity split |

Atomic blocks (tables, code, formulas) are never split across chunk boundaries
regardless of mode. Header blocks flush the current chunk before starting a new one.

The result is a flat `Vec<(text, ChunkMeta)>` where each `ChunkMeta` carries the
source block type, page number, and bounding box (if available from Docling).

### 4. Normalisation

Each chunk is normalised twice in parallel `map` passes:

- **Embed normalise** — Unicode NFC, whitespace collapse, ligature expansion.
  This is what goes to the embedding model.
- **Index normalise** — additionally lowercased and diacritic-stripped.
  This is what goes into Tantivy for full-text search.

Both passes record byte-in / byte-out metrics to the `canon_*` Prometheus counters.

### 5. Embedding (`embedder::embed_batch`)

All embed-normalised chunks are passed to the embedder in one call.
Internally the embedder splits them into batches of `config.batch_size` and runs
the ONNX model (bge-small-en-v1.5 by default) on each batch. Output is a
`Vec<Vec<f32>>` of 384-dim vectors.

### 6. Index write (Retriever lock)

The retriever mutex is acquired **once** for the whole batch. For each chunk:

- Vector inserted into the HNSW in-memory index and persisted to LanceDB.
- Index-normalised text inserted into Tantivy (full-text).
- Chunk offered to the golden-sample reservoir for training data.

After all chunks are written, `commit_batch()` flushes Tantivy to disk and the
mutex is released.

If Neo4j is enabled, graph indexing runs **after** the mutex is released — one
Cypher upsert per chunk, fire-and-forget, doesn't block the upload response.

---

## Memory profile (20 MB PDF, text layer, no Docling)

| Stage | Peak extra RAM | Released when |
|-------|---------------|---------------|
| Stream to disk | ~64 KB (chunk buffer) | Each chunk |
| `io_uring::read_file` | 20 MB | `drop(bytes)` in `extract_text_from_bytes` |
| `pdftotext` subprocess | ~5–20 MB (OS) | Subprocess exits |
| Extracted text | ~1–4 MB | After chunking |
| All chunks (×3 normalised) | ~3–12 MB | After embed |
| Embeddings (500 chunks) | ~0.75 MB | After index write |

Total peak: roughly **25–45 MB** above baseline for a 20 MB text-layer PDF.
Before the fixes, `pdf_extract` added 200 MB–1 GB to this profile and could hang.

---

## Configuration

```bash
UPLOAD_MAX_MB=150       # Hard cap; 413 before disk write
DOCLING_ENABLED=true    # Use sidecar for PDF/DOCX/PPTX
DOCLING_URL=http://localhost:5001
CHUNKER_MODE=fixed      # fixed | lightweight | semantic
```
