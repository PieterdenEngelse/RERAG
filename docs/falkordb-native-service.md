# FalkorDB as a Native systemd User Service

**Status:** Implemented and verified — `falkordb.service` runs natively (`redis-server` + `falkordb.so` on `127.0.0.1:6380`, ~32 MB RSS); a test ingest landed nodes in FalkorDB (Step 7 done).
**Depends on:** the code migration in [`falkordb-migration.md`](./falkordb-migration.md) (complete; `cargo check`/clippy clean)
**Scope:** Swap FalkorDB's deployment shell from a Docker container to a native
systemd **user** service. Multi-file: `docker-compose.yml`, two systemd units,
host binary setup, docs.

---

## 1. Why

Running FalkorDB as a `falkordb.service` user unit (rather than a compose
service) gives concrete control that matters on this single box:

- **Real dependency ordering** — `ag.service` can declare `Wants=` / `After=`
  `falkordb.service`; systemd starts and orders them as one unit. A compose
  container's lifecycle sits outside systemd, so `ag.service` cannot depend on it.
- **One pane of glass** — `systemctl --user`, `journalctl --user -u falkordb`,
  `systemd-cgtop` — the same tooling already used for `ag.service` and Ollama.
- **One less daemon in the path** — no dependency on `dockerd` being healthy.
- **cgroup limits in the unit file** — `MemoryMax=`, `CPUQuota=` (consistency
  with how `ag`/Ollama are tuned, not a new capability).
- **Consistency** — Ollama already runs as a user service; FalkorDB in a
  container was the odd one out.

**The code migration is untouched.** `ag` connects to `redis://localhost:6380`
either way — nothing in `src/`, `Cargo.toml`, or `.env` changes, and the
verified `cargo check` stays valid.

---

## 2. The crux (and the real risk): host binaries

Native FalkorDB needs two things the host almost certainly lacks — the cache
"redis" is itself a container, so there is likely no host `redis-server`:

- a `redis-server` binary
- the `falkordb.so` module, built against that exact Redis version

**Recommended acquisition** — extract *both* from the `falkordb/falkordb`
image, where they are a guaranteed-compatible pair:

```bash
docker create --name fdb-extract falkordb/falkordb
docker cp fdb-extract:<path>/redis-server  ~/.local/bin/
docker cp fdb-extract:<path>/falkordb.so   ~/.local/lib/
docker rm fdb-extract
```

**Risk to verify first:** if the image is Alpine/musl-based, the extracted
binaries will not run on a glibc host. **Execution step 1 is to confirm the
image base and that the extracted `redis-server` actually starts.** If it does
not: fallback to a distro `redis-server` package + a matching module build (or
build FalkorDB from source).

---

## 3. Steps

- [x] **1. Verify binary acquisition.** `redis-server`, `redis-cli`, and
  `falkordb.so` live under `~/.local/share/ag/falkordb/` and run on the host.
- [x] **2. `~/.config/systemd/user/falkordb.service`** — unit created.
  `ExecStart` runs `redis-server --loadmodule …/falkordb.so --port 6380
  --requirepass agpassword123 --appendonly yes --dir …/data`; `Restart=on-failure`,
  `MemoryMax=512M`.
- [x] **3. Data dir** — `~/.local/share/ag/falkordb/data/` holds the RDB/AOF.
- [x] **4. `ag.service` dependency** — drop-in `ag.service.d/falkordb.conf` adds
  `Wants=falkordb.service` + `After=falkordb.service`.
- [x] **5. `docker-compose.yml`** — `falkordb` service, `falkordb-data` volume,
  and header comments removed.
- [x] **6. Docs** — container-specific parts of `falkordb-migration.md` (header,
  §7, §8 rollback) reconciled to point here.
- [x] **7. Verify** — service starts cleanly; `ag` logs `Successfully connected
  to FalkorDB`; a test ingest (`CLAUDE.md`) landed 1 Document / 24 Chunks /
  1 Entity / 25 relationships in graph `ag`, cross-checked via `GRAPH.QUERY`.

---

## 4. Open decisions (recommendations)

| Decision | Recommendation |
|---|---|
| Port | Keep **6380** — no `.env` / `ag.env` change needed. |
| FalkorDB Browser | **Skip** — native service stays lean; run the Browser separately later if the UI is wanted. |
| `visual-cypher` compose service | **Resolved — dropped** from `docker-compose.yml` (Neo4j-era custom UI, no longer wanted). The `visual-cypher-builder/` source dir is left intact. |
| Keep the container as a fallback? | **Remove it entirely** — this doc records how to bring it back. |

---

## 5. Honest trade-off

The container "just works" today — smoke-testable in ~5 minutes. The native
service costs the binary-acquisition step up front, plus the small risk it
needs the fallback route. After that, the control/consistency wins in §1 apply.

---

## 6. Rollback

Everything here is reversible without touching application code:
- Re-add the `falkordb` service to `docker-compose.yml` (see
  `falkordb-migration.md` git history).
- `systemctl --user disable --now falkordb.service` and remove the unit +
  drop-in.
- The graph data is rebuildable from source documents via re-ingest regardless.
