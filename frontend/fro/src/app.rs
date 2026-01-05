use crate::components::header::Header;
use crate::components::ActiveDropdown;
use crate::pages::{
    About, Config, ConfigHardware, ConfigOther, ConfigPrompt, ConfigSampling, Home, MonitorAgentic,
    MonitorCache, MonitorIndex, MonitorLogs, MonitorOverview, MonitorRateLimits, MonitorRequests,
    PageNotFound, Parameters,
};
use dioxus::prelude::*;

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Layout)]
        #[route("/")]
        Home {},
        #[route("/about")]
        About {},
        #[route("/monitor")]
        MonitorOverview {},
        #[route("/monitor/agentic")]
        MonitorAgentic {},
        #[route("/config")]
        Config {},
        #[route("/config/parameters")]
        Parameters {},
        #[route("/config/sampling")]
        ConfigSampling {},
        #[route("/config/prompt")]
        ConfigPrompt {},
        #[route("/config/hardware")]
        ConfigHardware {},
        #[route("/config/other")]
        ConfigOther {},
        #[route("/monitor/requests")]
        MonitorRequests {},
        #[route("/monitor/cache")]
        MonitorCache {},
        #[route("/monitor/index")]
        MonitorIndex {},
        #[route("/monitor/rate-limits")]
        MonitorRateLimits {},
        #[route("/monitor/logs")]
        MonitorLogs {},
    #[end_layout]
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },
}

/// Signal for toggling the global help overlay
#[derive(Clone, Copy, Default)]
pub struct ShowHelpCommands(pub bool);

/// Signal for toggling the RAG info overlay
#[derive(Clone, Copy, Default)]
pub struct ShowRagInfo(pub bool);

struct HelpCommandInfo {
    name: &'static str,
    description: &'static str,
    example: &'static str,
}

#[derive(Clone, PartialEq)]
struct HelpCommandDetail {
    name: String,
    description: String,
    example: String,
}

const HELP_COMMANDS: [HelpCommandInfo; 32] = [
    HelpCommandInfo { name: "/abandon", description: "Stop tracking the current high-level goal and mark it as abandoned.", example: "/abandon" },
    HelpCommandInfo { name: "/brief", description: "Switch responses to a concise mode for faster scanning.", example: "/brief" },
    HelpCommandInfo { name: "/clear", description: "Clear the visible chat transcript without touching goals or memory.", example: "/clear" },
    HelpCommandInfo { name: "/debug", description: "Print internal diagnostics about the last agent run (tools, errors, timings).", example: "/debug" },
    HelpCommandInfo { name: "/dry-run <query>", description: "Plan what the agent would do for a query without executing tools.", example: "/dry-run audit prod deployment" },
    HelpCommandInfo { name: "/export", description: "Download your stored memories/goals as JSON for backup or sharing.", example: "/export" },
    HelpCommandInfo { name: "/focus <topic>", description: "Pin a temporary context topic so future answers stay on track.", example: "/focus tracing metrics" },
    HelpCommandInfo { name: "/forget <topic>", description: "Remove stored memories that mention a keyword or phrase.", example: "/forget alpha release" },
    HelpCommandInfo { name: "/goal <text>", description: "Create a new primary objective the agent should keep working toward.", example: "/goal Document the observability stack" },
    HelpCommandInfo { name: "/goals", description: "List all currently active goals with their status.", example: "/goals" },
    HelpCommandInfo { name: "/help", description: "Show this help window with the available slash commands.", example: "/help" },
    HelpCommandInfo { name: "/history", description: "Display the latest recorded conversations/episodes with timestamps.", example: "/history" },
    HelpCommandInfo { name: "/import <json>", description: "Upload a JSON export to restore memories or notes into the agent.", example: "/import {\"notes\": [...]}" },
    HelpCommandInfo { name: "/learn <url>", description: "Fetch a URL, chunk its contents, and add it to the knowledge base.", example: "/learn https://docs.rs/tracing/latest/tracing/" },
    HelpCommandInfo { name: "/model <name>", description: "Temporarily switch to another configured backend model.", example: "/model mistral:instruct" },
    HelpCommandInfo { name: "/models", description: "List the models that are currently registered and usable.", example: "/models" },
    HelpCommandInfo { name: "/note <text>", description: "Store a quick personal note that you can recall later.", example: "/note Check GPU memory at 5pm" },
    HelpCommandInfo { name: "/pause", description: "Pause progress on the current goal without abandoning it.", example: "/pause" },
    HelpCommandInfo { name: "/persona <name>", description: "Adopt a saved persona profile (tone, domain vocabulary, etc.).", example: "/persona architect" },
    HelpCommandInfo { name: "/reflect", description: "Generate a short self-reflection summary from the last 24h of work.", example: "/reflect" },
    HelpCommandInfo { name: "/resume", description: "Resume a goal that was previously paused.", example: "/resume" },
    HelpCommandInfo { name: "/retry", description: "Re-run the previous user query using the same parameters.", example: "/retry" },
    HelpCommandInfo { name: "/run <tool>", description: "Invoke a specific tool (calculator, web-search, fetch) with arguments.", example: "/run calculator 5 * (12 + 3)" },
    HelpCommandInfo { name: "/sources", description: "Show the document snippets that backed the last answer.", example: "/sources" },
    HelpCommandInfo { name: "/status", description: "Report on backend health, retriever readiness, and timestamp.", example: "/status" },
    HelpCommandInfo { name: "/subgoal <text>", description: "Attach a smaller task beneath the most recent goal.", example: "/subgoal Validate Prometheus scrape config" },
    HelpCommandInfo { name: "/temperature <n>", description: "Adjust creativity (sampling temperature) for future generations.", example: "/temperature 0.4" },
    HelpCommandInfo { name: "/tokens", description: "Estimate how many tokens the last exchange consumed.", example: "/tokens" },
    HelpCommandInfo { name: "/undo", description: "Undo the most recent change to focus/persona/model/notes.", example: "/undo" },
    HelpCommandInfo { name: "/unfocus", description: "Clear the current focus topic so the agent can consider new ones.", example: "/unfocus" },
    HelpCommandInfo { name: "/verbose", description: "Switch responses to a detailed reasoning/explanation mode.", example: "/verbose" },
    HelpCommandInfo { name: "/why", description: "Explain the reasoning steps taken during the last response.", example: "/why" },
];

#[component]
fn HelpCommandRow(
    detail: HelpCommandDetail,
    selected_command: Signal<Option<HelpCommandDetail>>,
) -> Element {
    let name = detail.name.clone();
    let description = detail.description.clone();
    let example = detail.example.clone();
    rsx! {
        p {
            button {
                class: "text-primary hover:underline",
                onclick: move |_| {
                    selected_command.set(Some(HelpCommandDetail {
                        name: name.clone(),
                        description: description.clone(),
                        example: example.clone(),
                    }));
                },
                "{detail.name}"
            }
            " - {detail.description}"
        }
    }
}

#[component]
pub fn App() -> Element {
    use_context_provider(|| Signal::new(true));  // Dark mode ON by default
    use_context_provider(|| Signal::new(ShowHelpCommands(false)));  // Help commands panel
    use_context_provider(|| Signal::new(ShowRagInfo(false)));  // RAG info panel
    use_context_provider(|| Signal::new(ActiveDropdown(None)));  // Active dropdown tracker

    rsx! {
        document::Link { rel: "icon", href: asset!("/assets/favicon.ico") }
        document::Link { rel: "stylesheet", href: asset!("/assets/styling/output.css") }

        Router::<Route> {}
    }
}

#[component]
fn Layout() -> Element {
    let is_dark = use_context::<Signal<bool>>();
    let mut show_help = use_context::<Signal<ShowHelpCommands>>();
    let mut selected_command = use_signal(|| Option::<HelpCommandDetail>::None);
    let help_entries: Vec<HelpCommandDetail> = HELP_COMMANDS
        .iter()
        .map(|info| HelpCommandDetail {
            name: info.name.to_string(),
            description: info.description.to_string(),
            example: info.example.to_string(),
        })
        .collect();

    // Apply dark class on mount and when toggled
    use_effect(use_reactive!(|is_dark| {
        let dark_mode = is_dark();
        web_sys::console::log_1(&format!("Dark mode effect running: {}", dark_mode).into());

        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(html) = document.document_element() {
                    let class_list = html.class_list();
                    if dark_mode {
                        web_sys::console::log_1(&"Adding dark class".into());
                        let _ = class_list.add_1("dark");
                    } else {
                        web_sys::console::log_1(&"Removing dark class".into());
                        let _ = class_list.remove_1("dark");
                    }
                    web_sys::console::log_1(&format!("HTML classes: {}", html.class_name()).into());
                }
            }
        }
    }));

    rsx! {
        div {
            class: "min-h-screen transition-colors bg-white dark:bg-gray-900 text-gray-900 dark:text-white",

            Header {},

            main {
                if show_help().0 {
                    div {
                        class: "fixed inset-0 z-40 bg-black/60 backdrop-blur-sm",
                        onclick: move |_| show_help.set(ShowHelpCommands(false)),
                    }
                    div {
                        class: "fixed inset-0 z-50 flex justify-center items-start px-4 pt-4",
                        style: "top: 3rem;",
                        "data-theme": "dark",
                        onclick: move |_| show_help.set(ShowHelpCommands(false)),

                        div {
                            class: "w-full max-w-4xl rounded-lg bg-gray-800 shadow-2xl p-4",
                            onclick: move |evt| evt.stop_propagation(),

                            div {
                                class: "flex items-center justify-between mb-3",
                                h3 { class: "text-lg font-semibold", "Available Commands" }
                                button {
                                    class: "btn btn-ghost btn-sm text-lg",
                                    onclick: move |_| show_help.set(ShowHelpCommands(false)),
                                    "✕"
                                }
                            }

                            div {
                                class: "grid grid-cols-2 gap-x-6 gap-y-2 font-mono text-sm pb-3",
                                for entry in help_entries.iter().cloned() {
                                    HelpCommandRow {
                                        detail: entry,
                                        selected_command: selected_command.clone(),
                                    }
                                }
                            }

                            div {
                                class: "mt-4 pt-3 border-t border-gray-600",
                                button {
                                    class: "btn btn-primary w-full",
                                    onclick: move |_| show_help.set(ShowHelpCommands(false)),
                                    "Close"
                                }
                            }
                        }
                    }
                }

                if let Some(detail) = selected_command() {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| selected_command.set(None),

                        div {
                            class: "bg-base-100 rounded-lg max-w-md w-[90vw] p-5 shadow-2xl border border-base-300",
                            onclick: move |evt| evt.stop_propagation(),

                            h3 { class: "text-lg font-bold mb-3", "Command info" }
                            p {
                                class: "font-mono text-sm text-primary mb-2",
                                "{detail.name}"
                            }
                            p {
                                class: "text-sm mb-4",
                                "{detail.description}"
                            }
                            p {
                                class: "text-xs text-base-content/60 font-mono",
                                strong { "Example:" }
                                " {detail.example}"
                            }

                            div {
                                class: "mt-4 flex justify-end",
                                button {
                                    class: "btn btn-primary btn-sm",
                                    onclick: move |_| selected_command.set(None),
                                    "Close"
                                }
                            }
                        }
                    }
                }

                Outlet::<Route> {}
            }
        }
    }
}
