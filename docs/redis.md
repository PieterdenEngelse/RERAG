# Redis / FalkorDB Configuration Parameters

**Context:** FalkorDB is a Redis module, so "Redis configuration" for ag spans
three layers at once — the connection ag opens, the Redis server itself, and
the FalkorDB graph module hosted inside it. The `/config/falkordb` page
currently exposes only layer 1.

Related: [`falkordb-migration.md`](./falkordb-migration.md) ·
[`falkordb-native-service.md`](./falkordb-native-service.md)

---

## Layer 1 — Client connection (ag owns these)

ag opens the connection, so these genuinely belong in ag's config and
`.env.graph`.

| Parameter | Status | Notes |
|---|---|---|
| `uri` / host / port | on page | `redis://host:port` |
| `password` | on page | Redis `AUTH` (default user) |
| graph key (`database`) | on page | the `GRAPH.QUERY` key name |
| `max_connections` | on page | connection pool size |
| connect `timeout_ms` | on page | time to establish a connection |
| command/response timeout | could add | distinct from *connect* timeout — caps a slow query client-side |
| TLS (`rediss://` + CA/cert paths) | could add | only if FalkorDB is exposed beyond localhost |
| logical DB index (`SELECT 0–15`) | could add | Redis has 16 numbered DBs — separate from the FalkorDB graph key |
| ACL username | could add | Redis 6+ ACLs support usernames; the "no username" rule is only the default-user case |
| retry / reconnect policy | could add | attempts, backoff |

## Layer 2 — FalkorDB module (`GRAPH.CONFIG SET`)

These tune the GraphBLAS query engine. Some are runtime-settable; some are
load-time only (passed as module args to `loadmodule`).

| Parameter | When | Notes |
|---|---|---|
| `THREAD_COUNT` | load-time | query-execution threads |
| `OMP_THREAD_COUNT` | load-time | OpenMP threads for GraphBLAS matrix ops |
| `CACHE_SIZE` | load-time | compiled-query cache per graph |
| `NODE_CREATION_BUFFER` | load-time | preallocation buffer |
| `TIMEOUT_DEFAULT` / `TIMEOUT_MAX` | runtime | server-side query timeout |
| `QUERY_MEM_CAPACITY` | runtime | memory cap per query |
| `RESULTSET_SIZE` | runtime | max rows returned |
| `MAX_QUEUED_QUERIES` | runtime | backpressure limit |

## Layer 3 — Redis server (`CONFIG SET` / `redis.conf`)

The Redis instance itself. On a 7 GB CPU-only box this is where the migration's
memory win is actually realized.

| Parameter | When | Notes |
|---|---|---|
| `maxmemory` + `maxmemory-policy` | runtime | hard memory ceiling — the lever that matters here |
| `appendonly` / `appendfsync` | runtime | AOF durability (the migration enables this) |
| `save` | runtime | RDB snapshot schedule |
| `dir`, `dbfilename`, `appendfilename` | restart | data location |
| `requirepass` | runtime | the password, server side |
| `port`, `bind` | restart | listen address |
| `timeout`, `tcp-keepalive`, `maxclients` | runtime | idle/connection handling |
| `io-threads` | restart | Redis I/O threads |
| `loglevel` | runtime | log verbosity |

---

## Where each layer is set

**Only layer 1 belongs in ag's config / `.env.graph`** — ag owns the
connection. Layers 2–3 are properties of the *FalkorDB deployment*, not of ag:

- **Today (container):** `docker-compose.yml` → `REDIS_ARGS` (sets password +
  AOF).
- **After the native-service migration:** the `falkordb.service` unit's
  `ExecStart` flags (`--loadmodule …`, `--maxmemory …`, `--appendonly yes`,
  `--requirepass …`) plus `MemoryMax=` in the unit file.

ag *could* still surface layers 2–3 as a **read-only "FalkorDB server" panel**
that runs `CONFIG GET` / `GRAPH.CONFIG GET` and displays `maxmemory`,
persistence mode, and thread counts. The runtime-settable rows could even get a
live "apply" button via `CONFIG SET` — though such a change would not survive a
FalkorDB restart unless also written into the unit file / `REDIS_ARGS`.

## What ag exposes today

- **Layer 1** — editable on `/config/falkordb` (connection: URI, password, graph
  key, pool size, connect timeout). Saved to `.env.graph`, which overrides env
  vars; applied on restart or Reconnect.
- **Layers 2–3** — editable on `/config/redis`, a live tuning console. It reads
  every parameter via `CONFIG GET` / `GRAPH.CONFIG GET` and applies the
  runtime-settable ones via `CONFIG SET` / `GRAPH.CONFIG SET`. Restart- and
  load-time-only parameters are shown read-only. **Live changes do not survive a
  FalkorDB restart** — the page is a tuning console, not persistent config; to
  persist, update the service args (`REDIS_ARGS`, or the `falkordb.service`
  unit once the native-service migration lands).

## The `/config/redis` page

`/config/redis` is a live tuning console for the FalkorDB instance. It exposes
every parameter in the Layer 2 and Layer 3 tables above — 15 Redis-server
params and 10 FalkorDB-module params.

**Behaviour:**

- On load it reads each parameter via `CONFIG GET` / `GRAPH.CONFIG GET`, plus
  `INFO` for the Redis version and memory use.
- Runtime-settable rows are editable text inputs; restart/load-time-only rows
  are shown read-only.
- **Apply** diffs the edited values and pushes only the changed ones via
  `CONFIG SET` / `GRAPH.CONFIG SET`, reporting per-parameter success/failure.
- Every parameter has an info button; the help text is supplied by the backend
  catalog, so adding a parameter is a backend-only change.
- A banner states the non-persistence caveat (live only, lost on restart).

**How it works:**

The backend opens a plain `redis`-crate connection to the FalkorDB instance —
the URL is built from `GraphConfig`, exactly as the graph client builds it —
and runs raw `CONFIG` / `GRAPH.CONFIG` commands. It does not depend on the
`falkordb` crate's API and does not require the `graph` cargo feature.

**Endpoints:**

- `GET /config/redis` — read all catalogued parameters + server `INFO`.
- `POST /config/redis` — apply a list of `{section, key, value}` changes; only
  catalogued, editable keys are accepted.

**Source:**

- Backend — `api/config_routes.rs` (parameter catalog + `get_redis_config` /
  `apply_redis_config`), `api/mod.rs` (routes).
- Frontend — `api.rs` (types + `fetch_redis_config` / `apply_redis_config`),
  `pages/redis.rs` (`ConfigRedis`), plus `pages/mod.rs`, `app.rs`,
  `components/config_nav.rs` wiring.

## Two-mode Apply: runtime vs restart parameters

Every row on `/config/redis` is editable. Each catalogued parameter has a
**mode**, and Apply routes the change accordingly:

| Mode | Mechanism | Persists? |
|---|---|---|
| `runtime` | `CONFIG SET` / `GRAPH.CONFIG SET` on the running process | no — live only |
| `restart` | rewrite of the `falkordb.service` unit + daemon-reload + restart | yes |

`restart` rows carry a `restart` tag in the UI. `runtime` rows are the params
the running process accepts live; `restart` rows are those it cannot change
without a restart (`port`, `bind`, `dir`, `io-threads`, `databases`) or a
module reload (`THREAD_COUNT`, `OMP_THREAD_COUNT`, `CACHE_SIZE`,
`NODE_CREATION_BUFFER`) — the split is per parameter, not per layer.

### How restart-mode Apply works

1. The `falkordb.service` unit is copied to `falkordb.service.bak`.
2. The values are spliced into `ExecStart`: Redis params become `--key value`
   flags before `--loadmodule`; FalkorDB module params become `KEY VALUE`
   pairs after the `.so` path. (Values must be single, whitespace-free tokens.)
3. `systemctl --user daemon-reload`, then `restart falkordb.service`.
4. Health check — Redis `PING`, retried for ~5 s.
5. If FalkorDB does not come up, the backup unit is restored, reloaded and
   restarted — an automatic rollback.

Apply does restart-mode changes first, then runtime-mode ones, so the restart
cannot wipe a freshly-`CONFIG SET` runtime value. A restart-mode Apply restarts
FalkorDB, so ag's graph connection should be re-established (Reconnect on the
FalkorDB page).

This means `/config/redis` now edits the FalkorDB **service definition**, not
just the running process — it crosses the boundary this doc otherwise draws
(ag *connects to* FalkorDB vs *manages how it starts*). The backup + rollback
keeps a malformed `ExecStart` from leaving FalkorDB down.

**Status:** implemented.

## Remaining next step

Layer 1 still misses a few client-owned params worth adding to
`/config/falkordb`: command/response timeout, TLS (`rediss://`), and the logical
DB index. Those are genuinely ag-controlled and persistable to `.env.graph` —
unlike layers 2–3, which are deployment-level and only tunable live.
