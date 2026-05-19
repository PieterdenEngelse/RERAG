# Neo4j → FalkorDB Migration

**Status:** Code migration + rename complete (Steps 0–8). Pending: end-to-end data
verification (ingest a corpus → confirm nodes land) and the `CLAUDE.md` architecture-label fix.
**Branch:** `falkordb-migration`
**Scope:** Replace the Neo4j knowledge-graph backend with FalkorDB. ~13 files.
**Deployment:** FalkorDB runs as a **native systemd user service**, not a Docker
container — see [`falkordb-native-service.md`](./falkordb-native-service.md).

---

## 1. Why

The knowledge-graph store is the heaviest single service in the stack and
the gain from swapping it is **memory and ops simplification**, not speed.

| Option | Real RSS | Cost |
|---|---|---|
| Neo4j today (untuned heap 256–512m) | ~1 GB | — |
| Neo4j with `pagecache=64m`, `heap=256m` | ~500–600 MB | 2 lines of YAML |
| FalkorDB | ~50–150 MB | this migration |

Measured anchor: the existing `ag-redis` container uses **3.3 MiB**. FalkorDB is
Redis + the GraphBLAS module + the graph itself.

The ~250–300 MB JVM floor (metaspace, code cache, GC, thread stacks) cannot be
tuned away — it exists before a single node is stored. Removing it is the point.
On a 7 GB CPU-only box that runs ONNX inference, reclaiming ~400–450 MB over
*tuned* Neo4j — plus dropping a JVM, APOC, and a second operational model — is
the justification. **Performance is explicitly not a reason** (see §3).

---

## 2. What FalkorDB is

FalkorDB is the maintained continuation of RedisGraph: a **Redis module** that
speaks OpenCypher via the `GRAPH.QUERY <key> "<cypher>"` command over the Redis
(RESP) protocol — **not** the Bolt protocol. Consequences:

- `neo4rs` (a Bolt driver) cannot talk to it. The whole client layer is replaced.
- A "database" becomes a **graph key name** — the first argument to `GRAPH.QUERY`.
- Auth is a single optional Redis password. There is no separate username.

---

## 3. The architectural fact that bounds this migration

The graph DB is **not on the read hot path**. At startup, `main.rs` compiles the
graph into an in-process `petgraph` runtime (`petgraph_runtime::RUNTIME_GRAPH`);
all query-time graph expansion reads that in-RAM structure (`retriever.rs:1150`).

The graph DB is touched only by:

1. **Ingestion writes** — `KnowledgeBuilder` / `AgentMemoryGraph`, one query per call.
2. **The one-time startup compile** — `compile_from_neo4j`, runs in a background
   task, does not block serving.
3. **Admin/inspection endpoints** — `api/graph_routes.rs`, human-triggered.

So FalkorDB's GraphBLAS traversal speed is irrelevant here — traversals run in
petgraph. This also means the migration is **low blast radius**: a botched graph
layer breaks ingestion and admin endpoints, but the app keeps serving search.

---

## 4. Locked decisions

| # | Decision | Choice | Reason |
|---|---|---|---|
| 1 | Rust client | Official **`falkordb`** crate | Avoids hand-rolling a RESP result decoder. Built on the `redis` crate, so raw commands remain available as an escape hatch. |
| 2 | Naming | Keep `neo4j` feature + `NEO4J_*` env vars **during** the migration; rename in a **separate final commit** | Migration discipline — change one variable (the backend) at a time. |
| 3 | `datetime()` / `randomUUID()` | Generate in Rust, pass as `$params` | One rule: magic values come from the app, not the DB. Testable, clock-independent. Timestamps become epoch-ms `i64`. |
| 4 | Strategy | Hard swap — no dual-backend abstraction trait | The user chose to replace, not to support both. An abstraction layer is gold-plating. |
| 5 | Isolation | Dedicated git branch; pending unrelated changes committed/stashed first | Keeps the ~13-file diff reviewable. |

---

## 5. Cypher compatibility

The graph stores a pure property graph (`Document → Chunk → Entity/Concept`,
`Agent → Episode → Goal/Reflection`). It is **not** a vector store — chunks hold
only an `embedding_id` string pointing into Tantivy. Nothing vector-related migrates.

**Ports unchanged (FalkorDB OpenCypher supports all of these):**
`MATCH`, `MERGE`, `ON CREATE SET` / `ON MATCH SET`, `CREATE`, `UNWIND`,
`OPTIONAL MATCH`, `DETACH DELETE`, `WITH`, `collect` / `count` / `sum`,
`count(DISTINCT …)`, `CASE WHEN`, `coalesce`, `toFloat`, `toLower`, `size`,
`<>`, `IN`, `CONTAINS`, `ORDER BY`, `LIMIT`. Queries use fixed 1–2 hop patterns —
no variable-length paths.

**Needs rework:**

| Neo4j construct | Where | Replacement |
|---|---|---|
| Bolt driver (`neo4rs` + `deadpool`) | `client.rs` | `falkordb` crate |
| `datetime()` | every write in `knowledge_builder.rs`, `agent_memory_graph.rs`, `schema.rs` | Rust `chrono::Utc::now().timestamp_millis()` as `$param` |
| `randomUUID()` | `knowledge_builder.rs:113`, `:178` | Rust `uuid::Uuid::new_v4()` as `$param` |
| `CREATE CONSTRAINT … REQUIRE … IS UNIQUE` (×7) | `schema.rs`, `client.rs` | `GRAPH.CONSTRAINT CREATE` command (needs a backing index first) |
| `CREATE FULLTEXT INDEX … ON EACH […]` (×3) | `schema.rs`, `client.rs` | `CALL db.idx.fulltext.createNodeIndex(…)` |
| `CREATE INDEX … IF NOT EXISTS` | `schema.rs` | `CREATE INDEX FOR (n:Label) ON (n.prop)` — existing code already swallows "already exists" errors |
| `row.get::<T>(name)` | every read site | `falkordb` crate's typed result API |
| `apoc` plugin | `docker-compose.yml:46` | Drop — no APOC calls exist in the code |

---

## 6. Config / env model change

`graph/config.rs` and `.env.example` change meaning (values, not names, until the
rename commit):

| Var | Before | After |
|---|---|---|
| `NEO4J_URI` | `bolt://localhost:7687` | `redis://localhost:6379` |
| `NEO4J_USER` | `neo4j` | **obsolete** — Redis has no separate user; drop from `GraphConfig` |
| `NEO4J_PASSWORD` | `agpassword123` | Redis password (optional) |
| `NEO4J_DATABASE` | `neo4j` | graph **key name** passed to `GRAPH.QUERY` |
| `NEO4J_MAX_CONNECTIONS` | pool size | pool size (falkordb crate pooling) |
| `NEO4J_CONNECTION_TIMEOUT_MS` | timeout | unchanged |

> Run FalkorDB as a **separate instance** from the L3 cache Redis. The cache runs
> with persistence off; the knowledge graph needs AOF/RDB durability.

---

## 7. Phased plan

### Step 0 — prep
- [x] Working tree cleaned (the 8 unrelated files were handled by the user).
- [x] Branch `falkordb-migration` created.

### Step 1 — infra
- [x] `backend/Cargo.toml`: `neo4rs` + `deadpool` → `falkordb 0.2`. `uuid`/`chrono` were already deps. `neo4j` feature name kept.
- [x] `docker-compose.yml`: `neo4j:5` service removed (APOC, conf/certs mounts and
  `neo4j-logs` volume gone with it). FalkorDB was briefly a `falkordb/falkordb`
  compose service, then moved to a **native systemd user service** — that compose
  service and the `falkordb-data` volume have since been removed too. See
  [`falkordb-native-service.md`](./falkordb-native-service.md).

### Step 2 — client layer
- [x] `graph/client.rs`: full rewrite. Adds `GraphHandle` (cloneable, replaces `Arc<neo4rs::Graph>`), a `lit` Cypher-literal encoder, positional `row_*` extractors, `now_millis()`, and the `params!` macro. Type names `Neo4jClient`/`Neo4jError` kept.

### Step 3 — DDL
- [x] **`graph/schema.rs` deleted** — it was dead code (no callers; the real schema init is `client.rs::init_schema`). Index DDL now lives in `init_schema`: range indexes on merge keys + full-text indexes. Explicit unique constraints omitted — `MERGE` already enforces uniqueness.

### Step 4 — write paths
- [x] `graph/knowledge_builder.rs`: ported; `datetime()` → `$now` (epoch-ms), `randomUUID()` → app-side `uuid` param.
- [x] `graph/agent_memory_graph.rs`: ported; same datetime/uuid treatment.

### Step 5 — read paths
- [x] `graph/graph_retriever.rs`: ported query sites + positional result extraction.
- [x] `graph/petgraph_runtime.rs`: `compile_from_neo4j` ported (`r.metadata` column dropped — never written).
- Note: `AgentMemoryGraph` and `GraphRetriever` have no external callers — compiled/exported only.

### Step 6 — wiring
- [x] `api/graph_routes.rs`: 6 handlers ported; `elementId()` → `ID()` (integer node handle).
- [x] `graph/mod.rs`: `schema` module removed, `GraphHandle` re-exported.
- `api/mod.rs` and `main.rs` needed **no changes** — the `Neo4jClient` API surface (`new`, `graph()`, `init_schema`, `Clone`) was preserved deliberately.

### Step 7 — verify
- [x] `cargo check` clean.
- [x] `cargo fmt` applied; `cargo clippy --all-targets` clean.
- [x] `falkordb.service` (native systemd user unit) runs healthy on `127.0.0.1:6380`; `GRAPH.QUERY` works.
- [x] **Migration code verified live** — across multiple service starts `ag` logs
  `Successfully connected to FalkorDB` → `FalkorDB schema initialization complete`
  → `Application Started Successfully`; `GET /graph/stats` queries FalkorDB and
  returns cleanly (empty — nothing ingested into the fresh graph yet);
  `GET /graph/rt/stats` works.
- [x] **`ag.service` runs stably** (`active`, `NRestarts=0`, `/graph/stats` serving
  live from FalkorDB). The crash loop hit during verification was a **pre-existing
  systemd misconfig, not the migration**: `ag.service` was active in *both* the user
  and the system systemd scope; both run `pre-start-clear-port.sh`'s `pkill -9 -x ag`,
  so the two instances SIGKILLed each other every cycle. Fixed by
  `sudo systemctl disable --now ag.service` (system scope) — see
  [memory: ag-service-single-scope].
- [ ] Ingest a test corpus; confirm entities/episodes land in FalkorDB.
- [ ] Update the architecture description in `CLAUDE.md` (fix the stale
  "Neo4jDB (vector)" label) and `AGENTS.md` if relevant.

### Step 8 — rename (done)
- [x] `neo4j` cargo feature → `graph`; `default` / `full` feature lists and every `#[cfg(feature = …)]` updated.
- [x] `Neo4jClient` / `Neo4jError` → `GraphClient` / `GraphError`.
- [x] `NEO4J_*` env vars → `FALKOR_*` in `.env.example` and `config.rs`.
- [x] Live runtime env (`~/.config/ag/ag.env`) updated: `FALKOR_URI`, `FALKOR_PASSWORD`, `FALKOR_ENABLED` (no `FALKOR_USER`).

---

## 8. Risks & rollback

| Risk | Mitigation |
|---|---|
| `falkordb` crate missing a feature | It wraps the `redis` crate — drop to a raw `GRAPH.QUERY` command. |
| Migration breaks ingestion | Low blast radius — petgraph fronts reads, search keeps working. Branch isolates the diff. |
| FalkorDB durability weaker than Neo4j | Enable AOF; separate instance from cache Redis; the graph is also rebuildable from source documents via re-ingest. |
| Lost Neo4j temporal type | Accepted — timestamps become epoch-ms `i64`; format at the display layer. |

**Rollback:** revert the branch (this restores the Neo4j `docker-compose.yml`
service), `systemctl --user disable --now falkordb.service`, then
`docker compose up -d neo4j` to return to Neo4j.

---

## 9. Files touched

`backend/Cargo.toml` · `docker-compose.yml` · `.env.example` ·
`backend/src/graph/{config,client,schema,knowledge_builder,agent_memory_graph,graph_retriever,petgraph_runtime,mod}.rs` ·
`backend/src/api/{graph_routes,mod}.rs` · `backend/src/main.rs` · `CLAUDE.md`
