# Plan: Windows installer — DockerMissing goes from Docker Desktop to Docker Compose

## Context

The `DockerMissing` prompt in `installer/src/prompts.rs` is identical on all platforms. Its context text references `get.docker.com` (a Linux bash script) and its install option is labelled "Install via get.docker.com (requires sudo)" — both are meaningless on Windows. The `DockerMissing = "install"` choice is recorded in `PromptAnswers` but never consumed by any install step (on either platform).

The goal is to give Windows users a proper path: detect that `docker` is missing, offer to install the Docker Compose standalone binary via `winget install Docker.DockerCompose`, actually execute that step during install, and show it in the progress pane when it fires.

**Triggering condition is unchanged.** `DockerMissing` fires when `d.docker_present.is_none()`, which is driven by `probe_docker()` running `docker --version`. If Docker Engine is absent, `docker compose` certainly won't work either — so the existing probe is the right trigger. We only change what the user sees and what happens next.

## Files to change

### 1. `installer/src/prompts.rs`

Cfg-branch `DockerMissing` inside `context()` and `options()` for Windows:

**context() Windows branch:**
```
"docker compose isn't on PATH. The stack (FalkorDB / Redis / observability) \
needs it. Install the Docker Compose standalone binary via winget, \
or install it manually from docs.docker.com/compose."
```

**options() Windows branch** (same key `"install"` so consumption code is platform-neutral):
```rust
PromptOption {
    key: "install",
    label: "Install Docker Compose via winget",
    description: "Runs `winget install --id Docker.DockerCompose --silent`. \
        Requires Docker Engine (Docker Desktop or WSL2) to be running.",
},
PromptOption {
    key: "abort",
    label: "Abort — I'll install Docker Compose manually",
    description: "Default. Re-run the installer once docker is on PATH.",
},
```

Unix branch stays identical to current code.

`default_choice` stays `"abort"` on both platforms.

### 2. `installer/src/platform/windows.rs`

Add a new exported async fn at the end of the file:

```rust
pub async fn install_docker(tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    let step = "Install Docker Compose";
    if skip_systemctl() {
        step_log(tx, tee, step,
            "SKIP_SCHTASKS=1 — would run: winget install --id Docker.DockerCompose --silent");
        return Ok(());
    }
    let out = Command::new("winget")
        .args(["install", "--id", "Docker.DockerCompose", "--silent",
               "--accept-package-agreements", "--accept-source-agreements"])
        .output()
        .await
        .with_context(|| "spawn winget install Docker.DockerCompose")?;
    if !out.status.success() {
        bail!("winget install Docker.DockerCompose exited {}\nstderr: {}",
              out.status, String::from_utf8_lossy(&out.stderr).trim());
    }
    step_log(tx, tee, step, "Docker Compose installed via winget");

    // Verify with `docker compose version`, not `docker --version`.
    // `winget install Docker.DockerCompose` installs the compose binary only —
    // Docker Engine is separate, so `docker --version` would still fail even
    // on a successful compose install, giving a false WARN.
    let compose_ok = Command::new("docker")
        .args(["compose", "version"])
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());
    match compose_ok {
        Some(v) => step_log(tx, tee, step, format!("verified: {}", v.trim())),
        None => step_log(tx, tee, step,
            "WARN: docker compose still not responding — you may need to reopen your terminal"),
    }
    Ok(())
}
```

### 3. `installer/src/platform/mod.rs`

Add Windows-only export:
```rust
#[cfg(windows)]
pub use windows::install_docker;
```

### 4. `installer/src/install_steps.rs`

Add a Windows-only step name constant (used by `progress.rs` to build the step list dynamically):
```rust
#[cfg(windows)]
pub const INSTALL_DOCKER_STEP_NAME: &str = "Install Docker Compose";
```

In `run()`, prepend a conditional step before "Ensure XDG tree". The `PromptId` import is **already present** at line 40 — no new import needed:
```rust
#[cfg(windows)]
if matches!(answers.choice(PromptId::DockerMissing), Some("install")) {
    step!(
        INSTALL_DOCKER_STEP_NAME,
        crate::platform::install_docker(&tx, &tee)
    );
}
```

### 5. `installer/src/ui/progress.rs`

Add `INSTALL_DOCKER_STEP_NAME` to the existing `install_steps` import at line 21:
```rust
// before:
use crate::install_steps::{self, ProgressEvent, STEP_NAMES};
// after:
use crate::install_steps::{self, ProgressEvent, INSTALL_DOCKER_STEP_NAME, STEP_NAMES};
```

Change `initial_steps` from a zero-arg fn to one that takes `&PromptAnswers`, so the step list includes "Install Docker Compose" only when that step will actually run:

```rust
use crate::prompts::PromptId;

fn initial_steps(answers: &PromptAnswers) -> Vec<InstallStep> {
    let mut names: Vec<&'static str> = Vec::new();
    #[cfg(windows)]
    if matches!(answers.choice(PromptId::DockerMissing), Some("install")) {
        names.push(INSTALL_DOCKER_STEP_NAME);
    }
    names.extend_from_slice(STEP_NAMES);
    names.iter().map(|&name| InstallStep {
        name,
        status: StepStatus::Pending,
        duration_s: 0,
    }).collect()
}
```

Update the call site in `ProgressScreen` (replacing the current `use_signal(initial_steps)` at line 28):
```rust
let answers_for_steps = answers_signal.read().clone();
let mut steps = use_signal(move || initial_steps(&answers_for_steps));
```

Update the hardcoded "Six steps" in the subtitle to be dynamic:
```rust
let step_count = steps.read().len();
// render as "{step_count} steps. ..."
```

## Verification

1. **Sandbox test (no docker, DockerMissing fires):**
   ```pwsh
   $env:AG_HOME = "C:\Temp\ag-test"
   $env:SKIP_SCHTASKS = "1"
   cargo run -p ag-installer --target x86_64-pc-windows-msvc
   ```
   - Detection screen: Docker row should show "not on PATH" / Warn
   - Prompts screen: "Docker is missing" card should show "docker compose isn't on PATH…" text with the winget option
   - Selecting "install" + "Begin install" → progress screen should show 7 steps with "Install Docker Compose" first

2. **Prompt text check:** Confirm the Linux build still shows `get.docker.com` text (run `cargo clippy --all-targets` on Linux or in CI).

3. **`cargo fmt && cargo clippy --all-targets -- -D warnings`** must pass on both platforms (or at minimum on the CI target).
