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
