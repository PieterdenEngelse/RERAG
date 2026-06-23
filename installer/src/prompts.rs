//! Prompt model — decides which forms to show based on detection results,
//! and carries each prompt's option set + default choice.
//!
//! Choices and defaults mirror `run_prompts()` in
//! `installers/install-linux.sh`. Submit handlers in
//! `ui/prompts.rs` write the user's selection into `PromptAnswers` for
//! Phase D's installer to consume.

use crate::detection::DetectionResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromptId {
    DiskLow,
    DockerMissing,
    PortBusy,
    LowRam,
    /// Native loki / tempo / otelcol units (Linux only — no analog on
    /// Windows). The variant exists on both platforms so the
    /// `title()`/`context()`/`options()` match stays exhaustive without
    /// per-platform branches; `required_prompts` is the only call site,
    /// and it's `#[cfg(unix)]`-gated. The Windows-side `dead_code`
    /// suppression below documents that asymmetry.
    #[cfg_attr(windows, allow(dead_code))]
    NativeObs,
    SystemRedis,
    /// "Existing ag service / scheduled task was edited" — Linux variant
    /// is the rendered `~/.config/systemd/user/ag.service`, Windows
    /// variant is the Scheduled-Task XML. Labels branch on `cfg!(windows)`
    /// in `title()` / `context()` / `options()`. The same field
    /// (`DetectionResult::ag_service_drift`) drives both — the bool's
    /// meaning generalizes cleanly across OSes.
    AgInstallDrift,
}

/// Disk warning threshold in GB. Matches bash `preflight_disk` `warn=20`.
/// Below `HARD_GB` bash aborts outright; Phase C surfaces a prompt for the
/// warn band only.
const DISK_WARN_GB: u64 = 20;
const DISK_HARD_GB: u64 = 10;

/// RAM threshold below which we recommend a smaller compose profile.
/// Matches bash `detect_low_ram` (`gb < 8`).
const LOW_RAM_THRESHOLD_GB: u64 = 8;

/// Returns the prompts that should fire for this detection result, in the
/// order they should be presented to the user.
pub fn required_prompts(d: &DetectionResult) -> Vec<PromptId> {
    let mut prompts = Vec::new();
    if d.disk_free_gb >= DISK_HARD_GB && d.disk_free_gb < DISK_WARN_GB {
        prompts.push(PromptId::DiskLow);
    }
    if d.docker_present.is_none() {
        prompts.push(PromptId::DockerMissing);
    }
    if d.backend_port_busy {
        prompts.push(PromptId::PortBusy);
    }
    if d.ram_gb > 0 && d.ram_gb < LOW_RAM_THRESHOLD_GB {
        prompts.push(PromptId::LowRam);
    }
    // NativeObs has no analog on Windows — there are no native loki /
    // tempo / otelcol units there, only the compose-managed observability
    // services. `DetectionResult::native_obs` is always `vec![]` on
    // Windows, so this branch is a no-op there; the explicit `#[cfg(unix)]`
    // makes the asymmetry intentional rather than incidental.
    #[cfg(unix)]
    if !d.native_obs.is_empty() {
        prompts.push(PromptId::NativeObs);
    }
    if d.system_redis {
        prompts.push(PromptId::SystemRedis);
    }
    if d.ag_service_drift {
        prompts.push(PromptId::AgInstallDrift);
    }
    prompts
}

/// One option in a prompt's radio group: stable key, label shown next to the
/// radio, and a one-line description underneath.
#[derive(Clone, PartialEq, Eq)]
pub struct PromptOption {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

impl PromptId {
    pub fn title(self) -> &'static str {
        match self {
            PromptId::DiskLow => "Disk is tight",
            PromptId::DockerMissing => "Docker is missing",
            PromptId::PortBusy => "Backend port is in use",
            PromptId::LowRam => "Compose stack profile",
            PromptId::NativeObs => "Native observability detected",
            PromptId::SystemRedis => "System Redis detected",
            PromptId::AgInstallDrift => {
                if cfg!(windows) {
                    "Existing ag scheduled task was edited"
                } else {
                    "Existing ag.service was edited"
                }
            }
        }
    }

    /// Context paragraph rendered above the form; pulls values from detection
    /// so each prompt explains the specific condition that triggered it.
    pub fn context(self, d: &DetectionResult) -> String {
        match self {
            PromptId::DiskLow => format!(
                "{} GB free on $HOME (recommended ≥ {} GB). Below {} GB the install would refuse to run. \
                Continuing is fine but the install may be tight if target/ rebuilds.",
                d.disk_free_gb, DISK_WARN_GB, DISK_HARD_GB
            ),
            PromptId::DockerMissing => {
                "docker isn't on PATH. The compose stack (FalkorDB / Redis / observability) \
                needs it. The official get.docker.com script is the standard route."
                    .to_string()
            }
            PromptId::PortBusy => "Something is already listening on port 3010. \
                If you continue with the default port, ag.service will fail to bind."
                .to_string(),
            PromptId::LowRam => format!(
                "Host has {} GB RAM. The full compose stack uses ~3 GB resident. \
                Pick a profile that fits.",
                d.ram_gb
            ),
            PromptId::NativeObs => format!(
                "Native observability units already active: {}. \
                ag can reuse them (skip our compose stack) or run its own alongside.",
                d.native_obs.join(", ")
            ),
            PromptId::SystemRedis => {
                "Something is responding to redis-cli on 127.0.0.1:6379. \
                ag can use it (and skip the compose Redis), or install ag-redis alongside."
                    .to_string()
            }
            PromptId::AgInstallDrift => {
                if cfg!(windows) {
                    "The `ag` scheduled task is registered, but its <Command> doesn't point \
                    at the installer-managed ag-start.cmd. Pick how to handle the rendered \
                    task from this install."
                        .to_string()
                } else {
                    "~/.config/systemd/user/ag.service exists but is missing one or more lines \
                    from our template — almost certainly hand-edited. Pick how to handle the \
                    rendered unit from this install."
                        .to_string()
                }
            }
        }
    }

    pub fn options(self) -> Vec<PromptOption> {
        match self {
            PromptId::DiskLow => vec![
                PromptOption {
                    key: "continue",
                    label: "Continue anyway",
                    description: "Default. Phase D will still abort if disk drops below the hard threshold mid-install.",
                },
                PromptOption {
                    key: "abort",
                    label: "Abort install",
                    description: "Free up space first, then re-run the installer.",
                },
            ],
            PromptId::DockerMissing => vec![
                PromptOption {
                    key: "install",
                    label: "Install via get.docker.com (requires sudo)",
                    description: "Equivalent to the bash installer's --install-docker.",
                },
                PromptOption {
                    key: "abort",
                    label: "Abort — I'll install Docker manually",
                    description: "Default. Re-run the installer once docker is on PATH.",
                },
            ],
            PromptId::PortBusy => vec![
                PromptOption {
                    key: "pick",
                    label: "Pick a different port",
                    description: "Default. First-Run Config (next screen) will ask for a free port in 1024-65535.",
                },
                PromptOption {
                    key: "force",
                    label: "Force port 3010 anyway",
                    description: "ag.service will likely fail to bind — only useful if you'll free the port before starting ag.",
                },
                PromptOption {
                    key: "abort",
                    label: "Abort install",
                    description: "Stop whatever is using 3010, then re-run.",
                },
            ],
            PromptId::LowRam => vec![
                PromptOption {
                    key: "core",
                    label: "--with-stack=core (Redis only)",
                    description: "Default for low-RAM hosts. Skips Loki / Tempo / OTel / Grafana / Prometheus.",
                },
                PromptOption {
                    key: "observability",
                    label: "--with-stack=observability",
                    description: "Loki + Tempo + OTel + Grafana + Prometheus, no Redis cache.",
                },
                PromptOption {
                    key: "all",
                    label: "Full stack",
                    description: "Bring up everything. Uses ~3 GB resident on this host.",
                },
                PromptOption {
                    key: "none",
                    label: "--no-stack — skip the compose stack entirely",
                    description: "Useful if you'll manage observability externally.",
                },
            ],
            PromptId::NativeObs => vec![
                PromptOption {
                    key: "natives",
                    label: "Use natives (skip ag-stack.service)",
                    description: "Default. Leaves OTEL_EXPORTER_OTLP_ENDPOINT pointing at the native otelcol.",
                },
                PromptOption {
                    key: "compose",
                    label: "Force compose stack",
                    description: "Bring up the full ag-stack alongside the natives.",
                },
                PromptOption {
                    key: "abort",
                    label: "Abort install",
                    description: "Decide later.",
                },
            ],
            PromptId::SystemRedis => vec![
                PromptOption {
                    key: "system",
                    label: "Use system Redis at 127.0.0.1:6379",
                    description: "Default. Sets REDIS_URL=redis://127.0.0.1:6379/ in ag.env.",
                },
                PromptOption {
                    key: "compose",
                    label: "Install ag-redis alongside",
                    description: "Compose Redis on :6379 internal — only used if your system Redis goes down.",
                },
                PromptOption {
                    key: "abort",
                    label: "Abort install",
                    description: "Decide later.",
                },
            ],
            PromptId::AgInstallDrift => {
                if cfg!(windows) {
                    vec![
                        PromptOption {
                            key: "keep",
                            label: "Keep existing ag task (skip rendering)",
                            description: "Default. Your registered scheduled task stays in place; this install does not overwrite it.",
                        },
                        PromptOption {
                            key: "backup",
                            label: "Back up → ag.xml.bak-<ts> and replace",
                            description: "Safe replace. Original XML is preserved with a timestamp suffix.",
                        },
                        PromptOption {
                            key: "replace",
                            label: "Replace without backup",
                            description: "Destructive. Only pick if you're certain you don't need the existing task.",
                        },
                    ]
                } else {
                    vec![
                        PromptOption {
                            key: "keep",
                            label: "Keep existing ag.service (skip rendering)",
                            description: "Default. Your hand-edited unit stays in place; this install does not overwrite it.",
                        },
                        PromptOption {
                            key: "backup",
                            label: "Back up → ag.service.bak-<ts> and replace",
                            description: "Safe replace. Original is preserved with a timestamp suffix.",
                        },
                        PromptOption {
                            key: "replace",
                            label: "Replace without backup",
                            description: "Destructive. Only pick if you're certain you don't need the existing unit.",
                        },
                    ]
                }
            }
        }
    }

    pub fn default_choice(self) -> &'static str {
        match self {
            PromptId::DiskLow => "continue",
            PromptId::DockerMissing => "abort",
            PromptId::PortBusy => "pick",
            PromptId::LowRam => "core",
            PromptId::NativeObs => "natives",
            PromptId::SystemRedis => "system",
            PromptId::AgInstallDrift => "keep",
        }
    }
}

/// User's answers for every prompt that fired this run. Phase D consumes this
/// to make install decisions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptAnswers {
    /// Map from `PromptId.title()` → chosen option `key`. Empty when no
    /// prompts fired.
    pub choices: std::collections::BTreeMap<&'static str, String>,
    /// Backend port the user picked when `PromptId::PortBusy` resolved to
    /// `"pick"`. None means default 3010.
    pub backend_port: Option<u16>,
}

impl PromptAnswers {
    pub fn set_choice(&mut self, id: PromptId, value: String) {
        self.choices.insert(id.title(), value);
    }
    pub fn choice(&self, id: PromptId) -> Option<&str> {
        self.choices.get(id.title()).map(String::as_str)
    }
}
