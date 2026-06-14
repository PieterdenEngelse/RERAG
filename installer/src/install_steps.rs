//! Install step orchestration — real writes.
//!
//! Each step mirrors `installers/install-linux.sh` one-for-one:
//! `ensure_xdg`, `seed_config`, `install_artifacts`, `falkordb`,
//! `systemd_step`, `health_check`. Per-step bodies do real fs work,
//! shell out to systemctl / docker / curl, and stream log lines via
//! `ProgressSender`.
//!
//! **Sandbox testing** (so this box's real ag install stays untouched):
//!
//! ```bash
//! HOME=/tmp/ag-test SKIP_SYSTEMCTL=1 cargo run -p ag-installer
//! ```
//!
//! - `HOME` redirects every install path (see `crate::paths`).
//! - `SKIP_SYSTEMCTL=1` makes the systemctl shellouts log what they
//!   would do instead of touching the real user systemd.
//!
//! See `docs/bin3 §Phase D` for the spec; `installers/install-linux.sh`
//! step_* functions for the bash reference.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{sleep, Duration};

use crate::bundled;
use crate::paths::{self, Paths};
use crate::prompts::{PromptAnswers, PromptId};

pub type ProgressSender = UnboundedSender<ProgressEvent>;

pub const DEFAULT_BACKEND_PORT: u16 = 3010;
pub const FALKORDB_PORT: u16 = 6380;
pub const FALKORDB_PASS: &str = "agpassword123";

#[derive(Clone, Debug)]
pub enum ProgressEvent {
    StepStart {
        name: &'static str,
    },
    StepLog {
        #[allow(dead_code)]
        name: &'static str,
        line: String,
    },
    StepDone {
        name: &'static str,
        duration: Duration,
    },
    StepFailed {
        name: &'static str,
        error: String,
    },
    /// Emitted exactly once after the final step completes successfully.
    /// Drives the "Continue" button on the Progress screen.
    InstallComplete,
}

pub const STEP_NAMES: &[&str] = &[
    "Ensure XDG tree",
    "Seed config",
    "Install artifacts",
    "FalkorDB native service",
    "Systemd user units",
    "Health check",
];

#[allow(dead_code)]
#[derive(Debug)]
pub struct InstallResult {
    pub success: bool,
    pub log_path: Option<PathBuf>,
}

/// Shared per-run state: the open install log file (so every step's log
/// lines get teed to disk for the failure-modal "Open log" button).
#[derive(Clone)]
struct LogTee(Arc<Mutex<Option<fs::File>>>);

impl LogTee {
    fn new() -> Self {
        LogTee(Arc::new(Mutex::new(None)))
    }
    fn set(&self, file: fs::File) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(file);
        }
    }
    fn write_line(&self, line: &str) {
        if let Ok(mut slot) = self.0.lock() {
            if let Some(f) = slot.as_mut() {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}

/// Helper: emit a log line via the sender AND tee it into the install log.
fn step_log(tx: &ProgressSender, tee: &LogTee, name: &'static str, line: impl Into<String>) {
    let line = line.into();
    tee.write_line(&format!("[{name}] {line}"));
    let _ = tx.send(ProgressEvent::StepLog { name, line });
}

pub async fn run(answers: PromptAnswers, tx: ProgressSender) -> InstallResult {
    let paths = Paths::resolve();
    let backend_port = answers.backend_port.unwrap_or(DEFAULT_BACKEND_PORT);
    let tee = LogTee::new();
    let mut log_path: Option<PathBuf> = None;

    // The macro centralizes the start/done/failed event plumbing so each
    // step body is a single Result-returning async block.
    macro_rules! step {
        ($name:expr, $body:expr) => {{
            let name = $name;
            if tx.send(ProgressEvent::StepStart { name }).is_err() {
                return InstallResult { success: false, log_path };
            }
            tee.write_line(&format!("=== {name} ==="));
            let start = Instant::now();
            match $body.await {
                Ok(()) => {
                    let duration = start.elapsed();
                    tee.write_line(&format!("=== {name} done in {:.1}s ===\n", duration.as_secs_f32()));
                    if tx.send(ProgressEvent::StepDone { name, duration }).is_err() {
                        return InstallResult { success: false, log_path };
                    }
                }
                Err(e) => {
                    let error_text = format!("{e:#}");
                    tee.write_line(&format!("FAILED: {error_text}\n"));
                    let _ = tx.send(ProgressEvent::StepFailed {
                        name,
                        error: error_text,
                    });
                    return InstallResult { success: false, log_path };
                }
            }
        }};
    }

    step!(
        "Ensure XDG tree",
        ensure_xdg(&paths, &tx, &tee, &mut log_path)
    );
    step!(
        "Seed config",
        seed_config(&paths, &tx, &tee, &answers, backend_port)
    );
    step!("Install artifacts", install_artifacts(&paths, &tx, &tee));
    step!("FalkorDB native service", falkordb(&paths, &tx, &tee));
    step!(
        "Systemd user units",
        systemd_step(&paths, &tx, &tee, &answers, backend_port)
    );
    step!("Health check", health_check(&tx, &tee, backend_port));

    let _ = tx.send(ProgressEvent::InstallComplete);
    InstallResult {
        success: true,
        log_path,
    }
}

// =============================================================================
// Step 1: ensure_xdg — make every dir, open install log
// =============================================================================

async fn ensure_xdg(
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
        step_log(tx, tee, "Ensure XDG tree", format!("created {}", d.display()));
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
// Step 2: seed_config — env file + compose file, preserve existing
// =============================================================================

async fn seed_config(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
    answers: &PromptAnswers,
    backend_port: u16,
) -> Result<()> {
    // ag.env: copy from bundled .env.example only if not present (never
    // overwrite); apply prompt-driven inline edits.
    let env_target = paths.ag_env();
    let env_source = bundled::env_example_path();
    if env_target.exists() {
        step_log(
            tx,
            tee,
            "Seed config",
            format!("{} exists — preserved (not overwritten)", env_target.display()),
        );
    } else {
        if !env_source.exists() {
            bail!(
                ".env.example missing at {} — cannot seed ag.env",
                env_source.display()
            );
        }
        fs::copy(&env_source, &env_target).with_context(|| {
            format!(
                "copy {} → {}",
                env_source.display(),
                env_target.display()
            )
        })?;
        // 0600: ag.env carries DB credentials and FalkorDB password.
        set_mode(&env_target, 0o600)?;
        step_log(
            tx,
            tee,
            "Seed config",
            format!("seeded {}", env_target.display()),
        );
        // Apply prompt-driven overrides.
        edit_env_file(&env_target, &[("BACKEND_PORT", &backend_port.to_string())])?;
        step_log(
            tx,
            tee,
            "Seed config",
            format!("set BACKEND_PORT={backend_port} in {}", env_target.display()),
        );
        if matches!(answers.choice(PromptId::SystemRedis), Some("system")) {
            edit_env_file(&env_target, &[("REDIS_URL", "redis://127.0.0.1:6379/")])?;
            step_log(
                tx,
                tee,
                "Seed config",
                "set REDIS_URL=redis://127.0.0.1:6379/ (system Redis reuse)",
            );
        }
    }

    // docker-compose.yml: copy if missing; warn (don't overwrite) if drifted.
    let compose_target = paths.docker_compose();
    let compose_source = bundled::docker_compose_path();
    if !compose_target.exists() {
        if !compose_source.exists() {
            bail!(
                "docker-compose.yml missing at {} — cannot seed",
                compose_source.display()
            );
        }
        fs::copy(&compose_source, &compose_target).with_context(|| {
            format!(
                "copy {} → {}",
                compose_source.display(),
                compose_target.display()
            )
        })?;
        step_log(
            tx,
            tee,
            "Seed config",
            format!(
                "copied {} → {}",
                compose_source.display(),
                compose_target.display()
            ),
        );
    } else {
        step_log(
            tx,
            tee,
            "Seed config",
            format!("{} exists — preserved", compose_target.display()),
        );
    }
    Ok(())
}

// =============================================================================
// Step 3: install_artifacts — ag binary, libtika, frontend dist, smoke-test
// =============================================================================

async fn install_artifacts(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()> {
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
// Step 4: falkordb — copy bundled binaries + render unit
// =============================================================================

async fn falkordb(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()> {
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
// Step 5: systemd_step — render ag.service + ag-stack.service, install drop-ins
// =============================================================================

async fn systemd_step(
    paths: &Paths,
    tx: &ProgressSender,
    tee: &LogTee,
    answers: &PromptAnswers,
    backend_port: u16,
) -> Result<()> {
    // ag.service — honor AgServiceDrift prompt choice.
    let ag_unit = paths.ag_service();
    let drift_choice = answers
        .choice(PromptId::AgServiceDrift)
        .unwrap_or("keep");
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
            let bak = paths
                .systemd_user_dir
                .join(format!("ag.service.bak-{ts}"));
            fs::rename(&ag_unit, &bak).with_context(|| {
                format!("rename {} → {}", ag_unit.display(), bak.display())
            })?;
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
            _ => "",             // "all" or no LowRam prompt
        };
        let tmpl = bundled::systemd_template_dir().join("ag-stack.service.tmpl");
        render_template(
            &tmpl,
            &paths.ag_stack_service(),
            &[
                (
                    "COMPOSE_FILE",
                    paths.docker_compose().display().to_string(),
                ),
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

// =============================================================================
// Step 6: health_check — poll /health
// =============================================================================

async fn health_check(tx: &ProgressSender, tee: &LogTee, backend_port: u16) -> Result<()> {
    if paths::skip_systemctl() {
        step_log(
            tx,
            tee,
            "Health check",
            "SKIP_SYSTEMCTL=1 — no service was started, skipping /health poll",
        );
        return Ok(());
    }
    let url = format!("http://127.0.0.1:{backend_port}/health");
    step_log(tx, tee, "Health check", format!("polling {url} (~20s)"));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .with_context(|| "build http client")?;
    for attempt in 1..=10u32 {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                step_log(
                    tx,
                    tee,
                    "Health check",
                    format!("/health responded {} on attempt {attempt}", resp.status()),
                );
                return Ok(());
            }
            Ok(resp) => {
                step_log(
                    tx,
                    tee,
                    "Health check",
                    format!("attempt {attempt}: {} — retrying", resp.status()),
                );
            }
            Err(_) => {
                step_log(
                    tx,
                    tee,
                    "Health check",
                    format!("attempt {attempt}: no response — retrying"),
                );
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    // Bash treats this as a warning, not a fatal. We mirror that — the
    // unit may still be starting; user can inspect journalctl.
    step_log(
        tx,
        tee,
        "Health check",
        "WARN: /health did not respond within ~20s. ag.service may still be starting.",
    );
    step_log(
        tx,
        tee,
        "Health check",
        "       check: journalctl --user -u ag.service -n 50",
    );
    Ok(())
}

// =============================================================================
// Helpers — template render, env edit, systemctl, file mode
// =============================================================================

/// `{{KEY}}` literal substitution, mirroring bash `render_template`.
fn render_template(src: &Path, dst: &Path, vars: &[(&str, String)]) -> Result<()> {
    if !src.exists() {
        return Err(anyhow!("template missing: {}", src.display()));
    }
    let mut content = fs::read_to_string(src)
        .with_context(|| format!("read template {}", src.display()))?;
    for (key, value) in vars {
        let placeholder = format!("{{{{{key}}}}}");
        content = content.replace(&placeholder, value);
    }
    fs::write(dst, content).with_context(|| format!("write rendered {}", dst.display()))?;
    set_mode(dst, 0o644)?;
    Ok(())
}

/// In-place `KEY=value` line replacement for ag.env. Adds the line at EOF
/// if no matching line exists.
fn edit_env_file(path: &Path, kvs: &[(&str, &str)]) -> Result<()> {
    let original = fs::read_to_string(path)
        .with_context(|| format!("read env file {}", path.display()))?;
    let mut lines: Vec<String> = original.lines().map(String::from).collect();
    for (key, value) in kvs {
        let prefix = format!("{key}=");
        let mut replaced = false;
        for line in lines.iter_mut() {
            let trimmed = line.trim_start();
            if trimmed.starts_with(&prefix)
                || trimmed.starts_with(&format!("#{prefix}"))
            {
                *line = format!("{key}={value}");
                replaced = true;
                break;
            }
        }
        if !replaced {
            lines.push(format!("{key}={value}"));
        }
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    fs::write(path, out).with_context(|| format!("write env file {}", path.display()))?;
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
    if paths::skip_systemctl() {
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

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, perms)
        .with_context(|| format!("chmod {:o} {}", mode, path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
