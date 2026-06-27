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
use include_dir::{include_dir, Dir, DirEntry};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{sleep, Duration};

use crate::bundled;
use crate::paths::{self, Paths};
use crate::prompts::{PromptAnswers, PromptId};

// Observability config trees embedded at compile time. docker-compose.yml
// bind-mounts these relative to the directory that holds the compose file
// (the per-user config dir), so they must be staged next to it or every
// config-mounting container (prometheus, grafana, loki, tempo, otel) fails
// to start with "not a directory: mounting a directory onto a file" — Docker
// auto-creates the missing bind source as an empty dir. Embedding (rather
// than adding them to the MSI / AppImage payload) keeps one source of truth
// and needs no packaging or CI changes. Paths are relative to this crate's
// Cargo.toml (installer/), so `../` reaches the repo root. See seed_config.
static OBSERVABILITY_CONFIG: Dir = include_dir!("$CARGO_MANIFEST_DIR/../ops/observability");
static DASHBOARD_CONFIG: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/../backend/src/monitoring/dashboards");

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
pub const STEP_FETCH_MODEL: &str = "Download embedding model";
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
    STEP_FETCH_MODEL,
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
    // Invariant: the Prompts screen disables "Begin install" whenever any
    // choice is "abort", so the install can't be launched in that state. This
    // documents (and in debug builds enforces) that contract at the boundary.
    debug_assert!(
        !answers.has_abort(),
        "install_steps::run reached with an abort choice — the Prompts screen should block it"
    );
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

    // The docker-setup action may come from DockerMissing (CLI absent) or
    // DockerEngineDown (CLI present but daemon down → user routed onto WSL2).
    #[cfg(windows)]
    match answers.docker_setup_choice() {
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
                    // In SKIP_SCHTASKS sandbox mode neither enable_wsl2 nor
                    // register_wsl2_resume touched the system — they only logged
                    // what they would do. Say so plainly; otherwise the user
                    // reboots expecting the auto-resume the real path promises,
                    // but nothing was enabled or registered (the trap that
                    // motivated this branch).
                    let message = if crate::platform::skip_systemctl() {
                        "SKIP_SCHTASKS sandbox — dry run only. WSL2 was NOT actually \
                            enabled and no logon-resume was registered, so restarting \
                            will not reopen the installer. Unset SKIP_SCHTASKS and \
                            re-run (ideally the installed ag-installer.exe) to enable \
                            WSL2 for real."
                            .to_string()
                    } else {
                        "WSL2 has been enabled, but Windows needs to restart to \
                            finish. After you restart and sign back in, this installer \
                            reopens automatically to install Docker and complete setup — \
                            the WSL2 Docker option will be preselected, so just click \
                            through."
                            .to_string()
                    };
                    let _ = tx.send(ProgressEvent::RebootRequired { message });
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
    step!(STEP_FETCH_MODEL, fetch_embedding_model(&paths, &tx, &tee));
    step!(STEP_STACK, async {
        // Mid-install disk guard: re-probe right before the heaviest step
        // (compose image pulls can be several GB). The Prompts-screen gate
        // already enforced the floor from detection, but space can drop
        // between then and now (earlier steps, the WSL2 rootfs download, or
        // other processes) — fail with a clear message rather than a cryptic
        // ENOSPC from docker mid-pull. Same floor/message as the pre-install
        // gate (`prompts::disk_blocker`).
        let free = crate::platform::disk_free_gb(&paths).await;
        if let Some(msg) = crate::prompts::disk_blocker(free) {
            anyhow::bail!("{msg}");
        }
        crate::platform::install_stack(&paths, &tx, &tee, &answers).await
    });
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

    // Observability config trees: prometheus / grafana / loki / tempo / otel
    // bind-mount these from paths relative to the compose file. If they're
    // absent, `docker compose up` fails to start every config-mounting
    // container ("not a directory: mounting a directory onto a file" — Docker
    // auto-creates the missing bind source as an empty dir). Stage the
    // embedded trees next to docker-compose.yml. The config dir is the
    // compose file's parent (portable across Linux ~/.config/ag and Windows
    // %APPDATA%\ag without reaching for a platform-specific field).
    let config_dir = compose_target
        .parent()
        .ok_or_else(|| anyhow!("docker-compose.yml path has no parent dir"))?;
    stage_embedded_tree(
        &OBSERVABILITY_CONFIG,
        &config_dir.join("ops").join("observability"),
        "prometheus.yml",
        tx,
        tee,
    )?;
    stage_embedded_tree(
        &DASHBOARD_CONFIG,
        &config_dir
            .join("backend")
            .join("src")
            .join("monitoring")
            .join("dashboards"),
        "datasources.yaml",
        tx,
        tee,
    )?;
    Ok(())
}

/// Write an embedded config tree to `dest`. Skips the write when
/// `dest/<marker>` already exists as a regular file — the user may have
/// edited the staged copy, and we treat config like docker-compose.yml
/// (copy-if-missing, never clobber). Re-extracts when the marker is absent
/// *or* present as something other than a regular file, which repairs the
/// empty directories a prior failed `docker compose up` left behind in place
/// of the real config files.
fn stage_embedded_tree(
    dir: &Dir,
    dest: &Path,
    marker: &str,
    tx: &ProgressSender,
    tee: &LogTee,
) -> Result<()> {
    if dest.join(marker).is_file() {
        step_log(
            tx,
            tee,
            "Seed config",
            format!("{} exists — preserved", dest.display()),
        );
        return Ok(());
    }
    extract_embedded(dir, dest)
        .with_context(|| format!("stage embedded config tree → {}", dest.display()))?;
    step_log(
        tx,
        tee,
        "Seed config",
        format!(
            "staged {} config file(s) → {}",
            count_embedded_files(dir),
            dest.display()
        ),
    );
    Ok(())
}

/// Recursively write every file in an embedded `Dir` under `base`,
/// reproducing the tree's directory structure. Built from the final path
/// component at each level (rather than `File::path()`, whose root-relative
/// vs. dir-relative semantics vary by include_dir version) so nesting is
/// always rebuilt from the traversal itself.
fn extract_embedded(dir: &Dir, base: &Path) -> std::io::Result<()> {
    fs::create_dir_all(base)?;
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => {
                let name = sub.path().file_name().unwrap_or_default();
                let target = base.join(name);
                // A prior failed run may have left a regular file where this
                // tree needs a directory; clear it so create_dir_all wins.
                if target.is_file() {
                    fs::remove_file(&target)?;
                }
                extract_embedded(sub, &target)?;
            }
            DirEntry::File(file) => {
                let name = file.path().file_name().unwrap_or_default();
                let target = base.join(name);
                // Docker auto-creates an empty *directory* at a missing bind
                // source; remove it so we can write the real config file
                // (fs::write would otherwise fail with "Is a directory").
                if target.is_dir() {
                    fs::remove_dir_all(&target)?;
                }
                fs::write(&target, file.contents())?;
            }
        }
    }
    Ok(())
}

fn count_embedded_files(dir: &Dir) -> usize {
    dir.entries()
        .iter()
        .map(|e| match e {
            DirEntry::Dir(sub) => count_embedded_files(sub),
            DirEntry::File(_) => 1,
        })
        .sum()
}

// =============================================================================
// Step: fetch_embedding_model — download the ONNX embedder the backend needs
// =============================================================================
//
// The backend's default embedder is in-process ONNX (bge-small-en-v1.5) and it
// *requires* `models/embedding_model.onnx` + `tokenizer.json` under AG_HOME —
// it panics on boot without them. The model is ~127 MB, so it's fetched here at
// install time (the install already pulls a WSL2 rootfs + GBs of container
// images) rather than bloating the MSI/AppImage payload. Idempotent: a present
// model is left alone, so reinstalls and post-reboot resumes don't re-download.

/// HuggingFace source for the default embedder. `onnx/model.onnx` is the fp32
/// model; `tokenizer.json` is its matching tokenizer. Pinned to the model the
/// backend's `EmbeddingModel` default expects (bge-small-en-v1.5, 384-dim).
const EMBED_MODEL_BASE: &str = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main";

/// Minimum size (bytes) for the staged `.onnx` to count as "really there".
/// Guards against a truncated download — or an HTML error page saved under the
/// model's name — satisfying a bare `exists()` check and panicking the backend.
const EMBED_MODEL_MIN_BYTES: u64 = 50 * 1024 * 1024;

/// True when a usable embedder is already staged in `models_dir` — a
/// non-trivial `.onnx` plus its tokenizer. Used to make the download step
/// idempotent across reinstalls and resume-after-reboot runs.
fn embedding_model_present(models_dir: &Path) -> bool {
    let model_ok = fs::metadata(models_dir.join("embedding_model.onnx"))
        .map(|m| m.len() >= EMBED_MODEL_MIN_BYTES)
        .unwrap_or(false);
    model_ok && models_dir.join("tokenizer.json").is_file()
}

async fn fetch_embedding_model(paths: &Paths, tx: &ProgressSender, tee: &LogTee) -> Result<()> {
    // The backend resolves ONNX_MODEL_PATH ("models/embedding_model.onnx")
    // relative to its working directory, which the service launches as AG_HOME.
    let models_dir = paths.ag_home.join("models");

    if embedding_model_present(&models_dir) {
        step_log(
            tx,
            tee,
            STEP_FETCH_MODEL,
            format!(
                "embedder already present in {} — skipping",
                models_dir.display()
            ),
        );
        return Ok(());
    }

    fs::create_dir_all(&models_dir).with_context(|| format!("create {}", models_dir.display()))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(900))
        .build()
        .with_context(|| "build http client for model download")?;

    step_log(
        tx,
        tee,
        STEP_FETCH_MODEL,
        "fetching ONNX embedder bge-small-en-v1.5 (~127 MB, one-time) from HuggingFace",
    );
    download_file(
        &client,
        &format!("{EMBED_MODEL_BASE}/onnx/model.onnx"),
        &models_dir.join("embedding_model.onnx"),
        tx,
        tee,
    )
    .await?;
    download_file(
        &client,
        &format!("{EMBED_MODEL_BASE}/tokenizer.json"),
        &models_dir.join("tokenizer.json"),
        tx,
        tee,
    )
    .await?;
    Ok(())
}

/// GET `url` into `dest`, failing on a non-2xx status. Buffers the whole body
/// before writing (the largest artifact is ~127 MB — fine in memory, and it
/// avoids leaving a partially-written file at `dest` on a mid-stream error).
async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    tx: &ProgressSender,
    tee: &LogTee,
) -> Result<()> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        bail!("download returned {} for {url}", resp.status());
    }
    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("read response body for {url}"))?;
    fs::write(dest, &bytes).with_context(|| format!("write {}", dest.display()))?;
    step_log(
        tx,
        tee,
        STEP_FETCH_MODEL,
        format!(
            "downloaded {:.1} MB → {}",
            bytes.len() as f64 / (1024.0 * 1024.0),
            dest.display()
        ),
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique scratch dir under the OS temp dir — avoids clobbering a real
    /// install and lets the test run without env setup.
    fn scratch(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("ag-installer-test-{tag}-{nanos}"))
    }

    /// `extract_embedded` must reproduce the exact relative paths
    /// docker-compose.yml bind-mounts — including the nested `loki/` and
    /// `tempo/` subdirs — or the observability containers fail to start.
    #[test]
    fn extract_observability_tree_matches_compose_mounts() {
        let dest = scratch("obs");
        extract_embedded(&OBSERVABILITY_CONFIG, &dest).expect("extract observability");
        for rel in [
            "prometheus.yml",
            "grafana.ini",
            "loki/config.yml",
            "tempo/config.yml",
            "otel-collector.yml",
        ] {
            assert!(
                dest.join(rel).is_file(),
                "expected staged file {rel} under {}",
                dest.display()
            );
        }
        let _ = fs::remove_dir_all(&dest);
    }

    /// The Grafana provisioning files plus the `ag/` and `extras/` dashboard
    /// directories (mounted as whole dirs) must all land.
    #[test]
    fn extract_dashboard_tree_matches_compose_mounts() {
        let dest = scratch("dash");
        extract_embedded(&DASHBOARD_CONFIG, &dest).expect("extract dashboards");
        for rel in ["datasources.yaml", "ag.yaml", "extras.yaml"] {
            assert!(
                dest.join(rel).is_file(),
                "expected staged file {rel} under {}",
                dest.display()
            );
        }
        for dir in ["ag", "extras"] {
            let d = dest.join(dir);
            assert!(d.is_dir(), "expected dashboard dir {dir}");
            let json_count = fs::read_dir(&d)
                .expect("read dashboard dir")
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .count();
            assert!(json_count > 0, "expected ≥1 .json in {dir}");
        }
        let _ = fs::remove_dir_all(&dest);
    }

    /// The copy-if-missing guard preserves a user-edited marker file and
    /// repairs the empty-directory artifact a failed `docker compose up`
    /// leaves in place of the marker.
    #[test]
    fn stage_guard_preserves_file_but_repairs_junk_dir() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let tee = LogTee::new();

        // Marker already a real file → preserved (no real config written).
        let dest = scratch("guard-keep");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("prometheus.yml"), b"user edit").unwrap();
        stage_embedded_tree(&OBSERVABILITY_CONFIG, &dest, "prometheus.yml", &tx, &tee).unwrap();
        assert_eq!(
            fs::read(dest.join("prometheus.yml")).unwrap(),
            b"user edit",
            "user-edited marker must be preserved"
        );
        assert!(
            !dest.join("loki/config.yml").exists(),
            "preserve path must not re-extract the tree"
        );
        let _ = fs::remove_dir_all(&dest);

        // Marker is a junk *directory* (Docker's auto-created bind source)
        // → not a regular file, so the tree is (re)extracted over it.
        let dest = scratch("guard-repair");
        fs::create_dir_all(dest.join("prometheus.yml")).unwrap();
        stage_embedded_tree(&OBSERVABILITY_CONFIG, &dest, "prometheus.yml", &tx, &tee).unwrap();
        assert!(
            dest.join("loki/config.yml").is_file(),
            "junk-dir marker must trigger re-extraction"
        );
        let _ = fs::remove_dir_all(&dest);
    }

    /// The download step's idempotency guard must require BOTH a tokenizer and
    /// an `.onnx` over the size floor — a missing tokenizer or a truncated
    /// model (an HTML error page saved under the model's name) must not count
    /// as "present", or the backend would panic on a bad file.
    #[test]
    fn embedding_model_present_requires_full_model_and_tokenizer() {
        let dir = scratch("embed");
        fs::create_dir_all(&dir).unwrap();
        assert!(!embedding_model_present(&dir), "empty dir → not present");

        // Truncated model + tokenizer → still not present (below size floor).
        fs::write(dir.join("embedding_model.onnx"), b"<html>404</html>").unwrap();
        fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        assert!(
            !embedding_model_present(&dir),
            "sub-floor .onnx must be rejected"
        );

        // Full-size model but missing tokenizer → not present. Use a sparse
        // file (set_len) so the test doesn't write 50 MB to disk.
        let f = fs::File::create(dir.join("embedding_model.onnx")).unwrap();
        f.set_len(EMBED_MODEL_MIN_BYTES + 1).unwrap();
        drop(f);
        fs::remove_file(dir.join("tokenizer.json")).unwrap();
        assert!(
            !embedding_model_present(&dir),
            "missing tokenizer must be rejected"
        );

        // Full-size model + tokenizer → present.
        fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        assert!(
            embedding_model_present(&dir),
            "full model + tokenizer → present"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
