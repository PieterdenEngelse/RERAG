//! Install step orchestration.
//!
//! The six-step `run()` orchestrator + `step!` macro live here. So do the
//! portable step bodies (`seed_config`, `health_check`) and the platform-
//! neutral helpers (`LogTee`, `step_log`, `render_template`,
//! `edit_env_file`, `set_mode`).
//!
//! The four OS-touching step bodies — directory tree + log open, artifact
//! copy/smoke-test, FalkorDB / compose stack, systemd / Scheduled Task —
//! live under `crate::platform::{linux,windows}` and are invoked via the
//! `ensure_install_tree` / `copy_artifacts` / `install_stack` /
//! `install_service` re-exports.
//!
//! **Sandbox testing** (so this box's real ag install stays untouched):
//!
//! ```bash
//! HOME=/tmp/ag-test SKIP_SYSTEMCTL=1 cargo run -p ag-installer
//! ```
//!
//! - `HOME` redirects every install path (see `crate::paths`).
//! - `SKIP_SYSTEMCTL=1` (Linux) / `SKIP_SCHTASKS=1` (Windows) makes the
//!   service-management shellouts log what they would do instead of
//!   touching the real user systemd / Task Scheduler.
//!
//! See `docs/bin3 §Phase D` for the Linux spec; `docs/wininstall.md §2`
//! for the Windows mapping.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
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
    /// Windows-only: WSL2 was just enabled but the machine must restart
    /// before the install can finish. A logon-resume hook has already been
    /// registered, so the installer reopens itself after the reboot. The
    /// Progress screen shows `message` and offers Restart-now / later.
    RebootRequired {
        message: String,
    },
}

#[cfg(windows)]
pub const INSTALL_DOCKER_STEP_NAME: &str = "Install Docker Desktop";
#[cfg(windows)]
pub const INSTALL_WSL2_DOCKER_STEP_NAME: &str = "Install WSL2 Docker Engine";
#[cfg(windows)]
pub const INSTALL_WSL2_ENABLE_STEP_NAME: &str = "Enable WSL2";

// Step display names. The three that name an OS-specific mechanism are
// cfg-branched so the progress screen never shows Linux terms (XDG, systemd,
// "native service") on Windows. These constants are the single source of
// truth: STEP_NAMES, the `step!` calls in `run()`, and the platform
// `step_log` calls all reference them so the UI step list, the per-step
// status transitions, and the install-log prefixes stay in sync.
pub const STEP_SEED_CONFIG: &str = "Seed config";
pub const STEP_INSTALL_ARTIFACTS: &str = "Install artifacts";
pub const STEP_HEALTH_CHECK: &str = "Health check";

#[cfg(windows)]
pub const STEP_ENSURE_TREE: &str = "Create install folders";
#[cfg(not(windows))]
pub const STEP_ENSURE_TREE: &str = "Ensure XDG tree";

#[cfg(windows)]
pub const STEP_STACK: &str = "Docker compose stack";
#[cfg(not(windows))]
pub const STEP_STACK: &str = "FalkorDB native service";

#[cfg(windows)]
pub const STEP_SERVICE: &str = "Scheduled Tasks";
#[cfg(not(windows))]
pub const STEP_SERVICE: &str = "Systemd user units";

pub const STEP_NAMES: &[&str] = &[
    STEP_ENSURE_TREE,
    STEP_SEED_CONFIG,
    STEP_INSTALL_ARTIFACTS,
    STEP_STACK,
    STEP_SERVICE,
    STEP_HEALTH_CHECK,
];

#[allow(dead_code)]
#[derive(Debug)]
pub struct InstallResult {
    pub success: bool,
    pub log_path: Option<PathBuf>,
}

/// Shared per-run state: the open install log file (so every step's log
/// lines get teed to disk for the failure-modal "Open log" button).
///
/// `pub(crate)` so platform impls (`platform::linux`, `platform::windows`)
/// can write into the same tee from their step bodies.
#[derive(Clone)]
pub(crate) struct LogTee(Arc<Mutex<Option<fs::File>>>);

impl LogTee {
    pub(crate) fn new() -> Self {
        LogTee(Arc::new(Mutex::new(None)))
    }
    pub(crate) fn set(&self, file: fs::File) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(file);
        }
    }
    pub(crate) fn write_line(&self, line: &str) {
        if let Ok(mut slot) = self.0.lock() {
            if let Some(f) = slot.as_mut() {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}

/// Helper: emit a log line via the sender AND tee it into the install log.
pub(crate) fn step_log(
    tx: &ProgressSender,
    tee: &LogTee,
    name: &'static str,
    line: impl Into<String>,
) {
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
                return InstallResult {
                    success: false,
                    log_path,
                };
            }
            tee.write_line(&format!("=== {name} ==="));
            let start = Instant::now();
            match $body.await {
                Ok(()) => {
                    let duration = start.elapsed();
                    tee.write_line(&format!(
                        "=== {name} done in {:.1}s ===\n",
                        duration.as_secs_f32()
                    ));
                    if tx.send(ProgressEvent::StepDone { name, duration }).is_err() {
                        return InstallResult {
                            success: false,
                            log_path,
                        };
                    }
                }
                Err(e) => {
                    let error_text = format!("{e:#}");
                    tee.write_line(&format!("FAILED: {error_text}\n"));
                    let _ = tx.send(ProgressEvent::StepFailed {
                        name,
                        error: error_text,
                    });
                    return InstallResult {
                        success: false,
                        log_path,
                    };
                }
            }
        }};
    }

    #[cfg(windows)]
    match answers.choice(PromptId::DockerMissing) {
        Some("install_docker_desktop") => {
            step!(
                INSTALL_DOCKER_STEP_NAME,
                crate::platform::install_docker(&tx, &tee)
            );
        }
        Some("install_wsl2_docker") => {
            step!(
                INSTALL_WSL2_DOCKER_STEP_NAME,
                crate::platform::install_docker_wsl2(&paths, &tx, &tee)
            );
        }
        Some("enable_wsl2_docker") => {
            // Enable the WSL2 feature first (elevated). The step! macro can't
            // express the branch on the enable outcome, so plumb the events
            // here. On ReadyNow we continue into the Docker-in-WSL2 install;
            // on RebootRequired we register the logon-resume hook and stop —
            // the install finishes after the user restarts and the installer
            // relaunches itself (detection-driven, so a plain relaunch
            // resumes where this left off).
            use crate::platform::WslEnableOutcome;
            let name = INSTALL_WSL2_ENABLE_STEP_NAME;
            if tx.send(ProgressEvent::StepStart { name }).is_err() {
                return InstallResult {
                    success: false,
                    log_path,
                };
            }
            let started = std::time::Instant::now();
            match crate::platform::enable_wsl2(&tx, &tee).await {
                Ok(WslEnableOutcome::ReadyNow) => {
                    let _ = tx.send(ProgressEvent::StepDone {
                        name,
                        duration: started.elapsed(),
                    });
                    step!(
                        INSTALL_WSL2_DOCKER_STEP_NAME,
                        crate::platform::install_docker_wsl2(&paths, &tx, &tee)
                    );
                }
                Ok(WslEnableOutcome::RebootRequired) => {
                    if let Err(e) = crate::platform::register_wsl2_resume(&tx, &tee).await {
                        let error = format!("{e:#}");
                        tee.write_line(&format!("FAILED: {error}\n"));
                        let _ = tx.send(ProgressEvent::StepFailed { name, error });
                        return InstallResult {
                            success: false,
                            log_path,
                        };
                    }
                    let _ = tx.send(ProgressEvent::StepDone {
                        name,
                        duration: started.elapsed(),
                    });
                    let _ = tx.send(ProgressEvent::RebootRequired {
                        message: "WSL2 has been enabled, but Windows needs to restart to \
                            finish. After you restart and sign back in, this installer reopens \
                            automatically to install Docker and complete setup — the WSL2 \
                            Docker option will be preselected, so just click through."
                            .to_string(),
                    });
                    return InstallResult {
                        success: true,
                        log_path,
                    };
                }
                Err(e) => {
                    let error = format!("{e:#}");
                    tee.write_line(&format!("FAILED: {error}\n"));
                    let _ = tx.send(ProgressEvent::StepFailed { name, error });
                    return InstallResult {
                        success: false,
                        log_path,
                    };
                }
            }
        }
        _ => {}
    }

    step!(
        STEP_ENSURE_TREE,
        crate::platform::ensure_install_tree(&paths, &tx, &tee, &mut log_path)
    );
    step!(
        STEP_SEED_CONFIG,
        seed_config(&paths, &tx, &tee, &answers, backend_port)
    );
    step!(
        STEP_INSTALL_ARTIFACTS,
        crate::platform::copy_artifacts(&paths, &tx, &tee)
    );
    step!(
        STEP_STACK,
        crate::platform::install_stack(&paths, &tx, &tee, &answers)
    );
    step!(
        STEP_SERVICE,
        crate::platform::install_service(&paths, &tx, &tee, &answers, backend_port)
    );
    step!(STEP_HEALTH_CHECK, health_check(&tx, &tee, backend_port));

    let _ = tx.send(ProgressEvent::InstallComplete);
    InstallResult {
        success: true,
        log_path,
    }
}

// =============================================================================
// Step 2: seed_config — env file + compose file, preserve existing
// =============================================================================
//
// Portable: only filesystem copies + env-file edits. `chmod 600` is a no-op
// on non-unix (see `set_mode`).

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
            format!(
                "{} exists — preserved (not overwritten)",
                env_target.display()
            ),
        );
    } else {
        if !env_source.exists() {
            bail!(
                ".env.example missing at {} — cannot seed ag.env",
                env_source.display()
            );
        }
        fs::copy(&env_source, &env_target)
            .with_context(|| format!("copy {} → {}", env_source.display(), env_target.display()))?;
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
            format!(
                "set BACKEND_PORT={backend_port} in {}",
                env_target.display()
            ),
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
// Step 6: health_check — poll /health
// =============================================================================
//
// Portable: `reqwest` over localhost. `skip_systemctl` is the
// cfg-selected sandbox gate (`SKIP_SYSTEMCTL` on Linux,
// `SKIP_SCHTASKS` on Windows).

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
// Helpers — template render, env edit, file mode
// =============================================================================

/// `{{KEY}}` literal substitution, mirroring bash `render_template`.
///
/// `pub(crate)` so platform impls can render systemd unit / Scheduled Task
/// XML files from their step bodies.
pub(crate) fn render_template(src: &Path, dst: &Path, vars: &[(&str, String)]) -> Result<()> {
    if !src.exists() {
        return Err(anyhow!("template missing: {}", src.display()));
    }
    let mut content =
        fs::read_to_string(src).with_context(|| format!("read template {}", src.display()))?;
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
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    fs::write(path, out).with_context(|| format!("write env file {}", path.display()))?;
    Ok(())
}

#[cfg(unix)]
pub(crate) fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, perms)
        .with_context(|| format!("chmod {:o} {}", mode, path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
