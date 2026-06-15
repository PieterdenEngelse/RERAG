# RERAG

**Rust Educational RAG · working name `ag`**

A Rust-first Retrieval-Augmented Generation stack with a built-in chat
interface, observability dashboard, and tool-calling agent mode. Backend is an
Actix Web server (port 3010 by default); the frontend is a Dioxus desktop /
web UI served from the same port.

RERAG is intentionally designed as a learning platform — most surfaces show
what the system is doing internally (chunk counts, retrieval scores, cache
layers, graph-RAG entity merges) so the reader can build a mental model of
how RAG works under the hood. The binary, paths, and code identifiers stay
on the short name `ag` for ergonomic reasons.

> **Status:** [`v1.0.0`](https://github.com/PieterdenEngelse/RERAG/releases/tag/v1.0.0)
> shipped — eight-phase installer roadmap ([`docs/bin3`](docs/bin3))
> complete. Backend / dashboard / installer all feature-complete for
> the v1 line. Future v1.x patches focus on bug fixes and the
> [deferred items](docs/distro-notes.md#if-broader-appimage-coverage-becomes-important-later)
> (older-glibc AppImages, programmatic modal focus, etc.).

## Install

### Quick install (GUI) — recommended

For end users. No terminal commands required after the download.

1. **Download** the latest AppImage:
   <https://github.com/PieterdenEngelse/RERAG/releases/latest/download/ag-installer-v1.0.0-x86_64.AppImage>
   (or browse all releases at
   <https://github.com/PieterdenEngelse/RERAG/releases>).
2. **Make it executable.** Right-click → Properties → Permissions →
   "Allow executing file as program". Or in a terminal:
   ```bash
   chmod +x ~/Downloads/ag-installer-*.AppImage
   ```
3. **Double-click** the AppImage. The installer opens, walks you through
   six screens (Welcome → Detection → Prompts → Install → First-Run →
   Done), and lands you on the dashboard at <http://127.0.0.1:3010/> when
   finished.

No root password needed; nothing is written outside your home directory.

### Developer install (terminal)

If you'd rather install from source or want to script the process,
[`installers/install-linux.sh`](installers/install-linux.sh) is the
shell-installer equivalent of the GUI (same XDG paths, same systemd
units, same prompts). Clone the repo and run:

```bash
./installers/install-linux.sh
```

Both install paths walk the same six steps and produce the same result.
The installer walks: Welcome → Detection → Prompts → Install Progress
→ First-Run Config → Done. By the end, ag is installed under
XDG-standard paths in your home directory, three `systemd --user`
services are running, and the dashboard is available at
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

No system files are modified.

### Uninstall

```bash
# Remove the binary, libraries, and systemd units. Keeps your ag.env
# (API keys, FalkorDB password) and ~/.local/share/ag/ (data, indexes,
# logs, FalkorDB store) — re-running the installer later restores the
# system with your config + data intact.
ag-installer --uninstall

# Same as above, but also removes ag.env and ~/.local/share/ag/.
# Destructive — your API keys, corpora, and indexes go with it.
ag-installer --uninstall --purge
```

Both modes prompt for confirmation before deleting anything and print
exactly which paths will be removed.

### Requirements

- Linux x86-64 with glibc 2.39+ (Ubuntu 24.04+, Fedora 40+, Arch,
  openSUSE Tumbleweed). Older distros use the bash installer below
  instead. See [`docs/distro-notes.md`](docs/distro-notes.md) for the
  full support matrix.
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

The installer's **First-Run Config** screen (Screen 5 of 6) prompts for
LLM API keys — OpenAI, OpenRouter, Anthropic — and writes them atomically
into `~/.config/ag/ag.env` with mode `0600`. Leave any field blank to
skip; users on Ollama-only don't need any external keys.

If you skip First-Run entirely or want to add keys later, the dashboard's
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
[`docs/bin3`](docs/bin3) for the full design and 8-phase execution
plan — all eight phases (0, A–H) are now ✅ shipped as of v1.0.0.

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
[github.com/PieterdenEngelse/RERAG](https://github.com/PieterdenEngelse/RERAG).
The repo was renamed from `RARAG` → `RERAG` after the v1.0.0 cut;
GitHub auto-redirects old links for ~6 months.

For coding conventions, build commands, and the project's UI/voice
rules, see [`CLAUDE.md`](CLAUDE.md). The installer's design history
is in [`docs/bin3`](docs/bin3) (all eight phases shipped; useful as
a retrospective of how the GUI installer came together). For the
distro support matrix and verification steps, see
[`docs/distro-notes.md`](docs/distro-notes.md).
