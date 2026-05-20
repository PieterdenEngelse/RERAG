//! Deployment-capability detection.
//!
//! At startup we probe the host for a few "can ag do X?" questions:
//!  - Is `docker compose` available + is a compose file reachable?
//!  - Is `journalctl --user` available?
//!  - Which deployment mode are we running under (systemd / compose / bin)?
//!
//! Results are cached in a process-global and exposed via
//! `GET /runtime/capabilities`. The UI uses them to hide controls that
//! would silently no-op in this deployment.
//!
//! Self-restart is intentionally NOT a capability flag — `restart_self`
//! works in every deployment (see `lifecycle.rs`).

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    Systemd,
    DockerCompose,
    Container,
    Bin,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    pub deployment_mode: DeploymentMode,
    pub can_manage_compose: bool,
    pub can_view_journal: bool,
    pub managed_compose_file: Option<String>,
}

impl Capabilities {
    pub fn detect() -> Self {
        let compose_file = managed_compose_path();
        let can_manage_compose = compose_file.as_ref().map(|p| p.exists()).unwrap_or(false)
            && binary_runs("docker", &["compose", "version"]);

        let can_view_journal = binary_runs("journalctl", &["--version"]);

        let deployment_mode = detect_deployment_mode(can_manage_compose);

        Self {
            deployment_mode,
            can_manage_compose,
            can_view_journal,
            managed_compose_file: compose_file.map(|p| p.display().to_string()),
        }
    }
}

static GLOBAL: OnceLock<Arc<Capabilities>> = OnceLock::new();

pub fn install_global(caps: Arc<Capabilities>) {
    let _ = GLOBAL.set(caps);
}

pub fn global() -> Option<Arc<Capabilities>> {
    GLOBAL.get().cloned()
}

/// Where ag looks for the bundled docker-compose file.
fn managed_compose_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AG_COMPOSE_FILE") {
        return Some(PathBuf::from(p));
    }
    let cwd = std::env::current_dir().ok()?;
    let candidate = cwd.join("docker-compose.yml");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

/// Best-effort: probe a binary by running it with safe flags. Treats any
/// error or non-zero exit as "not available".
fn binary_runs(bin: &str, args: &[&str]) -> bool {
    std::process::Command::new(bin)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Heuristic: pick the most specific mode the environment supports.
fn detect_deployment_mode(has_compose: bool) -> DeploymentMode {
    // Container: presence of /.dockerenv is a reliable marker.
    if std::path::Path::new("/.dockerenv").exists() {
        return DeploymentMode::Container;
    }
    // Systemd-user: invocation parent unit is set.
    if std::env::var("INVOCATION_ID").is_ok() || std::env::var("JOURNAL_STREAM").is_ok() {
        return DeploymentMode::Systemd;
    }
    if has_compose {
        return DeploymentMode::DockerCompose;
    }
    if std::env::args().next().is_some() {
        return DeploymentMode::Bin;
    }
    DeploymentMode::Unknown
}
