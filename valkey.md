# Migration Plan: Replace Redis with Valkey

> **Decision (2026-05-20): not proceeding.** Keeping Redis 7-alpine for the L3
> cache. The only real upside was licensing hygiene on one optional container,
> and the perf/memory wins for this workload land in low single digits (see
> "What this migration actually buys you" below). Net: real churn across
> backend, frontend, infra, and the live `~/.config/ag/ag.env` for a marginal
> gain. Plan kept on disk as a record of the analysis in case the calculus
> changes (e.g. Redis license tightens further, or the L3 cache grows enough
> that per-key overhead starts to matter).

## ⚠️ Key finding — "Redis" means two unrelated things in this codebase

| Role | What it is | Rename? |
|------|-----------|---------|
| **A — L3 cache** | A standalone `redis:7-alpine` container, the `RedisCache` type, `REDIS_*` env vars | **Yes → Valkey** |
| **B — FalkorDB's engine** | FalkorDB *is a Redis module* — it runs inside a real Redis server. The `/config/redis` page, `REDIS_PARAM_CATALOG`, `open_falkor_redis_conn`, and the `redis` Cargo crate all serve this. | **No — keep "Redis"** |

FalkorDB is built on Redis, not Valkey. Renaming Role B to "Valkey" would be
factually wrong and would mislabel the FalkorDB tuning page. **The "full rename"
applies to Role A only.**

No file collision: the frontend's `pages/redis.rs` is the *FalkorDB* tuning page
and stays put.

The `redis` Rust crate also stays — it's the wire-protocol client library, used
by both roles. Valkey is wire-compatible, so `ValkeyCache` keeps `use redis::...`
internally. (A `valkey` crate exists but it's just a rebrand of `redis` and adds
risk for zero gain.)

### Why FalkorDB itself can't move to Valkey

Per `docs/falkordb-native-service.md`, `falkordb.service` runs:

```
redis-server --loadmodule …/falkordb.so --port 6380 …
```

where `redis-server` **and** `falkordb.so` were extracted from the
`falkordb/falkordb` image as a *guaranteed-compatible pair*. "FalkorDB on
Valkey" would mean swapping that `redis-server` for `valkey-server`. That's a
bad trade:

1. **It might not even load.** `falkordb.so` is compiled against a specific
   Redis Module API version and checks it at `--loadmodule` time. Valkey kept
   the module API at the 7.2 fork point (Valkey 8 ships `RedisModule_*`
   compatibility aliases), so a Redis module *can* load into `valkey-server` in
   principle — but if Valkey reports a version FalkorDB doesn't accept, the
   server refuses to start.
2. **FalkorDB only tests/supports Redis.** The `redis-server` + `.so` are
   shipped as a matched pair *because* the coupling is tight (module API,
   internal structures). There is no "FalkorDB on Valkey" support matrix. For a
   graph store holding the knowledge graph, untested = data-integrity risk.
3. **No upside.** The reason to adopt Valkey is the Redis license change
   (SSPL/RSALv2) — but **FalkorDB itself is SSPL-licensed**, so running it on a
   Valkey core does not make the FalkorDB stack open-source. The licensing win
   is cancelled out by FalkorDB's own license: real risk for zero benefit.

The **L3 cache is the opposite case** — plain key/value with TTL, *no modules* —
so Valkey is a 100% wire-compatible drop-in. That asymmetry is exactly why this
plan swaps the L3 cache (Role A) and leaves FalkorDB's Redis foundation
(Role B) alone.

---

## What this migration actually buys you

The gain is confined to the L3 cache **because L3 is the only thing being
swapped** — FalkorDB (Role B) stays on Redis, so nothing changes there.

**1. Licensing — the real reason.** Redis 7.4+ is RSALv2/SSPL (source-available,
*not* OSI open-source). Valkey is BSD-3-Clause under the Linux Foundation. The L3
cache container moves to a genuinely open-source, community-governed server.

**2. Perf/memory — a small free bonus here, not a headline.** Valkey 8.x has real
wins (better I/O threading, lower per-key overhead), but their impact on *this*
deployment is small:

- *"~20% lower per-key overhead" ≠ "20% less memory."* Per-key overhead is the
  bookkeeping — dict entry, key SDS header, value `robj`, plus a second expires
  entry for TTL'd keys — roughly **60–100 bytes/key**. Valkey 8.1 trims ~15 bytes
  of that. But the **value** (serialized search results, hundreds of bytes to a
  couple KB) dominates each entry and does *not* shrink:

  ```
  entry ≈ [~80 B overhead] + [~500–2000 B value]
  20% off the overhead  ≈  ~15 B saved  ≈  1–3% of the entry
  ```

  Total RAM saving for this cache lands in low single digits. The 20% headline
  only approaches 20%-of-total for tiny values (counters, flags) at millions of
  keys.
- *I/O threading* pays off when network-IO-bound at tens of thousands of ops/sec.
  L3 is only hit on an L1+L2 miss, on a single CPU-only box; the app's bottleneck
  is Tantivy + the embedder, not cache socket throughput.

**3. Functionally — nothing changes.** Valkey is wire-compatible; the cache
behaves identically.

**Caveat — this does not make the stack "Redis-free."** FalkorDB still embeds a
`redis-server` (Role B). If the goal was eliminating Redis-licensed code, this
migration only gets you partway. Treat it as **licensing hygiene for one optional
container**, and don't size hardware or capacity expectations around the perf
numbers.

---

## Backend (Role A only)

1. `cache/redis_cache.rs` → rename file to `valkey_cache.rs`; `RedisCache`→`ValkeyCache`,
   `RedisCacheSummary`→`ValkeyCacheSummary`, param `redis_url`→`valkey_url`
2. `cache/mod.rs` — module path, re-export, `CacheConfig.l3_redis_url`→`l3_valkey_url`, L3 comment
3. `config.rs` — fields `redis_enabled/redis_url/redis_ttl`→`valkey_*`; env `REDIS_ENABLED/URL/TTL`→`VALKEY_*`
4. `main.rs` — import, `config.redis_*`, log strings ("Valkey L3 cache")
5. `api/monitor_routes.rs` — `RedisCacheSummary` type, `redis:` JSON field→`valkey:`, `REDIS_ENABLED` check, status strings
6. `retriever.rs` — `RedisCache` type usages in `set_l3_cache`/`get_l3_cache_summary`
7. `perf/connection_pool.rs` — comment; `ops/windows/winsw/ag.xml` — `REDIS_ENABLED` var

**Untouched (Role B):** `api/config_routes.rs`, `graph/client.rs`, `graph/config.rs`,
`Cargo.toml` `redis` dep, `FALKOR_URI`'s `redis://` scheme.

## Frontend

8. `api.rs` — `RedisSummary`→`ValkeySummary`, `CacheInfo.redis`→`valkey` (NOT `RedisConfigResponse` — Role B)
9. `monitor/cache.rs` — `data.redis`→`data.valkey`, labels + info tooltips (incl. the env-var list inside them)
10. `monitor/overview.rs` — `redis_enabled/connected` fields, `c.redis`, the "Redis (L3)" clear-cache note
11. `monitor/docker.rs` — container key `"redis"`→`"valkey"` (must match new container name), display name, descriptions, port note
12. `components/header.rs` — `cache_info.redis`, page-error key `"redis"`, status messages
13. `monitor/requests.rs`, `monitor/tip.rs`, `docu_index/threads.rs` + `tantivy.rs` — "L3 Redis" tier descriptions

**Untouched:** `pages/redis.rs`/`falkordb.rs`, `config_nav.rs` Redis tab, `app.rs`
ConfigRedis route (all Role B); `other.rs`/`memories.rs`/`entity_extractor.rs`
(generic word "redis").

## Infra / env / docs

14. `docker-compose.yml` — `image: valkey/valkey:8-alpine`, service `valkey:`,
    `container_name: ag-valkey`, volume `valkey-data`,
    `command: valkey-server --appendonly yes`, healthcheck `valkey-cli ping`, comments
15. `start-ag.sh` (`up -d valkey …`), `systemd/README.md`, `installers/install.sh` template
16. `.env.example` + `backend/src/ops/systemd/ag.env.example` — `REDIS_*`→`VALKEY_*`
17. `CLAUDE.md` — caching tiers + env var list
18. **`~/.config/ag/ag.env`** (the live runtime env file) — must rename `REDIS_*` there
    too, or the L3 cache silently disables on next restart

## Verify

```bash
cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings
```

## Post-migration cleanup

The migration is a 1-for-1 replacement of the L3 server — Valkey *instead of*
the L3 Redis, not in addition to it. FalkorDB's own `redis-server` is untouched,
so the running-process and baseline-memory count is unchanged (Valkey 8 is
marginally lighter per key, if anything).

Renaming the volume `redis-data` → `valkey-data` leaves the old volume and the
old image as orphans in Docker's local store. Once the new stack is verified,
reclaim them:

```bash
docker volume rm ag_redis-data      # old L3 cache data — disposable, just cache
docker image prune                  # drop the old redis:7-alpine image
```

L3 cache data is ephemeral (it rebuilds on the next searches), so dropping the
old volume costs nothing.

---

## Open decisions

### 1. How should `config.rs` handle the env var rename?

- **Fallback to `REDIS_*` (recommended):** Read `VALKEY_ENABLED/URL/TTL` first; if
  absent, fall back to the old `REDIS_*` names with a one-time deprecation `warn!`.
  Nothing breaks if an env file is missed. Can drop the fallback later.
- **Hard rename, no fallback:** Only `VALKEY_*` is read. Cleaner code, but every env
  file (`.env`, `ag.env.example`, AND the live `~/.config/ag/ag.env`) must be updated
  in lockstep or the L3 cache silently turns off.

### 2. Should the live `~/.config/ag/ag.env` runtime file be updated?

- **Yes:** rename `REDIS_*` to `VALKEY_*` there so the running service stays
  consistent after the next restart.
- **No:** leave it; the user updates it before restarting `ag.service`.
