# Windows installer — execution steps

Companion to `docs/wininstall.md`. The plan doc is **what + why**; this
doc is **exactly which keystrokes**. Sub-tasks correspond one-to-one
with the items in the conversation task list (PR1.1 through PR4).

Status legend:
- ✅ **done** in main
- 🟡 **in progress**
- ⬜ **pending**

---

## PR 1 — Refactor: carve out `platform/` boundary

Goal: produce a Linux-equivalent installer where every OS-specific
surface lives behind `crate::platform::{linux,windows}`. Linux behavior
must be byte-for-byte identical — `HOME=/tmp/ag-test SKIP_SYSTEMCTL=1
cargo run` produces the same install-log lines after PR 1 as before.

Scope budget: ~600 LOC of pure code movement, plus ~30 LOC of new
`mod.rs` / `windows.rs` scaffolding. No new dependencies. No backend
edits. No CI edits.

### PR1.1 — Scaffold `installer/src/platform/` ✅

1. Create `installer/src/platform/mod.rs` with cfg-selected re-exports:
   ```rust
   #[cfg(unix)]    mod linux;
   #[cfg(windows)] mod windows;

   #[cfg(unix)]    pub use linux::{ … };
   #[cfg(windows)] pub use windows::{ … };
   ```
   The re-export list starts empty in PR1.1 and grows as PR1.2–PR1.4
   move code in. Final list:
   `Paths, skip_systemctl, run_detection, ensure_install_tree,
   copy_artifacts, install_stack, install_service`.

2. Create `installer/src/platform/linux.rs` — empty (just a module-doc
   comment).

3. Create `installer/src/platform/windows.rs` — same.

4. In `installer/src/main.rs` add `mod platform;` to the module list
   (alphabetical-ish, between `paths` and `prompts`).

Verification: nothing functional — just `cargo check -p ag-installer`
should still pass (it will, because `platform::mod` has no items yet).

### PR1.2 — Move `Paths` into `platform/linux.rs` ✅

1. Open `installer/src/paths.rs`. Copy the entire `Paths` struct + `impl
   Paths` + `pub fn skip_systemctl()` block.

2. Paste into `installer/src/platform/linux.rs` under a `// === Paths
   (PR1.2) ===` heading. Imports needed: `use std::path::PathBuf;`.

3. Replace the body of `installer/src/paths.rs` with the re-export shim:
   ```rust
   //! Path resolution and sandbox-gate helper.
   //!
   //! Historical home of `Paths` + `skip_systemctl`. The real bodies
   //! live under `crate::platform::{linux,windows}`; this file is a
   //! thin re-export so every existing `use crate::paths::{Paths, …}`
   //! call site keeps working without an edit.
   pub use crate::platform::{skip_systemctl, Paths};
   ```

4. In `platform/mod.rs`, extend the re-export list:
   ```rust
   #[cfg(unix)]    pub use linux::{skip_systemctl, Paths};
   #[cfg(windows)] pub use windows::{skip_systemctl, Paths};
   ```

5. In `platform/windows.rs`, add stub `Paths` struct + `skip_systemctl()`
   so the cfg(windows) compile succeeds. Stub fields must include the
   names install_steps.rs touches (for the Linux build path the stub is
   dead code; for the Windows build path it's filled in by PR 2):
   ```rust
   pub struct Paths { pub ag_home: PathBuf, … pub scheduled_tasks_dir: PathBuf, }
   impl Paths {
       pub fn resolve() -> Self {
           unimplemented!("PR 2")
       }
       …
   }
   pub fn skip_systemctl() -> bool {
       std::env::var("SKIP_SCHTASKS").map(|v| !v.is_empty()).unwrap_or(false)
   }
   ```

Verification: `cargo check -p ag-installer` on Linux still passes. All
existing `use crate::paths::Paths` sites continue to resolve via the
shim.

### PR1.3 — Move detection probes into `platform/linux.rs` ✅

1. Open `installer/src/detection.rs`. Identify the keep/move split:
   - **Stay in detection.rs**: `pub const BACKEND_PORT`, `pub struct
     DetectionResult`, `#[cfg(test)] mod tests` (the
     `print_real_result` ignored test).
   - **Move to platform/linux.rs**: every `probe_*` fn, `pub async fn
     run`, `systemctl_user_is_active`, `xdg_config_dir`.

2. In `platform/linux.rs`, rename the moved `run` to `run_detection` so
   `platform::run_detection` is the public name (the orchestrator sits
   above the per-platform impl).

3. Replace `detection.rs::run`'s body with a thin wrapper:
   ```rust
   pub async fn run() -> DetectionResult {
       crate::platform::run_detection().await
   }
   ```
   The `ui/detection_screen.rs:22` caller (`detection::run().await`)
   keeps working unchanged.

4. Update `DetectionResult` field docs so they document **both**
   platforms' semantics inline (e.g. `ollama_active: "Linux: systemctl
   --user is-active ollama. Windows: HTTP GET /api/tags responds 2xx."`).
   This keeps the contract clear without proliferating types.

5. Extend `platform/mod.rs` re-exports:
   ```rust
   pub use linux::{run_detection, skip_systemctl, Paths};
   ```

6. Add Windows stub in `platform/windows.rs`:
   ```rust
   pub async fn run_detection() -> crate::detection::DetectionResult {
       unimplemented!("PR 2")
   }
   ```

Verification: `cargo check -p ag-installer` passes. The `[#ignore]`
`print_real_result` test still references `run()` via the wrapper —
which works because `DetectionResult` is unchanged.

### PR1.4 — Move OS-touching `install_steps` bodies into `platform/linux.rs` ✅

This is the largest move; ~470 LOC.

1. In `installer/src/install_steps.rs`, identify the keep/move split:
   - **Stay**: imports, `pub type ProgressSender`, `pub const
     DEFAULT_BACKEND_PORT`, `pub const FALKORDB_PORT`, `pub const
     FALKORDB_PASS`, `pub enum ProgressEvent`, `pub const STEP_NAMES`,
     `pub struct InstallResult`, `LogTee` (made `pub(crate)`), `step_log`
     (made `pub(crate)`), `pub async fn run` (orchestrator + `step!`
     macro), `seed_config`, `health_check`, `render_template` (made
     `pub(crate)`), `edit_env_file`, `set_mode` (made `pub(crate)`).
   - **Move to platform/linux.rs**: `ensure_xdg` → `ensure_install_tree`,
     `install_artifacts` → `copy_artifacts`, `falkordb` →
     `install_stack`, `systemd_step` → `install_service`,
     `systemctl_user` helper.

2. Update the orchestrator's `step!` invocations to delegate:
   ```rust
   step!("Ensure XDG tree", crate::platform::ensure_install_tree(&paths, &tx, &tee, &mut log_path));
   step!("Seed config",      seed_config(&paths, &tx, &tee, &answers, backend_port));
   step!("Install artifacts",crate::platform::copy_artifacts(&paths, &tx, &tee));
   step!("FalkorDB native service", crate::platform::install_stack(&paths, &tx, &tee));
   step!("Systemd user units",      crate::platform::install_service(&paths, &tx, &tee, &answers, backend_port));
   step!("Health check",     health_check(&tx, &tee, backend_port));
   ```

3. In `platform/linux.rs`, add the moved fns under `// === Step N: … ===`
   headings. Imports added:
   ```rust
   use std::fs;
   use std::process::Stdio;
   use anyhow::{bail, Context, Result};
   use chrono::Utc;
   use crate::bundled;
   use crate::install_steps::{
       render_template, set_mode, step_log, LogTee, ProgressSender,
       FALKORDB_PASS, FALKORDB_PORT,
   };
   use crate::prompts::{PromptAnswers, PromptId};
   ```

4. Extend `platform/mod.rs` re-exports to include the four step bodies.

5. Add stubs in `platform/windows.rs` for `ensure_install_tree`,
   `copy_artifacts`, `install_stack`, `install_service` so cfg(windows)
   compiles. Each one is one line of `unimplemented!()`.

Verification: `cargo check -p ag-installer` passes. `cargo fmt` runs
cleanly (it may re-flow braces — accept those changes).

### PR1.5 — Verify Linux regression ✅

1. `cd installer && cargo fmt && cargo clippy --all-targets -- -D warnings`
   — must pass cleanly. Fix any clippy warnings that the move
   introduced (likely just `unused_imports` in detection.rs if any
   probe-specific import got left behind).

2. Sandbox install run:
   ```bash
   HOME=/tmp/ag-test SKIP_SYSTEMCTL=1 cargo run -p ag-installer
   ```
   Walk all six screens. The install log at
   `/tmp/ag-test/.local/share/ag/logs/install-*.log` should show every
   step completing — same step names, same log line content, same
   timing as before the refactor.

3. (Optional) Build the AppImage:
   ```bash
   installer/build-appimage.sh
   ```
   Smoke-test it on this box: `./ag-installer-*-x86_64.AppImage` should
   launch the GUI identically to the pre-refactor build.

4. Commit:
   ```
   refactor(installer): carve out platform/{linux,windows} boundary

   Lifts every OS-specific surface — Paths, detection probes, install-step
   bodies — behind crate::platform. Linux behavior unchanged; Windows
   stubs are unimplemented!() placeholders filled in by PR 2.
   ```

---

## PR 2 — Windows code + backend hook + compose profile

Goal: real Windows installer executable that walks all six screens
end-to-end on a Windows 10/11 box with Docker Compose. No MSI yet —
that's PR 3.

### PR2.1 — Add cfg(windows) deps in `installer/Cargo.toml` ✅

Append:
```toml
[target.'cfg(windows)'.dependencies]
fs2     = "0.4"
sysinfo = "0.32"
winreg  = "0.55"
```

Update the `description` field:
```toml
description = "GUI installer for RERAG — Linux + Windows"
```

Verification: `cargo check -p ag-installer --target
x86_64-pc-windows-msvc` resolves the new crates (run from a Windows VM
or via `cross`; on this Linux box the cfg(windows) block won't activate
so a plain `cargo check` is a no-op).

### PR2.2 — Implement `platform/windows.rs` Paths + detection probes ✅

**Paths.** Replace the PR1 stub:
```rust
pub struct Paths {
    pub ag_home: PathBuf,                // %LOCALAPPDATA%\ag
    pub bin_dir: PathBuf,                // %LOCALAPPDATA%\ag\bin
    pub lib_dir: PathBuf,                // %LOCALAPPDATA%\ag\lib
    pub config_dir: PathBuf,             // %APPDATA%\ag
    pub scheduled_tasks_dir: PathBuf,    // %APPDATA%\ag\scheduled-tasks
}

impl Paths {
    pub fn resolve() -> Self {
        let local = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Default\AppData\Local"));
        let roaming = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Default\AppData\Roaming"));
        let ag_home = std::env::var("AG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| local.join("ag"));
        Paths {
            bin_dir: ag_home.join("bin"),
            lib_dir: ag_home.join("lib"),
            config_dir: roaming.join("ag"),
            scheduled_tasks_dir: roaming.join("ag").join("scheduled-tasks"),
            ag_home,
        }
    }

    pub fn ag_env(&self) -> PathBuf       { self.config_dir.join("ag.env") }
    pub fn docker_compose(&self) -> PathBuf { self.config_dir.join("docker-compose.yml") }
    pub fn ag_start_cmd(&self) -> PathBuf { self.bin_dir.join("ag-start.cmd") }
    pub fn ag_exe(&self) -> PathBuf       { self.bin_dir.join("ag.exe") }
    pub fn ag_task_xml(&self) -> PathBuf  { self.scheduled_tasks_dir.join("ag.xml") }
    pub fn ag_stack_task_xml(&self) -> PathBuf { self.scheduled_tasks_dir.join("ag-stack.xml") }
    pub fn install_log(&self, ts: &str) -> PathBuf {
        self.ag_home.join("logs").join(format!("install-{ts}.log"))
    }
}
```

**Detection probes** (replacing the `unimplemented!()` stub):

| Probe | Implementation |
|---|---|
| `docker_present` | `Command::new("docker").arg("--version")` — identical to Linux. |
| `compose_up` | `Command::new("docker").args(["compose","ls"]).env("COMPOSE_PROJECT_NAME","ag")` — identical to Linux. |
| `ollama_active` | `reqwest::get("http://127.0.0.1:11434/api/tags")` — 2xx = "responding". |
| `falkordb_healthy` | `docker inspect ag-falkordb --format "{{.State.Health.Status}}"` — string match against `"healthy"`. |
| `ag_env_exists` | `tokio::fs::metadata(paths.ag_env())`. |
| `backend_port_busy` | `std::net::TcpListener::bind("127.0.0.1:3010")` — `AddrInUse` = busy. |
| `system_redis` | `TcpStream::connect("127.0.0.1:6379")`, write `*1\r\n$4\r\nPING\r\n`, read 7 bytes, compare to `+PONG\r\n`. |
| `native_obs` | `vec![]` — no native loki/tempo on Windows. |
| `ag_service_drift` (Windows: scheduled-task drift) | `schtasks /Query /TN ag /XML`. If exit 0, parse the `<Command>` element; if it doesn't equal `%LOCALAPPDATA%\ag\bin\ag-start.cmd`, return `true`. |
| `disk_free_gb` | `fs2::available_space(paths.ag_home.parent().unwrap_or(&paths.ag_home)) >> 30`. |
| `ram_gb` | `sysinfo::System::new().total_memory() >> 30`. |
| `distro` | `winreg::RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")` → read `ProductName` + `DisplayVersion`, join with a space. |

Wire them into `pub async fn run_detection() -> DetectionResult` with
the same `tokio::join!` shape as Linux.

Verification: on a Windows VM, `target\…\release\ag-installer.exe`
launches; the Detection screen populates real values (non-zero RAM,
non-zero disk).

### PR2.3 — Implement `platform/windows.rs` install_steps ✅

For each of the four step bodies, replace the `unimplemented!()` stub
with a real implementation:

**Step 1 — `ensure_install_tree`**:
```rust
let dirs = vec![
    paths.bin_dir.clone(),
    paths.lib_dir.clone(),
    paths.config_dir.clone(),
    paths.scheduled_tasks_dir.clone(),
    paths.ag_home.join("data"),
    paths.ag_home.join("index"),
    paths.ag_home.join("db"),
    paths.ag_home.join("logs"),
    paths.ag_home.join("cache"),
    paths.ag_home.join("locks"),
    paths.ag_home.join("web"),
];
// Same dir-create loop + log-open pattern as Linux. No falkordb subdir
// (FalkorDB runs in compose, not as a native binary).
```

**Step 3 — `copy_artifacts`**:
```rust
// ag.exe
fs::copy(bundled::ag_binary_path(), paths.ag_exe())?;

// tika_native.dll (optional)
if let Some(src) = bundled::libtika_path() {
    fs::copy(src, paths.lib_dir.join("tika_native.dll"))?;
}

// Frontend dist — robocopy /MIR /NJH /NJS /NDL /NP (built into Windows).
// Note: robocopy's exit codes are weird; 0..=7 is success.
let status = Command::new("robocopy")
    .args([
        bundled::frontend_dist_dir().to_str().unwrap(),
        paths.ag_home.join("web").to_str().unwrap(),
        "/MIR", "/NJH", "/NJS", "/NDL", "/NP",
    ])
    .status().await?;
let code = status.code().unwrap_or(16);
if code > 7 {
    bail!("robocopy exited {code} (>7 = failure)");
}

// Write ag-start.cmd verbatim — no template substitution.
fs::write(paths.ag_start_cmd(),
    "@echo off\r\nset \"AG_ENV=%APPDATA%\\ag\\ag.env\"\r\nstart \"\" /B \"%~dp0ag.exe\"\r\n")?;

// Smoke-test: run ag.exe --version. PATH includes lib_dir so tika_native.dll resolves.
let out = Command::new(&paths.ag_exe())
    .arg("--version")
    .env("PATH", format!("{};{}", paths.lib_dir.display(), std::env::var("PATH").unwrap_or_default()))
    .output().await?;
if !out.status.success() {
    bail!("ag.exe --version failed: {}", String::from_utf8_lossy(&out.stderr).trim());
}
```

**Step 4 — `install_stack`**:
```rust
let profile = match answers.choice(PromptId::LowRam) {
    Some("none") => return Ok(()), // no stack
    Some("core") => "core",
    Some("observability") => "observability",
    _ => "",
};
let mut args = vec![
    "compose", "-f", paths.docker_compose().to_str().unwrap(),
    "--profile", "falkor-container",
];
if !profile.is_empty() { args.extend(["--profile", profile]); }
args.extend(["up", "-d"]);
let status = Command::new("docker").args(&args).status().await?;
if !status.success() { bail!("docker compose up failed"); }
```

**Step 5 — `install_service`**:
```rust
// Render ag.xml from installer/scheduled-tasks/ag.xml.tmpl.
let tmpl = bundled::scheduled_tasks_dir().join("ag.xml.tmpl");
render_template(&tmpl, &paths.ag_task_xml(), &[
    ("AG_BIN", paths.ag_start_cmd().display().to_string()),
    ("AG_HOME", paths.ag_home.display().to_string()),
    ("AG_ENV", paths.ag_env().display().to_string()),
    ("BACKEND_PORT", backend_port.to_string()),
])?;

// Honor AgInstallDrift answer: keep / backup / replace.
if matches!(answers.choice(PromptId::AgServiceDrift), Some("keep")) {
    return Ok(());
}
// Backup before replacing — current task XML lives in scheduled_tasks_dir.
if matches!(answers.choice(PromptId::AgServiceDrift), Some("backup")) {
    // schtasks /Query /TN ag /XML > backup-<ts>.xml
}

// Delete then create — /F on Create is unreliable on some Windows versions.
let _ = run_schtasks(&["/Delete", "/TN", "ag", "/F"]).await; // best-effort
run_schtasks(&["/Create", "/XML", paths.ag_task_xml().to_str().unwrap(), "/TN", "ag"]).await?;

// Same for ag-stack (skipped when LowRam = none).
```

`run_schtasks` is a Windows-only helper that mirrors Linux's
`systemctl_user`, honoring `SKIP_SCHTASKS=1`.

Verification: with `SKIP_SCHTASKS=1` set on a Windows VM, the install
log shows every step's would-run lines. With `SKIP_SCHTASKS` unset,
`schtasks /Query /TN ag` lists the task after step 5.

### PR2.4 — Add scheduled-tasks XML templates ✅

**`installer/scheduled-tasks/ag.xml.tmpl`**:
```xml
<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>RERAG (ag) backend — launched at user logon.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{{USER}}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{{USER}}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <DisallowStartOnRemoteAppSession>false</DisallowStartOnRemoteAppSession>
    <UseUnifiedSchedulingEngine>true</UseUnifiedSchedulingEngine>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{{AG_BIN}}</Command>
      <WorkingDirectory>{{AG_HOME}}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
```

`{{USER}}` is substituted at install time from `whoami /UPN` (or
`std::env::var("USERNAME")` joined with `whoami /upn` fallback).

**`installer/scheduled-tasks/ag-stack.xml.tmpl`** has the same shape,
but Action is:
```xml
<Exec>
  <Command>docker</Command>
  <Arguments>compose -f "{{COMPOSE_PATH}}" --profile falkor-container --profile {{STACK_PROFILE}} up -d</Arguments>
  <WorkingDirectory>{{AG_HOME}}</WorkingDirectory>
</Exec>
```

Verification: `schtasks /Create /XML ag.xml /TN test-ag` should succeed
manually on a Windows VM, then `schtasks /Delete /TN test-ag /F`.

### PR2.5 — Blockers A + B: docker-compose + backend ✅

**(A) `docker-compose.yml`** — add (NOT under empty profile `""`):
```yaml
  falkordb:
    image: falkordb/falkordb:latest
    container_name: ag-falkordb
    profiles: ["falkor-container"]
    ports:
      - "${FALKORDB_HOST_PORT:-6380}:6379"
    volumes:
      - falkordb-data:/data
    environment:
      - FALKORDB_PASSWORD=${FALKOR_PASSWORD:-agpassword123}
    healthcheck:
      test: ["CMD", "redis-cli", "-a", "${FALKOR_PASSWORD:-agpassword123}", "ping"]
      interval: 10s
      timeout: 3s
      retries: 5
    restart: unless-stopped
```
And in the `volumes:` block at file end add `falkordb-data:`.

Critical: profile must be `["falkor-container"]` only — NOT
`["", "falkor-container"]`. The empty-profile-string activates on
default Linux `docker compose up`; we must not bring up a second
FalkorDB container alongside the native systemd unit on Linux.

**(B) `backend/src/main.rs`** — insert the AG_ENV hook *before* the
existing `dotenvy::dotenv().ok()` call:
```rust
// Honor AG_ENV (set by systemd EnvironmentFile= on Linux and by
// ag-start.cmd on Windows). Load it before the default .env so the
// system-managed env file wins over a stray dev-tree .env.
if let Ok(p) = std::env::var("AG_ENV") {
    dotenvy::from_path(&p).ok();
}
dotenvy::dotenv().ok();
```

Verification: on Linux, `cd backend && cargo run` still picks up `.env`
as before (AG_ENV is unset). With `AG_ENV=/tmp/fake.env cargo run`, the
hook calls `from_path` and silently no-ops because the file doesn't
exist. Both paths log identically.

### PR2.6 — Cross-cutting Windows wiring ✅

**`installer/src/bundled.rs`**:
1. Rename `appimage_usr_dir()` → `bundle_share_dir()`.
2. New body:
   ```rust
   fn bundle_share_dir() -> Option<PathBuf> {
       if let Ok(p) = std::env::var("AG_INSTALLER_BUNDLE_ROOT") {
           if !p.is_empty() { return Some(PathBuf::from(p)); }
       }
       #[cfg(windows)] {
           let exe = std::env::current_exe().ok()?;
           let share_ag = exe.parent()?.parent()?.join("share").join("ag");
           if share_ag.exists() { return Some(share_ag); }
       }
       None
   }
   ```
3. Update every existing call to `appimage_usr_dir().is_some()` →
   `bundle_share_dir().is_some()`. The Linux AppImage continues to set
   `AG_INSTALLER_BUNDLE_ROOT=$APPDIR/usr/share/ag`, so the parent-walk
   only fires on Windows.
4. Make the libtika filename platform-conditional:
   ```rust
   #[cfg(windows)] const LIBTIKA_NAME: &str = "tika_native.dll";
   #[cfg(unix)]    const LIBTIKA_NAME: &str = "libtika_native.so";
   ```

**`installer/src/prompts.rs`**:
1. Rename enum variant `AgServiceDrift` → `AgInstallDrift`. Search-
   replace across the crate (call sites in `install_steps.rs`,
   `platform/linux.rs`, `ui/prompts.rs`).
2. In the prompt's `title()` and `context()` methods, branch:
   ```rust
   #[cfg(windows)] let what = "scheduled task";
   #[cfg(unix)]    let what = "systemd unit";
   ```
3. In `required_prompts(&DetectionResult)`, skip `NativeObs` when
   `cfg!(windows)` (no native loki/tempo on Windows).

**`installer/src/app.rs`** — `detection_rows`:
1. Branch row labels by `cfg!(windows)`:
   ```rust
   let ollama_label = if cfg!(windows) { "Ollama responding" } else { "Ollama active" };
   let drift_label  = if cfg!(windows) { "ag task drift" } else { "ag.service drift" };
   ```
2. Skip the "native observability units" row when `cfg!(windows)`.

Verification: `cargo check --target x86_64-pc-windows-msvc` resolves
everything. Linux build path unchanged.

---

## PR 3 — Packaging

Goal: ship a real MSI that, when double-clicked on a fresh Windows
machine, installs ag-installer to `%PROGRAMFILES%\ag\` and exposes a
Start Menu shortcut. The first-launch flow then walks the user through
detection / prompts / install.

### PR3.1 — WiX scaffold + Cargo metadata ✅

1. On a Windows VM (or via `cross`):
   ```pwsh
   cargo install cargo-wix --version 0.3
   cd installer
   cargo wix init -p ag-installer
   ```
   This produces `installer/wix/main.wxs`. Commit the generated GUIDs
   (`UpgradeCode`, `Id` for `Path` component).

2. Hand-tune `main.wxs` to lay down:
   ```
   %PROGRAMFILES%\ag\
     bin\ag.exe
     bin\ag-installer.exe
     share\ag\web\…
     share\ag\docker-compose.yml
     share\ag\.env.example
     share\ag\scheduled-tasks\ag.xml.tmpl
     share\ag\scheduled-tasks\ag-stack.xml.tmpl
   ```
   Add a Start Menu shortcut `RERAG installer` targeting
   `bin\ag-installer.exe`.

3. Add a TODO comment near the `<Package>` element:
   ```xml
   <!-- TODO PR 4: post-build signtool.exe step here.
        Cert + password from GitHub repo secrets:
          $env:CERT_PFX_B64 → cert.pfx
          $env:CERT_PASSWORD
        signtool sign /f cert.pfx /p "$env:CERT_PASSWORD" \
          /tr http://timestamp.digicert.com /td sha256 /fd sha256 \
          target\release\ag-installer.exe target\release\ag.exe ag-installer-vX.Y.Z-x86_64.msi
   -->
   ```

4. In `installer/Cargo.toml`:
   ```toml
   [package.metadata.wix]
   upgrade-guid = "<GUID from main.wxs>"
   path-guid    = "<GUID from main.wxs>"
   license      = false
   eula         = false
   ```

Verification: `cargo wix -p ag-installer --nocapture` produces an MSI
in `target/wix/`. Double-click on a Windows VM → SmartScreen warning →
"More info" → "Run anyway" → installer runs from Start Menu.

### PR3.2 — Add windows-msi CI job with install-verification ✅

In `.github/workflows/release.yml` add (sibling to the existing
AppImage job):

```yaml
windows-msi:
  runs-on: windows-latest
  needs: [verify-version]  # whatever the AppImage job depends on
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
      with: { targets: x86_64-pc-windows-msvc }
    - uses: actions/cache@v4
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    - run: cargo install cargo-wix --version 0.3 --locked
    - working-directory: frontend\fro
      run: |
        npm ci
        npm run css:build
    - working-directory: frontend\fro
      run: dx build --release --platform web -p fro
    - run: cargo build --release -p ag --target x86_64-pc-windows-msvc
    - run: cargo build --release -p ag-installer --target x86_64-pc-windows-msvc
    - name: Extract version
      id: ver
      shell: pwsh
      run: |
        $v = (Select-String -Path installer/Cargo.toml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
        Add-Content $env:GITHUB_OUTPUT "version=$v"
    - run: cargo wix -p ag-installer --nocapture --output "ag-installer-v${{ steps.ver.outputs.version }}-x86_64.msi"
    - name: Install verification
      shell: pwsh
      run: |
        msiexec /i "ag-installer-v${{ steps.ver.outputs.version }}-x86_64.msi" /quiet /qn /norestart
        & "${env:ProgramFiles}\ag\bin\ag-installer.exe" --version
        & "${env:ProgramFiles}\ag\bin\ag.exe" --version
        msiexec /x "ag-installer-v${{ steps.ver.outputs.version }}-x86_64.msi" /quiet /qn
    - name: sha256
      shell: pwsh
      run: |
        Get-FileHash "ag-installer-v${{ steps.ver.outputs.version }}-x86_64.msi" -Algorithm SHA256 `
          | Select-Object -ExpandProperty Hash `
          | Out-File "ag-installer-v${{ steps.ver.outputs.version }}-x86_64.msi.sha256"
    - uses: softprops/action-gh-release@v2
      with:
        files: |
          ag-installer-v${{ steps.ver.outputs.version }}-x86_64.msi
          ag-installer-v${{ steps.ver.outputs.version }}-x86_64.msi.sha256
```

Verification: push a `v*.*.*` tag to a fork; both `appimage` and
`windows-msi` jobs complete; the MSI install/uninstall step doesn't
fail; both artifacts upload to the release.

### PR3.3 — Delete ps1 stub + README Windows section ✅

1. `git rm installers/install-windows.ps1` — v1.1.0 stub, uses
   "Agentic RAG" wording explicitly banned by CLAUDE.md.

2. In `README.md`, add a Windows install section (under or beside the
   existing Linux section):

   ```markdown
   ## Install on Windows

   **Requirements**: Windows 10 (1809+) or 11, Docker Compose, 10 GB free
   disk minimum (20 GB recommended).

   1. Download `ag-installer-vX.Y.Z-x86_64.msi` from the
      [latest release](https://github.com/PieterdenEngelse/RERAG/releases/latest).
   2. Double-click. Windows will show *Windows protected your PC* —
      click **More info** → **Run anyway**. This is normal for unsigned
      installers; signed builds ship once a code-signing certificate
      is in place.
   3. After the MSI completes, launch **RERAG installer** from the
      Start Menu. Walk through detection / prompts / install.
   4. The first install brings up the compose stack (FalkorDB + Redis
      + observability). Open `http://127.0.0.1:3010` for the
      dashboard.
   5. ag relaunches at every logon via Task Scheduler. No admin
      rights, no system service.

   **Uninstall**:
   ```pwsh
   "%ProgramFiles%\ag\bin\ag-installer.exe" --uninstall --purge
   ```
   then uninstall via *Apps & Features* for the MSI program files.
   ```

3. In release-notes for the tag that introduces Windows support, note:
   "Removed `installers/install-windows.ps1` — superseded by the MSI."

Verification: README renders correctly on GitHub. Links work.

### PR3.4 — End-to-end Windows verification on clean VM ⬜

On a fresh Windows 10 or 11 VM with Docker Compose available (but no
prior ag install):

1. Download the MSI from the release page (or copy from a build dir).
2. Double-click — accept SmartScreen warning.
3. Launch **RERAG installer** from Start Menu.
4. **Welcome screen**: click Continue.
5. **Detection screen**: confirm rows show real values:
   - Docker: version string visible
   - Compose: Down (first install)
   - Ollama: responding / not responding (depending on host)
   - FalkorDB: not running
   - ag.env: missing (first install)
   - Port 3010: free
   - System Redis: probably not present
   - Native obs: (row hidden on Windows)
   - ag task drift: false
   - Disk free / RAM: real GB values
   - Distro: "Windows 11 23H2" or similar
6. **Prompts screen**: accept all defaults.
7. **Progress screen**: all six steps complete. Open log link works.
   Inspect `%LOCALAPPDATA%\ag\logs\install-*.log`.
8. Verify on disk:
   - `%LOCALAPPDATA%\ag\bin\ag.exe` exists
   - `%LOCALAPPDATA%\ag\bin\ag-start.cmd` exists
   - `%APPDATA%\ag\ag.env` exists
   - `%APPDATA%\ag\docker-compose.yml` exists
9. Verify Scheduled Tasks:
   ```pwsh
   schtasks /Query /TN ag
   schtasks /Query /TN ag-stack
   ```
10. Verify compose stack:
    ```pwsh
    docker compose ls   # project ag, status running
    docker ps           # ag-falkordb, ag-redis, etc.
    ```
11. **First-run screen** (or browser): open `http://127.0.0.1:3010` —
    RERAG dashboard renders. Drop a test PDF; confirm ingest works.
12. **Reboot**: log out, log back in. Open the dashboard URL again —
    ag is running (logon trigger fired ag-start.cmd).
13. **Uninstall**:
    ```pwsh
    "%LOCALAPPDATA%\ag\bin\ag-installer.exe" --uninstall --purge
    ```
    Confirm:
    - `schtasks /Query /TN ag` → ERROR
    - `docker compose ls` → no `ag` project
    - `%LOCALAPPDATA%\ag\` removed
    - `%APPDATA%\ag\` removed
14. MSI uninstall via *Apps & Features* removes program files.

Pass criterion: every numbered step works without manual intervention
beyond accepting SmartScreen on first run.

---

## PR 4 — Signed builds (blocked on certificate)

Goal: zero SmartScreen warning on first download + double-click on a
clean Windows VM.

### Prerequisites (out-of-band, not code)

- Obtain a code-signing certificate. Options: an EV cert from a CA
  (~$300/yr), a standard OV cert, or a Microsoft-trusted publisher
  enrollment. EV grants immediate SmartScreen reputation; OV builds
  reputation over downloads.
- Add to GitHub repo secrets:
  - `CERT_PFX_B64` — base64 of the `.pfx` file
  - `CERT_PASSWORD` — the PFX password

### Steps

1. In `.github/workflows/release.yml`'s `windows-msi` job, add a
   Decode-cert step before the `cargo build` calls:
   ```yaml
   - name: Decode signing certificate
     shell: pwsh
     run: |
       $b64 = "${{ secrets.CERT_PFX_B64 }}"
       [System.IO.File]::WriteAllBytes("cert.pfx",
         [System.Convert]::FromBase64String($b64))
   ```

2. Sign `ag.exe` and `ag-installer.exe` *before* `cargo wix`:
   ```yaml
   - name: Sign binaries
     shell: pwsh
     run: |
       $signtool = "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe"
       & $signtool sign /f cert.pfx /p "${{ secrets.CERT_PASSWORD }}" `
         /tr http://timestamp.digicert.com /td sha256 /fd sha256 `
         target\x86_64-pc-windows-msvc\release\ag.exe `
         target\x86_64-pc-windows-msvc\release\ag-installer.exe
   ```

3. After `cargo wix`, sign the MSI:
   ```yaml
   - name: Sign MSI
     shell: pwsh
     run: |
       $signtool = "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe"
       & $signtool sign /f cert.pfx /p "${{ secrets.CERT_PASSWORD }}" `
         /tr http://timestamp.digicert.com /td sha256 /fd sha256 `
         ag-installer-v${{ steps.ver.outputs.version }}-x86_64.msi
   ```

4. Add a cleanup step at job end (always-run): `Remove-Item cert.pfx`.

5. Remove the TODO comment from `installer/wix/main.wxs` (the signing
   step now lives in CI, not in WiX).

6. In `README.md`, remove the SmartScreen note from the Windows install
   section. Replace step 2 with simply: "Double-click. The installer
   opens." (no warning expected with a valid sign).

7. In `installer/scheduled-tasks/ag.xml.tmpl` / `ag-stack.xml.tmpl`:
   no changes — Task Scheduler does not enforce signatures on the
   executable it launches.

### Verification

1. On a clean Windows VM (no prior ag install, default SmartScreen
   settings): download the signed MSI from the release page.
2. Double-click. **Confirm: no SmartScreen popup appears.** The MSI
   installer dialog opens directly.
3. Complete install, then verify in PowerShell:
   ```pwsh
   Get-AuthenticodeSignature "${env:ProgramFiles}\ag\bin\ag.exe" `
     | Select-Object Status, SignerCertificate
   ```
   Expected: `Status=Valid`, `SignerCertificate` matches your cert.
4. Same check for `ag-installer.exe` and the MSI itself.
5. If signing was correctly applied but SmartScreen still warns: this
   is an OV-cert reputation issue, not a signing failure. Document
   in README as "until reputation builds, click *More info* → *Run
   anyway*" — or upgrade to an EV cert.

---

## Cross-PR notes

- **Conventional commits**: `feat(installer): …` for PR 2, `feat(ci): …`
  for PR 3.2, `docs: …` for README updates.
- **Version bumps**: PR 3 lands on a tag (e.g. v1.2.0). PR 4 lands on
  the next minor or as a patch. The
  `Verify Cargo.toml version matches tag` step in release.yml covers
  both binaries since they share `installer/Cargo.toml`.
- **Don't pin Windows-only deps in `Cargo.lock`** for the Linux build
  path — `[target.'cfg(windows)'.dependencies]` keeps them out of the
  Linux resolution graph automatically.
- **Test matrix going forward**:
  - Linux dev: `HOME=/tmp/ag-test SKIP_SYSTEMCTL=1 cargo run`
  - Linux release: `installer/build-appimage.sh`
  - Windows dev: `target\…\release\ag-installer.exe` with
    `$env:SKIP_SCHTASKS=1` and `$env:AG_HOME=C:\Temp\ag-test`
  - Windows release: install MSI, walk GUI, verify via PR3.4
    checklist.
