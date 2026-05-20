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
