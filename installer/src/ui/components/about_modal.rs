//! Phase F — About modal.
//!
//! Lightweight overlay surfaced from a button on the Welcome screen.
//! Shows what the installer is, what version of itself is running, when
//! that version was built (build.rs stamps git SHA + UTC timestamp +
//! runner name at compile time), and links out to the design docs +
//! releases page.

use dioxus::prelude::*;

pub const GIT_SHA: &str = env!("AG_INSTALLER_GIT_SHA");
pub const BUILT_AT: &str = env!("AG_INSTALLER_BUILT_AT");
pub const RUNNER: &str = env!("AG_INSTALLER_RUNNER");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Props, Clone, PartialEq)]
pub struct AboutModalProps {
    pub open: Signal<bool>,
}

#[component]
pub fn AboutModal(props: AboutModalProps) -> Element {
    let mut open = props.open;
    if !*open.read() {
        return rsx! {};
    }
    let releases_url = "https://github.com/PieterdenEngelse/RARAG/releases";
    let design_url = "https://github.com/PieterdenEngelse/RARAG/blob/main/docs/bin3";
    let readme_url = "https://github.com/PieterdenEngelse/RARAG/blob/main/README.md";

    let open_url = |url: &'static str| {
        move |_| {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
    };

    rsx! {
        div { class: "modal-overlay",
            onclick: move |_| open.set(false),
            // Keyboard escape: any key handler at the overlay level catches
            // Escape and closes, parity with click-to-close.
            onkeydown: move |evt| {
                if evt.key() == dioxus::prelude::Key::Escape {
                    open.set(false);
                }
            },
            div { class: "modal-content about-modal",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "about-modal-title",
                tabindex: "-1",
                onclick: move |evt| evt.stop_propagation(),
                h2 { id: "about-modal-title", class: "modal-title", "About RERAG installer" }
                p { class: "about-summary",
                    "Installs "
                    strong { "RERAG" }
                    " — Rust Educational RAG — under XDG paths in your home "
                    "directory. No root, no system files modified, no remote "
                    "credentials required (LLM API keys are entered in the "
                    "First-Run step and stay local)."
                }
                table { class: "about-table",
                    tbody {
                        tr {
                            td { class: "about-key", "Version" }
                            td { class: "about-value", "{VERSION}" }
                        }
                        tr {
                            td { class: "about-key", "Git SHA" }
                            td { class: "about-value", "{GIT_SHA}" }
                        }
                        tr {
                            td { class: "about-key", "Built" }
                            td { class: "about-value", "{BUILT_AT} ({RUNNER})" }
                        }
                    }
                }
                div { class: "about-links",
                    button {
                        class: "btn btn-link",
                        onclick: open_url(readme_url),
                        "Open README →"
                    }
                    button {
                        class: "btn btn-link",
                        onclick: open_url(design_url),
                        "Open design doc (docs/bin3) →"
                    }
                    button {
                        class: "btn btn-link",
                        onclick: open_url(releases_url),
                        "All releases →"
                    }
                }
                div { class: "modal-actions",
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| open.set(false),
                        "Close"
                    }
                }
            }
        }
    }
}
