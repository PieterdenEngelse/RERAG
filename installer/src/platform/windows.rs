//! Windows platform impls.
//!
//! Mirrors `platform::linux` for Windows hosts. Paths land under
//! `%LOCALAPPDATA%\ag` (binaries / data / logs) and `%APPDATA%\ag`
//! (env file / docker-compose.yml / scheduled-task XML). Detection
//! probes use Win32 APIs (`winreg`, `fs2`, `sysinfo`) and `schtasks`
//! / `docker` shellouts in place of Linux's `/proc`, `ss`, `systemctl`,
//! and `redis-cli`.
//!
//! Sandbox-testing recipe (parity with Linux's
//! `HOME=/tmp/ag-test SKIP_SYSTEMCTL=1 cargo run`):
//!
//! ```pwsh
//! $env:AG_HOME = "C:\Temp\ag-test"
//! $env:SKIP_SCHTASKS = "1"
//! cargo run -p ag-installer
//! ```

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use tokio::process::Command;
use tokio::time::sleep;

use crate::bundled;
use crate::detection::{DetectionResult, BACKEND_PORT};
use crate::install_steps::{
    render_template, step_log, LogTee, ProgressEvent, ProgressSender, FALKORDB_PORT,
};
use crate::prompts::{PromptAnswers, PromptId};
use crate::uninstall::{rm_dir_quiet, rm_quiet};

// =============================================================================
// Paths (PR2.2)
// =============================================================================

/// Install path resolution.
///
/// All install destinations derive from `%LOCALAPPDATA%` and `%APPDATA%`.
/// `AG_HOME` is the only env-var override — parity with Linux. Sandbox
/// testing redirects everything via `AG_HOME=C:\Temp\ag-test`.
///
/// `SKIP_SCHTASKS=1` is *not* a path override — it gates the `schtasks`
/// shellouts in `install_service`. Documented here because the sandbox
/// recipe needs it set alongside `AG_HOME`.
#[derive(Clone, Debug)]
pub struct Paths {
    /// `%LOCALAPPDATA%\ag` (or `AG_HOME`). Holds runtime state: bin/, lib/,
    /// data/, index/, db/, logs/, cache/, locks/, web/.
    pub ag_home: PathBuf,
    /// `%LOCALAPPDATA%\ag\bin`. `ag.exe` + `ag-start.cmd` wrapper land here.
    pub bin_dir: PathBuf,
    /// `%LOCALAPPDATA%\ag\lib`. `tika_native.dll` lands here.
    pub lib_dir: PathBuf,
    /// `%APPDATA%\ag`. `ag.env`, `docker-compose.yml` live here.
    pub config_dir: PathBuf,
    /// `%APPDATA%\ag\scheduled-tasks`. Rendered Task XML files live here for
    /// re-import / drift detection (analog of Linux's `systemd_user_dir`).
    pub scheduled_tasks_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        // Default-Profile fallback only fires when both env vars are missing,
        // which is unusual on a real interactive Windows session. Documents
        // the resolution chain explicitly rather than panicking.
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

    pub fn ag_env(&self) -> PathBuf {
        self.config_dir.join("ag.env")
    }

    pub fn docker_compose(&self) -> PathBuf {
        self.config_dir.join("docker-compose.yml")
    }

    pub fn ag_exe(&self) -> PathBuf {
        self.bin_dir.join("ag.exe")
    }

    pub fn ag_start_cmd(&self) -> PathBuf {
        self.bin_dir.join("ag-start.cmd")
    }

    pub fn ag_task_xml(&self) -> PathBuf {
        self.scheduled_tasks_dir.join("ag.xml")
    }

    pub fn ag_stack_task_xml(&self) -> PathBuf {
        self.scheduled_tasks_dir.join("ag-stack.xml")
    }

    pub fn install_log(&self, timestamp_utc: &str) -> PathBuf {
        self.ag_home
            .join("logs")
            .join(format!("install-{timestamp_utc}.log"))
    }
}

/// Windows analog of the Linux `SKIP_SYSTEMCTL` sandbox gate. Set to any
/// non-empty value to make the `schtasks` / `docker compose up` shellouts
/// log what they would do instead of touching the real Task Scheduler /
/// Docker daemon.
pub fn skip_systemctl() -> bool {
    std::env::var("SKIP_SCHTASKS")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

// =============================================================================
// Detection (PR2.2)
// =============================================================================
//
// Mirrors `platform::linux::run_detection`. Every probe returns the
// "not present" value (`false` / `None` / `0`) on any failure — missing
// tool, non-zero exit, parse failure — rather than propagating errors.
// Detection is best-effort.

pub async fn run_detection() -> DetectionResult {
    let paths = Paths::resolve();
    let (
        docker_present,
        ollama_active,
        compose_up,
        ag_env_exists,
        falkordb_healthy,
        backend_port_busy,
        system_redis,
        ag_service_drift,
        disk_free_gb,
        ram_gb,
        distro,
        wsl2_available,
        wsl2_docker_version,
        wsl2_distro_name,
    ) = tokio::join!(
        probe_docker(),
        probe_ollama_active(),
        probe_compose_up(),
        probe_ag_env_exists(&paths),
        probe_falkordb_healthy(),
        probe_backend_port_busy(BACKEND_PORT),
        probe_system_redis(),
        probe_ag_task_drift(&paths),
        probe_disk_free_gb(&paths),
        probe_ram_gb(),
        probe_distro(),
        probe_wsl2_available(),
        probe_wsl2_docker(),
        probe_wsl2_distro_name(),
    );
    DetectionResult {
        docker_present,
        ollama_active,
        compose_up,
        falkordb_healthy,
        ag_env_exists,
        backend_port_busy,
        system_redis,
        // No native loki / tempo / otelcol units on Windows — the observability
        // stack only exists inside the compose project.
        native_obs: Vec::new(),
        ag_service_drift,
        disk_free_gb,
        ram_gb,
        distro,
        wsl2_available,
        wsl2_docker_version,
        wsl2_distro_name,
    }
}

/// `wsl --status` exits 0 → WSL2 feature is enabled. `wsl.exe` is a
/// System32 shim present even when the optional feature isn't installed;
/// in that case `--status` exits non-zero, so `.success()` is the gate.
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

/// True when the ag-managed `ag-ubuntu` distro is registered. `wsl --list
/// --quiet` emits UTF-16LE; strip NUL bytes for an ASCII compare.
async fn probe_wsl2_distro_name() -> Option<String> {
    let out = Command::new("wsl")
        .args(["--list", "--quiet"])
        .output()
        .await
        .ok()?;
    let text = String::from_utf8(out.stdout.clone()).unwrap_or_else(|_| {
        let ascii: Vec<u8> = out.stdout.into_iter().filter(|&b| b != 0).collect();
        String::from_utf8_lossy(&ascii).into_owned()
    });
    text.lines()
        .any(|l| l.trim() == "ag-ubuntu")
        .then(|| "ag-ubuntu".to_string())
}

/// Docker Engine version inside the `ag-ubuntu` distro. Probes that distro
/// specifically (not the default) — install and runtime both target
/// `-d ag-ubuntu`, so detection must check the same place.
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

async fn probe_docker() -> Option<String> {
    let out = Command::new("docker")
        .arg("--version")
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let version = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

/// HTTP GET `/api/tags`. Semantics shift from Linux's `systemctl is-active`
/// ("process running") to "responding" — equally informative for the
/// detection screen. Row label adapts via `cfg!(windows)` in `app.rs`.
async fn probe_ollama_active() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    match client.get("http://127.0.0.1:11434/api/tags").send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

async fn probe_compose_up() -> bool {
    let Ok(out) = Command::new("docker")
        .args(["compose", "ls"])
        .env("COMPOSE_PROJECT_NAME", "ag")
        .output()
        .await
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|line| line.split_whitespace().next() == Some("ag"))
}

async fn probe_ag_env_exists(paths: &Paths) -> bool {
    tokio::fs::metadata(paths.ag_env()).await.is_ok()
}

async fn probe_falkordb_healthy() -> bool {
    let Ok(out) = Command::new("docker")
        .args([
            "inspect",
            "ag-falkordb",
            "--format",
            "{{.State.Health.Status}}",
        ])
        .output()
        .await
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    String::from_utf8_lossy(&out.stdout).trim() == "healthy"
}

/// Cross-platform port-busy probe: try to bind. `AddrInUse` → busy. Bind
/// runs in a blocking task to keep the tokio reactor healthy if the OS
/// stalls (rare, but the original `ss` probe also blocked for ~ms).
async fn probe_backend_port_busy(port: u16) -> bool {
    // bind succeeds → port was free (not busy); bind fails (AddrInUse) →
    // assume something else owns the port.
    tokio::task::spawn_blocking(move || TcpListener::bind(("127.0.0.1", port)).is_err())
        .await
        .unwrap_or(false)
}

/// Raw RESP `PING` over a TCP socket — no `redis-cli` on Windows by
/// default. Send `*1\r\n$4\r\nPING\r\n`, expect `+PONG\r\n`. Times out
/// fast so we don't stall detection on a slow / firewalled localhost.
async fn probe_system_redis() -> bool {
    tokio::task::spawn_blocking(|| -> bool {
        let Ok(addr) = "127.0.0.1:6379".parse::<SocketAddr>() else {
            return false;
        };
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
        if stream.write_all(b"*1\r\n$4\r\nPING\r\n").is_err() {
            return false;
        }
        let mut buf = [0u8; 7];
        if stream.read_exact(&mut buf).is_err() {
            return false;
        }
        &buf == b"+PONG\r\n"
    })
    .await
    .unwrap_or(false)
}

/// `schtasks /Query /TN ag /XML` returns the registered Task's XML. If
/// the `<Command>` element doesn't point at our `ag-start.cmd`, the task
/// has drifted (manually edited or installed by a previous tool). Lightweight
/// string slice — one element, no full XML parser.
async fn probe_ag_task_drift(paths: &Paths) -> bool {
    let Ok(out) = Command::new("schtasks")
        .args(["/Query", "/TN", "ag", "/XML"])
        .output()
        .await
    else {
        // No schtasks tool (extremely rare) → no drift to flag.
        return false;
    };
    if !out.status.success() {
        // Task not registered → no drift.
        return false;
    }
    let xml = String::from_utf8_lossy(&out.stdout);
    let expected = paths.ag_start_cmd().display().to_string();
    let open = "<Command>";
    let close = "</Command>";
    let cmd_text = xml
        .find(open)
        .and_then(|start| {
            let after = &xml[start + open.len()..];
            after.find(close).map(|end| &after[..end])
        })
        .unwrap_or("");
    // Case-insensitive compare — Windows path normalization is messy
    // (forward vs back slashes, drive-letter casing).
    cmd_text.trim().to_lowercase() != expected.trim().to_lowercase()
}

async fn probe_disk_free_gb(paths: &Paths) -> u64 {
    let target = paths
        .ag_home
        .parent()
        .unwrap_or(&paths.ag_home)
        .to_path_buf();
    tokio::task::spawn_blocking(move || fs2::available_space(&target).unwrap_or(0) >> 30)
        .await
        .unwrap_or(0)
}

async fn probe_ram_gb() -> u64 {
    tokio::task::spawn_blocking(|| {
        // sysinfo 0.32: total_memory() returns bytes (changed from KB in 0.30).
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        sys.total_memory() >> 30
    })
    .await
    .unwrap_or(0)
}

/// Reads `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` for
/// `ProductName` ("Windows 11 Pro", "Windows 10 Enterprise", …) and
/// `DisplayVersion` ("23H2", "22H2", …). Returns `None` if the registry
/// is unreadable or the values are absent.
async fn probe_distro() -> Option<String> {
    tokio::task::spawn_blocking(|| -> Option<String> {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm
            .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            .ok()?;
        let product: String = key.get_value("ProductName").ok()?;
        let display: Result<String, _> = key.get_value("DisplayVersion");
        match display {
            Ok(d) if !d.is_empty() => Some(format!("{product} {d}")),
            _ => Some(product),
        }
    })
    .await
    .ok()
    .flatten()
}

// =============================================================================
// Step 1: ensure_install_tree (PR2.3)
// =============================================================================

pub async fn ensure_install_tree(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
    log_path_out: &mut Option<PathBuf>,
) -> Result<()> {
    // No `falkordb/` subdir on Windows — FalkorDB runs in compose, not
    // as a native binary tree under ag_home like on Linux.
    let dirs: Vec<PathBuf> = vec![
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
    for d in &dirs {
        fs::create_dir_all(d).with_context(|| format!("create dir {}", d.display()))?;
        step_log(
            tx,
            tee,
            "Ensure XDG tree",
            format!("created {}", d.display()),
        );
    }

    // Open the install log AFTER the logs/ dir exists.
    let ts = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let log_path = paths.install_log(&ts);
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open install log {}", log_path.display()))?;
    tee.set(file);
    step_log(
        tx,
        tee,
        "Ensure XDG tree",
        format!("install log: {}", log_path.display()),
    );
    *log_path_out = Some(log_path);
    Ok(())
}

// =============================================================================
// Step 3: copy_artifacts (PR2.3)
// =============================================================================

pub async fn copy_artifacts(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    // ag.exe
    let ag_src = bundled::ag_binary_path();
    if !ag_src.exists() {
        bail!(
            "ag.exe missing at {} — build it first \
            (cargo build --release -p ag --target x86_64-pc-windows-msvc)",
            ag_src.display()
        );
    }
    let ag_target = paths.ag_exe();
    fs::copy(&ag_src, &ag_target)
        .with_context(|| format!("copy {} → {}", ag_src.display(), ag_target.display()))?;
    step_log(
        tx,
        tee,
        "Install artifacts",
        format!("installed {}", ag_target.display()),
    );

    // tika_native.dll (optional)
    if let Some(src) = bundled::libtika_path() {
        let dst = paths.lib_dir.join("tika_native.dll");
        fs::copy(&src, &dst)
            .with_context(|| format!("copy {} → {}", src.display(), dst.display()))?;
        step_log(
            tx,
            tee,
            "Install artifacts",
            format!("installed {} (from {})", dst.display(), src.display()),
        );
    } else {
        step_log(
            tx,
            tee,
            "Install artifacts",
            "tika_native.dll not bundled — PDF parsing will use fallback",
        );
    }

    // Frontend dist — robocopy /MIR replaces Linux's rsync. Built into
    // Windows since Vista; exit code 0..=7 = success, ≥8 = failure.
    let frontend = bundled::frontend_dist_dir();
    if frontend.exists() && frontend.is_dir() {
        let dst = paths.ag_home.join("web");
        fs::create_dir_all(&dst)?;
        let src_arg = frontend.display().to_string();
        let dst_arg = dst.display().to_string();
        let status = Command::new("robocopy")
            .args([
                &src_arg, &dst_arg, "/MIR", "/NJH", "/NJS", "/NDL", "/NP", "/R:2", "/W:1",
            ])
            .status()
            .await
            .with_context(|| "spawn robocopy")?;
        let code = status.code().unwrap_or(16);
        if code > 7 {
            bail!("robocopy exited {code} (>7 = failure)");
        }
        step_log(
            tx,
            tee,
            "Install artifacts",
            format!("robocopy {} → {} (exit {})", src_arg, dst_arg, code),
        );
    } else {
        step_log(
            tx,
            tee,
            "Install artifacts",
            format!(
                "frontend dist not present at {} — skipping (dashboard won't load until built)",
                frontend.display()
            ),
        );
    }

    // ag-start.cmd wrapper — verbatim CRLF, no template substitution.
    // Sets AG_ENV (consumed by backend/src/main.rs's dotenvy hook from
    // PR2.5) then launches ag.exe in the background.
    let wrapper = paths.ag_start_cmd();
    fs::write(
        &wrapper,
        b"@echo off\r\nset \"AG_ENV=%APPDATA%\\ag\\ag.env\"\r\nstart \"\" /B \"%~dp0ag.exe\"\r\n",
    )
    .with_context(|| format!("write {}", wrapper.display()))?;
    step_log(
        tx,
        tee,
        "Install artifacts",
        format!("wrote wrapper {}", wrapper.display()),
    );

    // Smoke-test ag.exe. PATH includes lib_dir so tika_native.dll resolves
    // (Windows DLL loader checks the binary's directory and PATH; no LD_LIBRARY_PATH).
    let path_env = format!(
        "{};{}",
        paths.lib_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new(&ag_target)
        .arg("--version")
        .env("PATH", &path_env)
        .output()
        .await
        .with_context(|| format!("spawn {} --version", ag_target.display()))?;
    if !out.status.success() {
        bail!(
            "smoke-test failed: {} --version exited {}\nstderr: {}",
            ag_target.display(),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
    step_log(
        tx,
        tee,
        "Install artifacts",
        format!("smoke-test OK: {ver}"),
    );
    Ok(())
}

// =============================================================================
// Step 4: install_stack — docker compose with falkor-container profile (PR2.3)
// =============================================================================

/// Dispatch by Docker mode: WSL2 ag-ubuntu distro vs native (Docker Desktop
/// or any Engine on the host PATH).
pub async fn install_stack(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
    answers: &PromptAnswers,
) -> Result<()> {
    if answers.use_wsl2_docker() {
        install_stack_wsl2(paths, tx, tee).await
    } else {
        install_stack_native(paths, tx, tee).await
    }
}

async fn install_stack_native(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    // No FalkorDB native binaries on Windows — the compose stack carries
    // a `falkordb` service under the `falkor-container` profile (added
    // to docker-compose.yml in PR 2.5).
    let compose = paths.docker_compose();
    if !compose.exists() {
        bail!(
            "docker-compose.yml missing at {} — seed_config should have copied it",
            compose.display()
        );
    }
    let compose_str = compose.display().to_string();

    if skip_systemctl() {
        step_log(
            tx,
            tee,
            "FalkorDB native service",
            format!(
                "SKIP_SCHTASKS=1 — would run: docker compose -f {compose_str} \
                --profile \"\" --profile falkor-container up -d"
            ),
        );
        return Ok(());
    }

    // Match the activation set the ag-stack scheduled task will use on
    // subsequent logons: `--profile ""` brings up Redis + observability
    // (services with `profiles: ["", …]`), and `--profile falkor-
    // container` adds FalkorDB (Windows-only path — no native binary).
    // Step 4 here brings up the FULL stack; LowRam pruning only affects
    // the ag-stack scheduled task rendered in step 5.
    let out = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_str,
            "--profile",
            "",
            "--profile",
            "falkor-container",
            "up",
            "-d",
        ])
        .env("COMPOSE_PROJECT_NAME", "ag")
        .output()
        .await
        .with_context(|| "spawn docker compose up")?;
    if !out.status.success() {
        bail!(
            "docker compose up exited {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    step_log(
        tx,
        tee,
        "FalkorDB native service",
        format!("brought up ag-falkordb container via {compose_str}"),
    );
    Ok(())
}

/// WSL2 path: run `docker compose` inside the `ag-ubuntu` distro. The
/// compose file lives on the Windows side, so its path is translated to
/// the distro's `/mnt/<drive>/…` view before being passed with `-f`.
async fn install_stack_wsl2(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    let compose = paths.docker_compose();
    if !compose.exists() {
        bail!(
            "docker-compose.yml missing at {} — seed_config should have copied it",
            compose.display()
        );
    }
    let compose_wsl = windows_path_to_wsl(&compose.display().to_string());

    if skip_systemctl() {
        step_log(
            tx,
            tee,
            "FalkorDB native service",
            format!(
                "SKIP_SCHTASKS=1 — would run: wsl -d ag-ubuntu -u root -- docker compose \
                -f {compose_wsl} --profile \"\" --profile falkor-container up -d"
            ),
        );
        return Ok(());
    }

    let out = Command::new("wsl")
        .args([
            "-d",
            "ag-ubuntu",
            "-u",
            "root",
            "--",
            "docker",
            "compose",
            "-f",
            &compose_wsl,
            "--profile",
            "",
            "--profile",
            "falkor-container",
            "up",
            "-d",
        ])
        .env("COMPOSE_PROJECT_NAME", "ag")
        .output()
        .await
        .with_context(|| "spawn wsl docker compose up")?;
    if !out.status.success() {
        bail!(
            "wsl docker compose up exited {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    step_log(
        tx,
        tee,
        "FalkorDB native service",
        format!("brought up ag-falkordb container via WSL2 (ag-ubuntu) using {compose_wsl}"),
    );
    Ok(())
}

// =============================================================================
// Step 5: install_service — render + register Scheduled Tasks (PR2.3)
// =============================================================================

pub async fn install_service(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
    answers: &PromptAnswers,
    backend_port: u16,
) -> Result<()> {
    // ag task — honor AgInstallDrift prompt choice. The PromptId's
    // title/context/options cfg-branch to say "scheduled task" instead
    // of "service" on Windows.
    let ag_task = paths.ag_task_xml();
    let drift_choice = answers.choice(PromptId::AgInstallDrift).unwrap_or("keep");
    let install_ag_task = match drift_choice {
        "keep" if ag_task.exists() => {
            step_log(
                tx,
                tee,
                "Systemd user units",
                format!("keeping existing {} per prompt choice", ag_task.display()),
            );
            false
        }
        "backup" if ag_task.exists() => {
            let ts = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
            let bak = paths.scheduled_tasks_dir.join(format!("ag.xml.bak-{ts}"));
            fs::rename(&ag_task, &bak)
                .with_context(|| format!("rename {} → {}", ag_task.display(), bak.display()))?;
            step_log(
                tx,
                tee,
                "Systemd user units",
                format!("backed up {} → {}", ag_task.display(), bak.display()),
            );
            true
        }
        _ => true,
    };

    let user_id = current_user_id();

    if install_ag_task {
        let tmpl = bundled::scheduled_tasks_template_dir().join("ag.xml.tmpl");
        render_template(
            &tmpl,
            &ag_task,
            &[
                ("AG_BIN", paths.ag_start_cmd().display().to_string()),
                ("AG_HOME", paths.ag_home.display().to_string()),
                ("USER", user_id.clone()),
            ],
        )
        .with_context(|| "render ag.xml")?;
        step_log(
            tx,
            tee,
            "Systemd user units",
            format!(
                "rendered {} (backend_port={})",
                ag_task.display(),
                backend_port
            ),
        );
        register_task(tx, tee, "ag", &ag_task).await?;
    }

    // ag-stack task — skipped when LowRam = none.
    let skip_stack = matches!(answers.choice(PromptId::LowRam), Some("none"));
    if skip_stack {
        step_log(
            tx,
            tee,
            "Systemd user units",
            "ag-stack task skipped (user chose no stack)",
        );
    } else {
        let profile = match answers.choice(PromptId::LowRam) {
            Some("core") => "core",
            Some("observability") => "observability",
            _ => "", // "all" / no LowRam prompt → bring up everything
        };
        let profile_args = if profile.is_empty() {
            "--profile \"\" --profile falkor-container".to_string()
        } else {
            format!("--profile {profile} --profile falkor-container")
        };
        // Mirror Linux's behavior (systemd/ag-stack.service.tmpl): the
        // default services in docker-compose.yml have
        // `profiles: ["", "<name>"]`, so the empty-string profile is the
        // activation token for "include the default stack".
        //
        // The scheduled-task <Command> differs by Docker mode:
        //  - native:  docker compose -f "<win_path>" <profiles> up -d
        //  - WSL2:    wsl -d ag-ubuntu -u root -- docker compose
        //             -f <wsl_path> <profiles> up -d
        let use_wsl2 = answers.use_wsl2_docker();
        let (stack_command, stack_args) = if use_wsl2 {
            let compose_wsl = windows_path_to_wsl(&paths.docker_compose().display().to_string());
            (
                "wsl".to_string(),
                format!(
                    "-d ag-ubuntu -u root -- docker compose -f {compose_wsl} {profile_args} up -d"
                ),
            )
        } else {
            (
                "docker".to_string(),
                format!(
                    "compose -f \"{}\" {profile_args} up -d",
                    paths.docker_compose().display()
                ),
            )
        };
        let stack_task = paths.ag_stack_task_xml();
        let tmpl = bundled::scheduled_tasks_template_dir().join("ag-stack.xml.tmpl");
        render_template(
            &tmpl,
            &stack_task,
            &[
                ("STACK_COMMAND", stack_command),
                ("STACK_ARGS", stack_args),
                ("AG_HOME", paths.ag_home.display().to_string()),
                ("USER", user_id),
            ],
        )
        .with_context(|| "render ag-stack.xml")?;
        step_log(
            tx,
            tee,
            "Systemd user units",
            format!(
                "rendered {} (profile={}, docker={})",
                stack_task.display(),
                if profile.is_empty() { "<all>" } else { profile },
                if use_wsl2 { "wsl2" } else { "native" }
            ),
        );
        register_task(tx, tee, "ag-stack", &stack_task).await?;
    }

    // Start the ag task immediately so the user sees the dashboard come
    // up without waiting for next logon. ag-stack will be triggered by
    // the same logon flow on next sign-in.
    schtasks(tx, tee, "Systemd user units", &["/Run", "/TN", "ag"]).await?;
    Ok(())
}

/// Delete-then-create: `schtasks /Create /F` is unreliable on some
/// Windows builds (leaves half-updated state). Best-effort `/Delete`
/// first (ignored if the task doesn't exist), then `/Create`.
async fn register_task(
    tx: &ProgressSender,
    tee: &LogTee,
    name: &str,
    xml: &std::path::Path,
) -> Result<()> {
    // Best-effort delete — log but don't fail on "task not found".
    let _ = schtasks_quiet(&["/Delete", "/TN", name, "/F"]).await;
    schtasks(
        tx,
        tee,
        "Systemd user units",
        &["/Create", "/XML", &xml.display().to_string(), "/TN", name],
    )
    .await
}

/// Run `schtasks <args>`. Honors `SKIP_SCHTASKS=1` the same way Linux's
/// `systemctl_user` honors `SKIP_SYSTEMCTL=1`.
async fn schtasks(
    tx: &ProgressSender,
    tee: &LogTee,
    step_name: &'static str,
    args: &[&str],
) -> Result<()> {
    let pretty = format!("schtasks {}", args.join(" "));
    if skip_systemctl() {
        step_log(
            tx,
            tee,
            step_name,
            format!("SKIP_SCHTASKS=1 — would run: {pretty}"),
        );
        return Ok(());
    }
    let out = Command::new("schtasks")
        .args(args)
        .output()
        .await
        .with_context(|| format!("spawn {pretty}"))?;
    if !out.status.success() {
        bail!(
            "{pretty} exited {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    step_log(tx, tee, step_name, format!("ran: {pretty}"));
    Ok(())
}

/// Best-effort `schtasks` for cleanup paths (delete-before-create,
/// uninstall). Swallows errors; returns the exit status.
async fn schtasks_quiet(args: &[&str]) -> Option<std::process::ExitStatus> {
    Command::new("schtasks")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .ok()
}

/// `<UserId>` for the rendered Scheduled-Task XML. The Task Scheduler
/// XSD requires the format `DOMAIN\username` or just `username` — we
/// use the `USERNAME` env var (set by every interactive Windows
/// session). Empty falls back to a placeholder that schtasks will
/// reject loudly, which is better than silently registering for the
/// wrong user.
fn current_user_id() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| String::from("UNKNOWN-USER"))
}

// =============================================================================
// Uninstall (PR2.3)
// =============================================================================

pub fn uninstall_targets(paths: &Paths) -> Vec<PathBuf> {
    vec![
        paths.ag_exe(),
        paths.ag_start_cmd(),
        paths.lib_dir.join("tika_native.dll"),
        paths.ag_task_xml(),
        paths.ag_stack_task_xml(),
    ]
}

pub async fn uninstall_managed(paths: &Paths) {
    // 1. Stop + delete Scheduled Tasks. Best-effort — task-not-found is
    //    fine.
    for name in ["ag", "ag-stack"] {
        if skip_systemctl() {
            println!("  SKIP_SCHTASKS=1 — would run: schtasks /End /TN {name}");
            println!("  SKIP_SCHTASKS=1 — would run: schtasks /Delete /TN {name} /F");
            continue;
        }
        let _ = schtasks_quiet(&["/End", "/TN", name]).await;
        let result = schtasks_quiet(&["/Delete", "/TN", name, "/F"]).await;
        match result {
            Some(s) if s.success() => println!("  removed scheduled task {name}"),
            _ => {} // task not present — idempotent uninstall
        }
    }

    // 2. Bring the compose stack down so ag-falkordb / ag-redis containers
    //    aren't left orphaned. Best-effort.
    if !skip_systemctl() {
        let compose = paths.docker_compose();
        if compose.exists() {
            let _ = Command::new("docker")
                .args([
                    "compose",
                    "-f",
                    &compose.display().to_string(),
                    "--profile",
                    "falkor-container",
                    "down",
                ])
                .env("COMPOSE_PROJECT_NAME", "ag")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;
            println!("  brought down compose project ag");
        }
    }

    // 3. Remove the ag-managed WSL2 distro if present (best-effort). This
    //    tears down the in-distro Docker Engine + all ag containers at once;
    //    the native `docker compose down` above is a no-op when WSL2 was used.
    if skip_systemctl() {
        println!("  SKIP_SCHTASKS=1 — would run: wsl --unregister ag-ubuntu");
    } else {
        let result = Command::new("wsl")
            .args(["--unregister", "ag-ubuntu"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
        if matches!(result, Ok(s) if s.success()) {
            println!("  unregistered WSL2 distro ag-ubuntu");
        }
    }

    // 4. Remove rendered Task XML + bin/lib files.
    rm_quiet(&paths.ag_task_xml());
    rm_quiet(&paths.ag_stack_task_xml());
    rm_dir_quiet(&paths.scheduled_tasks_dir);
    rm_quiet(&paths.ag_exe());
    rm_quiet(&paths.ag_start_cmd());
    rm_quiet(&paths.lib_dir.join("tika_native.dll"));
}

// =============================================================================
// First-run: FalkorDB password change + start_ag (PR2.3)
// =============================================================================

/// Edit `ag.env`'s `FALKOR_PASSWORD=` to the new value, then
/// `docker compose up -d --force-recreate ag-falkordb` so the container
/// picks up the new env. Verify by reading the new value back via raw
/// RESP `AUTH <pass>\r\nPING\r\n` (no `redis-cli` on Windows by
/// default).
pub async fn apply_falkordb_password(
    paths: &Paths,
    tx: &ProgressSender,
    new_password: &str,
) -> Result<()> {
    let step = "Start ag";
    send_log(tx, step, "changing FalkorDB password");

    // 1. Edit ag.env line. Same logic as install_steps::edit_env_file
    //    but reused here as an inline rewrite — edit_env_file is private
    //    to install_steps.
    edit_env_in_place(&paths.ag_env(), &[("FALKOR_PASSWORD", new_password)])
        .with_context(|| format!("update FALKOR_PASSWORD in {}", paths.ag_env().display()))?;
    send_log(
        tx,
        step,
        format!("set FALKOR_PASSWORD in {}", paths.ag_env().display()),
    );

    if skip_systemctl() {
        send_log(
            tx,
            step,
            "SKIP_SCHTASKS=1 — skipping ag-falkordb recreate + PING verify",
        );
        return Ok(());
    }

    // 2. Force-recreate the container so it picks up the new env.
    let compose_str = paths.docker_compose().display().to_string();
    let out = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_str,
            "--profile",
            "falkor-container",
            "up",
            "-d",
            "--force-recreate",
            "falkordb",
        ])
        .env("COMPOSE_PROJECT_NAME", "ag")
        .output()
        .await
        .with_context(|| "spawn docker compose up --force-recreate falkordb")?;
    if !out.status.success() {
        bail!(
            "docker compose up --force-recreate falkordb exited {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    send_log(tx, step, "recreated ag-falkordb container".to_string());

    // 3. Verify with raw RESP AUTH + PING. FalkorDB exposes FALKORDB_PORT
    //    on the host (mapped to container's 6379).
    let pwd = new_password.to_string();
    let addr = format!("127.0.0.1:{FALKORDB_PORT}");
    for attempt in 1..=10u32 {
        sleep(Duration::from_millis(500)).await;
        let pwd_clone = pwd.clone();
        let addr_clone = addr.clone();
        let ok = tokio::task::spawn_blocking(move || resp_auth_ping(&addr_clone, &pwd_clone))
            .await
            .unwrap_or(false);
        if ok {
            send_log(
                tx,
                step,
                format!("AUTH + PING OK with new password (attempt {attempt})"),
            );
            return Ok(());
        }
    }
    Err(anyhow!(
        "ag-falkordb recreated but AUTH + PING with the new password did not succeed within 5s"
    ))
}

/// `schtasks /Run /TN ag` — runs the Logon-triggered task on demand.
/// The shared `/health` poll lives in `crate::first_run::start_ag_and_wait`.
pub async fn start_ag(tx: &ProgressSender, step: &'static str) -> Result<()> {
    let pretty = "schtasks /Run /TN ag".to_string();
    if skip_systemctl() {
        send_log(tx, step, format!("SKIP_SCHTASKS=1 — would run: {pretty}"));
        return Ok(());
    }
    let out = Command::new("schtasks")
        .args(["/Run", "/TN", "ag"])
        .output()
        .await
        .with_context(|| format!("spawn {pretty}"))?;
    if !out.status.success() {
        bail!(
            "{pretty} exited {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    send_log(tx, step, format!("ran: {pretty}"));
    Ok(())
}

// =============================================================================
// Helpers (PR2.3)
// =============================================================================

fn send_log(tx: &ProgressSender, name: &'static str, line: impl Into<String>) {
    let _ = tx.send(ProgressEvent::StepLog {
        name,
        line: line.into(),
    });
}

/// Same KEY=value edit-in-place as `install_steps::edit_env_file`, kept
/// private here so apply_falkordb_password doesn't need to expose the
/// install_steps helper. Lifted bodily — keep the two in sync.
fn edit_env_in_place(path: &std::path::Path, kvs: &[(&str, &str)]) -> Result<()> {
    let original =
        fs::read_to_string(path).with_context(|| format!("read env file {}", path.display()))?;
    let mut lines: Vec<String> = original.lines().map(String::from).collect();
    for (key, value) in kvs {
        let prefix = format!("{key}=");
        let mut replaced = false;
        for line in lines.iter_mut() {
            let trimmed = line.trim_start();
            if trimmed.starts_with(&prefix) || trimmed.starts_with(&format!("#{prefix}")) {
                *line = format!("{key}={value}");
                replaced = true;
                break;
            }
        }
        if !replaced {
            lines.push(format!("{key}={value}"));
        }
    }
    let mut out = lines.join("\r\n");
    if !out.ends_with('\n') {
        out.push_str("\r\n");
    }
    fs::write(path, out).with_context(|| format!("write env file {}", path.display()))?;
    Ok(())
}

// =============================================================================
// Install Docker Compose via winget
// =============================================================================

pub async fn install_docker(tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    let step = "Install Docker Desktop";
    if skip_systemctl() {
        step_log(
            tx,
            tee,
            step,
            "SKIP_SCHTASKS=1 — would run: winget install --id Docker.DockerCompose --silent",
        );
        return Ok(());
    }
    let out = Command::new("winget")
        .args([
            "install",
            "--id",
            "Docker.DockerCompose",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ])
        .output()
        .await
        .with_context(|| "spawn winget install Docker.DockerCompose")?;
    if !out.status.success() {
        bail!(
            "winget install Docker.DockerCompose exited {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
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
        None => step_log(
            tx,
            tee,
            step,
            "WARN: docker compose still not responding — you may need to reopen your terminal",
        ),
    }
    Ok(())
}

// =============================================================================
// Install Docker Engine inside a WSL2 distro (ag-ubuntu)
// =============================================================================

/// Ubuntu WSL rootfs. NOTE: cloud-images filenames have shifted across
/// releases; this is verified at runtime (a non-200 → `bail!` with the URL)
/// rather than trusted blindly.
const WSL2_ROOTFS_URL: &str =
    "https://cloud-images.ubuntu.com/wsl/noble/current/ubuntu-noble-wsl-amd64-wsl.rootfs.tar.gz";

const WSL2_DISTRO: &str = "ag-ubuntu";

/// Create the `ag-ubuntu` WSL2 distro and install Docker CE inside it.
/// Only invoked when the user picked the WSL2 Docker option, which only
/// appears when `wsl2_available` was true in detection — so the WSL2
/// feature is already enabled and no Windows restart is needed here.
pub async fn install_docker_wsl2(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
) -> Result<()> {
    let step = "Install WSL2 Docker Engine";

    if skip_systemctl() {
        step_log(
            tx,
            tee,
            step,
            format!(
                "SKIP_SCHTASKS=1 — would: wsl --set-default-version 2; \
                download {WSL2_ROOTFS_URL}; \
                wsl --import {WSL2_DISTRO} {}\\wsl\\{WSL2_DISTRO} <rootfs> --version 2; \
                install docker-ce inside the distro; write /etc/wsl.conf dockerd autostart; \
                terminate + poll `docker info`",
                paths.ag_home.display()
            ),
        );
        return Ok(());
    }

    // a. Default-version guard (fast no-op since WSL2 is already enabled).
    let _ = Command::new("wsl")
        .args(["--set-default-version", "2"])
        .output()
        .await;

    // b. Reuse an existing ag-ubuntu distro if present.
    if probe_wsl2_distro_name().await.is_some() {
        step_log(
            tx,
            tee,
            step,
            format!("existing {WSL2_DISTRO} distro found — reusing it"),
        );
    } else {
        // c. Download the Ubuntu rootfs to %TEMP%, verifying the URL first.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .with_context(|| "build http client for rootfs download")?;
        step_log(tx, tee, step, format!("downloading rootfs: {WSL2_ROOTFS_URL}"));
        let resp = client
            .get(WSL2_ROOTFS_URL)
            .send()
            .await
            .with_context(|| format!("GET {WSL2_ROOTFS_URL}"))?;
        if !resp.status().is_success() {
            bail!(
                "rootfs download returned {} for {WSL2_ROOTFS_URL}\n\
                The Ubuntu WSL rootfs filename may have changed. Download a current \
                rootfs manually, import it as `{WSL2_DISTRO}`, and re-run the installer.",
                resp.status()
            );
        }
        let bytes = resp
            .bytes()
            .await
            .with_context(|| "read rootfs response body")?;
        let rootfs_path = std::env::temp_dir().join("ag-ubuntu-rootfs.tar.gz");
        fs::write(&rootfs_path, &bytes)
            .with_context(|| format!("write rootfs to {}", rootfs_path.display()))?;
        step_log(
            tx,
            tee,
            step,
            format!(
                "downloaded {:.0} MB → {}",
                bytes.len() as f64 / (1024.0 * 1024.0),
                rootfs_path.display()
            ),
        );

        // d. Import the distro. wsl --import needs the install dir to exist.
        let install_dir = paths.ag_home.join("wsl").join(WSL2_DISTRO);
        fs::create_dir_all(&install_dir)
            .with_context(|| format!("create {}", install_dir.display()))?;
        let out = Command::new("wsl")
            .args([
                "--import",
                WSL2_DISTRO,
                &install_dir.display().to_string(),
                &rootfs_path.display().to_string(),
                "--version",
                "2",
            ])
            .output()
            .await
            .with_context(|| "spawn wsl --import")?;
        if !out.status.success() {
            bail!(
                "wsl --import exited {}\nstderr: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        step_log(
            tx,
            tee,
            step,
            format!("imported {WSL2_DISTRO} → {}", install_dir.display()),
        );

        // e. Install Docker Engine from the official APT repo.
        let install_script = r#"set -e
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq ca-certificates curl gnupg lsb-release
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
chmod a+r /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" > /etc/apt/sources.list.d/docker.list
apt-get update -qq
apt-get install -y -qq docker-ce docker-ce-cli containerd.io docker-compose-plugin
"#;
        wsl_root_bash(step, tx, tee, install_script, "install docker-ce").await?;

        // f. dockerd autostart via /etc/wsl.conf (no systemd dependency).
        let wsl_conf = "[boot]\ncommand = \"/usr/bin/dockerd --host unix:///var/run/docker.sock --log-level error &>/tmp/dockerd.log &\"\n";
        let write_conf = format!("printf '%s' '{wsl_conf}' > /etc/wsl.conf");
        wsl_root_bash(step, tx, tee, &write_conf, "write /etc/wsl.conf").await?;

        // g. Restart the distro so /etc/wsl.conf takes effect.
        let _ = Command::new("wsl")
            .args(["--terminate", WSL2_DISTRO])
            .output()
            .await;
        sleep(Duration::from_secs(2)).await;
        step_log(tx, tee, step, "terminated distro to apply /etc/wsl.conf");
    }

    // h. Readiness poll: WSL re-runs the [boot] command on each cold start
    //    and dockerd needs a moment. Poll `docker info` rather than a single
    //    check. WARN (not fatal) if it never comes up — mirrors health_check.
    let mut ready = false;
    for attempt in 1..=10u32 {
        let out = Command::new("wsl")
            .args(["-d", WSL2_DISTRO, "-u", "root", "--", "docker", "info"])
            .output()
            .await;
        if let Ok(o) = out {
            if o.status.success() {
                step_log(
                    tx,
                    tee,
                    step,
                    format!("dockerd ready in {WSL2_DISTRO} (attempt {attempt})"),
                );
                ready = true;
                break;
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    if !ready {
        step_log(
            tx,
            tee,
            step,
            "WARN: dockerd did not report ready within ~20s — it may still be starting. \
            The stack step will retry the connection.",
        );
    }
    Ok(())
}

/// Run a bash snippet as root inside the ag-ubuntu distro, failing the
/// step on a non-zero exit.
async fn wsl_root_bash(
    step: &'static str,
    tx: &ProgressSender,
    tee: &LogTee,
    script: &str,
    label: &str,
) -> Result<()> {
    let out = Command::new("wsl")
        .args(["-d", WSL2_DISTRO, "-u", "root", "--", "bash", "-c", script])
        .output()
        .await
        .with_context(|| format!("spawn wsl bash: {label}"))?;
    if !out.status.success() {
        bail!(
            "{label} exited {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    step_log(tx, tee, step, format!("{label} OK"));
    Ok(())
}

/// Convert a Windows absolute path to its WSL2 `/mnt/` equivalent.
///   `C:\Users\foo\ag\docker-compose.yml`
///   → `/mnt/c/Users/foo/ag/docker-compose.yml`
/// Strips the extended-length `\\?\` prefix; passes relative / UNC paths
/// through with only separator normalization.
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

/// Send `AUTH <password>\r\nPING\r\n` to `addr`, read until we either
/// see `+PONG\r\n` (good — password accepted, ping responded) or the
/// connection closes / times out.
fn resp_auth_ping(addr: &str, password: &str) -> bool {
    let Ok(socket_addr) = addr.parse::<SocketAddr>() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let cmd = format!(
        "*2\r\n$4\r\nAUTH\r\n${}\r\n{}\r\n*1\r\n$4\r\nPING\r\n",
        password.len(),
        password
    );
    if stream.write_all(cmd.as_bytes()).is_err() {
        return false;
    }
    // Read what fits in a small buffer — both responses combined easily fit.
    let mut buf = [0u8; 256];
    let mut total = 0usize;
    loop {
        match stream.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if total >= buf.len() {
                    break;
                }
                // Done if we already saw +PONG.
                let s = std::str::from_utf8(&buf[..total]).unwrap_or("");
                if s.contains("+PONG\r\n") {
                    return true;
                }
            }
            Err(_) => break,
        }
    }
    let s = std::str::from_utf8(&buf[..total]).unwrap_or("");
    s.contains("+PONG\r\n") && !s.contains("-ERR")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_translation() {
        assert_eq!(
            windows_path_to_wsl(r"C:\Users\foo\ag\docker-compose.yml"),
            "/mnt/c/Users/foo/ag/docker-compose.yml"
        );
        assert_eq!(windows_path_to_wsl(r"D:\data"), "/mnt/d/data");
        assert_eq!(windows_path_to_wsl("relative"), "relative");
        assert_eq!(windows_path_to_wsl(r"\\?\C:\ext"), "/mnt/c/ext");
    }
}
