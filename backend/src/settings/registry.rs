//! Known-keys registry — minimal metadata for UI discoverability.
//!
//! Each entry is one line of code: key, description, kind, default, category,
//! and a restart-required flag. This is the minimal form of the typed
//! registry described in the persistence design doc — just enough metadata
//! for the UI to render a sensible control. Parsers and validators live in
//! `Kind`.
//!
//! Filling out all 182 known keys is mechanical work and can be done
//! incrementally. This starter set covers the UI-relevant keys that ag
//! exposes today.

use super::kind::Kind;

#[derive(Debug, Clone)]
pub struct KnownKey {
    pub key: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    pub default: Option<&'static str>,
    pub category: &'static str,
    pub restart_required: bool,
}

pub static KNOWN_KEYS: &[KnownKey] = &[
    // ── Cache (L3) ─────────────────────────────────────────────────────────
    KnownKey {
        key: "REDIS_ENABLED",
        description: "Enable the persistent L3 cache (Redis-backed search-result cache). Hot-swapped in-process by the retriever; no restart needed.",
        kind: Kind::Bool,
        default: Some("false"),
        category: "cache",
        restart_required: false,
    },
    KnownKey {
        key: "REDIS_URL",
        description: "Connection URL for the L3 Redis cache. Hot-swapped in-process.",
        kind: Kind::Url,
        default: Some("redis://127.0.0.1:6379/"),
        category: "cache",
        restart_required: false,
    },
    KnownKey {
        key: "REDIS_TTL",
        description: "Default TTL for L3 cache entries, in seconds.",
        kind: Kind::U64,
        default: Some("3600"),
        category: "cache",
        restart_required: false,
    },
    // ── Chunker ────────────────────────────────────────────────────────────
    KnownKey {
        key: "CHUNKER_MODE",
        description: "Chunking strategy: fixed, lightweight, or semantic. Hot-reloaded in-process. Precedence: a value saved on the Chunker config page (DB) wins; otherwise this override applies, both live and across restarts.",
        kind: Kind::Enum(&["fixed", "lightweight", "semantic"]),
        default: Some("fixed"),
        category: "chunker",
        restart_required: false,
    },
    KnownKey {
        key: "CHUNK_TARGET_SIZE",
        description: "Target chunk size in tokens. Hot-reloaded in-process. Precedence: a value saved on the Chunker config page (DB) wins; otherwise this override applies, both live and across restarts.",
        kind: Kind::U64,
        default: Some("256"),
        category: "chunker",
        restart_required: false,
    },
    KnownKey {
        key: "CHUNK_MAX_SIZE",
        description: "Maximum chunk size in tokens. Hot-reloaded in-process. Precedence: a value saved on the Chunker config page (DB) wins; otherwise this override applies, both live and across restarts.",
        kind: Kind::U64,
        default: Some("384"),
        category: "chunker",
        restart_required: false,
    },
    KnownKey {
        key: "CHUNK_OVERLAP",
        description: "Token overlap between adjacent chunks. Hot-reloaded in-process. Precedence: a value saved on the Chunker config page (DB) wins; otherwise this override applies, both live and across restarts.",
        kind: Kind::U64,
        default: Some("32"),
        category: "chunker",
        restart_required: false,
    },
    // ── Graph (FalkorDB) ───────────────────────────────────────────────────
    KnownKey {
        key: "FALKOR_ENABLED",
        description: "Enable the FalkorDB knowledge graph store.",
        kind: Kind::Bool,
        default: Some("false"),
        category: "graph",
        restart_required: true,
    },
    KnownKey {
        key: "FALKOR_URI",
        description: "Connection URI for FalkorDB (Redis-protocol).",
        kind: Kind::Url,
        default: Some("redis://localhost:6380"),
        category: "graph",
        restart_required: true,
    },
    KnownKey {
        key: "GRAPH_EXPANSION_ENABLED",
        description: "Expand search results with related graph entities.",
        kind: Kind::Bool,
        default: Some("true"),
        category: "graph",
        restart_required: true,
    },
    KnownKey {
        key: "GRAPH_EXPANSION_MAX_HOPS",
        description: "Maximum hops to traverse during graph expansion.",
        kind: Kind::U64,
        default: Some("2"),
        category: "graph",
        restart_required: true,
    },
    // ── Embedder ───────────────────────────────────────────────────────────
    KnownKey {
        key: "EMBEDDING_MODEL",
        description: "Embedding model identifier (FastEmbed model name).",
        kind: Kind::String,
        default: Some("bge-small-en-v1.5"),
        category: "embedder",
        restart_required: true,
    },
    KnownKey {
        key: "EMBEDDING_BATCH_SIZE",
        description: "Batch size for embedding generation.",
        kind: Kind::U64,
        default: Some("32"),
        category: "embedder",
        restart_required: true,
    },
    KnownKey {
        key: "EMBEDDING_CACHE_SIZE",
        description: "In-process embedding cache capacity (entries).",
        kind: Kind::U64,
        default: Some("10000"),
        category: "embedder",
        restart_required: true,
    },
    // ── Embedder · ort runtime knobs ───────────────────────────────────────
    // These are NOT ONNX-file settings — the .onnx file says what to compute,
    // not how. These knobs configure ONNX Runtime (Microsoft's C++ engine)
    // via the `ort` Rust crate. See /docu/index/onnx for the three-layer
    // explainer.
    KnownKey {
        key: "ONNX_ALLOW_SIMPLE_TOKENIZER",
        description: "Fall back to a hash-based tokenizer if tokenizer.json is missing next to the ONNX model. Default false: ag refuses to start with a missing tokenizer rather than silently producing embeddings that do not match the model's training. Flip to true only for experiments or smoke tests where you accept that recall will degrade silently.",
        kind: Kind::Bool,
        default: Some("false"),
        category: "embedder-ort",
        restart_required: true,
    },
    KnownKey {
        key: "ONNX_NUM_THREADS",
        description: "Intra-op thread count — how many threads ONNX Runtime uses inside a single operator (e.g. one MatMul). Pin to the host's physical core count; oversubscribing hurts. Default 4 is a conservative cross-platform choice; bump for fat CPU hosts, lower for shared/serverless boxes.",
        kind: Kind::U64,
        default: Some("4"),
        category: "embedder-ort",
        restart_required: true,
    },
    KnownKey {
        key: "ONNX_INTER_OP_NUM_THREADS",
        description: "Inter-op thread count — how many operators can run in parallel. Only matters when execution_mode is Parallel (ag uses Sequential by default), so leaving this at 1 is correct for most embedding workloads. Increasing it without flipping execution_mode wastes threads.",
        kind: Kind::U64,
        default: Some("1"),
        category: "embedder-ort",
        restart_required: true,
    },
    KnownKey {
        key: "ONNX_OPTIMIZATION_LEVEL",
        description: "Graph optimization level: disable / basic / extended / all. 'all' enables constant folding, op fusion, layout transforms — fastest steady-state inference but slower first-load. Drop to 'extended' if cold-start time matters, or 'disable' when chasing a correctness bug where you want the graph to run exactly as-exported. Default: all.",
        kind: Kind::Enum(&["disable", "basic", "extended", "all"]),
        default: Some("all"),
        category: "embedder-ort",
        restart_required: true,
    },
    KnownKey {
        key: "ONNX_ENABLE_MEM_PATTERN",
        description: "Enable memory-pattern optimization. ONNX Runtime pre-plans tensor allocations based on the first inference, then reuses that plan for subsequent calls. Big throughput win for fixed-shape workloads (like fixed-length embedding). First request after start is slower because the plan is being built. Flip off for short-lived processes or wildly variable input shapes.",
        kind: Kind::Bool,
        default: Some("true"),
        category: "embedder-ort",
        restart_required: true,
    },
    KnownKey {
        key: "ONNX_NORMALIZE_OUTPUT",
        description: "L2-normalize every output vector to unit length before returning. Required for HNSW DotProduct == cosine similarity, which is what BGE/MiniLM-style retrievers expect. Turn off only when diagnosing recall regressions; leaving this off in production silently breaks search ranking.",
        kind: Kind::Bool,
        default: Some("true"),
        category: "embedder-ort",
        restart_required: true,
    },
    KnownKey {
        key: "ONNX_POOLING",
        description: "How to collapse a [batch, seq, hidden] model output to one vector per input: 'cls' takes the first token (fast, correct for BERT classifiers, often the wrong choice for sentence embeddings); 'mean' averages over the unmasked positions (standard for sentence-transformers and BGE). The single biggest correctness lever — match this to what the model was trained with. Default cls to preserve historical behavior; flip to mean if your retrieval quality is suspect.",
        kind: Kind::Enum(&["cls", "mean"]),
        default: Some("cls"),
        category: "embedder-ort",
        restart_required: true,
    },
    // ── PDF (Native Extraction Pipeline) ───────────────────────────────────
    // Stage 0 of the upload pipeline: lopdf + heuristic/ORT classifier +
    // TableFormer + DocBlock assembly. Output feeds the Text Ingestion
    // Pipeline (/monitor/tip). The Cargo feature `layout_ml` must also be
    // compiled in for these to do anything.
    KnownKey {
        key: "LAYOUT_ML_ENABLED",
        description: "Default for new corpora: turn on the Native PDF Extraction pipeline (Stage 0 — lopdf word bboxes, layout classification, table detection, DocIR assembly). Without it, PDFs go through the plain pdftotext cascade and arrive at the chunker without block-type tags. Each corpus can override this on /config/corpus (no restart needed). Requires the layout_ml Cargo feature in the binary; check the Feature compiled tile on /config/onnx.",
        kind: Kind::Bool,
        default: Some("false"),
        category: "pdf",
        restart_required: true,
    },
    KnownKey {
        key: "LAYOUT_ML_MODEL_ID",
        description: "HuggingFace Hub spec for a DETR-style image-based layout model (Tier 0). Format: 'owner/repo' (defaults to model.onnx inside the repo) or 'owner/repo:custom-filename.onnx'. On first use ag downloads the file via hf-hub into ~/.cache/huggingface/hub/ and reuses it on subsequent boots; no network call once cached. This is the 'just works' path — preferred over LAYOUT_DETR_MODEL_PATH unless you need to pin to a local file. Tier 0 expects a DETR-style ONNX with 'pixel_values' input; pointing it at a word-feature checkpoint will fail at classify time. On download failure or load error, ag warns and falls through to the local-path tiers.",
        kind: Kind::String,
        default: None,
        category: "pdf",
        restart_required: true,
    },
    KnownKey {
        key: "LAYOUT_DETR_MODEL_PATH",
        description: "Local filesystem path to a DETR-style image-based PubLayNet layout model (Tier 1 — used when LAYOUT_ML_MODEL_ID isn't set or its download/load failed). Place the file on disk yourself (huggingface-cli download, wget, etc.) and point this at it. Leave blank to skip Tier 1 and fall through to Tier 2 / heuristic.",
        kind: Kind::String,
        default: None,
        category: "pdf",
        restart_required: true,
    },
    KnownKey {
        key: "LAYOUT_ORT_MODEL_PATH",
        description: "Local filesystem path to a word-feature ONNX layout classifier (Tier 2 — used if Tier 0 and Tier 1 aren't configured or fail to load). File must be pre-downloaded; this tier has no auto-download path (LAYOUT_ML_MODEL_ID is DETR-only). Leave blank to fall through to the pure-Rust heuristic classifier.",
        kind: Kind::String,
        default: None,
        category: "pdf",
        restart_required: true,
    },
    KnownKey {
        key: "LAYOUT_DETR_THRESHOLD",
        description: "Confidence cutoff for accepting a DETR region prediction (0.0-1.0). Lower values keep more low-confidence boxes (better recall, more noise); higher values keep only confident detections (better precision, may drop true regions). Applies to whichever DETR model is active — Tier 0 (LAYOUT_ML_MODEL_ID) or Tier 1 (LAYOUT_DETR_MODEL_PATH). No effect on Tier 2 / heuristic.",
        kind: Kind::F64,
        default: Some("0.7"),
        category: "pdf",
        restart_required: true,
    },
    KnownKey {
        key: "POINTERRAG_AUTO_GAP_THRESHOLD",
        description: "Auto-mode routing knob: when within-doc fragmentation (section_ratio - doc_ratio) is at least this value, Auto routes the query to PointerRag (full-section hydration) instead of Strict/Hybrid. 0.0 = always Pointer; 1.0 = never. Default 0.5 splits the bimodal corpus distribution from the pp4 analysis. Hot-reloaded; no restart needed.",
        kind: Kind::F64,
        default: Some("0.5"),
        category: "agent",
        restart_required: false,
    },
    KnownKey {
        key: "LAYOUT_DETR_NUM_CLASSES",
        description: "Number of layout classes the active DETR model predicts (background is added automatically — set this to the count of real classes). 11 is DocLayNet's canonical order (Caption, Footnote, Formula, List-item, Page-footer, Page-header, Picture, Section-header, Table, Text, Title) and works for any DocLayNet-trained DETR — both cmarkea/detr-layout-detection and neka-nat/rfdetr-doclaynet-onnx use it. Drop to 5 for a PubLayNet model (Text, Title, List, Figure, Table). Wrong count makes region tagging output garbage classes.",
        kind: Kind::U64,
        default: Some("11"),
        category: "pdf",
        restart_required: true,
    },
    KnownKey {
        key: "TABLE_FORMER_MODEL_PATH",
        description: "Local filesystem path to the TableFormer table-structure ONNX model (Stage 3 of the Native PDF pipeline). Download from huggingface.co/microsoft/table-transformer-structure-recognition manually and point this at the file. Leave blank to fall back to text-mode table clustering.",
        kind: Kind::String,
        default: None,
        category: "pdf",
        restart_required: true,
    },
    KnownKey {
        key: "PDF_RELATIONAL_ENABLED",
        description: "Default for new corpora: persist the relational PDF sidecar tables (pdf_lines, pdf_pages) and make the column-aware chunker treat cross-column transitions as strong boundaries. Lets ag answer questions about two-column documents (invoices, articles) without confusing left- and right-column content. Each corpus can override this on /config/corpus. Independent of LAYOUT_ML_ENABLED at the gate level, but the extractor itself requires the layout_ml Cargo feature — without it this setting is a no-op. No restart needed; takes effect on next document extraction.",
        kind: Kind::Bool,
        default: Some("false"),
        category: "pdf",
        restart_required: false,
    },
    // ── Observability ──────────────────────────────────────────────────────
    KnownKey {
        key: "OTEL_TRACES_ENABLED",
        description: "Export traces via OTLP. Hot-reloaded — the tracer provider is rebuilt and re-installed in place.",
        kind: Kind::Bool,
        default: Some("false"),
        category: "observability",
        restart_required: false,
    },
    KnownKey {
        key: "OTEL_EXPORTER_OTLP_ENDPOINT",
        description: "OTLP exporter endpoint (gRPC). Hot-reloaded — the current provider shuts down and a new one targeting this endpoint is installed.",
        kind: Kind::Url,
        default: Some("http://localhost:4318"),
        category: "observability",
        restart_required: false,
    },
    KnownKey {
        key: "RUST_LOG",
        description: "Tracing filter directive (e.g. info, debug, ag=trace). Hot-reloaded in place via tracing_subscriber::reload.",
        kind: Kind::String,
        default: Some("info"),
        category: "observability",
        restart_required: false,
    },
    KnownKey {
        key: "CHUNKING_SNAPSHOT_LOGGING",
        description: "Log per-chunk snapshots during ingestion.",
        kind: Kind::Bool,
        default: Some("true"),
        category: "observability",
        restart_required: true,
    },
    // ── Inference ──────────────────────────────────────────────────────────
    KnownKey {
        key: "INFERENCE_MAX_CONCURRENT_EMBEDDINGS",
        description: "Max concurrent embedding inferences.",
        kind: Kind::U64,
        default: Some("4"),
        category: "inference",
        restart_required: true,
    },
    KnownKey {
        key: "INFERENCE_MAX_CONCURRENT_LLM",
        description: "Max concurrent LLM inferences.",
        kind: Kind::U64,
        default: Some("2"),
        category: "inference",
        restart_required: true,
    },
    KnownKey {
        key: "INFERENCE_ACQUIRE_TIMEOUT_MS",
        description: "Timeout (ms) to acquire an inference slot before failing.",
        kind: Kind::U64,
        default: Some("30000"),
        category: "inference",
        restart_required: true,
    },
    // ── File watcher ──────────────────────────────────────────────────────
    KnownKey {
        key: "FILE_WATCHER_ENABLED",
        description: "Watch the upload directory for new files. Hot-reloaded — all registered watchers are aborted/respawned in place.",
        kind: Kind::Bool,
        default: Some("true"),
        category: "ingest",
        restart_required: false,
    },
    KnownKey {
        key: "FILE_WATCHER_DEBOUNCE_MS",
        description: "Debounce window for filesystem events (ms). Hot-reloaded — watchers are respawned with the new debounce.",
        kind: Kind::U64,
        default: Some("500"),
        category: "ingest",
        restart_required: false,
    },
    KnownKey {
        key: "FILE_WATCHER_DIR",
        description: "Absolute path of the directory the default corpus' watcher monitors. Empty = fall back to the PathManager-derived default (~/.local/share/ag/data/corpora/default/documents/). Per-corpus watch_dir overrides still apply to non-default corpora.",
        kind: Kind::Path,
        default: None,
        category: "ingest",
        restart_required: true,
    },
    KnownKey {
        key: "AUTO_EXPORT_ON_UPLOAD",
        description: "Automatically export the corpus after each upload.",
        kind: Kind::Bool,
        default: Some("true"),
        category: "ingest",
        restart_required: false,
    },
    // ── Search ─────────────────────────────────────────────────────────────
    KnownKey {
        key: "SEARCH_TOP_K",
        description: "Default number of search results to return.",
        kind: Kind::U64,
        default: Some("10"),
        category: "search",
        restart_required: true,
    },
    // ── Network ────────────────────────────────────────────────────────────
    KnownKey {
        key: "BACKEND_PORT",
        description: "TCP port the API server binds to.",
        kind: Kind::U64,
        default: Some("3010"),
        category: "network",
        restart_required: true,
    },
    KnownKey {
        key: "TRUST_PROXY",
        description: "Honor X-Forwarded-For headers (only enable behind a trusted reverse proxy).",
        kind: Kind::Bool,
        default: Some("false"),
        category: "network",
        restart_required: true,
    },
];

pub fn lookup(key: &str) -> Option<&'static KnownKey> {
    KNOWN_KEYS.iter().find(|k| k.key == key)
}
