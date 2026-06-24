# Plan: WSL2 Docker Engine Detection & Install (Windows Installer)

## Context

The Windows installer currently handles Docker one way: detect it via
`docker --version` on the native PATH, and if missing offer to install
`Docker.DockerCompose` via winget — which silently requires Docker Desktop to
provide the Engine. Docker Desktop is a heavyweight GUI app (~1–2 GB RAM
overhead, paid license for organizations).

WSL2 Docker Engine (Docker CE running inside a dedicated WSL2 Linux distro) is a
legitimate lightweight alternative: headless, free, ~200–500 MB RAM. This plan
adds it as a first-class option in the `DockerMissing` prompt — **only offered
when WSL2 is already enabled on the host**, completely avoiding the Windows
restart that enabling the WSL2 feature would require.

> **This plan supersedes parts of `docs/dockplan.md`.** It renames the existing
> `"install"` DockerMissing option to `"install_docker_desktop"` and the step
> constant value from "Install Docker Compose" to "Install Docker Desktop".
> All three sites that reference the old key (`prompts.rs`, `install_steps.rs`,
> `ui/progress.rs`) must be updated in lockstep — see §8/§10.

## Files to Modify

| File | Change |
|------|--------|
| `installer/src/detection.rs` | 3 new fields on `DetectionResult` + 3 test prints |
| `installer/src/platform/windows.rs` | 3 probes, `install_docker_wsl2()`, path util, `install_stack` split |
| `installer/src/platform/linux.rs` | **`install_stack` gains ignored `answers` param** (signature parity) |
| `installer/src/platform/mod.rs` | Export `install_docker_wsl2`, `windows_path_to_wsl` (Windows only) |
| `installer/src/prompts.rs` | `options()` gains `Option<&DetectionResult>`; WSL2 option; key rename |
| `installer/src/install_steps.rs` | New step constant + dispatch; pass `&answers` to `install_stack` |
| `installer/src/app.rs` | WSL2 detection row (built via `.push`, not a `#[cfg]` vec element) |
| `installer/src/ui/prompts.rs` | Pass `Some(&props.detection)` to `options()` |
| `installer/src/ui/progress.rs` | Map both Docker keys to their step names in `initial_steps()` |
| `installer/scheduled-tasks/ag-stack.xml.tmpl` | `{{STACK_COMMAND}}` variable |

---

## 1. New `DetectionResult` Fields (`detection.rs`)

```rust
pub struct DetectionResult {
    // ... existing fields unchanged ...

    /// Windows only. `wsl --status` exited 0 → WSL2 feature is enabled.
    /// Gates whether the WSL2 Docker option appears in the DockerMissing prompt.
    /// Always `false` on Linux (field exists on both platforms for struct sharing).
    pub wsl2_available: bool,

    /// Windows only. `wsl -d ag-ubuntu -- docker --version` succeeded → Docker
    /// Engine is already installed inside the ag-managed distro.
    pub wsl2_docker_version: Option<String>,

    /// Windows only. The ag-managed WSL2 distro (`ag-ubuntu`) already exists.
    /// Reinstalls detect and reuse it. `Some("ag-ubuntu")` when present.
    pub wsl2_distro_name: Option<String>,
}
```

`Default` is automatic via `derive`. Add 3 `println!` lines to
`print_real_result` at the bottom of `detection.rs`.

---

## 2. New Probes (`platform/windows.rs`)

### `probe_wsl2_available() -> bool`
```rust
async fn probe_wsl2_available() -> bool {
    Command::new("wsl")
        .args(["--status"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}
```
`wsl --status` exists on Windows 10 21H2+ and all Windows 11 builds. Note
`wsl.exe` is a System32 shim that exists even when the optional feature is not
installed; in that case `--status` exits non-zero, so the `.success()` check is
the right gate.

### `probe_wsl2_distro_name() -> Option<String>`
```rust
async fn probe_wsl2_distro_name() -> Option<String> {
    let out = Command::new("wsl").args(["--list", "--quiet"]).output().await.ok()?;
    // wsl --list --quiet emits UTF-16LE; strip NUL bytes to get ASCII.
    let text = String::from_utf8(out.stdout.clone()).unwrap_or_else(|_| {
        let ascii: Vec<u8> = out.stdout.into_iter().filter(|&b| b != 0).collect();
        String::from_utf8_lossy(&ascii).into_owned()
    });
    text.lines()
        .any(|l| l.trim() == "ag-ubuntu")
        .then(|| "ag-ubuntu".to_string())
}
```

### `probe_wsl2_docker() -> Option<String>`
**Probe the `ag-ubuntu` distro specifically, not the default distro** — install
and runtime both target `-d ag-ubuntu`, so detection must check the same place
or it gives false positives/negatives when the user's default distro differs.
```rust
async fn probe_wsl2_docker() -> Option<String> {
    let out = Command::new("wsl")
        .args(["-d", "ag-ubuntu", "--", "docker", "--version"])
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!v.is_empty()).then_some(v)
}
```

### Updated `run_detection()`
Add the 3 probes to `tokio::join!` and populate the 3 new fields.

---

## 3. Path Translation Utility (`platform/windows.rs`)

```rust
/// Convert a Windows absolute path to its WSL2 /mnt/ equivalent.
///   C:\Users\foo\ag\docker-compose.yml → /mnt/c/Users/foo/ag/docker-compose.yml
/// Strips extended-length `\\?\` prefix; passes relative/UNC paths through.
pub fn windows_path_to_wsl(path: &str) -> String {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let bytes = path.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = path[3..].replace('\\', "/");
        format!("/mnt/{drive}/{rest}")
    } else {
        path.replace('\\', "/")
    }
}
```

`#[cfg(test)]` cases:
```rust
assert_eq!(windows_path_to_wsl(r"C:\Users\foo\ag\docker-compose.yml"),
           "/mnt/c/Users/foo/ag/docker-compose.yml");
assert_eq!(windows_path_to_wsl(r"D:\data"), "/mnt/d/data");
assert_eq!(windows_path_to_wsl("relative"), "relative");
assert_eq!(windows_path_to_wsl(r"\\?\C:\ext"), "/mnt/c/ext");
```

---

## 4. `install_docker_wsl2()` (`platform/windows.rs`)

```rust
pub async fn install_docker_wsl2(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()>
```

All shellouts honor `skip_systemctl()` (log-only in sandbox).

**a. Default version guard** — `wsl --set-default-version 2` (fast no-op since
`wsl2_available` was true).

**b. Reuse check** — run `probe_wsl2_distro_name()` logic inline. If `ag-ubuntu`
exists, skip c–e and go straight to verify (h).

**c. Download rootfs** — stream the Ubuntu WSL rootfs to `%TEMP%` via `reqwest`.
**Do not hardcode a guessed filename.** The exact `cloud-images.ubuntu.com`
filename has changed across releases (`*-wsl.rootfs.tar.gz` vs
`*-wsl-amd64-ubuntu24.04lts.rootfs.tar.gz`). Resolve the current name at
implementation time and verify the URL returns 200 before importing; on a
4xx/5xx, `bail!` with the URL and a "download a rootfs manually and re-run"
hint rather than importing a truncated file. Log the byte size.

**d. Import** — ensure the target dir exists first
(`fs::create_dir_all(paths.ag_home.join("wsl"))`), then:
```
wsl --import ag-ubuntu <ag_home>\wsl\ag-ubuntu <tmp_rootfs> --version 2
```

**e. Install Docker Engine** (official APT repo, gives `docker-compose-plugin`):
```rust
let install_script = r#"set -e
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq ca-certificates curl gnupg lsb-release
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
chmod a+r /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
    https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" \
    > /etc/apt/sources.list.d/docker.list
apt-get update -qq
apt-get install -y -qq docker-ce docker-ce-cli containerd.io docker-compose-plugin
"#;
// wsl -d ag-ubuntu -u root -- bash -c <install_script>
```

**f. dockerd autostart** via `/etc/wsl.conf` (no systemd dependency):
```
[boot]
command = "/usr/bin/dockerd --host unix:///var/run/docker.sock --log-level error &>/tmp/dockerd.log &"
```
Write with `wsl -d ag-ubuntu -u root -- bash -c "printf '...' > /etc/wsl.conf"`.

**g. Restart distro** to apply `/etc/wsl.conf`:
`wsl --terminate ag-ubuntu`, then re-launch (`wsl -d ag-ubuntu -- true`).

**h. Verify with readiness poll** — don't just run `docker version` once; WSL
re-runs the `[boot] command` on each cold start and dockerd needs a moment.
Poll `wsl -d ag-ubuntu -u root -- docker info` up to ~10× with a short sleep;
log the first success. Non-zero after all attempts → WARN (not fatal — daemon
may still be starting), mirroring `health_check`'s tolerance.

**i. Wrapper scripts — only if `bin_dir` is on PATH.** *(Decision point — see §9.)*
The install and runtime paths call `wsl -d ag-ubuntu -- docker compose …`
directly and do **not** depend on these wrappers. Writing `docker.cmd` etc. to
`paths.bin_dir` is pointless unless that dir is added to the user PATH (it is
not by default on Windows). **Default for this PR: skip the wrappers.** If we
later decide a terminal `docker` shim is wanted, do it as a separate change that
also appends `bin_dir` to the user PATH and routes `probe_docker()` through it.

---

## 5. `install_stack` Split + Signature Parity

### Windows (`platform/windows.rs`)
```rust
pub async fn install_stack(
    paths: &Paths, tx: &ProgressSender, tee: &LogTee, answers: &PromptAnswers,
) -> Result<()> {
    if answers.use_wsl2_docker() {
        install_stack_wsl2(paths, tx, tee).await
    } else {
        install_stack_native(paths, tx, tee).await   // existing body, renamed
    }
}
```
`install_stack_wsl2()` translates the compose path and shells through WSL:
```rust
let compose_wsl = windows_path_to_wsl(&paths.docker_compose().display().to_string());
// wsl -d ag-ubuntu -u root -- docker compose -f <compose_wsl>
//     --profile "" --profile falkor-container up -d   (COMPOSE_PROJECT_NAME=ag)
```

### Linux (`platform/linux.rs`) — REQUIRED for the build to pass
`install_stack` is re-exported under one shared name and called from the
non-cfg-gated site in `install_steps.rs`. The Linux signature **must** match the
new 4-arg shape or the Linux build breaks:
```rust
pub async fn install_stack(
    paths: &Paths, tx: &ProgressSender, tee: &LogTee, _answers: &PromptAnswers,
) -> Result<()> {
    // body unchanged; _answers ignored on Linux
}
```
(Add `use crate::prompts::PromptAnswers;` to `linux.rs` if not already imported.)

---

## 6. Scheduled-Task Command (`install_service` + template)

`ag-stack.xml.tmpl` currently hardcodes `<Command>docker</Command>` (line 43).
Replace with `{{STACK_COMMAND}}`:
```xml
<Exec>
  <Command>{{STACK_COMMAND}}</Command>
  <Arguments>{{STACK_ARGS}}</Arguments>
  <WorkingDirectory>{{AG_HOME}}</WorkingDirectory>
</Exec>
```

In `install_service()`, add `STACK_COMMAND` to the **existing** `render_template`
vars for ag-stack (do not fork into a second call — both paths must supply it or
the non-WSL2 render emits a literal `{{STACK_COMMAND}}`):
```rust
("STACK_COMMAND", if answers.use_wsl2_docker() { "wsl".to_string() }
                  else { "docker".to_string() }),
```
When WSL2, build `STACK_ARGS` as:
```
-d ag-ubuntu -u root -- docker compose -f <wsl_compose_path> --profile "" --profile falkor-container up -d
```
(The existing native `STACK_ARGS` already carries `--profile ""`, so the
empty-profile token in XML `<Arguments>` is unchanged behavior.)

---

## 7. `prompts.rs` Changes

### `options()` signature
```rust
pub fn options(self, d: Option<&DetectionResult>) -> Vec<PromptOption>
```
Only one real call site (`ui/prompts.rs:124`, inside `PromptCard`, which already
holds `props.detection`). Any test callers must add `None`.

### `DockerMissing` options (Windows branch)
Build the `Vec` imperatively; prepend the WSL2 option when
`d.map(|d| d.wsl2_available).unwrap_or(false)`:
```rust
PromptOption {
    key: "install_wsl2_docker",
    label: "Install Docker Engine in WSL2 (lightweight, no GUI)",
    description: "Creates an ag-ubuntu WSL2 distro and installs Docker CE. \
        Free, headless, ~200 MB RAM. Downloads an Ubuntu rootfs (~500 MB).",
},
PromptOption {                       // renamed from key "install"
    key: "install_docker_desktop",
    label: "Install Docker Compose via winget (requires Docker Desktop)",
    description: "Runs `winget install --id Docker.DockerCompose --silent`. \
        Requires Docker Desktop or another Docker Engine already running.",
},
PromptOption {
    key: "abort",
    label: "Abort — I'll set up Docker manually",
    description: "Default. Re-run the installer once docker is on PATH.",
},
```
`default_choice()` stays `"abort"`.

Updated Windows `DockerMissing` context:
> "docker isn't on PATH. The stack (FalkorDB / Redis / observability) needs it.
> Install Docker Engine in WSL2 (lightweight, free) or install Docker Desktop
> manually from docs.docker.com."

### `PromptAnswers` helper
```rust
impl PromptAnswers {
    /// True when docker ops should route through the WSL2 ag-ubuntu distro.
    pub fn use_wsl2_docker(&self) -> bool {
        matches!(self.choice(PromptId::DockerMissing), Some("install_wsl2_docker"))
    }
}
```

---

## 8. `install_steps.rs` Changes

```rust
#[cfg(windows)]
pub const INSTALL_DOCKER_STEP_NAME: &str = "Install Docker Desktop"; // was "Install Docker Compose"
#[cfg(windows)]
pub const INSTALL_WSL2_DOCKER_STEP_NAME: &str = "Install WSL2 Docker Engine";
```

Dispatch (replaces the current `matches!(…, Some("install"))` block at line 177):
```rust
#[cfg(windows)]
match answers.choice(PromptId::DockerMissing) {
    Some("install_docker_desktop") =>
        step!(INSTALL_DOCKER_STEP_NAME, crate::platform::install_docker(&tx, &tee)),
    Some("install_wsl2_docker") =>
        step!(INSTALL_WSL2_DOCKER_STEP_NAME,
              crate::platform::install_docker_wsl2(&paths, &tx, &tee)),
    _ => {}
}
```

Updated `install_stack` call (line 199):
```rust
step!("FalkorDB native service",
      crate::platform::install_stack(&paths, &tx, &tee, &answers));
```

---

## 9. Decision: terminal `docker` wrappers

**Recommendation: skip wrapper scripts in this PR** (see §4.i). They are not on
PATH, nothing in the install/runtime path uses them, and they would make
`probe_docker()` falsely report "on PATH" only if we also mutate the user PATH.
If a terminal shim is desired, ship it separately with the PATH change and route
detection through it. This keeps the WSL2 PR focused and avoids dead files in
`bin_dir` (which would also need adding to `uninstall_targets`).

---

## 10. Detection UI (`app.rs`)

**Do not** put a `#[cfg(windows)]` attribute on a `vec!` element — attributes on
expressions are unstable and won't compile on stable, and it breaks the
`cfg!(windows)` convention every other row in this function uses. Instead, build
the vec, then push the Windows row:
```rust
pub fn detection_rows(d: &DetectionResult) -> Vec<DetectionRow> {
    let mut rows = vec![ /* ... existing rows ... */ ];

    #[cfg(windows)]
    rows.push(DetectionRow {
        label: "WSL2 Docker Engine",
        value: if let Some(v) = &d.wsl2_docker_version {
            format!("installed in WSL2 ({v})")
        } else if d.wsl2_available {
            "WSL2 available — Docker Engine not yet installed".to_string()
        } else {
            "WSL2 not detected — enable via Windows Features for lightweight Docker".to_string()
        },
        status: DetectionStatus::Ok, // informational; the Docker row is the real blocker
    });

    rows
}
```
(The fields are valid on both platforms — always `None`/`false` on Linux — but
the row itself is Windows-only, gated by `#[cfg(windows)]` on the statement.)

---

## 11. `ui/progress.rs` — step list must match steps actually run

`initial_steps()` currently pushes `INSTALL_DOCKER_STEP_NAME` for
`Some("install")`. After the key rename it must map **both** keys to their
respective step names, or the list and the real run diverge (a step stuck
Pending, or a run-step with no list row):
```rust
#[cfg(windows)]
match answers.choice(PromptId::DockerMissing) {
    Some("install_docker_desktop") => names.push(INSTALL_DOCKER_STEP_NAME),
    Some("install_wsl2_docker")    => names.push(INSTALL_WSL2_DOCKER_STEP_NAME),
    _ => {}
}
```
Import `INSTALL_WSL2_DOCKER_STEP_NAME` alongside the existing constant.

---

## 12. Uninstall (`platform/windows.rs`)

In `uninstall_managed()`, after the compose-down:
```rust
// Remove the ag-managed WSL2 distro if present (best-effort).
let _ = Command::new("wsl").args(["--unregister", "ag-ubuntu"]).status().await;
```
No wrapper-file cleanup needed (wrappers skipped per §9). If §9 is reversed
later, add the four files to `uninstall_targets()` then.

---

## 13. `platform/mod.rs`

```rust
#[cfg(windows)]
pub use windows::{
    // ... existing exports ...
    install_docker_wsl2, windows_path_to_wsl,
};
```

---

## Verification

1. **Unit tests:** `windows_path_to_wsl` cases pass.
2. **Linux build is clean:** `cargo clippy --all-targets -- -D warnings` on
   Linux — confirms the `install_stack` 4-arg parity change (§5) and the
   `#[cfg(windows)]`-statement detection row (§10) compile.
3. **WSL2 option appears:** `AG_HOME=C:\Temp\ag-test SKIP_SCHTASKS=1`, run on a
   host with WSL2 enabled → Prompts screen shows "Install Docker Engine in WSL2"
   as the first DockerMissing option.
4. **WSL2 option absent without WSL2:** force `probe_wsl2_available` false →
   prompt shows only Docker Desktop + Abort.
5. **Docker Desktop path unchanged:** choose `install_docker_desktop` → winget
   step runs, `ag-stack.xml` renders `<Command>docker</Command>`.
6. **WSL2 path (real host):** choose WSL2 → distro imported, dockerd readiness
   poll succeeds, `docker compose` brings up `ag` project inside `ag-ubuntu`.
7. **Step list matches run:** progress pane shows exactly the steps that execute
   for each of the three DockerMissing choices.
8. **fmt + clippy:** `cargo fmt && cargo clippy --all-targets -- -D warnings`
   pass on both targets.
```
