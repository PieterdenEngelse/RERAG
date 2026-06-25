//! Linux platform impls.
//!
//! PR1.2 brought over Paths + `skip_systemctl`. PR1.3 brings over the
//! detection probes + the `run_detection` orchestrator. PR1.4 brings
//! over the four install-step bodies (`ensure_install_tree`,
//! `copy_artifacts`, `install_stack`, `install_service`). PR1.6 brings
//! over the Linux bodies of `uninstall` + `first_run`'s
//! `change_falkordb_password` and `start_ag` so cross-compile to
//! `x86_64-pc-windows-gnu` succeeds.

use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use tokio::process::Command;
use tokio::time::sleep;

use crate::bundled;
use crate::detection::{DetectionResult, BACKEND_PORT};
use crate::install_steps::{
    render_template, set_mode, step_log, LogTee, ProgressEvent, ProgressSender, FALKORDB_PASS,
    FALKORDB_PORT,
};
use crate::prompts::{PromptAnswers, PromptId};
use crate::uninstall::{rm_dir_quiet, rm_quiet};

// =============================================================================
// Paths (PR1.2)
// =============================================================================

/// Install path resolution.
///
/// All install destinations derive from `$HOME` (matching
/// `installers/install-linux.sh`'s scheme: `$HOME/.local/bin`,
/// `$HOME/.local/lib`, `$HOME/.config/ag`, …). Sandbox testing on this
/// box uses `HOME=/tmp/ag-test cargo run -p ag-installer`, which
/// redirects every path here without touching the real ag install.
///
/// `AG_HOME` is the only env-var override the bash installer exposes;
/// we honor it here too so `AG_HOME=/somewhere cargo run` still works.
///
/// `SKIP_SYSTEMCTL=1` is *not* a path override — it gates the systemctl
/// shellouts in install_steps. Documented here because the sandbox
/// recipe needs it set alongside `HOME`.
#[derive(Clone, Debug)]
pub struct Paths {
    /// `$AG_HOME` or `$HOME/.local/share/ag`. Holds runtime state: data/,
    /// index/, db/, logs/, web/, falkordb/, falkordb/data/.
    pub ag_home: PathBuf,
    /// `$HOME/.local/bin`. `ag` binary lands here.
    pub bin_dir: PathBuf,
    /// `$HOME/.local/lib`. `libtika_native.so` lands here.
    pub lib_dir: PathBuf,
    /// `$HOME/.config/ag`. `ag.env`, `docker-compose.yml` live here.
    pub config_dir: PathBuf,
    /// `$HOME/.config/systemd/user`. The three rendered .service files
    /// and the ag.service.d/ drop-in dir live here.
    pub systemd_user_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        let ag_home = std::env::var("AG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".local/share/ag"));
        Paths {
            bin_dir: home.join(".local/bin"),
            lib_dir: home.join(".local/lib"),
            config_dir: home.join(".config/ag"),
            systemd_user_dir: home.join(".config/systemd/user"),
            ag_home,
        }
    }

    pub fn ag_env(&self) -> PathBuf {
        self.config_dir.join("ag.env")
    }

    pub fn docker_compose(&self) -> PathBuf {
        self.config_dir.join("docker-compose.yml")
    }

    pub fn ag_service(&self) -> PathBuf {
        self.systemd_user_dir.join("ag.service")
    }

    pub fn ag_stack_service(&self) -> PathBuf {
        self.systemd_user_dir.join("ag-stack.service")
    }

    pub fn falkordb_service(&self) -> PathBuf {
        self.systemd_user_dir.join("falkordb.service")
    }

    pub fn ag_service_drop_in_dir(&self) -> PathBuf {
        self.systemd_user_dir.join("ag.service.d")
    }

    pub fn install_log(&self, timestamp_utc: &str) -> PathBuf {
        self.ag_home
            .join("logs")
            .join(format!("install-{timestamp_utc}.log"))
    }
}

/// True when `SKIP_SYSTEMCTL` is set (any non-empty value). Sandbox tests
/// set this so the `systemctl --user` shellouts log what they would do
/// instead of touching the real user systemd.
pub fn skip_systemctl() -> bool {
    std::env::var("SKIP_SYSTEMCTL")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

// =============================================================================
// Detection (PR1.3)
// =============================================================================
//
// Mirrors the `detect_*` functions in `installers/install-linux.sh`.
// Every probe shells out to a system tool (`docker`, `systemctl`, `ss`,
// `redis-cli`, `df`) or reads a well-known file (`/proc/meminfo`,
// `~/.config/...`). On any failure — command-not-found, non-zero exit,
// parse failure — the probe returns the "not present" value (`false`,
// `None`, `0`) rather than propagating the error. Detection is best-
// effort: a missing tool is information, not a crash.

/// Runs every probe; independent probes go in parallel via `tokio::join!`.
/// `system_redis` runs after `compose_up` so it can skip when our own
/// ag-redis container owns 6379.
pub async fn run_detection() -> DetectionResult {
    let (
        docker_present,
        ollama_active,
        compose_up,
        ag_env_exists,
        ram_gb,
        disk_free_gb,
        backend_port_busy,
        native_obs,
        ag_service_drift,
        falkordb_healthy,
        distro,
    ) = tokio::join!(
        probe_docker(),
        probe_ollama_active(),
        probe_compose_up(),
        probe_ag_env_exists(),
        probe_ram_gb(),
        probe_disk_free_gb(),
        probe_backend_port_busy(BACKEND_PORT),
        probe_native_obs(),
        probe_ag_service_drift(),
        probe_falkordb_healthy(),
        probe_distro(),
    );
    let system_redis = probe_system_redis(compose_up).await;

    DetectionResult {
        docker_present,
        ollama_active,
        compose_up,
        falkordb_healthy,
        ag_env_exists,
        backend_port_busy,
        system_redis,
        native_obs,
        ag_service_drift,
        disk_free_gb,
        ram_gb,
        distro,
        // Windows-only fields (wsl2_*, virtualization_blocked) stay at their
        // defaults on Linux — the struct shape is shared across platforms.
        ..Default::default()
    }
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

async fn probe_ollama_active() -> bool {
    systemctl_user_is_active("ollama").await
}

async fn probe_compose_up() -> bool {
    // `docker compose ls` with COMPOSE_PROJECT_NAME=ag prints a header line
    // plus one row per project. We look for a row whose first column is "ag".
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

async fn probe_ag_env_exists() -> bool {
    let Some(path) = xdg_config_dir().map(|p| p.join("ag/ag.env")) else {
        return false;
    };
    tokio::fs::metadata(path).await.is_ok()
}

async fn probe_ram_gb() -> u64 {
    let Ok(content) = tokio::fs::read_to_string("/proc/meminfo").await else {
        return 0;
    };
    content
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kb_str| kb_str.parse::<u64>().ok())
        .map(|kb| kb / 1024 / 1024)
        .unwrap_or(0)
}

async fn probe_distro() -> Option<String> {
    // /etc/os-release is the standard since 2012 (systemd) — supported by
    // every distro the AppImage targets. PRETTY_NAME is the human-readable
    // form: "Ubuntu 24.04.1 LTS", "Fedora Linux 40 (Workstation Edition)",
    // "Arch Linux", etc. Values may be quoted; strip surrounding quotes.
    let content = tokio::fs::read_to_string("/etc/os-release").await.ok()?;
    let pretty = content
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').trim_matches('\'').to_string());
    if let Some(p) = pretty {
        if !p.is_empty() {
            return Some(p);
        }
    }
    // Fall back to NAME + VERSION_ID if PRETTY_NAME is absent. Rare but
    // possible on minimal/embedded distros.
    let name = content
        .lines()
        .find_map(|l| l.strip_prefix("NAME="))
        .map(|v| v.trim_matches('"').trim_matches('\'').to_string());
    let version = content
        .lines()
        .find_map(|l| l.strip_prefix("VERSION_ID="))
        .map(|v| v.trim_matches('"').trim_matches('\'').to_string());
    match (name, version) {
        (Some(n), Some(v)) => Some(format!("{n} {v}")),
        (Some(n), None) => Some(n),
        _ => None,
    }
}

async fn probe_disk_free_gb() -> u64 {
    let Ok(home) = std::env::var("HOME") else {
        return 0;
    };
    let Ok(out) = Command::new("df")
        .args(["-BG", "--output=avail", &home])
        .output()
        .await
    else {
        return 0;
    };
    if !out.status.success() {
        return 0;
    }
    // Output is "Avail\n  42G\n". Take the last non-empty line and strip the
    // trailing 'G'.
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .last()
        .map(|s| s.trim().trim_end_matches('G'))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

async fn probe_backend_port_busy(port: u16) -> bool {
    let Ok(out) = Command::new("ss").args(["-tln"]).output().await else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    // ss -tln output column 4 is Local Address:Port. Match exact port at end
    // after either ':' (IPv4 / IPv6 bracketed) or '.' (some IPv6 forms).
    let suffix_colon = format!(":{port}");
    let suffix_dot = format!(".{port}");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1) // header row
        .filter_map(|line| line.split_whitespace().nth(3))
        .any(|local| local.ends_with(&suffix_colon) || local.ends_with(&suffix_dot))
}

async fn probe_native_obs() -> Vec<String> {
    let mut active = Vec::new();
    for unit in ["loki", "tempo", "otelcol"] {
        if systemctl_user_is_active(unit).await {
            active.push(unit.to_string());
        }
    }
    active
}

async fn probe_ag_service_drift() -> bool {
    let Some(path) = xdg_config_dir().map(|p| p.join("systemd/user/ag.service")) else {
        return false;
    };
    let Ok(content) = tokio::fs::read_to_string(&path).await else {
        // No installed unit → no drift to flag.
        return false;
    };
    // Lightweight heuristic from bash: an installed unit that's missing any of
    // these load-bearing lines was almost certainly hand-edited.
    let has_env_file = content
        .lines()
        .any(|l| l.starts_with("EnvironmentFile=") && l.contains("ag.env"));
    let has_ld_lib = content
        .lines()
        .any(|l| l.contains("LD_LIBRARY_PATH=") && l.contains("lib"));
    let has_exec = content
        .lines()
        .any(|l| l.starts_with("ExecStart=") && l.contains(".local/bin/ag"));
    !(has_env_file && has_ld_lib && has_exec)
}

async fn probe_falkordb_healthy() -> bool {
    // Phase C: trust the systemd active check. Phase D will additionally PONG
    // the configured port with the configured password once both are known.
    systemctl_user_is_active("falkordb.service").await
}

async fn probe_system_redis(compose_up: bool) -> bool {
    // Skip when our ag-redis container is what's listening on 6379.
    if compose_up {
        if let Ok(out) = Command::new("docker")
            .args(["ps", "--format", "{{.Names}}"])
            .output()
            .await
        {
            if out.status.success() {
                let names = String::from_utf8_lossy(&out.stdout);
                if names.lines().any(|n| n == "ag-redis") {
                    return false;
                }
            }
        }
    }
    let Ok(out) = Command::new("redis-cli")
        .args(["-p", "6379", "ping"])
        .output()
        .await
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    String::from_utf8_lossy(&out.stdout).trim() == "PONG"
}

// --- small helpers --------------------------------------------------------

async fn systemctl_user_is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", unit])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

fn xdg_config_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("XDG_CONFIG_HOME") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config"))
}

// =============================================================================
// Step 1: ensure_install_tree — make every dir, open install log (PR1.4)
// =============================================================================

pub async fn ensure_install_tree(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
    log_path_out: &mut Option<PathBuf>,
) -> Result<()> {
    let dirs: Vec<PathBuf> = vec![
        paths.bin_dir.clone(),
        paths.lib_dir.clone(),
        paths.config_dir.clone(),
        paths.systemd_user_dir.clone(),
        paths.ag_service_drop_in_dir(),
        paths.ag_home.join("data"),
        paths.ag_home.join("index"),
        paths.ag_home.join("db"),
        paths.ag_home.join("logs"),
        paths.ag_home.join("cache"),
        paths.ag_home.join("locks"),
        paths.ag_home.join("web"),
        paths.ag_home.join("falkordb"),
        paths.ag_home.join("falkordb/data"),
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

    // Open the install log AFTER the logs/ dir exists. From here on, every
    // step's log lines tee into this file.
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
// Step 3: copy_artifacts — ag binary, libtika, frontend dist, smoke-test (PR1.4)
// =============================================================================

pub async fn copy_artifacts(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    // ag binary
    let ag_bin = bundled::ag_binary_path();
    if !ag_bin.exists() {
        bail!(
            "ag binary missing at {} — build it first (cargo build --release -p ag)",
            ag_bin.display()
        );
    }
    let ag_target = paths.bin_dir.join("ag");
    fs::copy(&ag_bin, &ag_target)
        .with_context(|| format!("copy {} → {}", ag_bin.display(), ag_target.display()))?;
    set_mode(&ag_target, 0o755)?;
    step_log(
        tx,
        tee,
        "Install artifacts",
        format!("installed {}", ag_target.display()),
    );

    // libtika (optional — PDF parsing degrades to fallback if absent)
    let libtika = bundled::libtika_path();
    if let Some(src) = libtika {
        let dst = paths.lib_dir.join("libtika_native.so");
        fs::copy(&src, &dst)
            .with_context(|| format!("copy {} → {}", src.display(), dst.display()))?;
        set_mode(&dst, 0o644)?;
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
            "libtika_native.so not bundled — PDF parsing will use fallback",
        );
    }

    // Frontend dist — rsync mirrors bash exactly. `rsync` is ubiquitous on
    // desktop Linux; failing to find it is a reasonable hard error.
    let frontend = bundled::frontend_dist_dir();
    if frontend.exists() && frontend.is_dir() {
        let dst = paths.ag_home.join("web");
        fs::create_dir_all(&dst)?;
        // rsync -a --checksum --delete <src>/ <dst>/
        // The trailing slash on src tells rsync "contents of, not the dir itself".
        let src_arg = format!("{}/", frontend.display());
        let dst_arg = format!("{}/", dst.display());
        let status = Command::new("rsync")
            .args(["-a", "--checksum", "--delete", &src_arg, &dst_arg])
            .status()
            .await
            .with_context(|| "spawn rsync (is it installed?)")?;
        if !status.success() {
            bail!("rsync exited with {status}");
        }
        step_log(
            tx,
            tee,
            "Install artifacts",
            format!("rsynced {} → {}", src_arg, dst_arg),
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

    // Smoke-test the installed binary (no daemon).
    let out = Command::new(&ag_target)
        .arg("--version")
        .env("LD_LIBRARY_PATH", &paths.lib_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
// Step 4: install_stack — FalkorDB native binaries + unit (PR1.4)
// =============================================================================

pub async fn install_stack(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
    // Windows uses this to route between native and WSL2 Docker; Linux
    // always installs the native FalkorDB binaries, so it's ignored here.
    // The param exists for signature parity across the shared re-export.
    _answers: &PromptAnswers,
) -> Result<()> {
    let stage = bundled::falkordb_stage_dir();
    let dst = paths.ag_home.join("falkordb");
    fs::create_dir_all(&dst)?;

    let bundles = [
        ("redis-server", 0o755),
        ("redis-cli", 0o755),
        ("falkordb.so", 0o644),
    ];
    let mut missing = Vec::new();
    for (name, mode) in &bundles {
        let src = stage.join(name);
        if !src.exists() {
            missing.push(name.to_string());
            continue;
        }
        let target = dst.join(name);
        fs::copy(&src, &target)
            .with_context(|| format!("copy {} → {}", src.display(), target.display()))?;
        set_mode(&target, *mode)?;
        step_log(
            tx,
            tee,
            "FalkorDB native service",
            format!("copied {} → {}", src.display(), target.display()),
        );
    }
    if !missing.is_empty() {
        bail!(
            "FalkorDB binaries missing from {}: {}. \
            In dev mode, run installer/build-appimage.sh's extract step or \
            populate installer/stage/falkordb/ manually.",
            stage.display(),
            missing.join(", ")
        );
    }

    // Smoke-test extracted redis-server on the host (catches musl/glibc).
    let smoke = Command::new(dst.join("redis-server"))
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| "spawn redis-server --version")?;
    if !smoke.status.success() {
        bail!(
            "extracted redis-server failed --version (likely musl/glibc mismatch).\n\
            See docs/falkordb-native-service.md §2 for fallbacks.\nstderr: {}",
            String::from_utf8_lossy(&smoke.stderr).trim()
        );
    }
    step_log(
        tx,
        tee,
        "FalkorDB native service",
        format!(
            "smoke-test OK: {}",
            String::from_utf8_lossy(&smoke.stdout).trim()
        ),
    );

    // Render the unit.
    let tmpl = bundled::systemd_template_dir().join("falkordb.service.tmpl");
    render_template(
        &tmpl,
        &paths.falkordb_service(),
        &[
            ("AG_HOME", paths.ag_home.display().to_string()),
            ("FDB_PORT", FALKORDB_PORT.to_string()),
            ("FDB_PASS", FALKORDB_PASS.to_string()),
        ],
    )
    .with_context(|| "render falkordb.service")?;
    step_log(
        tx,
        tee,
        "FalkorDB native service",
        format!("rendered {}", paths.falkordb_service().display()),
    );

    // Activate.
    systemctl_user(tx, tee, "FalkorDB native service", &["daemon-reload"]).await?;
    systemctl_user(
        tx,
        tee,
        "FalkorDB native service",
        &["enable", "--now", "falkordb.service"],
    )
    .await?;
    Ok(())
}

// =============================================================================
// Step 5: install_service — render ag.service + ag-stack.service + drop-ins (PR1.4)
// =============================================================================

pub async fn install_service(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
    answers: &PromptAnswers,
    backend_port: u16,
) -> Result<()> {
    // ag.service — honor AgInstallDrift prompt choice.
    let ag_unit = paths.ag_service();
    let drift_choice = answers.choice(PromptId::AgInstallDrift).unwrap_or("keep");
    let install_ag_service = match drift_choice {
        "keep" if ag_unit.exists() => {
            step_log(
                tx,
                tee,
                "Systemd user units",
                format!("keeping existing {} per prompt choice", ag_unit.display()),
            );
            false
        }
        "backup" if ag_unit.exists() => {
            let ts = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
            let bak = paths.systemd_user_dir.join(format!("ag.service.bak-{ts}"));
            fs::rename(&ag_unit, &bak)
                .with_context(|| format!("rename {} → {}", ag_unit.display(), bak.display()))?;
            step_log(
                tx,
                tee,
                "Systemd user units",
                format!("backed up {} → {}", ag_unit.display(), bak.display()),
            );
            true
        }
        _ => true,
    };
    if install_ag_service {
        let tmpl = bundled::systemd_template_dir().join("ag.service.tmpl");
        render_template(
            &tmpl,
            &ag_unit,
            &[
                ("AG_BIN", paths.bin_dir.join("ag").display().to_string()),
                ("AG_HOME", paths.ag_home.display().to_string()),
                ("AG_ENV", paths.ag_env().display().to_string()),
                ("AG_LIB_DIR", paths.lib_dir.display().to_string()),
                ("BACKEND_PORT", backend_port.to_string()),
            ],
        )
        .with_context(|| "render ag.service")?;
        step_log(
            tx,
            tee,
            "Systemd user units",
            format!("rendered {}", ag_unit.display()),
        );
    }

    // ag-stack.service — skipped if user chose "natives" on NativeObs.
    let skip_stack = matches!(answers.choice(PromptId::NativeObs), Some("natives"));
    if skip_stack {
        step_log(
            tx,
            tee,
            "Systemd user units",
            "ag-stack.service skipped (user chose native observability)",
        );
    } else {
        // Compose profile derives from LowRam prompt's stack choice.
        let profile = match answers.choice(PromptId::LowRam) {
            Some("core") => "core",
            Some("observability") => "observability",
            Some("none") => "", // unused (stack skipped); kept for safety
            _ => "",            // "all" or no LowRam prompt
        };
        let tmpl = bundled::systemd_template_dir().join("ag-stack.service.tmpl");
        render_template(
            &tmpl,
            &paths.ag_stack_service(),
            &[
                ("COMPOSE_FILE", paths.docker_compose().display().to_string()),
                ("COMPOSE_PROFILE", profile.to_string()),
            ],
        )
        .with_context(|| "render ag-stack.service")?;
        step_log(
            tx,
            tee,
            "Systemd user units",
            format!(
                "rendered {} (profile={})",
                paths.ag_stack_service().display(),
                if profile.is_empty() { "<all>" } else { profile }
            ),
        );
    }

    // Drop-ins (plain copies — no templating).
    let drop_in_src = bundled::systemd_template_dir().join("ag.service.d");
    let drop_in_dst = paths.ag_service_drop_in_dir();
    if drop_in_src.is_dir() {
        for name in ["falkordb.conf", "stack.conf"] {
            let src = drop_in_src.join(name);
            if !src.exists() {
                continue;
            }
            let dst = drop_in_dst.join(name);
            fs::copy(&src, &dst)
                .with_context(|| format!("copy {} → {}", src.display(), dst.display()))?;
            step_log(
                tx,
                tee,
                "Systemd user units",
                format!("installed {}", dst.display()),
            );
        }
    }

    // Activate.
    systemctl_user(tx, tee, "Systemd user units", &["daemon-reload"]).await?;
    if !skip_stack {
        systemctl_user(
            tx,
            tee,
            "Systemd user units",
            &["enable", "--now", "ag-stack.service"],
        )
        .await?;
    }
    systemctl_user(
        tx,
        tee,
        "Systemd user units",
        &["enable", "--now", "ag.service"],
    )
    .await?;
    Ok(())
}

/// Run `systemctl --user <args>`. When `SKIP_SYSTEMCTL=1` is set we log
/// what would have run and return Ok — useful for sandbox testing where
/// we don't want to touch the real user systemd.
async fn systemctl_user(
    tx: &ProgressSender,
    tee: &LogTee,
    step_name: &'static str,
    args: &[&str],
) -> Result<()> {
    let pretty = format!("systemctl --user {}", args.join(" "));
    if skip_systemctl() {
        step_log(
            tx,
            tee,
            step_name,
            format!("SKIP_SYSTEMCTL=1 — would run: {pretty}"),
        );
        return Ok(());
    }
    let mut cmd = Command::new("systemctl");
    cmd.arg("--user").args(args);
    let out = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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

// =============================================================================
// Uninstall (PR1.6)
// =============================================================================
//
// `uninstall_targets` returns the OS-specific paths shown in the
// "Will remove:" intro on the CLI. `uninstall_managed` does the actual
// stop+disable+rm, leaving the shared cleanup (docker-compose.yml,
// optionally ag.env + ag_home) to `crate::uninstall::run`.

pub fn uninstall_targets(paths: &Paths) -> Vec<PathBuf> {
    vec![
        paths.bin_dir.join("ag"),
        paths.lib_dir.join("libtika_native.so"),
        paths.ag_service(),
        paths.ag_stack_service(),
        paths.falkordb_service(),
        paths.ag_service_drop_in_dir(),
    ]
}

pub async fn uninstall_managed(paths: &Paths) {
    // 1. Stop + disable services. Best-effort — a stop on a service that
    //    isn't running, or a disable on one that isn't enabled, isn't an
    //    error worth bailing on.
    for unit in ["ag.service", "ag-stack.service", "falkordb.service"] {
        uninstall_systemctl(&["stop", unit]).await;
        uninstall_systemctl(&["disable", unit]).await;
    }
    uninstall_systemctl(&["daemon-reload"]).await;

    // 2. Remove rendered unit files + the drop-in dir.
    rm_quiet(&paths.ag_service());
    rm_quiet(&paths.ag_stack_service());
    rm_quiet(&paths.falkordb_service());
    rm_dir_quiet(&paths.ag_service_drop_in_dir());

    // 3. Binaries + bundled libs.
    rm_quiet(&paths.bin_dir.join("ag"));
    rm_quiet(&paths.lib_dir.join("libtika_native.so"));
}

/// Best-effort systemctl wrapper for the uninstall flow — prints
/// directly rather than going through the tx/tee plumbing used by
/// install_steps. The CLI uninstall already has `println!` output as its
/// primary surface.
async fn uninstall_systemctl(args: &[&str]) {
    let pretty = format!("systemctl --user {}", args.join(" "));
    if skip_systemctl() {
        println!("  SKIP_SYSTEMCTL=1 — would run: {pretty}");
        return;
    }
    let result = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    match result {
        Ok(status) if status.success() => println!("  {pretty}"),
        Ok(_) => {
            // Common case: stop/disable on a unit that isn't there.
            // Don't surface as an error — uninstall is idempotent.
        }
        Err(e) => println!("  ! {pretty} — spawn failed: {e}"),
    }
}

// =============================================================================
// First-run: start ag + FalkorDB password change (PR1.6)
// =============================================================================
//
// The portable parts (probe Ollama, atomic env-file write, /health poll)
// stay in `crate::first_run`. The OS-specific bodies — re-render
// falkordb.service + systemctl restart + redis-cli ping verify, plus the
// `systemctl --user start ag.service` call — live here.

/// Re-render `falkordb.service` with `new_password`, daemon-reload,
/// restart the unit, verify with `redis-cli ping` using the new
/// password. Caller skips this when `new_password` equals the install-
/// time default.
pub async fn apply_falkordb_password(
    paths: &Paths,
    tx: &ProgressSender,
    new_password: &str,
) -> Result<()> {
    let step = "Start ag";
    send_log(tx, step, "changing FalkorDB password".to_string());

    let tmpl = bundled::systemd_template_dir().join("falkordb.service.tmpl");
    if !tmpl.exists() {
        bail!(
            "falkordb.service.tmpl missing at {} — bundled artifacts incomplete",
            tmpl.display()
        );
    }
    let mut content =
        fs::read_to_string(&tmpl).with_context(|| format!("read {}", tmpl.display()))?;
    let vars = [
        ("AG_HOME", paths.ag_home.display().to_string()),
        ("FDB_PORT", FALKORDB_PORT.to_string()),
        ("FDB_PASS", new_password.to_string()),
    ];
    for (k, v) in &vars {
        content = content.replace(&format!("{{{{{k}}}}}"), v);
    }
    fs::write(paths.falkordb_service(), content)
        .with_context(|| format!("write {}", paths.falkordb_service().display()))?;
    send_log(
        tx,
        step,
        format!("re-rendered {}", paths.falkordb_service().display()),
    );

    first_run_systemctl(tx, step, &["daemon-reload"]).await?;
    first_run_systemctl(tx, step, &["restart", "falkordb.service"]).await?;

    if skip_systemctl() {
        send_log(
            tx,
            step,
            "SKIP_SYSTEMCTL=1 — skipping redis-cli verify".to_string(),
        );
        return Ok(());
    }

    // FalkorDB takes a moment to come back up after restart; poll PONG.
    let redis_cli = paths.ag_home.join("falkordb/redis-cli");
    for attempt in 1..=10u32 {
        sleep(Duration::from_millis(500)).await;
        let out = Command::new(&redis_cli)
            .args(["-p", &FALKORDB_PORT.to_string(), "-a", new_password, "ping"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;
        if let Ok(out) = out {
            if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "PONG" {
                send_log(
                    tx,
                    step,
                    format!("redis-cli ping with new password OK (attempt {attempt})"),
                );
                return Ok(());
            }
        }
    }
    Err(anyhow!(
        "falkordb.service restarted but redis-cli ping with the new password did not return PONG within 5s"
    ))
}

/// `systemctl --user start ag.service`. The shared `/health` poll lives
/// in `crate::first_run::start_ag_and_wait`.
pub async fn start_ag(tx: &ProgressSender, step: &'static str) -> Result<()> {
    first_run_systemctl(tx, step, &["start", "ag.service"]).await
}

/// First-run analog of `systemctl_user` — sends `StepLog` events via tx
/// only (no LogTee, since first-run isn't part of the install run's
/// tee'd log file).
async fn first_run_systemctl(
    tx: &ProgressSender,
    step_name: &'static str,
    args: &[&str],
) -> Result<()> {
    let pretty = format!("systemctl --user {}", args.join(" "));
    if skip_systemctl() {
        send_log(
            tx,
            step_name,
            format!("SKIP_SYSTEMCTL=1 — would run: {pretty}"),
        );
        return Ok(());
    }
    let out = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
    send_log(tx, step_name, format!("ran: {pretty}"));
    Ok(())
}

fn send_log(tx: &ProgressSender, name: &'static str, line: String) {
    let _ = tx.send(ProgressEvent::StepLog { name, line });
}
