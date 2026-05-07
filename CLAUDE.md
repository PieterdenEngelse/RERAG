# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Repository Does

**ag** is a Rust-first Retrieval-Augmented Generation (RAG) stack: an Actix Web backend handles document ingestion, semantic search, agent memory, and observability APIs; a Dioxus frontend provides a monitoring dashboard and chat UI.

**ag is also a learning platform.** The app is intentionally designed to teach the user how agentic RAG systems work — how agents use tools, how retrieval is layered with LLM generation, how memory and episodes are tracked, and how observability surfaces what the system is doing internally. UI decisions (monitoring dashboards, info modals, visible stats) should support this educational goal: make the invisible visible.

## Build, Test, and Development Commands

```bash
# Backend dev server
cd backend && RUST_LOG=info cargo run

# Quality gate (must pass before committing)
cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings

# Full test matrix
cd backend && cargo test --all

# Longer observability tests (usually skipped)
cd backend && cargo test -- --ignored

# Frontend CSS build
cd frontend/fro && npm install && npm run css:build

# Frontend live preview
cd frontend/fro && dx serve --platform web

# Full stack (Neo4j, Redis, Ollama, observability)
docker compose up -d
docker compose --profile core up -d       # Just Neo4j + Redis
docker compose --profile observability up -d
```


## Architecture

```
[UI / CLI / External]
        │ HTTP REST (port 3010)
        ▼
[Actix backend (backend/src/)] ──► [Tantivy (full-text) + Neo4jDB (vector) + SQLite]
        │                                         │
        ├─► [Redis L3 cache]*              [Neo4j GraphRAG]*
        └─► [OTel exporters] ──► [Tempo / Prometheus / Grafana / Loki]
```

### Data Flow
1. Documents arrive via `/upload` or file watcher → chunked by `CHUNKER_MODE`
2. `Retriever` persists chunks to Tantivy, warms L1/L2 in-process caches + optional Redis L3
3. Search requests hit Actix handlers; rate-limit middleware (token buckets) enforces quotas
4. Responses emit OTel spans, Prometheus samples, and structured logs

### Agent Modes (`backend/src/agent.rs`)
- `Rag` – document retrieval only
- `Llm` – LLM only, no retrieval
- `Hybrid` – search + LLM fallback (default)
- `RagStrict` – grounded answers only
- `Agentic` – Rig framework tool-calling loop

### Caching Tiers
- L1: in-process LRU (Rust)
- L2: DashMap (concurrent, in-process)
- L3: Redis (optional, toggled by `REDIS_ENABLED`)

## Key Files

| File | Purpose |
|------|---------|
| `backend/src/main.rs` | Startup: env parsing, tracing init, Redis/Neo4j feature wiring, background workers |
| `backend/src/api/mod.rs` | Central route registry (upload, search, memory, monitoring, rate-limits) |
| `backend/src/retriever.rs` | Tantivy/LanceDB orchestration, HNSW/PQ builds, cache management |
| `backend/src/chunker.rs` | Configurable chunkers with semantic thresholds and telemetry |
| `backend/src/monitoring/mod.rs` | Prometheus metrics, histogram buckets, OTel exporters, health trackers |
| `backend/tests/rate_limit.rs` | Integration coverage for middleware, proxy trust, bucket refill |
| `frontend/fro/src/app.rs` | Dioxus router, layouts, global signals |
| `frontend/fro/src/api.rs` | Fetch helpers for all backend endpoints |
| `.env.example` | Canonical reference for all 200+ runtime config variables |

## Collaboration Style

- **Confirmation threshold**: Don't ask for confirmation on small or single-file edits — only ask before major or multi-file changes.

## UI Color Rules

- **Minimum readable text on dark tiles**: `text-gray-400` — never use `text-gray-500` or darker for any label or secondary text the user needs to read
- **Preferred for secondary/muted labels**: `text-gray-300`
- **When asked to increase contrast**: shift 2 Tailwind steps toward white (e.g. `text-gray-500` → `text-gray-300`)
- These rules apply to all Dioxus components and pages without exception

## Coding Conventions

- **Indentation**: 4 spaces everywhere; tabs only in Makefiles
- **Rust naming**: `snake_case` modules/functions/variables, `SCREAMING_SNAKE_CASE` constants, `UpperCamelCase` types
- **Dioxus components**: `UpperCamelCase` files in `src/components/`; pages in `src/pages/`
- **Linting**: `cargo fmt` + `cargo clippy --all-targets -- -D warnings` must pass
- **Shared UI constants**: Info buttons use `QUICK_ACTION_INFO_BUTTON_CLASS` (wrapper) + `INFO_ICON_SVG_CLASS` (5×5 white SVG)

## Key Environment Variables

```bash
BACKEND_HOST=127.0.0.1
BACKEND_PORT=3010
RUST_LOG=info
CHUNKER_MODE=fixed|lightweight|semantic
REDIS_ENABLED=true
NEO4J_ENABLED=true
OTEL_TRACES_ENABLED=true
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
OLLAMA_URL=http://localhost:11434
OLLAMA_MODEL=phi:latest
SKIP_INITIAL_INDEXING=false
```

Histogram bucket shapes are tunable without rebuilds: `SEARCH_HISTO_BUCKETS` and `REINDEX_HISTO_BUCKETS` feed `monitoring/histogram_config.rs`.

## Cargo Feature Flags

```toml
default = ["onnx", "io_uring", "neo4j"]
onnx     # ONNX Runtime (FastEmbed embeddings)
io_uring # tokio-uring async I/O (Linux 5.1+)
neo4j    # GraphRAG via neo4rs + deadpool
```

## Security Notes

- `TRUST_PROXY` must be `false` unless behind a trusted reverse proxy (controls `X-Forwarded-For` honor)
- `.env`, `agent.db`, and `.env.backup-*` contain credentials — never commit them
- Grafana/Tempo launched via docker-compose use default credentials; secure before exposing beyond localhost
- Rate-limit middleware (`monitoring/rate_limit_middleware.rs`): update env defaults and integration tests when changing search/upload budgets


## Info Buttons

Here's your info button canonical spec from memory:

**Element:** `btn class=PARAM_ICON_BUTTON_CLASS style=PARAM_ICON_BUTTON_STYLE`

**Constants:**
- `PARAM_ICON_BUTTON_CLASS` = `"w-6 h-6 min-w-6 min-h-6 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80"`
- `PARAM_ICON_BUTTON_STYLE` = `"background-color:#7C2A02;border:1px solid #7C2A02;"` (Rust brand color)
- `INFO_ICON_SVG_CLASS` = `"w-5 h-5 text-white"`

**SVG spec:**
- `stroke=currentColor stroke_width=1.5` on the SVG element (inherited)
- Circle: `r=9` (no explicit stroke-width)
- Line: `y1=8 y2=14 stroke_width=1.5`
- Dot: `cy=6.3 r=1 fill=currentColor stroke=none`

**Wrapper pattern:**
- `div flex items-center gap-2 mb-3` containing `h3 + btn`
- `use_signal` toggle for show/hide
- Import from `crate::pages::hardware::constants`

## Frontend Dev Server

**Never start `dx serve` via the Bash tool.** The process ends up with its stdout/stderr piped to a deleted temp file — output disappears, hot reload is invisible to the user, and there is no way to interact with it.

The user must run it themselves in their own terminal:
```bash
cd /home/pde/ag/frontend/fro
dx serve --package fro --platform web --port 1789
```
