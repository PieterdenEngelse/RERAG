# Runtime Persistence & Hot Reload — generic design

> **Problem this solves.** ag currently reads each of its ~180 env vars once at
> startup. When the UI wants to flip one of those values at runtime, the only
> path today is "rewrite the env file → restart the process." That assumes a
> systemd-managed deployment with a writable env file and the ability to call
> `systemctl --user restart`. It does not work for an exe/bin distribution, a
> plain `cargo run`, or a container. We need a generic mechanism that works
> across deployments and avoids restarts whenever possible.

## Surface area (the actual numbers)

| Thing | Count / Where |
|------|---------------|
| Unique env vars read | 182 |
| Read sites in backend | 250 |
| Lifecycle shell-outs (`systemctl`, `docker compose`, `journalctl`) | `api/monitor_routes.rs`, scattered detection code, the upcoming L3 toggle |
| Existing typed-config structs | e.g. `graph::config::GraphConfig::from_env()` — read once, never refreshed |
| Existing data-dir owner | `path_manager.rs` — `<base_dir>` defaults to `~/.local/share/ag` (override `AG_HOME`) |

The base directory already exists and is owned by `PathManager`. That is the
natural home for runtime overrides — same directory as ag's other persistent
state (`db/`, `index/`, `cache/`, etc.).

## Three-layer architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Layer 3 — Capabilities                                       │
│    Detected once at boot. "What can this deployment do?"      │
│    can_restart_self · can_manage_compose · can_view_journal · │
│    can_write_systemd_env · managed_compose_file · …           │
│    Frontend fetches /runtime/capabilities, hides UI it can't  │
│    back up with action.                                       │
└──────────────────────────────────────────────────────────────┘
                            ▲
┌──────────────────────────────────────────────────────────────┐
│  Layer 2 — Hot-reload protocol                                │
│    Subsystems subscribe to keys they care about; on change,   │
│    they rebuild their handle and ArcSwap::store it.           │
│    Boot-only keys return RestartRequired{reason} and the UI   │
│    shows a "pending restart" banner.                          │
└──────────────────────────────────────────────────────────────┘
                            ▲
┌──────────────────────────────────────────────────────────────┐
│  Layer 1 — Settings store                                     │
│    effective(key) = override ?? env ?? None                   │
│    Overrides persist to <base_dir>/overrides.json (atomic).   │
│    The env file is never modified by the running app.         │
└──────────────────────────────────────────────────────────────┘
```

Each layer is independently useful. Layer 1 alone fixes "rewriting ag.env".
Layer 2 alone fixes "we have to restart to apply changes". Layer 3 alone fixes
"the UI offers buttons that fail in this deployment".

---

## Layer 1 — Settings store (`backend/src/settings.rs`)

Single source of truth for any config value that can change at runtime.

```rust
pub struct Settings {
    overrides: ArcSwap<HashMap<String, String>>,    // hot path: lock-free reads
    path: PathBuf,                                   // <base_dir>/overrides.json
    write_tx: mpsc::Sender<WriteRequest>,            // single writer task
    listeners: RwLock<HashMap<String, Vec<Listener>>>,
}

impl Settings {
    pub fn load(path: PathBuf) -> Arc<Self>;

    // Effective value: override → env → default
    pub fn effective(&self, key: &str) -> Option<String>;
    pub fn effective_or(&self, key: &str, default: &str) -> String;
    pub fn effective_bool(&self, key: &str, default: bool) -> bool;
    pub fn effective_u64(&self, key: &str, default: u64) -> u64;

    // Mutations (async because they persist + notify)
    pub async fn set(&self, key: &str, value: Option<String>) -> Result<()>;
    pub async fn reset(&self, key: &str) -> Result<()>;  // alias for set(_, None)

    // Hot reload
    pub fn subscribe(&self, key: &str,
                     handler: impl Fn(&str) + Send + Sync + 'static);

    // Introspection (for UI)
    pub fn snapshot(&self) -> SettingsSnapshot;
}

pub struct SettingsSnapshot {
    pub entries: Vec<SettingEntry>,
}
pub struct SettingEntry {
    pub key: String,
    pub env_value: Option<String>,
    pub override_value: Option<String>,
    pub effective: Option<String>,
    pub source: Source,            // Override | Env | Default
    pub restart_required: bool,    // true if this key has no subscriber and is boot-bound
}
```

### Persistence rules

- Overrides live in `<base_dir>/overrides.json` — same directory ag already owns.
- Format: a flat `{ "KEY": "value", ... }` JSON object. No nesting, no types — values are strings, parsed by the consumer (same contract as env vars).
- Writes go through one background task. The task writes to `overrides.json.tmp`, fsyncs, then renames over `overrides.json`. Atomic.
- The env file (`.env`, `~/.config/ag/ag.env`, etc.) is **never** written to by the running app. Those files are install-time defaults — the boundary is clear.
- If `overrides.json` is missing or malformed at startup, ag logs a warning and continues with no overrides.

### Migration shape

```rust
// Before:
let enabled = std::env::var("REDIS_ENABLED")
    .map(|v| v == "true" || v == "1")
    .unwrap_or(false);

// After:
let enabled = settings.effective_bool("REDIS_ENABLED", false);
```

Identical default behavior when no override is set. Migration can proceed file
by file with no breakage.

---

## Layer 2 — Hot-reload protocol

Subsystems hold their mutable state behind `ArcSwap<T>` and subscribe to the
keys that drive that state. No global event bus, no async channels in the hot
path.

```rust
// In the retriever's constructor:
let cache: Arc<ArcSwap<Option<Arc<RedisCache>>>> =
    Arc::new(ArcSwap::from_pointee(initial_cache()));

settings.subscribe("REDIS_ENABLED", {
    let cache = cache.clone();
    move |val| {
        let enabled = val == "true" || val == "1";
        let new = if enabled { Some(build_redis_cache()) } else { None };
        cache.store(Arc::new(new));
    }
});
```

Readers do `cache.load()` — lock-free, cheap, sees the latest swap.

### Categorising the 182 keys

| Category | Strategy | Examples |
|----|----|----|
| **Boot-only** (bound at startup, cannot meaningfully change live) | Stay as `env::var`. Subscribers, if any, return `RestartRequired { reason }`; UI surfaces it. | `BACKEND_HOST`, `BACKEND_PORT`, `AG_HOME`, `AG_DATA_DIR`, `ONNX_MODEL_PATH`, `PDFIUM_LIBRARY_PATH` |
| **Hot-swappable handle** (a connection or background worker) | Subscribe, rebuild handle, `ArcSwap::store`. | `REDIS_ENABLED`, `FALKOR_ENABLED`, `FILE_WATCHER_ENABLED`, `OTEL_TRACES_ENABLED`, `DOCLING_ENABLED`, `RUST_LOG` (via `tracing_subscriber::reload`) |
| **Re-read on use** (numeric knob, no resource attached) | No subscription. Reader calls `settings.effective_u64(...)` each time. | `CHUNK_TARGET_SIZE`, `CHUNK_MAX_SIZE`, `CHUNK_OVERLAP`, `RATE_LIMIT_*`, `INFERENCE_MAX_CONCURRENT_*`, `ENTITY_*_THRESHOLD` |

The third category is the cheapest to migrate — it's one function call swap and
the value becomes runtime-tunable for free.

### Subsystem reload examples

| Subsystem | Keys it owns | Reload action |
|----|----|----|
| L3 cache | `REDIS_ENABLED`, `REDIS_URL`, `REDIS_TTL` | Drop and recreate the `RedisCache` handle; readers transparently see the swap. |
| FalkorDB | `FALKOR_ENABLED`, `FALKOR_URI`, `FALKOR_PASSWORD`, pool sizes | Drain the connection pool; rebuild with new config. |
| Tracing | `RUST_LOG`, `OTEL_TRACES_ENABLED`, `OTEL_EXPORTER_OTLP_ENDPOINT` | `tracing_subscriber::reload::Handle` reloads the filter; OTel exporter recreated on endpoint change. |
| File watcher | `FILE_WATCHER_ENABLED`, `FILE_WATCHER_DEBOUNCE_MS` | Stop the notify task; start a new one with the new debounce. |
| Chunker | `CHUNKER_MODE`, `CHUNK_TARGET_SIZE`, etc. | Re-read on each chunk call; no resource to recycle. |
| Rate limiter | `RATE_LIMIT_*` | Re-read on each request; bucket capacities are looked up live. |

### Pending-restart UX

If a UI override targets a boot-only key, the API returns `202 Accepted` with
`{ effective_immediately: false, restart_required: true, reason: "..." }`. The
frontend renders an inline "restart required to apply" pill next to that
setting and an "Apply (restart)" button that calls
`/runtime/actions/restart-self`. That endpoint is universally available — see
the next section.

---

## Layer 3 — Capabilities (`backend/src/capabilities.rs`)

One startup detector. Cached for the process lifetime. Honest about what this
deployment can do. The capability layer only covers actions that touch
*external* resources — self-restart is universal and not gated (see "Self
re-exec" below).

```rust
pub struct Capabilities {
    pub deployment_mode: DeploymentMode,    // Systemd | DockerCompose | Bin | Container | Unknown
    pub can_manage_compose: bool,           // docker compose found + compose file present
    pub can_view_journal: bool,             // journalctl --user works
    pub managed_compose_file: Option<PathBuf>,
    pub managed_service_name: Option<String>,
}

pub enum DeploymentMode {
    Systemd,         // running under systemd --user, ag.service unit detected
    DockerCompose,   // bundled compose file present, running ag locally
    Bin,             // bare binary, no systemd, no compose
    Container,       // /.dockerenv exists, running inside a container
    Unknown,
}

impl Capabilities {
    pub fn detect() -> Self;     // synchronous, runs during ag startup
}
```

Detection is `Command::new(...).output()` plus a few path checks. Cached in an
`Arc<Capabilities>` injected into the route layer. `deployment_mode` is
informational only — for telemetry and UI labels — it never gates behavior.

### Self re-exec — universal restart

ag restarts itself by replacing its own process image. No systemd, no docker,
no supervisor required. Every deployment can apply boot-bound settings.

```rust
// backend/src/lifecycle.rs

/// Drain the HTTP server, then `execve` the same binary with the same argv.
/// Returns only on failure; on success this function never returns.
pub async fn restart_self() -> Result<std::convert::Infallible, LifecycleError>;
```

Flow:

1. Receive request. Set a "draining" flag the health endpoint reports.
2. Stop accepting new HTTP connections (`Server::handle().stop(true)`).
3. Wait for in-flight requests to finish, capped by a short grace period.
4. Flush in-memory state that the new process can't reconstruct (search-cache
   snapshot, monitoring counters that aren't already journaled).
5. On Linux/macOS: `std::os::unix::process::CommandExt::exec(argv[0], argv)` —
   the kernel replaces the process image; PID is preserved; the new ag boots
   and reads `overrides.json` on the way up. On Windows: spawn a new process
   with the same argv and exit; a tiny wrapper handles socket handoff.

Why this works in every deployment:

| Deployment | What happens |
|---|---|
| **bin/exe** | Binary replaces itself in place. ~2–5s of unavailability, then back up. No supervisor needed. |
| **systemd** | `execve` happens inside the existing cgroup and unit. systemd sees the PID change but the service stays "active" the entire time. We never have to talk to `systemctl`. |
| **container** | If ag is PID 1 (standard pattern), `execve` swaps the container's entrypoint without exiting the container. The container's lifecycle is undisturbed. |

The old systemd-only restart path (`systemctl --user restart ag.service` plus
the transient `ag-l3-toggle-*` orchestration unit) is removed entirely. One
restart mechanism, all deployments.

### Lifecycle module (`backend/src/lifecycle.rs`)

The lifecycle module owns all process and resource lifecycle actions. Self-
restart is universal; external-resource actions are capability-gated. No route
handler shells out to `systemctl`, `docker`, or `journalctl` itself.

```rust
pub async fn restart_self() -> Result<std::convert::Infallible, LifecycleError>;

pub async fn start_managed_container(caps: &Capabilities, name: &str)
    -> Result<(), LifecycleError>;
pub async fn stop_managed_container(caps: &Capabilities, name: &str)
    -> Result<(), LifecycleError>;
pub async fn view_journal(caps: &Capabilities, unit: &str, lines: usize)
    -> Result<String, LifecycleError>;

pub enum LifecycleError {
    NotSupportedInDeployment { reason: String },  // capability missing
    Failed { stderr: String },                    // command ran but failed
    ExecFailed { errno: i32 },                    // self re-exec only
}
```

`restart_self` takes no capability argument because it has none to check —
every deployment can re-exec. The external-resource actions still consult
capabilities and return `NotSupportedInDeployment` when the relevant resource
isn't manageable from inside ag (e.g. `stop_managed_container("redis")` in a
bin/exe deployment that doesn't ship with a compose file).

---

## HTTP surface

```
GET  /runtime/settings              → SettingsSnapshot
PUT  /runtime/settings/:key         → { value: String } | { value: null }
DEL  /runtime/settings/:key         → clear override

GET  /runtime/capabilities          → Capabilities

POST /runtime/actions/restart-self
POST /runtime/actions/start-container/:name
POST /runtime/actions/stop-container/:name
GET  /runtime/actions/journal?unit=…&lines=…
```

Each action endpoint returns:
- `202 Accepted` for asynchronous actions (`restart-self` — the response
  always commits to the restart; the connection is closed as ag drains).
- `200 OK` for synchronous actions that completed successfully.
- `503 NotSupportedInDeployment` with `{ reason }` for capability-gated
  actions when the capability is absent. **`restart-self` never returns
  503** — it is universal.
- `500 Failed` with `{ stderr }` if the command ran but failed.

The frontend fetches `/runtime/capabilities` once at app load into a global
`use_context::<Signal<Capabilities>>` and reads it from any component. Only
the *external-resource* lifecycle controls render conditionally; the
restart-self button is always available:

```rust
// Always shown — every deployment can re-exec itself.
button { onclick: restart_self, "Restart to apply" }

// Conditional — only deployments where ag manages the compose stack.
if caps.can_manage_compose {
    checkbox { "Also stop the redis container" }
}
```

---

## Migration plan

The whole point of designing this generically is that we land it once and then
adopt it incrementally. There is no big-bang migration.

1. **Plumbing PR.** Add `settings.rs`, `capabilities.rs`, `lifecycle.rs`, plus
   the four HTTP routes and the frontend hooks (`use_capabilities()`,
   `use_setting(key)`). Nothing in the existing code uses them yet. The new
   modules ship dormant. **~1 day.**
2. **L3 toggle becomes the first consumer.** Delete
   `set_redis_enabled_in_env_file`, `spawn_l3_toggle_orchestration`, and the
   `ag-l3-toggle-*` systemd-run scope. The toggle becomes
   `settings.set("REDIS_ENABLED", …)`; the retriever's subscriber swaps the
   cache handle. Container ops route through
   `lifecycle::stop_managed_container("redis")`, gated on
   `can_manage_compose`. No restart needed. The `systemctl --user restart
   ag.service` path disappears entirely — replaced by `restart_self` (self
   re-exec) for the rare boot-bound case. **~half day.**
3. **Convert callers opportunistically.** Each time a setting gets a UI toggle
   or sees a bug filed against its env-var-only behavior, switch that one
   call-site to `settings.effective_*(...)`. Don't preemptively rewrite the
   other 180+ — most of them have never been touched at runtime.
4. **Document the keys that are runtime-tunable.** As subsystems opt in, list
   their keys in `docs/runtime-config.md` with a short note on how the value
   takes effect (subscribed, re-read, restart-required).

---

## Making it complete for binary distributions

The three layers above give bin/exe users the same setting-mutation API as
systemd users — settings persist, hot-reload, and apply across deployments.
*Settings themselves* are already universally settable. But two practical
gaps remain that bite specifically in a binary-app context, and a third is
worth listing as a soft follow-up. These are part of the core design, not
optional polish.

### 1. Discoverability — a minimal known-keys registry

A binary user does not have `.env.example` open in another window and may not
have read the docs. The UI has to show *what* is tunable, not just accept
arbitrary keys.

The previously-described `GET /runtime/settings` returns only keys read by
some code path so far. That misses cold paths — graph settings if FalkorDB is
disabled, OCR settings if no PDF has triggered OCR yet, etc. The fix is a
minimal registry of known keys: one line per key, just enough metadata to
render a sensible UI control.

```rust
// backend/src/settings/registry.rs
pub struct KnownKey {
    pub key: &'static str,
    pub description: &'static str,
    pub kind: Kind,                // Bool | U64 | F64 | String | Enum(&[&str]) | Path | Url
    pub default: Option<&'static str>,
    pub category: &'static str,    // for UI grouping: "cache" | "graph" | "chunker" | …
    pub restart_required: bool,
}

pub static KNOWN_KEYS: &[KnownKey] = &[
    KnownKey {
        key: "REDIS_ENABLED",
        description: "Enable the persistent L3 cache.",
        kind: Kind::Bool, default: Some("false"),
        category: "cache", restart_required: false,
    },
    KnownKey {
        key: "CHUNK_TARGET_SIZE",
        description: "Target chunk size in tokens.",
        kind: Kind::U64, default: Some("512"),
        category: "chunker", restart_required: false,
    },
    // … one line per known key
];
```

This is the *minimal* form of the typed registry listed in "Future
extensions" — no validators, no parsers, no unit metadata, just the bare
minimum the UI needs. Filling out all 182 known keys is mechanical work, not
a design problem.

`GET /runtime/settings` then returns:

- All registry entries with their env value, current override, effective
  value, source, category, kind, and `restart_required` flag.
- Plus any *unregistered* keys that already have an override or have been
  read at runtime, marked `registered: false` — so the registry can be
  completed incrementally without losing visibility into stragglers.

The UI groups by category and renders kind-appropriate controls (checkbox
for `Bool`, number input for `U64`, drop-down for `Enum`, file picker for
`Path`). For a binary user this turns "what can I configure?" from "go read
.env.example" into a browsable, grouped page.

### 2. Boot-failure recovery — last-known-good overrides

This is the gap that bites hardest in bin/exe.

In a systemd deployment, a bad override (`BACKEND_PORT=80` for a non-root
user, `AG_DATA_DIR=/root/unwritable`, an `ONNX_MODEL_PATH` that no longer
exists, …) breaks startup, but the user can `journalctl`, edit the env file
by hand, and restart. A binary user has no UI to revert from — ag isn't up
— and probably less terminal expertise. They need an automatic safety net.

The pattern is "last known good overrides," modelled on Windows' boot
recovery: a single marker file records that startup is in progress and is
cleared once the new process has proven healthy. If the marker is found at
the next startup, the previous boot crashed before reaching healthy and the
overrides are rolled back automatically.

```text
On startup:
  1. If <base_dir>/overrides.boot.marker exists:
       — Previous boot did not reach "healthy".
       — Rename overrides.json → overrides.json.bad-<timestamp>.
       — Boot with no overrides applied. Log the rollback.
       — Record the rollback so /runtime/settings returns:
            { rolled_back_at: <ts>, last_bad_file: "overrides.json.bad-…" }
         and the UI shows a banner: "previous overrides caused a boot
         failure and were rolled back — review them here." The UI can open
         the .bad file and let the user re-apply individual keys after
         inspection.
  2. Write the boot marker (atomic create).
  3. Apply overrides and continue startup.

After serving the first /healthz response successfully (or after N seconds
of uptime, configurable):
  4. Delete the marker. From this point the boot is "known good".
```

Properties:

- **No daemon, no supervisor.** Just a file marker.
- **Survives self re-exec.** The new process runs the same startup path and
  the same check, so the safety net covers every restart pathway.
- **Detects the case the binary user actually hits.** "I changed a setting
  and now nothing starts" becomes "you changed a setting, that boot failed,
  here's the bad set ready to review."
- **Bad overrides are preserved**, not deleted. The user (or the UI) can
  cherry-pick the safe ones back.
- **Cheap.** ~30 lines plus one HTTP-state field.

A future refinement is per-key rollback via a small change journal
(`overrides.log` records each `set` with timestamp + old + new; recovery
undoes just the last change instead of clearing everything). Listed in
"Future extensions"; not required for v1.

### 3. Per-key validation feedback (soft follow-up)

Bad values currently fail at the call site with whatever error the consumer
happens to throw. A binary user would benefit from "expected u64, got
'fast'" at submit time. The `Kind` enum in the known-keys registry is
enough to do a parse check before persisting — one match arm per kind. Not
required for v1 (boot-failure recovery handles the worst case), but cheap
once the registry exists.

### What these add to the migration plan

Slot in between current steps 1 and 2:

- **1a.** Land the known-keys registry skeleton (a handful of entries to
  prove the shape; the rest can be filled in incrementally as subsystems
  migrate).
- **1b.** Add the boot-marker recovery to startup. ~30 lines and a small
  field on the settings snapshot.

Steps 2–4 stay as written. The validation hook (3 above) lands whenever the
registry is fleshed out enough to be useful.

---

## Trade-offs

The exclusions below are not a uniform list — they fall into three honest
categories. Calling them all "what this doesn't do" obscures which are real
limitations and which are deliberately better-by-design.

### Reality limits — no implementation can dodge these

- **Not every env var can be live-reloadable without a restart.** Boot-bound
  values stay boot-bound; the design surfaces that honestly via
  `restart_required: true` and provides a universal self re-exec to apply
  them with a few seconds of unavailability.
  **Why:** Some values get baked into long-lived objects at startup — the
  HTTP listener is bound to a port; the ONNX session is loaded against a
  model path; the SQLite connection points at one DB file. Hot-swapping
  those means tearing down and rebuilding the entire object graph, which is
  functionally identical to restarting the process but with more bugs. Any
  system would have to either restart or lie about effectiveness — this is a
  limitation of the world, not of the design. The mitigation is that
  "restart" no longer means "depends on systemd" — `restart_self` works in
  bin, systemd, container, anywhere.

### Liftable limitations — accepted for now, can be added later

These are real limitations relative to a fancier system. We're accepting them
to ship the layering first; each can be lifted incrementally without redoing
the foundation.

- **No typed schema / registry.** Values stay as strings; consumers parse on
  read, same contract as today.
  **Why:** Writing a `Setting<T>` entry for each of 182 keys — default,
  parser, description, validation, units — is real work for keys that may
  never get UI exposure. Until a registry exists, runtime-settings UI will be
  mostly text inputs and checkboxes (no enum drop-downs, no range sliders
  with units). Once 20–30 keys have actually been migrated through real UI
  work, we'll know what the registry should look like and which fields earn
  their keep. Designing it before that data exists is premature abstraction.

- **No atomic group swaps.** Each key changes independently; if a subsystem
  wants a stable snapshot of several keys, it reads them all at the same
  point and stores its own copy.
  **Why:** Group atomicity needs either a transactional store or a barrier
  across subscribers — both real complexity. The visible failure mode is "one
  request runs against mixed old/new values for a few milliseconds." Fine
  for the knobs we have (chunk size, rate-limit thresholds); would matter for
  safety-critical groups. Liftable later with a `set_many` API + a barrier
  if a real use case appears.

- **Best-effort deployment detection only.** `Capabilities::detect` checks
  that the relevant binary is on PATH and probes succeed; it does not parse
  systemd unit files, talk to dbus, or inspect the docker socket directly.
  **Why:** Capabilities exist to hide impossible UI buttons, not to guarantee
  an action will succeed. The binary-present + probe-succeeds heuristic is
  ~99% reliable; the 1% leftover (binary present but permission denied at
  use) is caught by the action's own error path. Richer detection adds code
  and dependencies and risks converting "button visible but action fails"
  into "button hidden when it would have worked" — net wash or worse.

### Better-by-design — these aren't limitations, they're deliberate

These read like exclusions but adding them would make the system *worse*, not
just larger.

- **The env file is not replaced.** `.env` and `ag.env` remain install-time
  defaults; runtime overrides live in their own file, owned by the running
  app.
  **Why:** Install-time defaults and runtime overrides have different
  audiences and lifecycles. The env file is versioned with the deployment
  (Ansible/Docker/Nix), read by operators, shared across instances. The
  override file is per-instance and changes via UI clicks. Merging them
  raises questions with no good answer: which file wins on restart? Do
  redeploys nuke the user's UI tweaks? Two files with clear ownership
  sidesteps the whole class. This is a separation of concerns, not a
  missing feature.

- **No global pub-sub bus.** Listeners are direct callbacks registered
  against a `RwLock<HashMap<String, Vec<Listener>>>`.
  **Why:** We have ~10–15 subsystems that would subscribe (Layer-2 table).
  At that scale a direct callback table is the right shape. A pub-sub
  framework (typed topics, broadcast channels, fan-out tasks, backpressure
  policy) earns its keep at hundreds of subscribers across crates or
  processes — for us it would be more code, worse latency, no benefit. If a
  future use case (e.g. streaming changes to an external dashboard) needs
  one, the dispatch primitive can be swapped without touching subscribers.

---

## Future extensions (out of scope for the initial landing)

- **Typed setting registry.** Compile-time list of `Setting<T>` with default,
  parser, description, and a `restart_required` flag. Gives the UI better
  metadata (categories, units, ranges) and removes per-callsite type parsing.
- **Audit log.** Every `settings.set` writes a line to
  `<base_dir>/overrides.log` so the user can see who flipped what when.
- **Profiles.** Named bundles of overrides — "fast-dev", "memory-tight",
  "no-graph". One click swaps a whole set of keys.
- **Export / import.** Dump the current override file as a `.env` snippet so
  the user can promote a runtime-tuned configuration into an install default.
- **SIGHUP support.** Re-detect capabilities and re-read `overrides.json` on
  signal — useful when the user installs the systemd unit *after* first run.

---

## Related design: bringing containers inside ag

The persistence/restart layers above let the UI flip settings cleanly in any
deployment, but they still have to *manage* external resources (start/stop a
docker container, watch a system service) in deployments where those exist.
A complementary design question is whether any of those external resources
should stop being external at all — i.e. brought *inside* the ag binary.

### Which of ag's containers are candidates?

ag's bundled stack ships several containers; they're not equivalent.

| Container | Role | Embed in ag? | Why |
|---|---|---|---|
| `redis` (L3 cache) | Persistent KV with TTL | **Yes — recommended.** | Single-box use case, no external readers, no multi-instance sharing — exactly what a pure-Rust embedded KV is for. |
| `prometheus`, `loki`, `tempo`, `otel-collector` | Telemetry storage + transport | **Yes — recommended for the common case.** | All four solve problems ag doesn't have at its scale (multi-source aggregation, long retention, transport between services). In-process ring buffers cover ag's actual use: "recent metrics / logs / traces for the one service that's emitting them." Heavy external stack stays opt-in for users who need long retention or multi-service aggregation. |
| `grafana` | Generic visualization UI | **No — its role is already filled.** | ag's Dioxus monitor pages already render charts. The thing missing today is the data behind the panels, not the panels themselves. With the in-process buffers above, the existing monitor UI grows to cover what users went to Grafana for. |
| `falkordb` | Graph database (Cypher, AOF, multi-graph) | No | A full graph server isn't a library you embed. `petgraph_runtime` already exists as the in-process fallback for read-only paths; full FalkorDB stays external. |

### L3 as an embedded persistent KV — the headline move

The L3 cache's design goal is "survive ag restarts." Today that's achieved by
running it in a separate process (the redis container). But L3 in ag is
strictly per-instance — nothing else reads that cache, no other ag node shares
it. So the separate process buys persistence at the cost of:

- a docker dependency,
- a compose file,
- a port (or socket),
- connection pooling and health probing,
- a "start/stop container" UI control that's deployment-gated,
- ~10–30 MB of resident memory for `redis-server`.

A pure-Rust embedded KV — **`redb`** is the obvious choice (MIT, single-file,
ACID, actively maintained; `sled` is the older option but is no longer
recommended for new code) — gives the same persistence with none of the above.

```rust
// backend/src/cache/embedded_kv.rs
pub struct EmbeddedKv {
    db: redb::Database,                    // <base_dir>/cache.redb
    sweeper: tokio::task::JoinHandle<()>,  // background TTL eviction
}

impl EmbeddedKv {
    pub fn open(path: &Path, default_ttl: Duration) -> Result<Self>;
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    pub fn set(&self, key: &[u8], value: &[u8], ttl: Option<Duration>);
    pub fn delete(&self, key: &[u8]);
    pub fn stats(&self) -> CacheStats;
}
```

TTL is handled in-process: each value stores its expiry alongside the bytes,
the sweeper runs every N seconds and deletes anything past expiry. Reads stay
sub-millisecond (no socket, no serialisation round-trip).

### What goes away

- The `redis` service in `docker-compose.yml` (and the host-port override).
- All the L3 URL/password/pool plumbing in `RedisCache`, the `redis://` parsing
  in `monitor_routes.rs::sanitize_redis_url`, the connection-manager timeouts.
- The "Also start/stop the redis container" checkbox and its info modal —
  there is no container to manage.
- The container-management half of `lifecycle.rs` for the L3 path. `lifecycle`
  keeps `start/stop_managed_container` for the optional observability stack
  but the common case no longer touches it.
- The `redis` Cargo dependency *for the cache role*. It stays for the
  FalkorDB protocol client.

### What stays

- The settings + hot-reload layers from this doc — still the right way to flip
  L3 on/off at runtime. The subscriber action becomes "open or drop the
  `EmbeddedKv` handle," same shape as today's "build or drop the
  `RedisCache`."
- The capabilities layer — still needed for the observability stack and any
  other genuinely external resources that remain optional.
- FalkorDB as an external service (or system unit) — it's not a candidate for
  embedding.
- The L1 / L2 / L3 tiering. L3 is still the persistent tier; only its
  *implementation* changes from out-of-process redis to in-process `redb`.

### Net effect on deployment

The bin/exe distribution becomes one binary plus its data directory. No
docker required for core retrieval. Observability is a documented opt-in for
users who want it; FalkorDB is a documented dependency for users who want the
knowledge graph. Combined with the universal self re-exec from Layer 3, every
runtime-tunable setting can be set, persisted, and applied without any
deployment-specific machinery — settings stay universal, and the only
genuinely external pieces left are the optional ones.

### Trade-offs of the L3 move

- **No Redis compatibility.** If anything outside ag ever needs to read the
  L3 cache, this closes that door. Today nothing does, but it's a constraint
  worth naming.
- **No "shared cache across multiple ag instances."** ag is single-box, so
  this never mattered — but if a future multi-node deployment shape appears,
  it would have to bring redis (or memcached, or a remote KV) back as an
  opt-in alternative backend behind the same `L3Cache` trait.
- **One more on-disk file format to maintain compatibility for.** `redb` has
  a stable on-disk format; migrations between major versions are documented.
  Same posture as Tantivy index versions.
- **Binary size grows slightly.** `redb` is small (~200 KB); negligible
  against ag's current binary size.

### Migration sketch

1. Introduce `cache::EmbeddedKv` next to the existing `RedisCache`. Both
   implement the same `L3CacheBackend` trait.
2. Add a `L3_BACKEND` setting with values `embedded` (default for new
   deployments) and `redis` (back-compat for existing setups). Use the
   runtime-settings layer to switch backends without restart.
3. Default new deployments to `embedded`. Existing deployments keep `redis`
   until explicitly migrated.
4. After a quiet period: drop the `redis` backend and the container from the
   default compose file. The `redis` Cargo dep stays for FalkorDB only.

This is a separate landing from the persistence/runtime-settings PR — it
benefits from that infrastructure being in place but doesn't block on it.

### Observability as in-process buffers — the second move

Today ag emits OTLP to the bundled otel-collector → tempo/loki/prometheus → grafana
stack. Five containers, a config file each, and a separate UI to learn. For
ag's actual scale (one service, recent data, learning-platform UX), the same
information can live in process.

The shape of each piece:

| Today (external) | In-process replacement |
|---|---|
| **Prometheus** scrapes `/metrics` and stores TSDB on disk | An in-process ring buffer of the last N hours (default 24h, configurable), sampled at the existing emit cadence. ~bytes per metric per sample × number of metrics × samples — easily under 10 MB for ag's metric volume. Backed optionally by `redb` for persistence across restarts. |
| **Loki** receives log lines and indexes labels for filtering | A custom `tracing_subscriber::Layer` writes recent events into a ring buffer keyed by level + target. The existing structured-log machinery already produces the right shape; we just route a copy in-process instead of (or in addition to) stdout. |
| **Tempo** receives OTLP spans and stores them for trace queries | A custom span exporter writes into an in-process trace store. Smart sampling: keep all slow / errored traces, sample fast ones. Last N minutes by default, configurable. |
| **otel-collector** receives + fans out telemetry between services | Disappears. With one source (ag) and one destination (the in-process buffers), there is nothing to collect. |
| **Grafana** renders all of the above | ag's existing Dioxus monitor pages grow new panels backed by the buffers above. No external UI to learn. |

What this looks like in code:

```rust
// backend/src/observability/mod.rs

pub struct InProcessTelemetry {
    pub metrics: MetricsHistory,    // ring buffer + optional redb persistence
    pub logs: LogRing,              // tracing Layer feeds this
    pub traces: TraceStore,         // span exporter feeds this
}

// HTTP surface (in addition to the existing /metrics Prometheus endpoint):
//   GET /telemetry/metrics?name=…&from=…&to=…&step=…  → samples
//   GET /telemetry/logs?level=…&target=…&from=…&limit=…
//   GET /telemetry/traces?slow_than_ms=…&from=…&limit=…
//   GET /telemetry/traces/:trace_id                   → full span tree
```

The monitor pages already know how to call ag's backend; they grow new panels
(rate, p50/p95/p99 latency, log tail, slow-trace list) that read from these
endpoints. The look-and-feel matches the rest of the app instead of being a
foreign embedded Grafana iframe.

#### What goes away

- All five observability containers from `docker-compose.yml`.
- The OTLP exporter plumbing for the common case (kept behind a config flag
  for users who still want to ship telemetry out).
- A whole class of "Grafana isn't running" / "OTel collector port conflict" /
  "Tempo retention misconfigured" support issues.
- Five sets of credentials / config files / port mappings to think about.

#### What stays

- `/metrics` in Prometheus format. External Prometheus can still scrape ag
  directly if a user wants the data outside.
- The OTLP exporter as opt-in. Users with an existing observability platform
  flip a setting; ag exports OTLP to their stack the same as today.
- The current emit cadence and metric names. The in-process buffers consume
  the same data the OTel exporter does today — no duplication of measurement
  code.

#### Trade-offs of embedding observability

- **No long-term history out of the box.** External Prometheus retains weeks;
  in-process is bounded by RAM (and optionally `redb` for slightly longer
  windows). Users who care about week-over-week comparisons keep the
  external stack.
- **No mature alerting platform.** Grafana / Prometheus alerting are
  battle-tested; an in-process equivalent would either be a simple rules
  engine (alerts fire to log + UI banner + optional webhook) or simply
  absent in v1. Worth deciding explicitly.
- **No multi-service aggregation.** If ag ever runs alongside other
  services and someone wants one pane of glass, external Grafana is the
  answer. Currently ag is single-service.
- **Real implementation effort.** Larger than L3 — three buffer
  implementations, frontend panels for each, a small query API per buffer.
  Not a weekend, but each piece is bounded and incremental.
- **Loses interoperability with the broader OTel ecosystem for the default
  path.** Mitigated by keeping the OTLP exporter as an opt-in.

#### Migration sketch

1. Build the three in-process buffers (`MetricsHistory`, `LogRing`,
   `TraceStore`) as standalone modules with HTTP endpoints. The OTLP
   exporter still ships data to the bundled stack — buffers run alongside.
2. Add monitor-page panels that read from the new endpoints. Users can
   compare side-by-side with Grafana to validate they show the same numbers.
3. Add a setting `OBSERVABILITY_BACKEND` with values `embedded` (default for
   new installs), `external` (keep emitting OTLP, hide in-process panels),
   `both` (transitional). Runtime-tunable via the settings layer.
4. After validation: default new installs to `embedded`, leave existing
   deployments on whatever they had. Remove the observability containers
   from the default compose file but keep `ops/observability/` available for
   users who set `OBSERVABILITY_BACKEND=external`.

This is independent from the L3 move and from the runtime-settings landing.
The three pieces compose well — settings + restart give the toggle UX, L3
removes one container, observability removes five — but each can land on its
own schedule.

### Costs across all the embeddings together

The per-piece estimates above (small build-time bump, ~200 KB for `redb`,
"a few hundred lines" of buffer code, etc.) are individually accurate but
*cumulatively misleading*. Reading them piece by piece makes the project look
like a free lunch. It isn't. Sized as a whole, the costs are non-trivial —
and they're concentrated in places (engineering surface area, ongoing
maintenance) that per-piece estimates don't surface.

This subsection exists so the next person reading this doc gets the
cumulative picture, not just the per-piece optimism.

#### What grows

- **Net code volume.** The deletions on the way out (compose blocks, the
  `redis://` URL plumbing, the OTLP exporter, port healthchecks, container
  orchestration) are mostly *plumbing*. The additions are *implementations*
  of things that today live in separate mature projects — a persistent KV
  with TTL eviction, a time-series ring buffer with query, a log ring with
  label indexing, a trace store with smart sampling, plus the Dioxus panels
  for each. Even simplified, five new implementations add up. Realistic net
  is more code in ag, not less.

- **Frontend compile time.** Backend stays close to flat if we discipline
  dependencies (no PromQL parser, no time-series compression library, no
  generic query engine — just custom Rust on existing crates). But Dioxus +
  Tailwind compile is already the slow part of the build, and every new
  monitor panel is a new Dioxus component. Cumulative impact: a single-build
  minute or two added, landing in the frontend cycle developers iterate on
  most.

- **Binary size.** Summed: ~1–2 MB across `redb`, the buffer code, and the
  expanded frontend bundle. Not catastrophic against ag's current size, but
  visible.

- **Idle memory.** Ring buffers exist whether or not anyone is looking at
  them. At sensible defaults: ~30–50 MB cumulative for L3 + metrics + logs +
  traces. Configurable, but it's always paid.

- **Conceptual surface area.** ag becomes responsible for code paths in
  domains it doesn't own today: persistent key/value, time-series storage,
  log aggregation, distributed-trace storage, observability UI. New
  contributors have to learn "where do metrics live?", "how is the log ring
  evicted?", "what's the trace sampling rule?" The cost scales with the
  number of new domains, not the number of bytes.

- **Feature-parity drift.** Prometheus, Grafana, Loki, Tempo, and Redis all
  keep evolving. ag's embedded versions diverge from upstream over time.
  Users who got a new histogram type or a new chart variant for free from a
  Grafana upgrade now wait for an ag PR.

- **Test surface.** Each embedded piece needs unit tests, frontend tests,
  and at least one realistic integration scenario. The test suite grows
  proportionally to the number of new domains absorbed.

- **Ongoing maintenance.** Bugs in our ring buffer become *our* bugs, not
  Prometheus's. Security advisories in our embedded code become *our*
  on-call problem. This burden is small per piece (the implementations are
  small) but cumulative across five domains it's a real ongoing cost.

#### What the value side actually is, at the cumulative scale

Per-piece, the wins look modest — "one less container." That framing
under-sells the cumulative effect. Across all the embeddings together, the
change is *qualitative*:

- ag becomes a **single-binary application** that needs zero infrastructure
  setup for its full feature set.
- The default deployment story collapses from "install ag + docker + run
  docker-compose with 6 services + configure 5 telemetry sinks" to "run the
  binary."
- `ops/observability/` and most of `docker-compose.yml` disappear from the
  default path.
- The learning-platform goal — *make the invisible visible, no setup
  friction* — is genuinely better served by in-process observability than by
  asking users to learn Grafana to see what ag is doing.
- New-user onboarding goes from "you need Docker Compose and 6 GB of RAM
  for the stack" to "download a binary and run it."

This is a different product than ag-today, not just a smaller deployment of
the same product. Whether it's a *better* product depends on whether
"single-binary, zero-setup ag" is a stated goal or a nice-to-have.

#### Honest summary

The dominant cost is not compile time, not binary size, not runtime memory —
it's **engineering surface area**. Cumulatively, "absorb every external
container" roughly *doubles* ag's effective scope: ag is no longer "a Rust
RAG backend with a Dioxus monitoring UI" but also "an embedded persistent
KV + an embedded metrics TSDB + an embedded log aggregator + an embedded
trace store + an embedded observability UI."

That's a real expansion. It's worth taking on if and only if the
"single-binary zero-setup" deployment story is genuinely a project goal — in
which case the cumulative move is more valuable than any of the individual
pieces and the doubling of scope is the cost of getting there. If the
single-binary story isn't a goal, do the L3 move (small, contained, clear
win) and leave the observability stack external (where it already works and
is maintained by people who care about it full-time).
