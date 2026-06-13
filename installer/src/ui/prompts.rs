//! Screen 3 — Prompts.
//!
//! Phase B: one mock prompt is shown (the low-RAM compose profile choice,
//! since the mock detection flagged RAM as warn). The form layout is the
//! same shape Phase C uses when real prompts fire from real detection.

use dioxus::prelude::*;

use crate::ui::components::{NavFooter, PromptRadio};
use crate::ui::components::prompt_radio::RadioOption;

#[component]
pub fn PromptsScreen() -> Element {
    let selected = use_signal(|| "core".to_string());

    let options = vec![
        RadioOption {
            key: "core".to_string(),
            label: "--with-stack=core (Redis only)".to_string(),
            description: "Recommended on low-RAM hosts. Skips Loki/Tempo/OTel/Grafana/Prometheus.".to_string(),
        },
        RadioOption {
            key: "observability".to_string(),
            label: "--with-stack=observability".to_string(),
            description: "Loki + Tempo + OTel + Grafana + Prometheus, no Redis cache.".to_string(),
        },
        RadioOption {
            key: "all".to_string(),
            label: "Full stack (default)".to_string(),
            description: "Bring up everything. Uses ~3 GB resident on this host.".to_string(),
        },
        RadioOption {
            key: "none".to_string(),
            label: "--no-stack — skip the compose stack entirely".to_string(),
            description: "Useful if you'll manage observability externally.".to_string(),
        },
    ];

    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title", "One choice to make" }
                p { class: "screen-subtitle",
                    "Detection found something that warrants a decision before "
                    "the install starts."
                }
            }
            div { class: "screen-body",
                div { class: "prompt-card",
                    div { class: "prompt-card-header",
                        h2 { "Compose stack profile" }
                        p { class: "prompt-card-context",
                            "Host has 7 GB RAM. Full compose stack uses ~3 GB "
                            "resident. Pick a profile:"
                        }
                    }
                    PromptRadio {
                        name: "compose_profile".to_string(),
                        options: options,
                        selected: selected,
                    }
                }
                p { class: "prompts-footnote",
                    "(Phase B: this prompt is mocked. Phase C decides which "
                    "prompts to show based on real detection, and may show "
                    "more than one — they all fire here before any writes.)"
                }
            }
            NavFooter { next_label: "Begin install".to_string() }
        }
    }
}
