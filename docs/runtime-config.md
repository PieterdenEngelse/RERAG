# Runtime configuration — overrides, hot reload, recovery

This page is the operator's reference for ag's runtime settings layer. The
design rationale lives in `persisentenc.md` at the repo root; this doc focuses
on day-to-day use.

## TL;DR

- Every registered setting can be changed at runtime from the **Config →
  Runtime** page (`/config/runtime`).
- Overrides persist to a single JSON file at `<base_dir>/overrides.json`.
  Install-time env files (`.env`, `~/.config/ag/ag.env`) are never modified.
- Precedence: **override → env → hard-coded default**.
- Most settings apply immediately (hot reload). The ones marked with the
  orange **restart** badge need a self re-exec — a universal restart that
  works in any deployment.
- If a saved override breaks the next boot, ag detects the failure on
  restart and rolls the overrides aside automatically.

## Where overrides live

```
<base_dir>/
├── overrides.json           ← the live override file (atomic writes)
├── overrides.boot.marker    ← present while a boot is in progress
└── overrides.json.bad-<ts>  ← the rolled-back file from a crashed boot, if any
```

`<base_dir>` defaults to `~/.local/share/ag` (override via `AG_HOME`).

## Precedence in detail

When ag asks for a setting, it consults sources in this order:

1. `overrides.json` (set by the Runtime UI)
2. Environment (read from the env file at process start)
3. Hard-coded default in the relevant subsystem

For chunker keys specifically, the chain prepends one more step:

```
DB save (Chunker config page) → override → env → default
```

The DB save is the user's deliberate persisted value; the runtime override
is the in-process tweak that applies when the DB has no value for that key.

## The 27 registered keys

| Key | Hot? | Notes |
|---|---|---|
| `REDIS_ENABLED` | hot | Drops/creates the L3 cache handle. |
| `REDIS_URL` | hot | Triggers a cache rebuild against the new URL. |
| `REDIS_TTL` | hot | New TTL applied to new entries. |
| `AUTO_EXPORT_ON_UPLOAD` | hot | Re-read on each upload. |
| `RUST_LOG` | hot | `tracing_subscriber::reload`; live log level. |
| `CHUNKER_MODE` | hot | DB save (Chunker page) takes precedence at boot. |
| `CHUNK_TARGET_SIZE` / `MAX_SIZE` / `OVERLAP` | hot | Same DB caveat. |
| `OTEL_TRACES_ENABLED` | hot | Tracer provider torn down + rebuilt. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | hot | New exporter built and installed globally. |
| `FILE_WATCHER_ENABLED` | hot | Watcher tasks aborted + respawned. |
| `FILE_WATCHER_DEBOUNCE_MS` | hot | Same. |
| `BACKEND_PORT` | restart | Bound at startup. |
| `TRUST_PROXY` | restart | Wired into the rate-limit middleware at boot. |
| `EMBEDDING_MODEL` / `BATCH_SIZE` / `CACHE_SIZE` | restart | Embedding stack constructed once. |
| `FALKOR_ENABLED` / `FALKOR_URI` | restart | Graph client constructed once. |
| `GRAPH_EXPANSION_ENABLED` / `MAX_HOPS` | restart | Loaded into `GraphConfig` at boot. |
| `INFERENCE_MAX_CONCURRENT_*` / `ACQUIRE_TIMEOUT_MS` | restart | Semaphore sizes baked in. |
| `CHUNKING_SNAPSHOT_LOGGING` | restart | Monitoring switch read once. |
| `SEARCH_TOP_K` | restart | Inlined into `ApiConfig`. |

Keys not in this table can still be set via the UI — they're shown in the
"Unregistered overrides" panel with no validation and no kind hints.

## Applying a restart-required change

Saving a `restart-required` key surfaces a banner with a **Restart now**
button. Clicking it:

1. Posts to `POST /runtime/actions/restart-self`.
2. ag drains its HTTP server briefly, then `execve`s the same binary.
3. The new process starts, reads `overrides.json`, and applies your value.
4. The frontend polls `/monitoring/health` and clears the overlay as soon as
   ag is back (up to a 60 s ceiling).

This is universal — works on bare-metal binary, in a systemd unit, or in a
container.

## Boot-failure recovery

A bad override (`BACKEND_PORT=80` for a non-root user, an `ONNX_MODEL_PATH`
that no longer exists, …) would normally brick startup. The recovery layer
catches this:

```
On startup:
  if overrides.boot.marker EXISTS and overrides.json EXISTS:
    move overrides.json → overrides.json.bad-<timestamp>
    log "rolled back overrides"
    record the rollback for /runtime/settings to surface
    boot with NO overrides applied
  write overrides.boot.marker

After ag becomes healthy (~30 s of uptime by default):
  delete overrides.boot.marker
```

If a rollback fires, the Runtime page shows a banner at the top with the
path to the `.bad-<ts>` file. Open it, copy the keys you want to keep,
re-apply them one at a time.

## Deployment capabilities

`GET /runtime/capabilities` returns one-shot detection at startup:

```json
{
  "deployment_mode": "systemd",
  "can_manage_compose": true,
  "can_view_journal": true,
  "managed_compose_file": "/home/pde/ag/docker-compose.yml"
}
```

The UI uses `can_manage_compose` to hide the "Also start/stop the redis
container" checkbox in deployments where docker isn't available.
`can_view_journal` is reserved for future log-viewing UI.

Self-restart is **not** capability-gated — it works in every deployment.

## HTTP surface

```
GET    /runtime/settings              # full snapshot + last_rollback
PUT    /runtime/settings/{key}        # body: { "value": "..." } or null to clear
DELETE /runtime/settings/{key}        # same as PUT with null
GET    /runtime/capabilities          # detected at startup
POST   /runtime/actions/restart-self  # universal self re-exec
```

All four endpoints live at the root scope (same level as `/monitor/*`).

## Adding a new key to the registry

When you write code that reads a new env var that should be runtime-tunable:

1. Replace `env::var("X")` with `crate::settings::effective_*("X", default)`.
2. Add a `KnownKey` entry in `backend/src/settings/registry.rs` with
   description, kind, default, category, and `restart_required`.
3. If the value should hot-reload (no restart needed), wire a subscriber in
   `main.rs` that rebuilds the relevant subsystem on change and set
   `restart_required: false`. Otherwise leave it `true`.
4. Add the key to the table above.

For settings without UI exposure (most of the 180+ remaining env vars), no
registry entry is needed — they still work as before through
`settings::effective_*` falling through to env and then defaults.
