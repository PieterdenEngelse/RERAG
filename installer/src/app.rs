//! Global app state — Screen enum + helpers + the row/step/summary view-model
//! types the screens render.
//!
//! Detection (Screen 2) and Prompts (Screen 3) are wired to real probes in
//! Phase C — see `crate::detection` and `crate::prompts`. Progress (4) /
//! Summary (6) still read from the `mock_*` helpers below; Phase D and E
//! replace those.

use dioxus::prelude::*;

use crate::detection::DetectionResult;

/// The six screens in fixed flow order. See docs/bin3 §Screen flow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Welcome,
    Detection,
    Prompts,
    Progress,
    FirstRun,
    Done,
}

impl Screen {
    pub fn next(self) -> Self {
        match self {
            Screen::Welcome => Screen::Detection,
            Screen::Detection => Screen::Prompts,
            Screen::Prompts => Screen::Progress,
            Screen::Progress => Screen::FirstRun,
            Screen::FirstRun => Screen::Done,
            Screen::Done => Screen::Done,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            // Once Progress starts, writes happen — no back navigation.
            Screen::Welcome | Screen::Progress | Screen::FirstRun | Screen::Done => self,
            Screen::Detection => Screen::Welcome,
            Screen::Prompts => Screen::Detection,
        }
    }
    pub fn can_go_back(self) -> bool {
        matches!(self, Screen::Detection | Screen::Prompts)
    }
    pub fn step_number(self) -> u8 {
        match self {
            Screen::Welcome => 1,
            Screen::Detection => 2,
            Screen::Prompts => 3,
            Screen::Progress => 4,
            Screen::FirstRun => 5,
            Screen::Done => 6,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Screen::Welcome => "Welcome",
            Screen::Detection => "Detection",
            Screen::Prompts => "Choices",
            Screen::Progress => "Install",
            Screen::FirstRun => "First-Run Config",
            Screen::Done => "Done",
        }
    }
}

/// Helper hook to access the current-screen signal from any component.
pub fn use_screen() -> Signal<Screen> {
    use_context::<Signal<Screen>>()
}

// =============================================================================
// Detection row view-model
// =============================================================================

#[derive(Clone, PartialEq, Eq)]
pub struct DetectionRow {
    pub label: &'static str,
    pub value: String,
    pub status: DetectionStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DetectionStatus {
    Ok,
    Warn,
}

/// Render a `DetectionResult` as the ten-row table the screen shows.
/// Ok / Warn classification mirrors the bash installer's reuse policy:
/// anything that would trigger a prompt or block the install is Warn;
/// anything safe to keep / install fresh is Ok.
///
/// Labels and values are cfg-branched where the underlying mechanism
/// differs between Linux (systemd units, `/proc`, `$HOME`) and Windows
/// (Scheduled Tasks, HTTP probes, the install volume). The rows
/// themselves are the same set on both platforms — `native_obs` is
/// always empty on Windows so no row would surface there anyway.
pub fn detection_rows(d: &DetectionResult) -> Vec<DetectionRow> {
    #[allow(unused_mut)]
    let mut rows = vec![
        DetectionRow {
            label: "Distro",
            value: d.distro.clone().unwrap_or_else(|| "unknown".to_string()),
            // Always Ok — informational. By the time the GUI loads we
            // already passed the platform's launcher gate (AppRun glibc
            // check on Linux, MSI requirements on Windows), so anything
            // that gets us here is "supported enough."
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "Disk",
            value: if d.disk_free_gb == 0 {
                "unknown".to_string()
            } else if cfg!(windows) {
                format!(
                    "{} GB free on install volume (≥ 20 GB recommended)",
                    d.disk_free_gb
                )
            } else {
                format!("{} GB free on $HOME (≥ 20 GB recommended)", d.disk_free_gb)
            },
            status: if d.disk_free_gb >= 20 || d.disk_free_gb == 0 {
                DetectionStatus::Ok
            } else {
                DetectionStatus::Warn
            },
        },
        DetectionRow {
            label: "Docker",
            value: d
                .docker_present
                .clone()
                .unwrap_or_else(|| "not on PATH".to_string()),
            status: if d.docker_present.is_some() {
                DetectionStatus::Ok
            } else {
                DetectionStatus::Warn
            },
        },
        DetectionRow {
            label: if cfg!(windows) {
                "Ollama responding"
            } else {
                "Ollama active"
            },
            value: if d.ollama_active {
                if cfg!(windows) {
                    "/api/tags responded 2xx".to_string()
                } else {
                    "user systemd service, active".to_string()
                }
            } else if cfg!(windows) {
                "no response at 127.0.0.1:11434 — LLM modes will return 503".to_string()
            } else {
                "user systemd service not active — LLM modes will return 503".to_string()
            },
            // Soft signal — bash logs this as a warning but doesn't prompt or
            // abort. Surface as Ok so it doesn't look like a blocker.
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: if cfg!(windows) {
                "FalkorDB container"
            } else {
                "FalkorDB unit"
            },
            value: if d.falkordb_healthy {
                if cfg!(windows) {
                    "ag-falkordb healthy — will reuse".to_string()
                } else {
                    "active — will reuse".to_string()
                }
            } else if cfg!(windows) {
                "not running — will start via compose".to_string()
            } else {
                "not active — will install".to_string()
            },
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "Compose stack",
            value: if d.compose_up {
                "project=ag already running".to_string()
            } else {
                "not running — will start".to_string()
            },
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "ag.env",
            value: if d.ag_env_exists {
                "present — install will preserve it".to_string()
            } else {
                "not present — install will create it".to_string()
            },
            // Always Ok: bash treats this as silent-reuse (the install does the
            // right thing either way, no prompt fires). Surfacing it here is a
            // "make the invisible visible" win — the user knows their config
            // won't be overwritten.
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "Backend port 3010",
            value: if d.backend_port_busy {
                "in use by another process".to_string()
            } else {
                "free".to_string()
            },
            status: if d.backend_port_busy {
                DetectionStatus::Warn
            } else {
                DetectionStatus::Ok
            },
        },
        DetectionRow {
            label: "RAM",
            value: if d.ram_gb == 0 {
                "unknown".to_string()
            } else {
                format!("{} GB total — compose stack uses ~3 GB resident", d.ram_gb)
            },
            status: if d.ram_gb > 0 && d.ram_gb < 8 {
                DetectionStatus::Warn
            } else {
                DetectionStatus::Ok
            },
        },
        DetectionRow {
            label: if cfg!(windows) {
                "Existing ag task"
            } else {
                "Existing ag.service"
            },
            value: if d.ag_service_drift {
                if cfg!(windows) {
                    "registered but Command points elsewhere".to_string()
                } else {
                    "present but hand-edited (drift detected)".to_string()
                }
            } else if cfg!(windows) {
                "not registered (or points at ag-start.cmd)".to_string()
            } else {
                "not present (or matches template)".to_string()
            },
            status: if d.ag_service_drift {
                DetectionStatus::Warn
            } else {
                DetectionStatus::Ok
            },
        },
    ];

    // WSL2 Docker Engine — Windows-only row. The fields exist on both
    // platforms (always None/false on Linux), but the row itself only makes
    // sense on Windows. Gated on the statement, not a `#[cfg]` vec element
    // (attributes on expressions are unstable on stable Rust).
    #[cfg(windows)]
    rows.push(DetectionRow {
        label: "WSL2 Docker Engine",
        value: if let Some(v) = &d.wsl2_docker_version {
            format!("installed in WSL2 ({v})")
        } else if d.wsl2_available {
            "WSL2 enabled — installer can add Docker Engine here (no admin needed)".to_string()
        } else if d.virtualization_blocked {
            "blocked — hardware virtualization is off in firmware; enable Intel VT-x / \
            AMD-V (SVM) in BIOS/UEFI and reboot before WSL2 or Docker Desktop can run"
                .to_string()
        } else {
            "WSL2 not enabled — the installer can enable it (one-time admin + restart) and \
            add a lightweight Docker Engine; it reopens automatically after the restart"
                .to_string()
        },
        // Usually informational (the Docker row is the real blocker), but a
        // firmware-virtualization-off machine can't run any Docker path, so
        // flag it.
        status: if d.virtualization_blocked {
            DetectionStatus::Warn
        } else {
            DetectionStatus::Ok
        },
    });

    rows
}

// =============================================================================
// Install steps + log mock
// =============================================================================

#[derive(Clone, PartialEq, Eq)]
pub struct InstallStep {
    pub name: &'static str,
    pub status: StepStatus,
    pub duration_s: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
}

// =============================================================================
// Summary buckets (the four-list view on the Done screen)
// =============================================================================

#[derive(Clone, PartialEq, Eq)]
pub struct SummaryItem {
    pub key: &'static str,
    pub detail: &'static str,
}

pub fn mock_reused_silent() -> Vec<SummaryItem> {
    vec![
        SummaryItem {
            key: "docker",
            detail: "28.0.1",
        },
        SummaryItem {
            key: "ollama",
            detail: ":11434 + /api/tags OK",
        },
        SummaryItem {
            key: "~/.cargo",
            detail: "warm",
        },
        SummaryItem {
            key: "target/",
            detail: "warm",
        },
        SummaryItem {
            key: "ag.env",
            detail: "preserved",
        },
        SummaryItem {
            key: "libtika",
            detail: "newer at XDG; skipped",
        },
    ]
}
pub fn mock_reused_confirmed() -> Vec<SummaryItem> {
    vec![] // Phase B: no prompts triggered
}
pub fn mock_installed_fresh() -> Vec<SummaryItem> {
    vec![
        SummaryItem {
            key: "ag.service",
            detail: "~/.config/systemd/user/",
        },
        SummaryItem {
            key: "ag-stack.service",
            detail: "~/.config/systemd/user/",
        },
        SummaryItem {
            key: "falkordb.service",
            detail: "~/.config/systemd/user/",
        },
        SummaryItem {
            key: "ag binary",
            detail: "~/.local/bin/ag",
        },
        SummaryItem {
            key: "web/",
            detail: "~/.local/share/ag/web/",
        },
        SummaryItem {
            key: "FalkorDB binaries",
            detail: "~/.local/share/ag/falkordb/",
        },
    ]
}
pub fn mock_assumptions() -> Vec<SummaryItem> {
    vec![] // Phase B: no prompts triggered
}
