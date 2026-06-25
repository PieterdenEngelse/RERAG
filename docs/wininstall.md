# Windows installer for ag

**Plan version: v2.1** (2026-06-22) — supersedes v2; promotes code
signing from a footnote to a tracked PR 4 milestone (with concrete
wiring location and acceptance criteria). v2 already resolved three
blocking gaps (FalkorDB in compose, ag.env loading on Windows, payload-
path consistency) and added CI install-verification + explicit deletion
of the old PowerShell stub.

## Context

A polished Linux installer already exists as the `ag-installer` Rust + Dioxus
crate at `installer/`. It detects the host, prompts the user about
ambiguities, lays down files into XDG paths, renders three systemd `--user`
units, and ships as an AppImage built by `.github/workflows/release.yml`.

Today there's only a hollow `installers/install-windows.ps1` stub (v1.1.0,
still using the old "Agentic RAG" wording — contradicting the current
"RERAG" branding rule in CLAUDE.md). Windows users have no real install
path.

This plan extends the existing `installer/` crate with `#[cfg(windows)]`
modules so the same Dioxus UI, prompt model, update-check, and bundled-asset
resolution serve both platforms. Windows-specific behavior — paths, service
management (Scheduled Task instead of systemd), detection probes (Win32 APIs
instead of `/proc` and `ss`) — lives in a new `platform/` submodule with two
implementations selected at compile time. The shipped artifact is an MSI
built by `cargo-wix` on a `windows-latest` runner, uploaded to the same
GitHub Release as the AppImage.

The Windows install requires Docker Compose (`docker compose` available on PATH). ag itself runs as a per-user
Scheduled Task triggered at logon — no admin, no UAC, no system-wide
Service. To stay reviewable, the work is staged into three PRs (see
Staging at the end).

## Three blockers that needed concrete fixes

### A. FalkorDB is not in the compose file today

`docker-compose.yml:49` explicitly excludes FalkorDB ("intentionally absent
here — it runs as a native systemd user service"). Today's compose ships
only `redis`, `prometheus`, `grafana`, `loki`, `tempo`, `otel-collector`.

**Fix**: add a `falkordb` service to `docker-compose.yml` under a new
profile `falkor-container` (active by default — empty profile string `""`
is *not* added so it doesn't fire on Linux's profile-less default run).
Linux's step 4 (`falkordb` in `install_steps.rs:439`) stays unchanged — it
brings up the native systemd unit and never invokes the `falkor-container`
profile. Windows's new step 4 runs
`docker compose --profile falkor-container --profile <stack> up -d`. The
service mounts a named volume for `/data`, exposes `6380:6379` to match
the Linux port, and passes `FALKOR_PASSWORD` via the same `ag.env`
substitution Compose already supports.

This is one file edit, no Linux behavior change.

### B. ag.exe has no way to read ag.env on Windows

The backend's only env-loading path is `dotenvy::dotenv().ok()` in
`backend/src/main.rs:35`, which reads a literal `.env` from the working
directory. On Linux, systemd's `EnvironmentFile={{AG_ENV}}` directive
(`systemd/ag.service.tmpl:16`) loads `~/.config/ag/ag.env` into the process
environment *before* ag starts, so dotenvy's miss is harmless. On Windows,
Task Scheduler can't reliably load env files.

Two options were considered:
- (B1) Wrapper `ag-start.cmd` that parses `%APPDATA%\ag\ag.env` line-by-line
  with `for /f` and `setlocal` then `start "" ag.exe`. Pure installer-side
  fix, no backend change.
- (B2) Backend change: read `$AG_ENV` env-var path with
  `dotenvy::from_filename($AG_ENV)` in `main.rs` before the existing
  `dotenv()` call. Two lines added; portable to Linux too (would let us
  drop `EnvironmentFile=` in the future).

**Fix**: do **B2** — it's two lines and aligns Linux + Windows on a single
env-loading source of truth. Specifically, add in `backend/src/main.rs`
just before line 35:

```rust
if let Ok(p) = std::env::var("AG_ENV") {
    dotenvy::from_path(&p).ok();
}
```

The Linux installer's Scheduled Task analog already sets `AG_ENV` (the
service unit substitution does), so this is a no-op on Linux today and
unlocks Windows. Step 5 on Windows sets `AG_ENV=%APPDATA%\ag\ag.env` in the
Scheduled Task's `<Actions/Exec/Arguments>` via a tiny wrapper, OR — cleaner
— in the user environment via `SetEnvironmentVariableW`; we go with the
wrapper because env vars set by the installer would leak into other
processes.

So step 5 still registers a wrapper: `%LOCALAPPDATA%\ag\bin\ag-start.cmd`
that does `set AG_ENV=%APPDATA%\ag\ag.env` then `start "" /B ag.exe`. The
Scheduled Task points at `ag-start.cmd`, not `ag.exe` directly. This keeps
env wiring localized and removes the "what about other processes" risk.

This is the only backend change in the entire plan. It's listed under
**Modified files** below.

### C. MSI payload layout vs. resolver disagreement

The previous draft laid down assets at `%PROGRAMFILES%\ag\share\` while the
resolver did `current_exe().parent().parent().join("share\\ag")`, landing
at `share\ag` — one extra segment.

**Fix**: the MSI lays files under `%PROGRAMFILES%\ag\share\ag\…` (mirroring
the AppImage's `usr/share/ag/…`). The resolver becomes
`current_exe().parent().parent().join("share").join("ag")` on Windows —
which is exactly the same string-walk shape as the Linux AppImage's
`bundle_root` (`installer/src/bundled.rs:92-100`), just without the `usr/`
segment.

So both platforms end with `…/share/ag/{web,systemd,docker-compose.yml,
.env.example,scheduled-tasks}`. The shared `bundled.rs` only has to learn
that on Windows the resolver walks one fewer parent.

## Approach

### 1. Refactor: carve out a `platform/` boundary (PR 1)

Move OS-specific surfaces behind a small module boundary; keep everything
else shared.

Create:

```
installer/src/platform/
    mod.rs           # cfg-selected re-exports
    linux.rs         # Linux-specific code lifted from existing files
    windows.rs       # stub returning unimplemented!() (PR 2 fills it in)
```

PR 1's diff is mechanical — bodies move, public function names stay the
same — so Linux regression is trivially verifiable: run the existing
sandbox recipe and the AppImage build, both must still pass. No Windows
code yet beyond an empty `windows.rs`.

`mod.rs` exposes:

```rust
pub use imp::{
    Paths,                 // resolve(), ag_env(), data dirs, service file
    run_detection,         // -> DetectionResult
    install_artifacts,     // step 3
    install_service,       // step 5: systemd_step on Linux, schtasks on Windows
    install_stack,         // step 4: falkordb-native on Linux, compose on Windows
    start_service,         // first-run start
    uninstall,             // CLI --uninstall path
};
```

`DetectionResult`, `PromptAnswers`, `ProgressEvent`, the `Screen` state
machine, and the entire `ui/` tree stay shared. `PromptAnswers` has no
serde derive (checked: `grep` in `installer/src/prompts.rs` finds none) so
renames are safe.

### 2. Windows implementations (PR 2)

#### Paths (`platform/windows.rs::paths`)

Honor `AG_HOME` env-var override (parity with Linux). Otherwise:

| Item | Path |
|---|---|
| Data home (`AG_HOME`) | `%LOCALAPPDATA%\ag` |
| Binary | `%LOCALAPPDATA%\ag\bin\ag.exe` |
| Wrapper | `%LOCALAPPDATA%\ag\bin\ag-start.cmd` |
| Native lib (Tika) | `%LOCALAPPDATA%\ag\lib\tika_native.dll` (optional) |
| Env file | `%APPDATA%\ag\ag.env` |
| Compose file | `%APPDATA%\ag\docker-compose.yml` |
| Scheduled-Task XML | `%APPDATA%\ag\scheduled-tasks\ag.xml` (retained for drift detection) |
| Install log | `%LOCALAPPDATA%\ag\logs\install-<utc>.log` |

Use `std::env::var("LOCALAPPDATA")` / `"APPDATA"`. The `set_mode` 0o600 in
`install_steps.rs:849` is already a non-unix no-op; user-profile ACLs are
user-only-readable by default, which is sufficient for `ag.env`.

#### Detection (`platform/windows.rs::detection`)

Reuse the `tokio::join!` shape from `detection.rs:54-95`:

- `docker_present` → `docker --version` (identical to Linux).
- `compose_up` → `docker compose ls` with `COMPOSE_PROJECT_NAME=ag`
  (identical to Linux).
- `ollama_active` → HTTP GET `http://127.0.0.1:11434/api/tags`.
  **Semantics shift**: this means "responsive" rather than Linux's
  `systemctl is-active` which means "process running". Document the
  difference in the Windows row label as "Ollama responding" vs Linux's
  "Ollama active". Equally informative.
- `falkordb_healthy` →
  `docker inspect ag-falkordb --format "{{.State.Health.Status}}"`.
- `ag_env_exists` → `tokio::fs::metadata(paths.ag_env())`.
- `backend_port_busy` → try `TcpListener::bind("127.0.0.1:3010")`;
  `AddrInUse` → busy. Cross-platform; no `ss` needed.
- `system_redis` → TCP connect to `127.0.0.1:6379`, send
  `*1\r\n$4\r\nPING\r\n`, read `+PONG\r\n`. No `redis-cli` on Windows.
- `native_obs` → `vec![]` (no native loki/tempo on Windows).
- `ag_install_drift` (renamed from `ag_service_drift`) →
  `schtasks /Query /TN ag /XML` exits 0, and the returned XML's `Command`
  element doesn't point at `%LOCALAPPDATA%\ag\bin\ag-start.cmd`.
- `disk_free_gb` → `fs2::available_space(parent_of_ag_home)` >> 30.
- `ram_gb` → `sysinfo::System::new().total_memory()` >> 30.
- `distro` → `HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion`
  `ProductName` + `DisplayVersion` via the `winreg` crate.

Add under `installer/Cargo.toml`:

```toml
[target.'cfg(windows)'.dependencies]
fs2     = "0.4"
sysinfo = "0.32"
winreg  = "0.55"
```

#### Install steps (`platform/windows.rs::install_steps`)

Keep the six-step `STEP_NAMES` array from `install_steps.rs:68-75`. The
`run()` orchestrator + `step!` macro (`install_steps.rs:114-172`) and the
`LogTee`, `step_log`, `render_template`, `edit_env_file`, `health_check`
helpers all stay in the shared module — they're already platform-neutral.

| # | Linux step | Windows step |
|---|---|---|
| 1 | Ensure XDG tree | Ensure data tree (Windows paths) |
| 2 | Seed config | Same logic; `chmod 600` is already a non-unix no-op |
| 3 | Install artifacts | Copy `ag.exe`, `tika_native.dll`, web/ assets, plus write `ag-start.cmd`. Replace `rsync` (`install_steps.rs:382`) with `robocopy /MIR /NJH /NJS /NDL /NP` |
| 4 | FalkorDB native service | `docker compose -f %APPDATA%\ag\docker-compose.yml --profile falkor-container --profile <prof> up -d` |
| 5 | Systemd user units | Render `installer/scheduled-tasks/ag.xml.tmpl` with `{{AG_BIN}}`, `{{AG_HOME}}`, `{{AG_ENV}}`, `{{BACKEND_PORT}}`, then `schtasks /Delete /TN ag /F` (best-effort) followed by `schtasks /Create /XML <rendered> /TN ag` |
| 6 | Health check | Unchanged; `reqwest` poll of `/health` |

The Scheduled Task XML template specifies:
- `LogonTrigger` (per-user, at logon)
- `Settings/RestartOnFailure/Interval=PT1M`, `Count=3`
- `Settings/Priority=7`
- `Actions/Exec/Command=%LOCALAPPDATA%\ag\bin\ag-start.cmd`
- `Actions/Exec/WorkingDirectory=%LOCALAPPDATA%\ag`

A second task `ag-stack` registers identically, pointing at
`docker compose -f %APPDATA%\ag\docker-compose.yml up -d`. Skipped when
the `LowRam` answer is `none` (mirrors `install_steps.rs:600`).

**`ag-start.cmd`** (written by step 3 verbatim, no template substitution
needed):

```cmd
@echo off
set "AG_ENV=%APPDATA%\ag\ag.env"
start "" /B "%~dp0ag.exe"
```

The `AG_ENV` value is consumed by the new two-line backend hook described
in blocker B.

**Drift handling**: on `replace` or `backup` answers, the task is deleted
(`schtasks /Delete /TN ag /F`) before re-creating, not overwritten with
`/F` on `Create`. Some Windows versions leave half-updated state on
`/Create /F`. On `keep`, step 5 logs and skips the re-render entirely.

#### First-run config (`platform/windows.rs::first_run`)

The atomic env-write helper from `first_run.rs:141-150` is portable
(`fs::write` + `fs::rename`) — keep it shared. The Ollama probe is HTTP,
already portable. The systemctl-start step becomes:

1. `schtasks /Run /TN ag`
2. Poll `/health` with the existing helper (`install_steps.rs:687-748`),
   shared.

The `/Run` racing the logon trigger is benign — both call `ag-start.cmd`,
and ag.exe self-binds the port idempotently. Health polling proves whichever
won.

#### Uninstall (`platform/windows.rs::uninstall`)

`ag-installer.exe --uninstall [--purge]`:

- Always: `schtasks /Delete /TN ag /F`, `schtasks /Delete /TN ag-stack /F`,
  `docker compose -f %APPDATA%\ag\docker-compose.yml down`, delete
  `%LOCALAPPDATA%\ag\bin\` and `\lib\`, delete the wrapper.
- `--purge`: also delete `%APPDATA%\ag\` and `%LOCALAPPDATA%\ag\`.

The MSI's `Apps & Features` uninstall removes program files only — it does
not invoke `--uninstall`. The README's "Uninstall" section calls this out
and tells the user to run `ag-installer.exe --uninstall --purge` *before*
running the MSI uninstaller for a clean removal. (Future polish: add an MSI
`StopServices` custom action that calls `--uninstall` automatically.)

### 3. Bundled-asset resolution (PR 2)

`bundled.rs` becomes:

```rust
fn bundle_share_dir() -> Option<PathBuf> {
    // Linux AppImage sets AG_INSTALLER_BUNDLE_ROOT explicitly.
    if let Ok(p) = std::env::var("AG_INSTALLER_BUNDLE_ROOT") {
        if !p.is_empty() { return Some(PathBuf::from(p)); }
    }
    // Windows MSI install: <prog>\bin\ag-installer.exe →
    // <prog>\share\ag\… is two parents up + share/ag.
    #[cfg(windows)] {
        let exe = std::env::current_exe().ok()?;
        let share_ag = exe.parent()?.parent()?.join("share").join("ag");
        if share_ag.exists() { return Some(share_ag); }
    }
    None
}
```

Both AppImage and MSI artifact trees end in `share/ag/{web,systemd,
docker-compose.yml,.env.example,scheduled-tasks}`. Dev mode falls back to
`repo_root()` walk (`bundled.rs:102-118`) unchanged on both platforms.

### 4. MSI packaging (PR 3)

Add at the repo root:

- `installer/wix/main.wxs` — WiX source, initially generated via
  `cargo wix init`, then hand-tuned.
- `installer/scheduled-tasks/ag.xml.tmpl` and `ag-stack.xml.tmpl` —
  templates rendered at install time, not baked into the MSI.

Cargo.toml additions:

```toml
[package.metadata.wix]
upgrade-guid = "<generated once>"
path-guid    = "<generated once>"
license      = false
eula         = false
```

The MSI lays down (under `%PROGRAMFILES%\ag\`):

```
bin\ag.exe
bin\ag-installer.exe
share\ag\web\…
share\ag\docker-compose.yml
share\ag\.env.example
share\ag\scheduled-tasks\ag.xml.tmpl
share\ag\scheduled-tasks\ag-stack.xml.tmpl
```

Start Menu shortcut "RERAG installer" pointing at `ag-installer.exe`.
First launch runs the Dioxus GUI through detection / prompts / install
(matching the AppImage UX). The MSI does not create per-user data.

### 5. Code signing — tracked as PR 4

Unsigned MSIs and unsigned `ag.exe` will draw SmartScreen "unrecognized
app" warnings and Defender flags on first run. No code-signing certificate
exists for the project today, so v1 ships unsigned with a documented
warning. Signing is a tracked follow-up, not an indefinite TODO — see
PR 4 in **Staging** below.

PR 3 prepares the ground:

- README's Windows install section explicitly says: "On first run Windows
  will show *Windows protected your PC*. Click *More info* → *Run anyway*.
  This is normal for unsigned installers; signed builds ship once a
  certificate is in place." (This note is removed by PR 4 once signing
  is live.)
- `installer/wix/main.wxs` carries a TODO near the `Package` element
  marking exactly where the `signtool.exe` post-build step goes (cert +
  password come from GitHub repo secrets).

PR 4 is blocked only on the cert; the wiring location and acceptance
criteria are already specified.

### 6. CI: add a Windows job + install-verification step (PR 3)

In `.github/workflows/release.yml` add a job running on `windows-latest`:

```yaml
windows-msi:
  runs-on: windows-latest
  steps:
    - checkout
    - cache cargo
    - install rust stable + cargo-wix
    - npm ci && npm run css:build (in frontend\fro)
    - dx build --release --platform web -p fro
    - cargo build --release -p ag --target x86_64-pc-windows-msvc
    - cargo build --release -p ag-installer --target x86_64-pc-windows-msvc
    - cargo wix -p ag-installer --nocapture
                --output ag-installer-v${{ env.VERSION }}-x86_64.msi
    # NEW — verify before uploading
    - name: Install verification
      run: |
        msiexec /i ag-installer-v${{ env.VERSION }}-x86_64.msi /quiet /qn /norestart
        "${env:ProgramFiles}\ag\bin\ag-installer.exe" --version
        "${env:ProgramFiles}\ag\bin\ag.exe" --version
        msiexec /x ag-installer-v${{ env.VERSION }}-x86_64.msi /quiet /qn
    - sha256 + upload to the same gh release
```

The existing `Verify Cargo.toml version matches tag` step covers both
artifacts since they share `installer/Cargo.toml`.

### 7. Tika and FalkorDB on Windows

- **Tika**: `tika_native.dll` is built by `extractous` the same way
  `libtika_native.so` is on Linux. The dev-mode finder in
  `bundled.rs:120-146` already globs for `extractous-*` build outputs —
  it just needs the filename to be platform-conditional
  (`libtika_native.so` vs `tika_native.dll`). Optional; if absent, PDF
  parsing degrades gracefully.
- **FalkorDB**: no native Windows build. Step 4 on Windows runs the
  compose stack with the new `falkor-container` profile (see blocker A).
  No bundling, no `redis-server.exe` extraction, no `installer/stage/`
  on Windows.

### 8. Fate of `installers/install-windows.ps1`

Delete the v1.1.0 stub in PR 3. It uses "Agentic RAG" wording that the
"Describing the app" rule in CLAUDE.md explicitly bans, and the MSI fully
replaces its narrow CLI scope. Note the deletion in the release notes for
the tag that introduces Windows support.

## Critical files

**New** (PR 2 unless noted):
- `installer/src/platform/mod.rs` — cfg-select between linux.rs / windows.rs (PR 1)
- `installer/src/platform/linux.rs` — Linux code lifted from existing files (PR 1)
- `installer/src/platform/windows.rs` — Windows impls (PR 2)
- `installer/scheduled-tasks/ag.xml.tmpl` (PR 2)
- `installer/scheduled-tasks/ag-stack.xml.tmpl` (PR 2)
- `installer/wix/main.wxs` (PR 3)

**Modified**:
- `backend/src/main.rs` — add the two-line `AG_ENV` hook before line 35
  (PR 2; blocker B)
- `docker-compose.yml` — add the `falkordb` service under the
  `falkor-container` profile, exposing `6380:6379` and mounting a named
  volume `falkordb-data` (PR 2; blocker A)
- `installer/Cargo.toml` — add `[target.'cfg(windows)'.dependencies]`
  (`fs2`, `sysinfo`, `winreg`) and `[package.metadata.wix]`; update the
  package description to "GUI installer for RERAG — Linux + Windows"
  (PR 2 / PR 3)
- `installer/src/paths.rs` — thin re-export of `platform::Paths` (PR 1)
- `installer/src/detection.rs` — `run()` becomes
  `pub use platform::run_detection`; Linux probes move to `linux.rs` (PR 1)
- `installer/src/install_steps.rs` — keep orchestrator, `step!`, `LogTee`,
  `step_log`, `render_template`, `edit_env_file`, `health_check`; step
  bodies delegate to `platform::install_stack` / `install_service` /
  `copy_artifacts` (PR 1)
- `installer/src/first_run.rs` — `write_first_run_settings` shared;
  service-start helper delegates to platform (PR 1)
- `installer/src/uninstall.rs` — shape unchanged; body delegates (PR 1)
- `installer/src/app.rs` — `detection_rows` emits Windows-flavored row
  labels when `cfg!(windows)` (PR 2)
- `installer/src/bundled.rs` — rename `appimage_usr_dir` →
  `bundle_share_dir`; Windows resolves via `current_exe()` parent walk
  ending in `share/ag/` (PR 2)
- `installer/src/prompts.rs` — rename `AgServiceDrift` → `AgInstallDrift`;
  prompt title/context branch on `cfg!(windows)` ("service" vs "task");
  drop `NativeObs` when `cfg!(windows)` (PR 2)
- `installers/install-windows.ps1` — **delete** (PR 3)
- `.github/workflows/release.yml` — add `windows-msi` job (PR 3)
- `README.md` — add Windows install section with the SmartScreen note
  (PR 3)

**Templates that remain Linux-only**:
- `systemd/ag.service.tmpl`, `ag-stack.service.tmpl`, `falkordb.service.tmpl`

## Reused existing helpers

- `render_template` (`install_steps.rs:755`) — Windows Task XML uses the
  same `{{KEY}}` placeholder format
- `edit_env_file` (`install_steps.rs:772`) — atomic env-file editing
- `step_log` / `LogTee` (`install_steps.rs:87-112`) — log teeing
- `health_check` (`install_steps.rs:687-748`) — already uses `reqwest`
- `write_first_run_settings` (`first_run.rs:141-150`) — atomic env write
- `bundled::repo_root` (`bundled.rs:102-118`) — works on Windows in
  dev mode

## Staging (four PRs)

1. **PR 1 — Refactor only**. Carve out `platform/` with Linux as thin
   `pub use` aliases over the moved-but-unchanged Linux bodies. No
   Windows code. Verification: existing Linux sandbox recipe (`HOME=…
   SKIP_SYSTEMCTL=1 cargo run`) and AppImage build both green. ~600-line
   move-only diff.
2. **PR 2 — Windows code + backend hook + compose profile**.
   `platform/windows.rs`, scheduled-task templates, the
   `backend/src/main.rs` `AG_ENV` two-liner, the `falkordb` compose
   service under `falkor-container`. Builds on `windows-latest` in CI but
   no MSI yet; Linux unchanged.
3. **PR 3 — Packaging**. cargo-wix scaffold, MSI build, CI
   install-verification step, README updates (including the
   SmartScreen note), delete `installers/install-windows.ps1`. Leaves a
   TODO in `installer/wix/main.wxs` marking the future signing hook.
4. **PR 4 — Signed builds** *(blocked on certificate availability)*.
   Wire `signtool.exe` post-build step in `installer/wix/main.wxs` (or
   as a separate CI step), pull cert + password from GitHub repo
   secrets, sign `ag.exe` before MSI build and the MSI itself after
   `cargo wix`. Remove the SmartScreen note from README. **Acceptance**:
   a fresh download + double-click installs without any SmartScreen
   warning on a clean Windows 10/11 VM.

## Verification

End-to-end on a Windows 10/11 box with Docker Compose available:

1. **Dev-mode smoke test** (after PR 2 lands, before PR 3):
   ```pwsh
   cd C:\src\RERAG
   cargo build --release -p ag --target x86_64-pc-windows-msvc
   cargo build --release -p ag-installer --target x86_64-pc-windows-msvc
   cd frontend\fro; npm ci; npm run css:build; dx build --release --platform web -p fro; cd ..\..
   $env:SKIP_SCHTASKS = "1"   # parity with SKIP_SYSTEMCTL on Linux
   $env:AG_HOME = "C:\Temp\ag-test"
   target\x86_64-pc-windows-msvc\release\ag-installer.exe
   ```
   Walk all six screens; verify the install log under
   `C:\Temp\ag-test\logs\install-*.log` shows every step succeeded.

2. **Unit-level checks**:
   - `detection::run()` populates non-zero `disk_free_gb` and `ram_gb`
   - `bundled::ag_binary_path()` returns
     `target\…\release\ag.exe` in dev mode
   - `Paths::resolve()` honors `AG_HOME` and falls back to `%LOCALAPPDATA%\ag`
   - The `print_real_result` ignored test (`detection.rs:95`) runs
     under PowerShell

3. **MSI build** (PR 3):
   ```pwsh
   cargo install cargo-wix --version 0.3
   cargo wix -p ag-installer --nocapture
   ```
   The CI install-verification step covers this automatically on every
   tag push.

4. **First real install** (manual, on a clean Windows VM):
   - `%LOCALAPPDATA%\ag\bin\ag.exe` and `ag-start.cmd` exist
   - `schtasks /Query /TN ag` lists the task
   - `docker compose ls` lists project `ag` with status `running`,
     including the new `ag-falkordb` container
   - Browser at `http://127.0.0.1:3010` shows the RERAG dashboard
   - Reboot → log back in → ag.exe is running (logon trigger fired)
   - SmartScreen warning shows on first run (expected; documented)

5. **Uninstall path**:
   ```pwsh
   ag-installer.exe --uninstall --purge
   ```
   Scheduled tasks gone, compose stack down, `%LOCALAPPDATA%\ag\` and
   `%APPDATA%\ag\` removed.

6. **Linux regression** (after every PR):
   ```bash
   cd installer && cargo fmt && cargo clippy --all-targets -- -D warnings
   HOME=/tmp/ag-test SKIP_SYSTEMCTL=1 cargo run -p ag-installer
   ```
   The Linux install must still produce a clean six-step run.

7. **Backend `AG_ENV` hook** (cross-platform):
   `AG_ENV=/tmp/fake.env cargo run -p ag` should log a dotenv-load
   attempt (with a "file not found" if /tmp/fake.env doesn't exist —
   silenced by `.ok()`). With a real file, its `KEY=value` lines
   override `.env`'s.

8. **CI**:
   Push a `v*.*.*` tag to a fork; confirm both jobs (`appimage` and
   `windows-msi`) complete, upload artifacts, and the
   install-verification step passes on the MSI.
