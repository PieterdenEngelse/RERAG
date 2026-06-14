//! Real detection probes — mirrors the `detect_*` functions in
//! `installers/install-linux.sh`.
//!
//! Every probe shells out to a system tool (`docker`, `systemctl`, `ss`,
//! `redis-cli`, `df`) or reads a well-known file (`/proc/meminfo`,
//! `~/.config/...`). On any failure — command-not-found, non-zero exit, parse
//! failure — the probe returns the "not present" value (`false`, `None`, `0`)
//! rather than propagating the error. Detection is best-effort: a missing
//! tool is information, not a crash.
//!
//! See `docs/bin3 §Phase C` for the spec.

use std::process::Stdio;
use tokio::process::Command;

const BACKEND_PORT: u16 = 3010;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DetectionResult {
    /// `docker --version` string when present, `None` otherwise.
    pub docker_present: Option<String>,
    /// `systemctl --user is-active ollama` exits 0.
    pub ollama_active: bool,
    /// `docker compose ls` lists a project named `ag`.
    pub compose_up: bool,
    /// `falkordb.service` is active (Phase D will also verify PONG once the
    /// configured port + password are known).
    pub falkordb_healthy: bool,
    /// `~/.config/ag/ag.env` exists.
    pub ag_env_exists: bool,
    /// Something is bound on `BACKEND_PORT` (per `ss -tln`).
    pub backend_port_busy: bool,
    /// `redis-cli -p 6379 ping` returns PONG and the listener isn't our own
    /// ag-redis container.
    pub system_redis: bool,
    /// Active native observability units (loki / tempo / otelcol).
    pub native_obs: Vec<String>,
    /// `~/.config/systemd/user/ag.service` exists but is missing load-bearing
    /// lines from our template (likely hand-edited).
    pub ag_service_drift: bool,
    /// `df -BG --output=avail $HOME` in GB.
    pub disk_free_gb: u64,
    /// `MemTotal:` from `/proc/meminfo` in GB.
    pub ram_gb: u64,
}

/// Runs every probe; independent probes go in parallel via `tokio::join!`.
/// `system_redis` runs after `compose_up` so it can skip when our own
/// ag-redis container owns 6379.
pub async fn run() -> DetectionResult {
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
    }
}

async fn probe_docker() -> Option<String> {
    let out = Command::new("docker").arg("--version").output().await.ok()?;
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

fn xdg_config_dir() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("XDG_CONFIG_HOME") {
        if !p.is_empty() {
            return Some(std::path::PathBuf::from(p));
        }
    }
    std::env::var("HOME")
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".config"))
}
