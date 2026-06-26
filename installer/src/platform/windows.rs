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
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;

use crate::bundled;
use crate::detection::{DetectionResult, BACKEND_PORT};
use crate::install_steps::{
    render_template, step_log, LogTee, ProgressEvent, ProgressSender, FALKORDB_PORT,
    INSTALL_WSL2_ENABLE_STEP_NAME, STEP_ENSURE_TREE, STEP_SERVICE, STEP_STACK,
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

/// Number of probes `run_detection` runs — the denominator for the detection
/// screen's progress bar. Keep in sync with the `tokio::join!` arms below.
pub const DETECTION_PROBE_COUNT: usize = 17;

pub async fn run_detection(progress: Option<UnboundedSender<()>>) -> DetectionResult {
    let paths = Paths::resolve();
    // Wrap each probe so it sends a tick the instant it resolves — the
    // detection screen advances its progress bar one notch per completed
    // probe. Keep DETECTION_PROBE_COUNT in sync with these arms.
    macro_rules! ticked {
        ($p:expr) => {
            async {
                let r = $p.await;
                if let Some(tx) = progress.as_ref() {
                    let _ = tx.send(());
                }
                r
            }
        };
    }
    let (
        docker_present,
        docker_engine_version,
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
        wsl2_reboot_pending,
        wsl2_docker_version,
        wsl2_distro_name,
        virtualization_blocked,
    ) = tokio::join!(
        ticked!(probe_docker()),
        ticked!(probe_docker_engine()),
        ticked!(probe_ollama_active()),
        ticked!(probe_compose_up()),
        ticked!(probe_ag_env_exists(&paths)),
        ticked!(probe_falkordb_healthy()),
        ticked!(probe_backend_port_busy(BACKEND_PORT)),
        ticked!(probe_system_redis()),
        ticked!(probe_ag_task_drift(&paths)),
        ticked!(probe_disk_free_gb(&paths)),
        ticked!(probe_ram_gb()),
        ticked!(probe_distro()),
        ticked!(probe_wsl2_available()),
        ticked!(probe_wsl2_reboot_pending()),
        ticked!(probe_wsl2_docker()),
        ticked!(probe_wsl2_distro_name()),
        ticked!(probe_virtualization_blocked()),
    );
    DetectionResult {
        docker_present,
        docker_engine_version,
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
        wsl2_reboot_pending,
        wsl2_docker_version,
        wsl2_distro_name,
        virtualization_blocked,
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

/// `true` when a Windows servicing reboot is pending (the Component Based
/// Servicing `RebootPending` key exists). Enabling Virtual Machine Platform —
/// which `wsl --install` does under the hood — stages such a reboot, so this
/// is how we tell "WSL2 feature enabled" (`wsl --status` exits 0) apart from
/// "WSL2 enabled but a restart is still needed". Without it the installer
/// would treat a just-enabled, not-yet-rebooted WSL2 as ready and install
/// Docker into a distro that can't start. The CBS key is readable without
/// elevation; any failure falls through to `false` (best-effort — never
/// invent a reboot prompt we can't justify). `Test-Path` doesn't throw on a
/// missing key, so a clean machine simply reports `ok`.
async fn probe_wsl2_reboot_pending() -> bool {
    let ps = "if (Test-Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Component Based Servicing\\RebootPending') { 'pending' } else { 'ok' }";
    match Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim() == "pending",
        _ => false,
    }
}

/// `true` only when we can positively confirm hardware virtualization is
/// off at the firmware level and no hypervisor is running — the one state
/// where enabling WSL2 can't succeed without a BIOS/UEFI change (and Docker
/// Desktop can't run either). `wsl --install` reports success regardless, so
/// detection has to read the machine, not the install exit code.
///
/// Ordering is load-bearing: when a hypervisor is already present, WMI
/// reports `VirtualizationFirmwareEnabled` as false/null even though VT-x is
/// on, so we short-circuit to "not blocked" before ever trusting that
/// property. We only return `true` on an *explicit* `false` reading with no
/// hypervisor present; null / unreadable / probe-failed all fall through to
/// `false` (best-effort — never false-block a machine that might be fine).
/// `Select-Object -First 1` collapses the per-socket `Win32_Processor`
/// collection so the comparison stays scalar on multi-socket hosts.
async fn probe_virtualization_blocked() -> bool {
    let ps = "$cs = Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue; \
        $cpu = Get-CimInstance Win32_Processor -ErrorAction SilentlyContinue | \
        Select-Object -First 1; \
        if ($cs.HypervisorPresent) { 'ok' } \
        elseif ($cpu.VirtualizationFirmwareEnabled -eq $true) { 'ok' } \
        elseif ($cpu.VirtualizationFirmwareEnabled -eq $false) { 'blocked' } \
        else { 'unknown' }";
    match Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim() == "blocked",
        _ => false,
    }
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

/// Engine/daemon version via `docker version --format {{.Server.Version}}`.
/// Unlike `--version` (which only reads the CLI binary), this round-trips to
/// `dockerd`, so it exits non-zero when the daemon is unreachable — exactly
/// the "Docker Desktop installed but not started" case the compose stack
/// would otherwise hit at `docker compose up`. The `{{...}}` is a Go
/// template, passed literally as one arg.
async fn probe_docker_engine() -> Option<String> {
    let out = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
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

/// Public re-probe of free space on the install volume, for the mid-install
/// disk guard in `install_steps::run`. Same measurement as detection's
/// `probe_disk_free_gb`, exposed with a uniform cross-platform signature.
pub async fn disk_free_gb(paths: &Paths) -> u64 {
    probe_disk_free_gb(paths).await
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
            STEP_ENSURE_TREE,
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
        STEP_ENSURE_TREE,
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
            STEP_STACK,
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
        STEP_STACK,
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
            STEP_STACK,
            format!(
                "SKIP_SCHTASKS=1 — would run: wsl -d ag-ubuntu -u root -- \
                /usr/local/bin/ag-stack-up -f {compose_wsl} --profile \"\" \
                --profile falkor-container up -d"
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
            // Wait-for-dockerd wrapper (installed by install_docker_wsl2), not
            // `docker compose` directly — the [boot] dockerd autostart is async.
            "/usr/local/bin/ag-stack-up",
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
        STEP_STACK,
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
                STEP_SERVICE,
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
                STEP_SERVICE,
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
            STEP_SERVICE,
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
            STEP_SERVICE,
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
        //  - WSL2:    wsl -d ag-ubuntu -u root -- /usr/local/bin/ag-stack-up
        //             -f <wsl_path> <profiles> up -d
        // The WSL2 path goes through the ag-stack-up wrapper (not `docker
        // compose` directly) so the per-logon bring-up waits for the async
        // [boot] dockerd autostart instead of racing it.
        let use_wsl2 = answers.use_wsl2_docker();
        let (stack_command, stack_args) = if use_wsl2 {
            let compose_wsl = windows_path_to_wsl(&paths.docker_compose().display().to_string());
            (
                "wsl".to_string(),
                format!(
                    "-d ag-ubuntu -u root -- /usr/local/bin/ag-stack-up -f {compose_wsl} {profile_args} up -d"
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
            STEP_SERVICE,
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
    schtasks(tx, tee, STEP_SERVICE, &["/Run", "/TN", "ag"]).await?;
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
        STEP_SERVICE,
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
// Enable the WSL2 Windows feature (elevated) + logon-resume registration
// =============================================================================

/// Outcome of enabling the WSL2 Windows feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WslEnableOutcome {
    /// WSL2 is usable immediately — the install can proceed straight into
    /// installing Docker Engine in the distro without a reboot.
    ReadyNow,
    /// The feature was enabled but Windows needs a restart before WSL2
    /// works. The caller registers a resume hook and stops here.
    RebootRequired,
}

/// Enable the WSL2 Windows feature, then report whether a reboot is needed.
///
/// Enabling Virtual Machine Platform + WSL is a machine-level operation, so
/// this is the one install path that needs administrator rights. It elevates
/// via PowerShell's `Start-Process -Verb RunAs` — a single UAC prompt. The
/// app itself still installs entirely under the user account; this is a
/// one-time OS prerequisite, not the app asking for admin.
///
/// `--no-distribution` keeps `wsl --install` from creating a stray default
/// Ubuntu — we manage our own `ag-ubuntu` distro in `install_docker_wsl2`.
/// `wsl --update` ensures a current kernel, closing the stale-kernel gap on
/// machines where WSL was only half-enabled.
pub async fn enable_wsl2(tx: &ProgressSender, tee: &LogTee) -> Result<WslEnableOutcome> {
    let step = INSTALL_WSL2_ENABLE_STEP_NAME;
    if skip_systemctl() {
        step_log(
            tx,
            tee,
            step,
            "SKIP_SCHTASKS=1 — would run (elevated): wsl --install --no-distribution; wsl --update",
        );
        // Exercise the resume path in dev by reporting a reboot is needed.
        return Ok(WslEnableOutcome::RebootRequired);
    }

    step_log(
        tx,
        tee,
        step,
        "requesting elevation (UAC) for: wsl --install --no-distribution",
    );
    run_wsl_elevated(&["--install", "--no-distribution"])
        .await
        .with_context(|| "elevated wsl --install --no-distribution")?;

    // Best-effort kernel update — a post-reboot `wsl --update` also works, so
    // a failure here isn't fatal.
    step_log(
        tx,
        tee,
        step,
        "requesting elevation (UAC) for: wsl --update",
    );
    if let Err(e) = run_wsl_elevated(&["--update"]).await {
        step_log(
            tx,
            tee,
            step,
            format!("WARN: wsl --update failed ({e:#}) — continuing"),
        );
    }

    // Decide reboot-vs-ready from the live feature state, not `wsl --install`'s
    // exit code (which is unreliable across Windows builds). `wsl --status`
    // exits 0 the moment the feature is staged — even while a reboot is still
    // pending — so a 0 exit alone is a false "ready". Gate on the absence of a
    // pending servicing reboot too: enabling Virtual Machine Platform stages
    // one, and Docker installed into a not-yet-rebooted WSL2 fails to start.
    let ready_now = probe_wsl2_available().await && !probe_wsl2_reboot_pending().await;
    if ready_now {
        step_log(tx, tee, step, "WSL2 is active — no restart needed");
        Ok(WslEnableOutcome::ReadyNow)
    } else {
        step_log(
            tx,
            tee,
            step,
            "WSL2 feature enabled — a Windows restart is required to finish",
        );
        Ok(WslEnableOutcome::RebootRequired)
    }
}

/// Run `wsl <args>` elevated via a single UAC prompt, waiting for it to
/// finish. `Start-Process -Verb RunAs` raises the prompt; if the user denies
/// it, PowerShell throws and exits non-zero, which surfaces here as an error.
/// The actual success of the WSL operation is judged by the caller's
/// `wsl --status` re-probe, not this exit code.
async fn run_wsl_elevated(wsl_args: &[&str]) -> Result<()> {
    let arg_list = wsl_args
        .iter()
        .map(|a| format!("'{a}'"))
        .collect::<Vec<_>>()
        .join(",");
    let ps = format!(
        "Start-Process -FilePath wsl.exe -ArgumentList {arg_list} -Verb RunAs -Wait \
        -WindowStyle Hidden"
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
        .output()
        .await
        .with_context(|| "spawn powershell Start-Process -Verb RunAs")?;
    if !out.status.success() {
        bail!(
            "elevated `wsl {}` failed (exit {}) — UAC may have been declined\nstderr: {}",
            wsl_args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// Register an `HKCU` `RunOnce` entry so the installer relaunches itself at
/// the next logon after the reboot.
///
/// `HKCU` (not `HKLM`) keeps this admin-free; Windows deletes `RunOnce`
/// entries before executing them, so it fires exactly once and can't loop.
/// The install is detection-driven and idempotent, so "resume" is just
/// "relaunch" — no saved state.
///
/// We point `RunOnce` at the *current* exe path, not a copy. `bundled`
/// resolves the MSI payload relative to `current_exe()`
/// (`%PROGRAMFILES%\ag\bin\ag-installer.exe` → `..\..\share\ag`), so the
/// resumed run must launch from that same install location for
/// `copy_artifacts` to find `share\ag`. Staging a copy elsewhere would break
/// that relative resolution. The MSI install path is stable across the
/// reboot, so no copy is needed.
pub async fn register_wsl2_resume(tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    let step = INSTALL_WSL2_ENABLE_STEP_NAME;
    let self_exe = std::env::current_exe().with_context(|| "locate current installer exe")?;
    let self_exe_str = self_exe.display().to_string();

    if skip_systemctl() {
        step_log(
            tx,
            tee,
            step,
            format!(
                "SKIP_SCHTASKS=1 — would: reg add HKCU\\…\\RunOnce /v ag-installer-resume /d {self_exe_str}"
            ),
        );
        return Ok(());
    }

    let out = Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce",
            "/v",
            "ag-installer-resume",
            "/t",
            "REG_SZ",
            "/d",
            &format!("\"{self_exe_str}\""),
            "/f",
        ])
        .output()
        .await
        .with_context(|| "spawn reg add RunOnce")?;
    if !out.status.success() {
        bail!(
            "reg add RunOnce exited {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    step_log(
        tx,
        tee,
        step,
        format!("registered logon resume (HKCU RunOnce → {self_exe_str})"),
    );
    Ok(())
}

// =============================================================================
// Install Docker Engine inside a WSL2 distro (ag-ubuntu)
// =============================================================================

/// Ubuntu base rootfs release directory (Noble / 24.04 LTS). The tarball
/// name embeds the point release (`ubuntu-base-24.04.<N>-base-amd64.tar.gz`)
/// and changes with every point release, so we never hardcode the filename —
/// `resolve_rootfs_url` reads the `SHA256SUMS` in this directory and picks the
/// newest amd64 base tarball. (The old `cloud-images.ubuntu.com/wsl/...`
/// rootfs path now ships only manifests, no downloadable rootfs — which is
/// what the previously-hardcoded URL 404'd on.)
const WSL2_ROOTFS_RELEASE_DIR: &str =
    "https://cdimage.ubuntu.com/ubuntu-base/releases/24.04/release/";

const WSL2_DISTRO: &str = "ag-ubuntu";

/// Resolve the current Ubuntu base rootfs URL by reading the `SHA256SUMS` in
/// [`WSL2_ROOTFS_RELEASE_DIR`] and selecting the newest
/// `ubuntu-base-24.04.<N>-base-amd64.tar.gz`. This runtime resolution replaces
/// the formerly-hardcoded filename, which broke whenever Canonical published a
/// new point release.
async fn resolve_rootfs_url(client: &reqwest::Client) -> Result<String> {
    let sums_url = format!("{WSL2_ROOTFS_RELEASE_DIR}SHA256SUMS");
    let body = client
        .get(&sums_url)
        .send()
        .await
        .with_context(|| format!("GET {sums_url}"))?
        .error_for_status()
        .with_context(|| format!("fetch {sums_url}"))?
        .text()
        .await
        .with_context(|| "read SHA256SUMS body")?;
    // Lines look like "<sha256> *ubuntu-base-24.04.<N>-base-amd64.tar.gz".
    // Pick the highest <N> so we always grab the latest point release.
    let mut best: Option<(u32, String)> = None;
    for line in body.lines() {
        let file = match line.split_whitespace().nth(1) {
            Some(f) => f.trim_start_matches('*'),
            None => continue,
        };
        if let Some(rest) = file.strip_prefix("ubuntu-base-24.04.") {
            if let Some(point) = rest.strip_suffix("-base-amd64.tar.gz") {
                if let Ok(n) = point.parse::<u32>() {
                    if best.as_ref().is_none_or(|(b, _)| n > *b) {
                        best = Some((n, file.to_string()));
                    }
                }
            }
        }
    }
    let (_, file) =
        best.with_context(|| format!("no ubuntu-base 24.04 amd64 rootfs listed in {sums_url}"))?;
    Ok(format!("{WSL2_ROOTFS_RELEASE_DIR}{file}"))
}

/// Create the `ag-ubuntu` WSL2 distro and install Docker CE inside it.
/// Only invoked when the user picked the WSL2 Docker option, which only
/// appears when `wsl2_available` was true in detection — so the WSL2
/// feature is already enabled and no Windows restart is needed here.
pub async fn install_docker_wsl2(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    let step = "Install WSL2 Docker Engine";

    if skip_systemctl() {
        step_log(
            tx,
            tee,
            step,
            format!(
                "SKIP_SCHTASKS=1 — would: wsl --set-default-version 2; \
                download latest ubuntu-base rootfs from {WSL2_ROOTFS_RELEASE_DIR}; \
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

    // b. Reuse an existing ag-ubuntu distro only if it already has a working
    //    Docker. A distro that exists but lacks Docker (e.g. a prior run that
    //    imported the rootfs but failed during docker-ce install) must not be
    //    silently reused — the stack step would then fail with no Docker. Tear
    //    it down and re-import clean (wsl --import refuses an existing distro
    //    name, so the unregister has to come first).
    let distro_exists = probe_wsl2_distro_name().await.is_some();
    let docker_ready = distro_exists && probe_wsl2_docker().await.is_some();
    if docker_ready {
        step_log(
            tx,
            tee,
            step,
            format!("existing {WSL2_DISTRO} distro with Docker found — reusing it"),
        );
    } else {
        if distro_exists {
            step_log(
                tx,
                tee,
                step,
                format!("{WSL2_DISTRO} exists but Docker isn't installed — re-importing clean"),
            );
            let _ = Command::new("wsl")
                .args(["--unregister", WSL2_DISTRO])
                .output()
                .await;
        }
        // c. Download the Ubuntu base rootfs to %TEMP%. Resolve the exact
        //    filename at runtime (it embeds the point release), then verify
        //    the URL returns 200 before importing.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .with_context(|| "build http client for rootfs download")?;
        let rootfs_url = resolve_rootfs_url(&client).await?;
        step_log(tx, tee, step, format!("downloading rootfs: {rootfs_url}"));
        let resp = client
            .get(&rootfs_url)
            .send()
            .await
            .with_context(|| format!("GET {rootfs_url}"))?;
        if !resp.status().is_success() {
            bail!(
                "rootfs download returned {} for {rootfs_url}\n\
                The Ubuntu base rootfs may have moved. Download a current \
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

        // e. Install Docker Engine from the official APT repo. The repo
        //    codename is hardcoded to `noble`: we pin the rootfs to Ubuntu
        //    24.04, and on the minimal ubuntu-base image both `lsb_release -cs`
        //    (flaky right after install) and `/etc/os-release`'s
        //    VERSION_CODENAME (empty) are unreliable — a blank codename writes
        //    a bad docker.list and breaks `apt-get update`.
        let install_script = r#"set -e
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq ca-certificates curl gnupg lsb-release
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
chmod a+r /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu noble stable" > /etc/apt/sources.list.d/docker.list
apt-get update -qq
apt-get install -y -qq docker-ce docker-ce-cli containerd.io docker-compose-plugin
"#;
        wsl_root_bash(step, tx, tee, install_script, "install docker-ce").await?;

        // f. Docker daemon startup is owned by the ag-stack-up wrapper below,
        //    NOT /etc/wsl.conf [boot]: launching dockerd from [boot] runs it
        //    too early in distro init, where it hangs without ever creating
        //    /var/run/docker.sock (manual start after boot works fine). So we
        //    deliberately write no [boot] command and let the wrapper start
        //    dockerd on demand.
    }

    // f2. Install the `ag-stack-up` wrapper that BOTH the install-time stack
    //     bring-up and the ag-stack scheduled task call. This WSL distro has
    //     no systemd, so the wrapper owns the daemon: it starts dockerd if it
    //     isn't already serving, waits for the socket, then execs docker
    //     compose. Without this, `docker compose up` on a freshly-booted
    //     distro fails with "no such file: /var/run/docker.sock". Written
    //     unconditionally (outside the fresh-import branch) so reused/repaired
    //     distros get it too.
    let helper = "#!/usr/bin/env bash\n\
        # No systemd here: ensure dockerd is up, then exec docker compose with\n\
        # the passed-through args. dockerd is started on demand (after boot is\n\
        # complete) rather than via /etc/wsl.conf [boot], which hangs.\n\
        if ! docker info >/dev/null 2>&1; then\n\
        pkill -x dockerd 2>/dev/null\n\
        nohup dockerd --host unix:///var/run/docker.sock --log-level error >/var/log/ag-dockerd.log 2>&1 &\n\
        fi\n\
        for i in $(seq 1 60); do docker info >/dev/null 2>&1 && break; sleep 1; done\n\
        exec docker compose \"$@\"\n";
    let write_helper = format!(
        "install -d /usr/local/bin && printf '%s' '{helper}' > /usr/local/bin/ag-stack-up \
         && chmod +x /usr/local/bin/ag-stack-up"
    );
    wsl_root_bash(step, tx, tee, &write_helper, "install ag-stack-up wrapper").await?;

    // h. Bring the daemon up now via the wrapper — the exact path the stack
    //    step and scheduled task use. `ag-stack-up version` starts dockerd,
    //    waits for the socket, then runs `docker compose version`. This both
    //    confirms the engine works at install time and leaves dockerd running
    //    for the stack step. WARN (not fatal) on failure — the stack step
    //    would surface a real error anyway.
    let out = Command::new("wsl")
        .args([
            "-d",
            WSL2_DISTRO,
            "-u",
            "root",
            "--",
            "/usr/local/bin/ag-stack-up",
            "version",
        ])
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() => {
            step_log(
                tx,
                tee,
                step,
                format!("Docker Engine ready in {WSL2_DISTRO}"),
            );
        }
        _ => {
            step_log(
                tx,
                tee,
                step,
                "WARN: Docker Engine didn't confirm ready — the stack step will start it",
            );
        }
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
