# Neo4j → FalkorDB Migration

**Status:** Planned — not started
**Branch:** _(create before step 2)_
**Scope:** Replace the Neo4j knowledge-graph backend with FalkorDB. ~13 files.

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
- [ ] Commit or stash the 8 unrelated modified files on `main`.
- [ ] Create branch, e.g. `falkordb-migration`.

### Step 1 — infra
- [ ] `backend/Cargo.toml`: replace `neo4rs` + `deadpool` with `falkordb`; add `uuid`. Keep the `neo4j` feature name for now.
- [ ] `docker-compose.yml`: replace the `neo4j:5` service with `falkordb/falkordb`; drop the APOC plugin, the `/var/lib/neo4j/conf` and `/certs` mounts, the `neo4j-logs` volume; expose `:6379` (+ `:3000` browser, optional).

### Step 2 — client layer
- [ ] `graph/client.rs`: full rewrite — connection, `init_schema`, `get_stats`, `health_check`, `execute_query`, `run_query` against the `falkordb` API. Keep the `Neo4jClient` / `Neo4jError` type names for now (renamed later).

### Step 3 — DDL
- [ ] `graph/schema.rs`: constraints via `GRAPH.CONSTRAINT`, indexes via `CREATE INDEX FOR …`, full-text via `db.idx.fulltext.*`. `_Meta` schema-version node uses a Rust-supplied epoch-ms timestamp.

### Step 4 — write paths
- [ ] `graph/knowledge_builder.rs`: port every `neo4rs::query(...).param(...)`; `datetime()` → `$now`, `randomUUID()` → `$id`.
- [ ] `graph/agent_memory_graph.rs`: same — this file is the heaviest `datetime()` user.

### Step 5 — read paths
- [ ] `graph/graph_retriever.rs`: port query call sites and result deserialization.
- [ ] `graph/petgraph_runtime.rs`: port `compile_from_neo4j`, `initialize_from_neo4j`, `new` / `new_with_neo4j`.

### Step 6 — wiring
- [ ] `api/graph_routes.rs`: port ~9 `neo4rs::query` sites.
- [ ] `api/mod.rs`: the `NEO4J_CLIENT` static + getter/setter (type change only).
- [ ] `main.rs`: the Phase 5.5 init block.
- [ ] `graph/mod.rs`: re-exports.

### Step 7 — verify
- [ ] User restarts `ag.service` to surface build errors (no speculative builds).
- [ ] Fix compile errors.
- [ ] `docker compose --profile core up -d` (FalkorDB); ingest a test corpus; confirm entities/episodes appear; hit `/graph/*` admin endpoints; confirm petgraph compiles at startup.
- [ ] Update the architecture description in `CLAUDE.md` (fix the stale "Neo4jDB (vector)" label) and `AGENTS.md` if relevant.

### Step 8 — rename (separate commit, only after Step 7 is green)
- [ ] `neo4j` cargo feature → `graph`; update `default` / `full` feature lists and every `#[cfg(feature = "neo4j")]`.
- [ ] `Neo4jClient` / `Neo4jError` → `GraphClient` / `GraphError`.
- [ ] `NEO4J_*` env vars → `FALKOR_*` in `.env.example` and `config.rs`.
- [ ] Live `.env` is gitignored — update it by hand:
      `sed -i 's/^NEO4J_/FALKOR_/' .env` (then fix `FALKOR_URI` / drop `FALKOR_USER`).

---

## 8. Risks & rollback

| Risk | Mitigation |
|---|---|
| `falkordb` crate missing a feature | It wraps the `redis` crate — drop to a raw `GRAPH.QUERY` command. |
| Migration breaks ingestion | Low blast radius — petgraph fronts reads, search keeps working. Branch isolates the diff. |
| FalkorDB durability weaker than Neo4j | Enable AOF; separate instance from cache Redis; the graph is also rebuildable from source documents via re-ingest. |
| Lost Neo4j temporal type | Accepted — timestamps become epoch-ms `i64`; format at the display layer. |

**Rollback:** the work is one branch and one `docker-compose.yml` service. Revert
the branch and `docker compose up -d neo4j` to return to Neo4j.

---

## 9. Files touched

`backend/Cargo.toml` · `docker-compose.yml` · `.env.example` ·
`backend/src/graph/{config,client,schema,knowledge_builder,agent_memory_graph,graph_retriever,petgraph_runtime,mod}.rs` ·
`backend/src/api/{graph_routes,mod}.rs` · `backend/src/main.rs` · `CLAUDE.md`
