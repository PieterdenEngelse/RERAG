//! Phase E — First-run config.
//!
//! Driven by the FirstRunForm screen. Three responsibilities:
//!
//! 1. **Probe Ollama** for installed models so the dropdown shows the user's
//!    real model list instead of the Phase B hardcoded suggestions.
//! 2. **Atomic env file write** — read the existing `~/.config/ag/ag.env`,
//!    replace specific KEY=VALUE lines, write to a temp file in the same
//!    directory, and rename over the original. Newline preservation +
//!    comment preservation are the load-bearing properties; getting them
//!    wrong corrupts the user's ag.env.
//! 3. **FalkorDB password change** — re-render falkordb.service.tmpl with
//!    the new password, daemon-reload, restart, verify with redis-cli ping.
//!
//! Plus the "Start ag now" flow at the end: `systemctl --user start
//! ag.service` followed by a `/health` poll up to 20s. Success transitions
//! to the Done screen.
//!
//! Sandbox testing: same env vars as install_steps — `HOME=/tmp/ag-test
//! SKIP_SYSTEMCTL=1`. With SKIP_SYSTEMCTL=1 the service restarts log what
//! they'd do; /health poll is skipped.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use tokio::process::Command;
use tokio::time::sleep;

use crate::bundled;
use crate::install_steps::{ProgressEvent, ProgressSender, DEFAULT_BACKEND_PORT, FALKORDB_PORT};
use crate::paths::{self, Paths};

/// Default Ollama API endpoint. Matches the detection probe; both will
/// fail the same way if Ollama isn't running.
const OLLAMA_TAGS_URL: &str = "http://127.0.0.1:11434/api/tags";

/// User choices collected from the FirstRunForm UI.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FirstRunChoices {
    /// `OLLAMA_MODEL=` in ag.env. Empty = leave whatever's there.
    pub ollama_model: String,
    /// `FALKOR_PASSWORD=` in ag.env, and the rendered falkordb.service.
    /// Empty = leave the install_steps default ("agpassword123").
    pub falkordb_password: String,
    /// Informational only — ag doesn't read a default agent mode from env.
    /// Stored for the Done screen summary; selectable per-chat in the
    /// dashboard once ag is up.
    pub agent_mode: String,
    /// Optional LLM API keys. Blank = don't write the line; users without
    /// a paid API key just stay on Ollama.
    pub openai_api_key: String,
    pub openrouter_api_key: String,
    pub anthropic_api_key: String,
}

impl FirstRunChoices {
    /// Returns the (key, value) pairs that should be written to ag.env.
    /// Skips empty values so we don't blank out keys the user already had.
    fn env_pairs(&self) -> Vec<(&'static str, &str)> {
        let mut out: Vec<(&'static str, &str)> = Vec::new();
        if !self.ollama_model.is_empty() {
            out.push(("OLLAMA_MODEL", &self.ollama_model));
        }
        if !self.falkordb_password.is_empty() {
            out.push(("FALKOR_PASSWORD", &self.falkordb_password));
        }
        if !self.openai_api_key.is_empty() {
            out.push(("OPENAI_API_KEY", &self.openai_api_key));
        }
        if !self.openrouter_api_key.is_empty() {
            out.push(("OPENROUTER_API_KEY", &self.openrouter_api_key));
        }
        if !self.anthropic_api_key.is_empty() {
            out.push(("ANTHROPIC_API_KEY", &self.anthropic_api_key));
        }
        out
    }
}

// =============================================================================
// Ollama probe
// =============================================================================

#[derive(Clone, Debug)]
pub enum OllamaProbe {
    /// Ollama responded; here are the model names (e.g. "phi:latest",
    /// "llama3.2:3b"). Empty Vec means Ollama is up but no models pulled.
    Ok(Vec<String>),
    /// Couldn't reach Ollama. UI should show the hint to start it.
    Unreachable,
}

pub async fn probe_ollama_models() -> OllamaProbe {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return OllamaProbe::Unreachable,
    };
    let resp = match client.get(OLLAMA_TAGS_URL).send().await {
        Ok(r) => r,
        Err(_) => return OllamaProbe::Unreachable,
    };
    if !resp.status().is_success() {
        return OllamaProbe::Unreachable;
    }
    let body: serde_json::Value = match resp.json().await {
        Ok(b) => b,
        Err(_) => return OllamaProbe::Ok(Vec::new()),
    };
    let models = body
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    OllamaProbe::Ok(models)
}

// =============================================================================
// Atomic env-file write
// =============================================================================

/// Rewrite `env_path` so each (key, value) pair from `choices` is in the
/// file exactly once. Existing lines for these keys are replaced in place;
/// keys not currently in the file are appended at the end. All other
/// lines (including comments and blank lines) are preserved verbatim.
///
/// The write is atomic: we write to `<env_path>.first-run.tmp` in the same
/// directory, then rename. Either the new content is fully in place, or
/// the file is untouched.
pub fn write_first_run_settings(env_path: &Path, choices: &FirstRunChoices) -> Result<()> {
    let pairs = choices.env_pairs();
    if pairs.is_empty() {
        return Ok(());
    }

    let original = fs::read_to_string(env_path)
        .with_context(|| format!("read {}", env_path.display()))?;

    let mut lines: Vec<String> = original.lines().map(String::from).collect();
    let mut applied = std::collections::HashSet::<&'static str>::new();

    for line in lines.iter_mut() {
        let trimmed = line.trim_start();
        for (key, value) in &pairs {
            let needle = format!("{key}=");
            let commented_needle = format!("#{key}=");
            if trimmed.starts_with(&needle) || trimmed.starts_with(&commented_needle) {
                *line = format!("{key}={value}");
                applied.insert(key);
                break;
            }
        }
    }

    for (key, value) in &pairs {
        if !applied.contains(key) {
            lines.push(format!("{key}={value}"));
        }
    }

    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }

    let tmp_path = env_path.with_extension("first-run.tmp");
    {
        let mut tmp = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .with_context(|| format!("create {}", tmp_path.display()))?;
        tmp.write_all(out.as_bytes())
            .with_context(|| format!("write {}", tmp_path.display()))?;
        tmp.sync_all().ok();
    }
    fs::rename(&tmp_path, env_path).with_context(|| {
        format!("rename {} → {}", tmp_path.display(), env_path.display())
    })?;
    // 0600 in case the user's umask is permissive — ag.env contains the
    // FalkorDB password and LLM API keys.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(env_path, perms).ok();
    }
    Ok(())
}

// =============================================================================
// FalkorDB password change
// =============================================================================

/// Re-render falkordb.service.tmpl with `new_password`, daemon-reload,
/// restart falkordb.service, verify with redis-cli ping using the new
/// password. No-op if `new_password` equals the install-time default
/// (caller can skip the call entirely in that case).
pub async fn change_falkordb_password(
    paths: &Paths,
    tx: &ProgressSender,
    new_password: &str,
) -> Result<()> {
    let step = "Start ag";
    send_log(tx, step, format!("changing FalkorDB password"));

    let tmpl = bundled::systemd_template_dir().join("falkordb.service.tmpl");
    if !tmpl.exists() {
        bail!(
            "falkordb.service.tmpl missing at {} — bundled artifacts incomplete",
            tmpl.display()
        );
    }
    let mut content = fs::read_to_string(&tmpl)
        .with_context(|| format!("read {}", tmpl.display()))?;
    let vars = [
        ("AG_HOME", paths.ag_home.display().to_string()),
        ("FDB_PORT", FALKORDB_PORT.to_string()),
        ("FDB_PASS", new_password.to_string()),
    ];
    for (k, v) in &vars {
        content = content.replace(&format!("{{{{{k}}}}}"), v);
    }
    fs::write(&paths.falkordb_service(), content)
        .with_context(|| format!("write {}", paths.falkordb_service().display()))?;
    send_log(
        tx,
        step,
        format!("re-rendered {}", paths.falkordb_service().display()),
    );

    systemctl_user(tx, step, &["daemon-reload"]).await?;
    systemctl_user(tx, step, &["restart", "falkordb.service"]).await?;

    if paths::skip_systemctl() {
        send_log(tx, step, "SKIP_SYSTEMCTL=1 — skipping redis-cli verify");
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
            if out.status.success()
                && String::from_utf8_lossy(&out.stdout).trim() == "PONG"
            {
                send_log(
                    tx,
                    step,
                    format!(
                        "redis-cli ping with new password OK (attempt {attempt})"
                    ),
                );
                return Ok(());
            }
        }
    }
    bail!("falkordb.service restarted but redis-cli ping with the new password did not return PONG within 5s");
}

// =============================================================================
// Start ag + health poll
// =============================================================================

/// `systemctl --user start ag.service` then poll `/health` up to 20s.
/// Returns Ok once /health responds 2xx; Err with a user-displayable
/// message otherwise.
pub async fn start_ag_and_wait(tx: &ProgressSender, backend_port: u16) -> Result<()> {
    let step = "Start ag";
    systemctl_user(tx, step, &["start", "ag.service"]).await?;

    if paths::skip_systemctl() {
        send_log(
            tx,
            step,
            "SKIP_SYSTEMCTL=1 — no service started, skipping /health poll",
        );
        return Ok(());
    }

    let url = format!("http://127.0.0.1:{backend_port}/health");
    send_log(tx, step, format!("polling {url} (~20s)"));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .with_context(|| "build http client")?;
    for attempt in 1..=10u32 {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                send_log(
                    tx,
                    step,
                    format!("/health: {} on attempt {attempt}", resp.status()),
                );
                return Ok(());
            }
            Ok(resp) => send_log(
                tx,
                step,
                format!("attempt {attempt}: {} — retrying", resp.status()),
            ),
            Err(_) => send_log(
                tx,
                step,
                format!("attempt {attempt}: no response — retrying"),
            ),
        }
        sleep(Duration::from_secs(2)).await;
    }
    Err(anyhow!(
        "ag.service started but /health did not respond 2xx within 20s. \
        Inspect journalctl: `journalctl --user -u ag.service -n 50`"
    ))
}

/// Returns the resolved ag.env path for FirstRunForm's submit handler.
pub fn ag_env_path() -> PathBuf {
    Paths::resolve().ag_env()
}

/// Default backend port for the /health poll. Will eventually read from
/// the ag.env we just wrote, but Phase E sticks to the install-time
/// default; First-Run doesn't expose a port-change field.
pub fn backend_port() -> u16 {
    DEFAULT_BACKEND_PORT
}

// =============================================================================
// Helpers — small wrappers around progress / systemctl, mirroring
// install_steps so the UX is consistent across phases
// =============================================================================

fn send_log(tx: &ProgressSender, name: &'static str, line: impl Into<String>) {
    let _ = tx.send(ProgressEvent::StepLog {
        name,
        line: line.into(),
    });
}

async fn systemctl_user(
    tx: &ProgressSender,
    step_name: &'static str,
    args: &[&str],
) -> Result<()> {
    let pretty = format!("systemctl --user {}", args.join(" "));
    if paths::skip_systemctl() {
        send_log(tx, step_name, format!("SKIP_SYSTEMCTL=1 — would run: {pretty}"));
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
