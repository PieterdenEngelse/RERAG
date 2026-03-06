# Repository Guidelines

## Command Usage in Qodo

**Treat AGENTS.md as your own operating manual—read and follow it, don't just write it.**

Always tell the user when you have finished a requested task, even if there were no code changes.

Info buttons across the app share the same styling constants: `QUICK_ACTION_INFO_BUTTON_CLASS` (wrapper) + `INFO_ICON_SVG_CLASS` (5×5 white SVG).

- After editing AGENTS.md or reorganizing the repo, run:

```bash
qodo --init
```

This refreshes metadata for Qodo Command.

- When documenting or replying with shell steps, always include the full commands explicitly (no hidden copy buttons).

- **Break difficult tasks into steps.** When a task involves creating or modifying a large file (100+ lines), don't try to write it all at once. Instead:
  1. Create a skeleton first (imports, struct/function signatures, placeholder comments).
  2. Fill in one section at a time with targeted edits.
  3. Verify after each step (`cargo fmt`, quick read-back).
  
  This avoids stalling on large writes and makes each change reviewable.

- **Answer questions before acting.** When the user asks a question (e.g., "do you see X?", "is Y correct?"), answer the question first and wait for instructions. Do not assume intent and make changes unprompted.

## Project Structure & Module Organization
The workspace is defined in `Cargo.toml` and contains `backend/` (Actix Web API, retrieval core, monitoring), `frontend/fro/` (Dioxus Web UI plus Tailwind assets), and `tools/memory_cli/` for operational helpers. Backend modules live under `backend/src/` (`api/`, `memory/`, `monitoring/`, `middleware/`, `retriever.rs`, etc.), while long‑running assets such as Tantivy segments, LanceDB pages, and SQLite files are created under repo‑level folders like `documents/`, `tantivy_index/`, and `db/`. Integration and reliability suites mirror runtime modules inside `backend/tests/`. The frontend keeps UI primitives in `src/components/`, views/pages under `src/pages/monitor/*`, and shared fetch logic in `src/api.rs`.

## Build, Test, and Development Commands

```bash
# Backend dev server with structured logs
cd backend && RUST_LOG=info cargo run

# Backend quality gate
cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings

# Full backend test matrix
cd backend && cargo test --all

# Frontend CSS toolchain
cd frontend/fro && npm install && npm run css:build

# Dioxus live preview
cd frontend/fro && dx serve --platform web
```

## Coding Style & Naming Conventions
- **Indentation**: 4 spaces across Rust, TOML, and Dioxus components (see `backend/src/main.rs`). Tabs are reserved for Makefiles.
- **File naming**: Rust modules use `snake_case` (`monitoring_config.rs`), Dioxus components prefer `UpperCamelCase` files sitting in `src/components/`, and scripts/configs lean on kebab-case (`docker-compose.yml`).
- **Function/variable naming**: Follow Rust defaults (`snake_case` for functions and locals, `SCREAMING_SNAKE_CASE` constants). Frontend hooks and helpers use `camelCase` inside `api.rs` and component logic.
- **Linting**: Always run `cargo fmt` and `cargo clippy --all-targets -- -D warnings`. Frontend code respects `dx fmt` (bundled with the Dioxus CLI) and the Tailwind CLI handles deterministic CSS builds.

## Testing Guidelines
- **Framework**: Native Rust unit/integration tests, many of them Tokio-enabled, under `backend/tests/` (`retriever_tests.rs`, `rate_limit.rs`, `trace_propagation.rs`, etc.).
- **Test files**: Mirror runtime modules; scenario suites live directly under `backend/tests/` with optional subfolders such as `backend/tests/integrations/`.
- **Running tests**: `cd backend && cargo test` for the default set. Append `-- --ignored` when you need the longer observability tests.
- **Coverage**: No enforced threshold, but `docu/TODO.MD` and release notes expect retriever, rate limit, and monitoring smoke tests before tagging.

## Commit & Pull Request Guidelines
- **Commit format**: Favor short, imperative subjects with optional subsystem prefixes. `git log` shows examples like `18-1-6`, `fix header unknown status color`, or `feat: implement 12 agent tools with monitoring dashboard`.
- **PR process**: Reference any configs you touched (`.env.example`, `docu/PLAN-FRO`, dashboards) and include `cargo test` output or frontend screenshots when relevant.
- **Branch naming**: Keep `main` deployable and branch as `feature/<scope>` or `fix/<ticket>` to align with automation and reviewer expectations.

---

# Repository Tour

## 🎯 What This Repository Does

**ag** is a Rust-first Retrieval-Augmented Generation stack that combines an Actix Web backend, Tantivy/LanceDB persistence, and a Dioxus dashboard to ingest documents, build semantic indexes, and monitor agentic workflows.

**Key responsibilities:**
- Ingest and chunk files, persist embeddings to Tantivy plus SQLite/LanceDB metadata stores, and optionally Neo4j.
- Serve search, memory, monitoring, and management APIs consumed by the web UI, CLI tools, or external automations.
- Export traces, metrics, and logs to Prometheus, Tempo, Loki, and Redis-backed caches for observability and rate enforcement.

---

## 🏗️ Architecture Overview

### System Context
```
[Operators / CLI / UI]
        │ HTTP + WebSocket
        ▼
[Actix backend (backend/src)] ──► [Tantivy index + SQLite + LanceDB]
        │                              │
        ├─► [Redis L3 cache]*          │
        └─► [OTel exporters] ──► [Tempo, Prometheus, Grafana, Loki]
(* optional features enabled via env + Cargo features)
```

### Key Components
- **API & routing (`backend/src/api/`)** – Actix scopes for upload, search, memory, rate limits, logs, docker, and monitoring endpoints.
- **Retriever & chunking (`retriever.rs`, `chunker.rs`, `index/`)** – Builds chunkers based on `CHUNKER_MODE`, manages Tantivy writers, LanceDB vectors, semantic caches, and background reindexing.
- **Monitoring stack (`backend/src/monitoring/`)** – Tracing/metrics initializers, histogram tunables, rate-limit middleware, chunking telemetry, and Docker health collectors.
- **Memory & agent tools (`backend/src/memory/`, `tools/*`)** – SQLite-backed agent memories, GraphRAG hooks (Neo4j feature), and CLI tooling for maintenance.
- **Frontend (`frontend/fro/src/`)** – Dioxus router (`app.rs`), monitoring pages (requests, cache, rate-limits, index dashboards), and Tailwind-driven styling with DaisyUI.

### Data Flow
1. Documents arrive via `/upload` or the file watcher and are chunked according to `CHUNKER_MODE` + semantic threshold.
2. `Retriever` persists chunks to Tantivy/LanceDB, records statistics, and warms caches (in-process L1/L2 plus optional Redis L3).
3. Search/memory requests hit Actix handlers; rate-limit middleware enforces token buckets before calls reach retrieval or agent layers.
4. Responses include references and metrics. Tracing spans, Prometheus samples, and logs are emitted through OpenTelemetry exporters toward Tempo/Grafana/Loki.

---

## 📁 Project Structure [Partial Directory Tree]

```
ag/
├── Cargo.toml                 # Workspace (backend, frontend/fro, tools)
├── backend/
│   ├── Cargo.toml             # Actix + retrieval crate
│   ├── src/
│   │   ├── api/               # HTTP handlers, reindex logic, monitoring routes
│   │   ├── monitoring/        # Metrics, tracing, rate-limit middleware, chunking stats
│   │   ├── memory/            # Agent memory + GraphRAG support
│   │   ├── middleware/        # Request guards, auth/rate limiting
│   │   └── retriever.rs       # Tantivy/LanceDB orchestration & caches
│   └── tests/                 # Integration/observability suites
├── frontend/
│   └── fro/
│       ├── package.json       # Tailwind + DaisyUI scripts
│       └── src/
│           ├── app.rs         # Dioxus router & layouts
│           ├── api.rs         # Fetch helpers (upload, monitoring, config)
│           ├── components/    # Shared UI elements (header, panels, toasts)
│           └── pages/monitor/ # Requests, cache, index, rate-limit dashboards
├── tools/memory_cli/          # CLI helpers for agent memory maintenance
├── docs/ & docu/              # Operational plans (`docu/PLAN-FRO`, TODOs)
├── scripts/                   # TLS, tracing, observability bootstrap scripts
├── docker-compose.*.yml       # Full-stack & observability compositions
└── prometheus/, grafana-*.json # Metrics & dashboard manifests
```

### Key Files to Know

| File | Purpose | When You'd Touch It |
|------|---------|---------------------|
| `backend/src/main.rs` | Bootstraps env parsing, tracing, Redis/Neo4j features, background indexing, and Actix server startup. | Adjust startup order, add env wiring, or new background workers. |
| `backend/src/api/mod.rs` | Central registry for upload/search/memory routes, trace/rate-limit endpoints, and monitoring helpers. | Adding HTTP APIs, toggling rate limits, wiring new telemetry endpoints. |
| `backend/src/retriever.rs` | Search and indexing engine (Tantivy writers, caches, HNSW/PQ builds). | Tuning retrieval performance, adding chunker modes, exposing metrics. |
| `backend/src/chunker.rs` | Configurable chunkers with semantic thresholds and stats logging. | Experimenting with chunk sizes/modes or surfacing telemetry. |
| `backend/src/monitoring/mod.rs` | Metrics registry, histogram buckets, OpenTelemetry exporters, health trackers. | Adding Prometheus series or OTLP exporters. |
| `backend/tests/rate_limit.rs` | Integration coverage for middleware, proxy trust, and bucket refill logic. | Validating rate-limit changes. |
| `frontend/fro/src/app.rs` | Dioxus router and layout; wires monitoring pages and global signals. | Adding new routes, overlays, or global state providers. |
| `frontend/fro/src/pages/monitor/index_page.rs` | Index dashboard wiring (reindex controls, storage cards). | Surfacing new backend stats or UX tweaks for indexing. |
| `frontend/fro/src/api.rs` | Fetch helpers covering upload, monitoring, configuration, and file picker logic. | Extending API bindings or adjusting error handling. |
| `.env.example` | Canonical runtime configuration (ports, Redis/Neo4j toggles, OTEL endpoints, chunker mode, histogram buckets). | Onboarding, documenting new env vars, or rotating defaults. |

---

## 🔧 Technology Stack

### Core Technologies
- **Language:** Rust 1.75+ (Edition 2021) for backend correctness, async (Tokio 1.47.1), and shared types.
- **Web Framework:** Actix Web 4.11.0 with actix-cors/multipart/service for HTTP, upload, and middleware ergonomics.
- **Retrieval & Storage:** Tantivy 0.24.2 for vector search, LanceDB/SQLite via `rusqlite` for metadata, optional Neo4j (via `neo4rs`) for GraphRAG, Redis 0.32 as L3 cache.
- **Frontend:** Dioxus 0.6 rendered to web via `dx serve`, styled by Tailwind CLI 4.1.14 + DaisyUI 5.5.5.
- **Observability:** `tracing` + OpenTelemetry 0.21, Prometheus 0.13 exporters, Grafana/Tempo/Loki stacks orchestrated by `docker-compose*.yml`.

### Key Libraries
- `llm 1.3.4` for model execution hooks (Ollama/ONNX bridging).
- `fastembed 5.8.1` (non-Windows) and `instant-distance 0.6` for ANN acceleration.
- `dashmap`, `lz4_flex`, `wide`, `bloomfilter`, and `tokio-uring` for high-throughput chunking, caching, and file IO.

### Development Tools
- Cargo workspaces, `cargo fmt`, and `cargo clippy` for Rust QA.
- Dioxus CLI (`dx serve`, `dx fmt`) for frontend hot reload.
- Tailwind CLI for CSS builds; shell scripts like `complete-tracing-setup.sh` automate observability bring-up.

---

## 🌐 External Dependencies

### Required Services
- **Ollama / LLM backend** – Provides embedding and chat models referenced by backend LLM settings.
- **Prometheus + Grafana + Tempo** – Defined in `docker-compose.yml` and `prometheus/` with dashboards under `grafana-*.json`; scrape `/monitoring/metrics` and ingest OTLP traces.
- **Redis (optional)** – Toggled via `REDIS_ENABLED` env vars to host the L3 cache for retrieval workloads.
- **Neo4j (optional)** – Enabled via Cargo `neo4j` feature and envs to store graph relationships for GraphRAG.

### Environment Variables

```bash
# Core server
BACKEND_HOST=127.0.0.1
BACKEND_PORT=3010
RUST_LOG=info
SKIP_INITIAL_INDEXING=false

# Chunking & retrieval
CHUNKER_MODE=fixed|lightweight|semantic
SEMANTIC_SIMILARITY_THRESHOLD=0.82
SEARCH_HISTO_BUCKETS=1,2,5,10,20,50,100,250,500,1000
REINDEX_HISTO_BUCKETS=50,100,250,500,1000,2000,5000

# Caching / storage
REDIS_ENABLED=true
REDIS_URL=redis://127.0.0.1:6379/
NEO4J_ENABLED=true

# Observability
OTEL_TRACES_ENABLED=true
OTEL_OTLP_EXPORT=true
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
TEMPO_ENABLED=true

# LLM routing
OLLAMA_URL=http://localhost:11434
OLLAMA_MODEL=phi:latest
OLLAMA_EMBEDDING_MODEL=nomic-embed-text
```

---

## 🔄 Common Workflows

### Document ingestion & indexing
1. Upload via `curl -F "file=@docs/sample.txt" http://127.0.0.1:3010/upload` or drop files into `documents/` for the watcher.
2. Watch `/monitoring/chunking/latest` or `/monitoring/metrics` for chunk stats and progress; disable auto-indexing with `SKIP_INITIAL_INDEXING=true` when needed.
3. Trigger manual indexing with `curl -X POST http://127.0.0.1:3010/reindex` (sync) or `/reindex/async` (background) and monitor job IDs via `/reindex/status/{job}`.

### Semantic search & agent memory
1. Query `GET /search?q=<term>` for vector/BM25 fusion results.
2. Store or retrieve episodic memory through `/memory/store_rag` and `/memory/search_rag` endpoints exposed in `api.rs`.
3. Frontend monitoring pages (`/monitor/requests`, `/monitor/memories`) call the same APIs via `frontend/fro/src/api.rs` helpers.

### Observability bootstrap
1. Start infra: `docker compose up -d`.
2. Run `./complete-tracing-setup.sh` or `./setup-tempo-tls.sh` to align certificates and OTLP endpoints.
3. Import dashboards from `grafana-*.json` and ensure Prometheus scrapes `http://<host>:3010/monitoring/metrics` successfully.

---

## 📈 Performance & Scale
- **Chunker flexibility**: `CHUNKER_MODE` selects fixed, lightweight, or semantic chunkers; stats are captured via `/monitoring/chunking/latest` for tuning thresholds.
- **Caching tiers**: In-process caches plus optional Redis keep Tantivy hits down; instrumentation lives in `monitoring/rate_limit_middleware.rs` and cache snapshots are returned from `/monitor/cache/info`.
- **Background workers**: Startup spawns indexing, file watchers, trace alerting, and resource attribution tasks to keep request threads lean.
- **Histograms**: `SEARCH_HISTO_BUCKETS` and `REINDEX_HISTO_BUCKETS` env vars feed `monitoring/histogram_config.rs`, enabling custom latency buckets without rebuilds.

### Monitoring
- Prometheus scrapes `/monitoring/metrics`; Grafana dashboards (trace alerting, multi-source logs, request health) live alongside JSON manifests in repo root.
- Tempo receives OTLP traces from the backend when `OTEL_TRACES_ENABLED=true`; Loki + Vector configs live under `vector_*.toml` for log shipping.
- Docker observability endpoints (`/monitoring/docker`) run shell commands to inspect containers and stats, with warnings captured in tracing logs.

---

## 🚨 Things to Be Careful About

### 🔒 Security Considerations
- **Rate limiting**: `monitoring/rate_limit_middleware.rs` enforces per-IP token buckets; always update env defaults and integration tests when changing search/upload budgets.
- **Proxy trust**: `TRUST_PROXY` controls whether `X-Forwarded-For` headers are honored—leave false unless you sit behind a trusted reverse proxy.
- **Secrets & data**: `.env`, `agent.db`, and `.env.backup-*` files contain credentials; never commit them. TLS scripts such as `setup-prometheus-tls.sh` modify system stores—review before running.
- **Observability endpoints**: Grafana/Tempo instances launched via docker-compose ship with default credentials; secure them before exposing beyond localhost.

*Last updated: 2026-01-18*
