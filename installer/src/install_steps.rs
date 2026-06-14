//! Install step orchestration.
//!
//! **D.1 scope (this file as-is):** each step emits `ProgressEvent`s describing
//! what *would* happen but performs no filesystem writes, no subprocess calls,
//! no systemctl. The Progress screen consumes the events end-to-end so the UI
//! wiring (step list updates, log streaming, completion gating) can be
//! verified safely against your real ag install.
//!
//! **D.2 scope (next commit):** replace each step's `simulate_*` body with
//! real file copies / template rendering / `systemctl --user` calls,
//! sandboxed via `XDG_DATA_HOME` / `XDG_CONFIG_HOME` / `SKIP_SYSTEMCTL` for
//! testing without disturbing the real install on this box.
//!
//! Step list mirrors `installers/install-linux.sh` 1:1; conditional steps
//! (install_docker, falkordb-skip, native_obs reuse) land in D.2 when
//! `plan_steps()` ports over from bash.

use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;

use crate::bundled;
use crate::prompts::PromptAnswers;

pub type ProgressSender = UnboundedSender<ProgressEvent>;

#[derive(Clone, Debug)]
pub enum ProgressEvent {
    StepStart {
        name: &'static str,
    },
    StepLog {
        // `name` carries through so D.2 can route per-step output (e.g. error
        // context for the failure modal); D.1's receiver doesn't read it yet.
        #[allow(dead_code)]
        name: &'static str,
        line: String,
    },
    StepDone {
        name: &'static str,
        duration: Duration,
    },
    // D.2 constructs this when a real step body returns Err. D.1's skeleton
    // never fails, but the variant is consumed by the Progress screen's
    // failure-modal handler.
    #[allow(dead_code)]
    StepFailed {
        name: &'static str,
        error: String,
    },
    /// Emitted exactly once after the final step completes successfully.
    /// Drives the "Continue" button on the Progress screen.
    InstallComplete,
}

/// Fixed step list for D.1. D.2 makes this conditional on PromptAnswers
/// (install_docker only if DockerMissing="install", falkordb skipped if
/// --no-falkordb, etc.) per bash `plan_steps()`.
pub const STEP_NAMES: &[&str] = &[
    "Ensure XDG tree",
    "Seed config",
    "Install artifacts",
    "FalkorDB native service",
    "Systemd user units",
    "Health check",
];

// D.1 returns this from `run` but the Progress screen doesn't read it —
// completion is driven by the InstallComplete event. D.2 will return
// summary info (steps run, time elapsed, log path) that the Done screen
// surfaces.
#[allow(dead_code)]
#[derive(Debug)]
pub struct InstallResult {
    pub success: bool,
}

/// D.1 skeleton: simulate each step with descriptive "would-do" log lines
/// and a short delay so the UI animation is visible. No writes.
pub async fn run(_answers: PromptAnswers, tx: ProgressSender) -> InstallResult {
    let bundle_label = if bundled::is_appimage() {
        format!("bundled (AppImage at {})", bundled::share_dir().display())
    } else {
        format!("dev tree at {}", bundled::share_dir().display())
    };

    let ag_bin = bundled::ag_binary_path();
    let libtika = bundled::libtika_path();
    let frontend = bundled::frontend_dist_dir();
    let falkordb_stage = bundled::falkordb_stage_dir();
    let compose = bundled::docker_compose_path();
    let env_example = bundled::env_example_path();
    let systemd_dir = bundled::systemd_template_dir();

    let steps: Vec<(&'static str, Vec<String>)> = vec![
        (
            "Ensure XDG tree",
            vec![
                "would create ~/.local/share/ag/{data,index,db,logs,web}".to_string(),
                "would create ~/.local/share/ag/falkordb/".to_string(),
                "would create ~/.config/ag/".to_string(),
                "would create ~/.config/systemd/user/".to_string(),
                "would create ~/.local/bin/ and ~/.local/lib/".to_string(),
            ],
        ),
        (
            "Seed config",
            vec![
                format!("source: {bundle_label}"),
                format!("would copy {} → ~/.config/ag/ag.env (preserve if exists)", env_example.display()),
                format!("would copy {} → ~/.config/ag/docker-compose.yml", compose.display()),
            ],
        ),
        (
            "Install artifacts",
            vec![
                format!("would copy {} → ~/.local/bin/ag", ag_bin.display()),
                match libtika {
                    Some(ref p) => format!("would copy {} → ~/.local/lib/", p.display()),
                    None => "libtika not bundled — skipping (PDF parsing degrades to fallback)".to_string(),
                },
                format!("would rsync {} → ~/.local/share/ag/web/", frontend.display()),
                "would smoke-test ~/.local/bin/ag --version".to_string(),
            ],
        ),
        (
            "FalkorDB native service",
            vec![
                format!("would copy {}/{{redis-server,redis-cli,falkordb.so}} → ~/.local/share/ag/falkordb/", falkordb_stage.display()),
                format!("would render {}/falkordb.service.tmpl → ~/.config/systemd/user/", systemd_dir.display()),
            ],
        ),
        (
            "Systemd user units",
            vec![
                format!("would render {}/ag.service.tmpl → ~/.config/systemd/user/", systemd_dir.display()),
                format!("would render {}/ag-stack.service.tmpl → ~/.config/systemd/user/", systemd_dir.display()),
                "would run: systemctl --user daemon-reload".to_string(),
                "would run: systemctl --user enable --now ag.service".to_string(),
            ],
        ),
        (
            "Health check",
            vec![
                "would poll http://127.0.0.1:3010/health for 30s".to_string(),
                "would poll http://127.0.0.1:3010/ready for 30s".to_string(),
            ],
        ),
    ];

    for (name, log_lines) in steps {
        if tx.send(ProgressEvent::StepStart { name }).is_err() {
            // Receiver dropped — screen was unmounted, abort.
            return InstallResult { success: false };
        }
        let start = std::time::Instant::now();
        for line in log_lines {
            // Small delay between log lines so the UI animation feels real.
            // D.2 replaces this with actual subprocess output streaming, so
            // the delay disappears naturally.
            sleep(Duration::from_millis(120)).await;
            if tx
                .send(ProgressEvent::StepLog {
                    name,
                    line: format!("  {line}"),
                })
                .is_err()
            {
                return InstallResult { success: false };
            }
        }
        // Brief pause so the step's "running" state is visible before flipping
        // to "done". Real steps in D.2 will dwarf this.
        sleep(Duration::from_millis(180)).await;
        if tx
            .send(ProgressEvent::StepDone {
                name,
                duration: start.elapsed(),
            })
            .is_err()
        {
            return InstallResult { success: false };
        }
    }

    let _ = tx.send(ProgressEvent::InstallComplete);
    InstallResult { success: true }
}
