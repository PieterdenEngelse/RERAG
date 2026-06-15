//! Screen 1 — Welcome.
//!
//! Static branding + one-paragraph overview + collapsible "what gets installed"
//! preview. Buttons: Cancel / Next → Detection.

use dioxus::prelude::*;

use crate::ui::components::NavFooter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[component]
pub fn Welcome() -> Element {
    let mut show_details = use_signal(|| false);

    rsx! {
        div { class: "screen",
            div { class: "screen-header",
                h1 { class: "screen-title", "RERAG installer" }
                p { class: "screen-subtitle",
                    "Install RERAG (Rust Educational RAG) to your user account."
                }
            }
            div { class: "screen-body screen-body-centered",
                div { class: "welcome-card",
                    p {
                        "This installer copies the ag binary and its runtime "
                        "files into XDG-standard locations under your home "
                        "directory, sets up three "
                        em { "systemd --user" }
                        " services, and configures graph and observability "
                        "back-ends. No system files are modified; no root is "
                        "required for the default install."
                    }
                    div { class: "welcome-actions",
                        button {
                            class: "btn btn-link",
                            onclick: move |_| {
                                let cur = *show_details.read();
                                show_details.set(!cur);
                            },
                            if *show_details.read() { "Hide what gets installed" } else { "What gets installed?" }
                        }
                    }
                    if *show_details.read() {
                        ul { class: "welcome-paths",
                            li { code { "~/.local/bin/ag" } "  — the ag binary" }
                            li { code { "~/.local/lib/libtika_native.so" } "  — document-parser native lib" }
                            li { code { "~/.local/share/ag/" } "  — runtime state (data, index, db, logs, FalkorDB, web/)" }
                            li { code { "~/.config/ag/ag.env" } "  — env file (seeded from .env.example; never overwritten)" }
                            li { code { "~/.config/ag/docker-compose.yml" } "  — observability stack definition" }
                            li { code { "~/.config/systemd/user/{{ag,ag-stack,falkordb}}.service" } " — three composable units" }
                        }
                    }
                    div { class: "welcome-meta",
                        div { class: "welcome-meta-row",
                            span { class: "meta-label", "Installer version" }
                            span { class: "meta-value", "{VERSION}" }
                        }
                    }
                }
            }
            NavFooter { next_label: "Begin".to_string(), hide_back: true }
        }
    }
}
