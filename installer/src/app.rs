//! Global app state — Phase B scope: Screen enum + helpers + static mock data
//! for Detection / Prompts / Progress / Summary screens.
//!
//! Real detection lands in Phase C; real install in Phase D; real first-run
//! config in Phase E. Until then, every screen reads from the mock_* functions
//! below so we can iterate on layout and navigation without wiring I/O.

use dioxus::prelude::*;

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
// Detection mock data
// =============================================================================

#[derive(Clone)]
pub struct DetectionRow {
    pub label: &'static str,
    pub value: &'static str,
    pub status: DetectionStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DetectionStatus {
    Ok,
    Warn,
}

pub fn mock_detections() -> Vec<DetectionRow> {
    vec![
        DetectionRow {
            label: "Disk",
            value: "22 GB free on $HOME (≥ 20 GB recommended)",
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "Docker",
            value: "present (28.0.1)",
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "Ollama",
            value: "running on :11434 (8 models available)",
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "FalkorDB unit",
            value: "not present — will install",
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "Compose stack",
            value: "not running — will start",
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "Backend port 3010",
            value: "free",
            status: DetectionStatus::Ok,
        },
        DetectionRow {
            label: "RAM",
            value: "7 GB total — compose stack uses ~3 GB",
            status: DetectionStatus::Warn,
        },
        DetectionRow {
            label: "Existing ag.service",
            value: "not present",
            status: DetectionStatus::Ok,
        },
    ]
}

// =============================================================================
// Install steps + log mock
// =============================================================================

#[derive(Clone)]
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

pub fn mock_install_steps() -> Vec<InstallStep> {
    vec![
        InstallStep { name: "Ensure XDG tree", status: StepStatus::Done, duration_s: 0 },
        InstallStep { name: "Seed config", status: StepStatus::Done, duration_s: 0 },
        InstallStep { name: "Install artifacts", status: StepStatus::Done, duration_s: 3 },
        InstallStep { name: "FalkorDB native service", status: StepStatus::Running, duration_s: 4 },
        InstallStep { name: "Systemd user units", status: StepStatus::Pending, duration_s: 0 },
        InstallStep { name: "Health check", status: StepStatus::Pending, duration_s: 0 },
    ]
}

pub fn mock_log_lines() -> Vec<&'static str> {
    vec![
        "[1/6] ▶ Ensure XDG tree",
        "  created/verified ~/.local/share/ag tree",
        "[1/6] ✓ Ensure XDG tree  (0s)",
        "[2/6] ▶ Seed config",
        "  seeded ~/.config/ag/ag.env",
        "  copied docker-compose.yml → ~/.config/ag/",
        "[2/6] ✓ Seed config  (0s)",
        "[3/6] ▶ Install artifacts to XDG paths",
        "  installed ~/.local/bin/ag",
        "  installed ~/.local/lib/libtika_native.so",
        "  rsynced frontend/fro/dist/ → ~/.local/share/ag/web/",
        "  binary smoke-test passed",
        "[3/6] ✓ Install artifacts  (3s)",
        "[4/6] ▶ FalkorDB native service",
        "  extracting binaries from falkordb/falkordb…",
    ]
}

// =============================================================================
// Summary buckets (the four-list view on the Done screen)
// =============================================================================

#[derive(Clone)]
pub struct SummaryItem {
    pub key: &'static str,
    pub detail: &'static str,
}

pub fn mock_reused_silent() -> Vec<SummaryItem> {
    vec![
        SummaryItem { key: "docker", detail: "28.0.1" },
        SummaryItem { key: "ollama", detail: ":11434 + /api/tags OK" },
        SummaryItem { key: "~/.cargo", detail: "warm" },
        SummaryItem { key: "target/", detail: "warm" },
        SummaryItem { key: "ag.env", detail: "preserved" },
        SummaryItem { key: "libtika", detail: "newer at XDG; skipped" },
    ]
}
pub fn mock_reused_confirmed() -> Vec<SummaryItem> {
    vec![] // Phase B: no prompts triggered
}
pub fn mock_installed_fresh() -> Vec<SummaryItem> {
    vec![
        SummaryItem { key: "ag.service", detail: "~/.config/systemd/user/" },
        SummaryItem { key: "ag-stack.service", detail: "~/.config/systemd/user/" },
        SummaryItem { key: "falkordb.service", detail: "~/.config/systemd/user/" },
        SummaryItem { key: "ag binary", detail: "~/.local/bin/ag" },
        SummaryItem { key: "web/", detail: "~/.local/share/ag/web/" },
        SummaryItem { key: "FalkorDB binaries", detail: "~/.local/share/ag/falkordb/" },
    ]
}
pub fn mock_assumptions() -> Vec<SummaryItem> {
    vec![] // Phase B: no prompts triggered
}
