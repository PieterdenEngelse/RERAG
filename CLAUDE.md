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

# Full stack (FalkorDB, Redis, Ollama, observability)
docker compose up -d
docker compose --profile core up -d       # Just FalkorDB + Redis
docker compose --profile observability up -d
```


## Architecture

```
[UI / CLI / External]
        │ HTTP REST (port 3010)
        ▼
[Actix backend (backend/src/)] ──► [Tantivy (full-text) + FalkorDB (graph) + SQLite]
        │                                         │
        ├─► [Redis L3 cache]*              [FalkorDB GraphRAG]*
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
| `backend/src/main.rs` | Startup: env parsing, tracing init, Redis/graph feature wiring, background workers |
| `backend/src/api/mod.rs` | Central route registry (upload, search, memory, monitoring, rate-limits) |
| `backend/src/retriever.rs` | Tantivy orchestration, HNSW/PQ builds, cache management |
| `backend/src/chunker.rs` | Configurable chunkers with semantic thresholds and telemetry |
| `backend/src/monitoring/mod.rs` | Prometheus metrics, histogram buckets, OTel exporters, health trackers |
| `backend/tests/rate_limit.rs` | Integration coverage for middleware, proxy trust, bucket refill |
| `frontend/fro/src/app.rs` | Dioxus router, layouts, global signals |
| `frontend/fro/src/api.rs` | Fetch helpers for all backend endpoints |
| `.env.example` | Canonical reference for all 200+ runtime config variables |

## Collaboration Style

- **Confirmation threshold**: Don't ask for confirmation on small or single-file edits — only ask before major or multi-file changes.
- **No speculative pre-builds**: Don't run `cargo build` to check for errors after making changes. Build errors surface when the user restarts the service (`systemctl --user restart ag.service`). Running a build check just wastes the user's time.

## UI Color Rules

- **Minimum readable text on dark tiles**: `text-gray-400` — never use `text-gray-500` or darker for any label or secondary text the user needs to read
- **Preferred for secondary/muted labels**: `text-gray-300`
- **When asked to increase contrast**: shift 2 Tailwind steps toward white (e.g. `text-gray-500` → `text-gray-300`)
- **Links are always blue**: use `text-blue-400 hover:text-blue-300` for all clickable links — never orange, teal, or any other color
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
FALKOR_ENABLED=true
OTEL_TRACES_ENABLED=true
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
OLLAMA_URL=http://localhost:11434
OLLAMA_MODEL=phi:latest
SKIP_INITIAL_INDEXING=false
```

Env vars are the **install-time defaults**. Any of the 27 registered keys
(REDIS_*, CHUNK_*, OTEL_*, FILE_WATCHER_*, INFERENCE_*, BACKEND_PORT,
RUST_LOG, …) can be overridden at runtime from **Config → Runtime**
(`/config/runtime`) without editing the env file.

## Runtime settings layer

There's a second configuration path that sits on top of env vars: runtime
overrides saved to `<base_dir>/overrides.json` (default
`~/.local/share/ag/overrides.json`). Read these via
`crate::settings::effective_*(key, default)` instead of `env::var(key)` when
you want a setting to be tunable from the UI.

- **Precedence**: override (UI) → env (install-time) → hard-coded default.
- **Persistence**: written atomically to one JSON file ag owns; the env
  file is never modified by the running app.
- **Hot vs restart-required**: 13 keys hot-reload in place via dedicated
  subscribers in `main.rs` (REDIS_*, RUST_LOG, CHUNK_*, OTEL_*,
  FILE_WATCHER_*, AUTO_EXPORT_ON_UPLOAD); the rest surface a banner that
  drives `/runtime/actions/restart-self` (universal `execve`-based
  self-restart — works on bin/exe, systemd, or container).
- **Boot-failure recovery**: if a bad override prevents ag from reaching
  healthy, on the next start ag moves it aside as
  `overrides.json.bad-<ts>` and boots with no overrides.

Full reference: `docs/runtime-config.md`. Design rationale and the broader
"what's planned" picture: `persisentenc.md`.

Histogram bucket shapes are tunable without rebuilds: `SEARCH_HISTO_BUCKETS` and `REINDEX_HISTO_BUCKETS` feed `monitoring/histogram_config.rs`.

## Cargo Feature Flags

```toml
default = ["onnx", "io_uring", "graph"]
onnx     # ONNX Runtime (FastEmbed embeddings)
io_uring # tokio-uring async I/O (Linux 5.1+)
graph    # GraphRAG via the falkordb crate
```

## Security Notes

- `TRUST_PROXY` must be `false` unless behind a trusted reverse proxy (controls `X-Forwarded-For` honor)
- `.env`, `agent.db`, and `.env.backup-*` contain credentials — never commit them
- Grafana/Tempo launched via docker-compose use default credentials; secure before exposing beyond localhost
- Rate-limit middleware (`monitoring/rate_limit_middleware.rs`): update env defaults and integration tests when changing search/upload budgets


## Info Modal Voice

Info modals are written from the perspective of the compiled app's end user. They do not know or care about "the frontend" or "the backend" — to them there is only **the app**. Use "the app" or "ag" instead of "the backend", "the frontend", or "the server" when those words would leak implementation structure. Technical terms that are user-visible (env files, config variables, ports) are fine; source-code architecture terms (Actix, Dioxus, HttpServer, handler) are not.

## Toggles (boolean controls)

Two acceptable patterns for boolean controls — pick by placement:

| Placement | Use | Why |
|-----------|-----|-----|
| Inside a `PARAM_COLUMN_CLASS` / `PARAM_BLOCK_CLASS` column on a parameter page (e.g. Repetition Control on `/config/hardware`) | **daisyUI toggle** (recipe below) | Matches the visual rhythm of the surrounding number/select inputs in those columns. |
| Inline next to a panel header, in a tile alongside non-parameter content, or anywhere the daisyUI toggle's `currentColor`-driven sub-elements look tinted | **`.onnx-checkbox`** (`enable_profiling` on `/config/onnx`) | Brand-color fill when checked, fixed gray border when unchecked — no `currentColor` traps, looks identical in every context. |

### `.onnx-checkbox` form (preferred outside parameter columns)

```rust
div { class: "flex items-center gap-2",
    input {
        r#type: "checkbox",
        class: PARAM_CHECKBOX_CLASS,          // "checkbox checkbox-xs onnx-checkbox"
        checked: value,
        onchange: move |evt| /* persist */,
    }
    label { class: PARAM_LABEL_CLASS, "PARAM_NAME" }
    button {
        class: PARAM_ICON_BUTTON_CLASS,
        style: PARAM_ICON_BUTTON_STYLE,
        onclick: move |_| info_signal.set(true),
        title: "What this control does",
        InfoIcon {}
    }
}
```

CSS lives in `assets/styling/index.css` under `.onnx-checkbox` — 2px gray-400
border, brand `#1D6B9A` fill + white checkmark when checked.

**Never use the HTML `disabled` attribute on `.onnx-checkbox` (or daisyUI
toggles).** Browsers fall back to native user-agent rendering for disabled
form controls, which silently overrides the `appearance: none` /
`background-color` / `border` set by our CSS — the control reverts to a small
gray-on-gray box with no border.

**Also avoid `opacity-50` on the wrapper to "gray out" the checkbox.**
Opacity multiplies through to children, so the brand blue and white checkmark
both get dimmed and look "wrong" instead of "disabled". The user usually has
no way to tell the difference between "broken" and "intentionally inactive".

If you genuinely need to gate a checkbox:
- Reject the click in the `onchange` handler.
- Surface the reason in a sibling label, tile, or callout — not by dimming the
  control itself. The Native PDF Extraction Feature-compiled tile is a good
  example.
- If you want a hover hint, put it as a `title=` on the wrapper.

In most cases it's fine to just let the save fire — the override gets stored
and either takes effect on next restart or sits inert until the underlying
capability appears. The save is harmless.

### daisyUI toggle form (for parameter-column placements)

Canonical pattern that originally appeared on `/config/hardware` (Repetition
Control, Memory blocks). Use this when the control sits inside a parameter
column next to number/select inputs.

**Layout:** `flex items-end gap-2 text-gray-200` row containing the toggle
input followed immediately by an info button (matches the `[control] [info]`
rhythm used elsewhere on the parameter pages).

```rust
div { class: "flex items-end gap-2 text-gray-200",
    input {
        r#type: "checkbox",
        class: "toggle toggle-sm !border !border-white",
        style: format!(
            "border: 1px solid white; background-color: {}; --input-color: #fff;",
            if value { "" } else { "#d1d5db" },
        ),
        checked: value,
        onchange: move |_| /* persist */,
    }
    button {
        class: PARAM_ICON_BUTTON_CLASS,
        style: PARAM_ICON_BUTTON_STYLE,
        onclick: move |_| info_signal.set(true),
        title: "What this toggle does",
        InfoIcon {}
    }
}
```

**Notes:**
- daisyUI `toggle toggle-sm` + forced white border via `!border !border-white`.
- Inline `background-color` style: empty string when on (default brand fill),
  `#d1d5db` (Tailwind gray-300) when off — gives the off-state a clear neutral
  look without depending on daisyUI theme variables.
- **`--input-color: #fff` in the inline style is load-bearing**: daisyUI's
  `.toggle` defaults `--input-color` to `color-mix(base-content 50%, transparent)`
  for the off state. That half-transparent color is used for the thumb's
  background and the ring's inset box-shadow, so without the override the
  "white" parts mix with whatever bleeds through and read as tinted /
  yellowish. Forcing it to `#fff` keeps the thumb and ring crisp white in
  both on and off states.
- **`text-gray-200` on the wrapper is load-bearing too**: daisyUI's toggle
  uses `currentColor` for some sub-elements; the canonical hardware-page form
  inherits it from `PARAM_BLOCK_CLASS`. In free-standing placements (e.g.
  inline next to a panel header) set it explicitly on the wrapper.
- When the toggle's underlying capability isn't available (e.g. a Cargo
  feature isn't compiled in), set `disabled: true` on the input and pass a
  `title=` that explains why.
- The info button is mandatory — every toggle gets its own modal explaining
  the trade-off, not just a tooltip. See **Info Buttons** below for the
  button spec; reuse `PARAM_ICON_BUTTON_CLASS` / `PARAM_ICON_BUTTON_STYLE`
  / `InfoIcon` rather than redeclaring.
- When the underlying knob is `restart_required`, the toggle's save handler
  should surface a Restart-now banner (same pattern as `/config/runtime`).
  Don't silently save and leave the user wondering why nothing changed.

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
