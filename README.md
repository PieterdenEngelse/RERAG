# ag

A Rust-first Retrieval-Augmented Generation stack with a built-in chat
interface, observability dashboard, and tool-calling agent mode. Backend is an
Actix Web server (port 3010 by default); the frontend is a Dioxus desktop /
web UI served from the same port.

> **Status:** pre-1.0. The first functional public release is
> [`v0.2.5`](https://github.com/PieterdenEngelse/RARAG/releases/tag/v0.2.5).
> APIs, configuration knobs, and storage layouts may still change.

## Install

The supported install path is the GUI installer, distributed as an AppImage.

```bash
# Download the AppImage from the latest release
curl -L -o ag-installer.AppImage \
  https://github.com/PieterdenEngelse/RARAG/releases/latest/download/ag-installer-x86_64.AppImage
chmod +x ag-installer.AppImage

# Run it (single-window installer, no root required for the default install)
./ag-installer.AppImage
```

The installer walks six screens: Welcome → Detection → Prompts → Install
Progress → First-Run Config → Done. By the end of the Done screen ag is
installed under XDG-standard paths in your home directory, three
`systemd --user` services are running, and the dashboard is available at
<http://127.0.0.1:3010/>.

### What gets installed

| Path | Contents |
| --- | --- |
| `~/.local/bin/ag` | The ag binary |
| `~/.local/lib/libtika_native.so` | Document-parser native lib |
| `~/.local/share/ag/` | Runtime state: data, index, db, logs, FalkorDB, web/ |
| `~/.config/ag/ag.env` | Environment file (per-user; never overwritten on reinstall) |
| `~/.config/ag/docker-compose.yml` | Observability stack definition |
| `~/.config/systemd/user/{ag,ag-stack,falkordb}.service` | Three composable user services |

No system files are modified. Removal is a single `ag-installer.AppImage
--uninstall` once Phase F lands.

### Requirements

- Linux x86-64 with glibc 2.39+ (Ubuntu 24.04 baseline)
- Docker (for the optional observability stack; the installer prompts to
  install via `get.docker.com` if missing)
- ~10 GB free disk on `$HOME`
- 7 GB RAM minimum; the installer detects low RAM and offers a smaller
  compose profile

## What ag does

Once installed, the dashboard at `http://127.0.0.1:3010/` lands on the **Home**
screen. From there:

- **Home** — chat with the agent, upload documents, switch between modes
  (RAG / LLM / Hybrid / RagStrict / Agentic). Documents land in per-corpus
  Tantivy indexes with HNSW vector retrieval.
- **Monitor** — live observability over what ag is doing: request timings,
  cache hit/miss across the three caching tiers, datastore health
  (Tantivy / FalkorDB / Redis / SQLite), chunk pipeline activity, RAG
  retrieval breakdowns, agent tool-call traces.
- **Config** — runtime-tunable settings across ~14 sub-pages: hardware
  budget, chunker mode, embedding model, ONNX runtime, FalkorDB, Redis,
  per-corpus overrides. Changes write to `overrides.json` and either
  hot-reload or surface a Restart-now banner.
- **Train** — custom embedding/classifier training UI.
- **Docu** — browse uploaded documents.

ag is intentionally designed as a learning platform: most surfaces show what
the system is doing internally (chunk counts, retrieval scores, cache
layers, graph-RAG entity merges) so the user can build a mental model of
how RAG works under the hood.

## API keys

Phase E of the installer (next release) wires the First-Run Config screen
to prompt for API keys (Google, OpenAI, OpenRouter, ...) and write them to
your local `~/.config/ag/ag.env`. Until then, the dashboard's
**Config → Runtime** page accepts them at runtime; they persist in
`~/.local/share/ag/overrides.json`.

API keys are never bundled in the AppImage, never committed to this repo,
and never shared between users. Each install has its own.

## Architecture (briefly)

```
[Browser / CLI]
      │ HTTP REST :3010
      ▼
[Actix backend] ──► Tantivy (full-text + vectors)
      │             FalkorDB (graph, optional)
      │             SQLite (settings, memory)
      │             Redis (L3 cache, optional)
      │
      └─► OpenTelemetry ──► Tempo / Prometheus / Grafana / Loki
```

Three caching tiers (in-process LRU → DashMap → Redis), five agent modes,
configurable chunker (fixed / lightweight / semantic), bundled Ollama
backend for local LLM inference.

The installer is a separate Dioxus desktop binary that bundles the
backend, frontend dist, FalkorDB binaries (extracted from the pinned
docker image), systemd templates, and the env example. See
[`docs/bin3`](docs/bin3) for the full installer design and 8-phase
execution plan.

## Build from source

```bash
# Backend
cd backend && cargo build --release

# Frontend (uses dx, not trunk — trunk doesn't process Dioxus 0.7 asset!())
cd frontend/fro && npm ci && npm run css:build && dx build --package fro --platform web --release

# Installer
cargo build --release -p ag-installer

# Everything (workspace build)
cargo build --release
```

CI builds the AppImage on `v*.*.*` tag push via
[`.github/workflows/release.yml`](.github/workflows/release.yml).

## License

[TODO: pick MIT / Apache-2.0 / AGPL-3.0 — see LICENSE]

Until a license is in place, all rights are reserved by the copyright
holder. The AppImage is downloadable but not yet legally redistributable.

## Contributing

Issues and pull requests welcome on
[github.com/PieterdenEngelse/RARAG](https://github.com/PieterdenEngelse/RARAG).
The installer design plan ([`docs/bin3`](docs/bin3)) is the best entry
point for understanding the roadmap; the eight phases (A through H) each
have their own scope, risks, and acceptance criteria.

For coding conventions, build commands, and the project's UI/voice rules,
see [`CLAUDE.md`](CLAUDE.md).
