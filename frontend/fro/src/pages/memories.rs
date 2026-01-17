use crate::api;
use crate::api::{ManualObservationSummary, RagMemoryItem, StoreRagRequest};
use crate::app::Route;
use crate::components::config_nav::{ConfigNav, ConfigTab};
use crate::components::monitor::*;
use dioxus::prelude::*;

// Memory type descriptions
const MEMORY_TYPE_INFO: &[(&str, &str, &[&str])] = &[
    // Core types
    (
        "fact",
        "Fact Memory",
        &[
            "Facts store what is true about the project, codebase, or user.",
            "Examples:",
            "• 'The project uses PostgreSQL 15 with TimescaleDB extension'",
            "• 'The main API is hosted on AWS ECS in us-east-1'",
            "• 'User's name is John and they work on the backend team'",
            "Use facts for objective, verifiable information that doesn't change frequently.",
        ],
    ),
    (
        "preference",
        "Preference Memory",
        &[
            "Preferences capture what the user likes or dislikes.",
            "Examples:",
            "• 'User prefers TypeScript over JavaScript'",
            "• 'User likes detailed explanations with code examples'",
            "• 'User dislikes verbose logging in production code'",
            "Preferences help personalize responses to match user expectations.",
        ],
    ),
    (
        "instruction",
        "Instruction Memory",
        &[
            "Instructions are standing rules or guidelines to follow.",
            "Examples:",
            "• 'Always use async/await, never callbacks'",
            "• 'Follow the team's naming convention: camelCase for variables'",
            "• 'Never commit directly to main branch'",
            "Instructions act as persistent directives that shape behavior across sessions.",
        ],
    ),
    (
        "context",
        "Context Memory",
        &[
            "Context provides background information about the domain or project.",
            "Examples:",
            "• 'This is a fintech app handling payment processing'",
            "• 'The codebase follows hexagonal architecture'",
            "• 'We're in the middle of migrating from monolith to microservices'",
            "Context helps understand the bigger picture when answering questions.",
        ],
    ),
    (
        "summary",
        "Summary Memory",
        &[
            "Summaries condense past interactions or discussions.",
            "Examples:",
            "• 'Summary of auth implementation: chose JWT with refresh tokens'",
            "• 'Last session: debugged memory leak in worker service'",
            "• 'Project kickoff notes: MVP due in 6 weeks, focus on core features'",
            "Summaries help maintain continuity across sessions without storing full conversations.",
        ],
    ),
    (
        "task",
        "Task Memory",
        &[
            "Tasks capture current work context and objectives.",
            "Examples:",
            "• 'Currently working on user registration flow'",
            "• 'Next up: implement password reset functionality'",
            "• 'Blocked on: waiting for API keys from third-party provider'",
            "Tasks help track what's being worked on and what's coming next.",
        ],
    ),
    // Extended types
    (
        "conversation",
        "Conversation Memory",
        &[
            "Conversations store past dialogue snippets worth remembering.",
            "Examples:",
            "• 'User asked about authentication, I explained JWT tokens'",
            "• 'Discussed trade-offs between REST and GraphQL'",
            "• 'User mentioned they're new to Rust'",
            "Use for important exchanges that provide context for future interactions.",
        ],
    ),
    (
        "decision",
        "Decision Memory",
        &[
            "Decisions record choices made and their rationale.",
            "Examples:",
            "• 'Chose Redis for caching due to speed requirements'",
            "• 'Decided to use SQLite for simplicity over PostgreSQL'",
            "• 'Selected React over Vue for better TypeScript support'",
            "Decisions help explain why things are the way they are.",
        ],
    ),
    (
        "correction",
        "Correction Memory",
        &[
            "Corrections fix mistakes from previous responses.",
            "Examples:",
            "• 'Actually the API uses v2, not v1 as I said earlier'",
            "• 'The config file is in YAML, not JSON'",
            "• 'The function returns Option<T>, not Result<T>'",
            "Corrections prevent repeating the same mistakes.",
        ],
    ),
    (
        "feedback",
        "Feedback Memory",
        &[
            "Feedback captures user reactions to responses.",
            "Examples:",
            "• 'User said the explanation was too technical'",
            "• 'User appreciated the step-by-step breakdown'",
            "• 'User found the code example helpful'",
            "Feedback helps improve future responses.",
        ],
    ),
    (
        "persona",
        "Persona Memory",
        &[
            "Persona defines communication style preferences.",
            "Examples:",
            "• 'User prefers concise, technical responses'",
            "• 'User likes humor and casual tone'",
            "• 'User wants detailed explanations with analogies'",
            "Persona shapes how responses are delivered.",
        ],
    ),
    (
        "note",
        "Note Memory",
        &[
            "Notes are general-purpose user-added reminders.",
            "Examples:",
            "• 'Remember to check the deployment logs tomorrow'",
            "• 'TODO: refactor the auth module'",
            "• 'Meeting with team at 3pm about API design'",
            "Notes are flexible catch-all memories for anything else.",
        ],
    ),
];

const ALL_TYPES: &[&str] = &[
    "context",
    "conversation",
    "correction",
    "decision",
    "fact",
    "feedback",
    "instruction",
    "note",
    "persona",
    "preference",
    "summary",
    "task",
];

fn get_type_info(memory_type: &str) -> Option<(&'static str, &'static [&'static str])> {
    // Special case for add memory help
    if memory_type == "_add_memory_help" {
        return Some((
            "How to Add RAG Memories",
            &[
                "RAG memories provide context to the LLM during queries. They are embedded and stored for semantic search.",
                "Steps to add a memory:",
                "1. Select a memory type (Core or Extended)",
                "2. Enter the content you want to remember",
                "3. Click 'Add Memory' to save",
                "Core Types (most common):",
                "• fact - Project/user facts",
                "• preference - User likes/dislikes",
                "• instruction - Rules to follow",
                "• context - Background info",
                "• summary - Condensed interactions",
                "• task - Current work context",
                "Extended Types:",
                "• conversation, decision, correction, feedback, persona, note",
                "Click the ⓘ button next to each type for detailed examples.",
            ],
        ));
    }
    MEMORY_TYPE_INFO
        .iter()
        .find(|(t, _, _)| *t == memory_type)
        .map(|(_, title, paragraphs)| (*title, *paragraphs))
}

fn is_core_type(t: &str) -> bool {
    matches!(
        t,
        "fact" | "preference" | "instruction" | "context" | "summary" | "task"
    )
}

#[derive(Clone, Default)]
struct MemoriesState {
    loading: bool,
    error: Option<String>,
    rag_memories: Vec<RagMemoryItem>,
    observations: Vec<ManualObservationSummary>,
}

#[derive(Clone, Default)]
struct FormState {
    memory_type: String,
    content: String,
    submitting: bool,
    message: Option<String>,
    is_error: bool,
}

/// Info icon component
#[component]
fn InfoIcon() -> Element {
    rsx! {
        svg {
            class: "w-5 h-5 text-white",
            view_box: "0 0 20 20",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            circle { cx: "10", cy: "10", r: "9" }
            line { x1: "10", y1: "9", x2: "10", y2: "14" }
            circle { cx: "10", cy: "6", r: "1.2", fill: "currentColor", stroke: "none" }
        }
    }
}

/// Info modal component
#[component]
fn InfoModal(title: String, paragraphs: Vec<String>, on_close: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| on_close.call(()),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-lg max-h-[80vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100", "{title}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-2",
                    for paragraph in paragraphs.iter() {
                        p { "{paragraph}" }
                    }
                }
            }
        }
    }
}

/// Memory type button with info icon
#[component]
fn MemoryTypeButton(
    memory_type: &'static str,
    is_selected: bool,
    is_core: bool,
    on_select: EventHandler<()>,
    on_info: EventHandler<()>,
) -> Element {
    let base_class = if is_selected {
        "px-3 py-1.5 rounded-l text-sm bg-blue-600 text-white"
    } else {
        "px-3 py-1.5 rounded-l text-sm bg-gray-700 text-gray-300 hover:bg-gray-600"
    };

    let info_class = "px-2 py-1.5 rounded-r text-sm hover:brightness-110";
    let info_style = "background-color: #1D6B9A;";

    rsx! {
        div { class: "inline-flex",
            button {
                class: "{base_class}",
                onclick: move |_| on_select.call(()),
                "{memory_type}"
            }
            button {
                class: "{info_class}",
                style: "{info_style}",
                onclick: move |evt| {
                    evt.stop_propagation();
                    on_info.call(());
                },
                InfoIcon {}
            }
        }
    }
}

/// Modal to show memories of a specific type
#[component]
fn MemoriesListModal(
    memory_type: String,
    memories: Vec<RagMemoryItem>,
    on_close: EventHandler<()>,
) -> Element {
    let is_all = memory_type == "all";
    let filtered: Vec<&RagMemoryItem> = if is_all {
        memories.iter().collect()
    } else {
        memories
            .iter()
            .filter(|m| m.memory_type == memory_type)
            .collect()
    };

    let display_title = if is_all {
        "All Memories".to_string()
    } else {
        memory_type.clone()
    };
    let header_color = "text-white";

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| on_close.call(()),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-3xl max-h-[80vh] overflow-hidden shadow-xl flex flex-col",
                onclick: move |evt| evt.stop_propagation(),
                // Header
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold {header_color}",
                        "{display_title}"
                        span { class: "text-gray-400 ml-2", "({filtered.len()} memories)" }
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                // Content
                if filtered.is_empty() {
                    div { class: "text-gray-500 text-sm py-8 text-center",
                        "No memories of type '"
                        span { class: "font-semibold", "{memory_type}" }
                        "' stored yet."
                    }
                } else {
                    div { class: "overflow-y-auto flex-1",
                        div { class: "space-y-3",
                            for (i, mem) in filtered.iter().enumerate() {
                                div { class: "bg-gray-900 rounded p-3 border border-gray-700",
                                    div { class: "flex justify-between items-start mb-2",
                                        span { class: "text-xs text-gray-500", "#{i + 1}" }
                                        span { class: "text-xs text-gray-500", "{mem.timestamp}" }
                                    }
                                    p { class: "text-white text-sm whitespace-pre-wrap", "{mem.content}" }
                                    div { class: "text-xs text-gray-500 mt-2", "Agent: {mem.agent_id}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Filter tab component for memory types
#[component]
fn FilterTab(
    label: String,
    count: usize,
    is_core: Option<bool>,
    on_click: EventHandler<()>,
) -> Element {
    let base_class = "px-3 py-1.5 rounded text-sm bg-gray-700 text-gray-300 hover:bg-gray-600 border border-gray-600";

    rsx! {
        button {
            class: "{base_class}",
            onclick: move |_| on_click.call(()),
            "{label}"
            span { class: "ml-1 text-xs opacity-75", "({count})" }
        }
    }
}

#[component]
pub fn ConfigMemories() -> Element {
    let state = use_signal(MemoriesState::default);
    let mut form = use_signal(FormState::default);
    let mut show_info_modal = use_signal(|| Option::<String>::None);
    let mut show_list_modal = use_signal(|| Option::<String>::None);
    let mut refresh_counter = use_signal(|| 0u32);

    // Load data - runs on mount and when refresh_counter changes
    let counter_val = refresh_counter();
    use_future(move || {
        let mut state = state.clone();
        let _counter = counter_val; // Capture to create dependency
        async move {
            state.set(MemoriesState {
                loading: true,
                error: None,
                rag_memories: vec![],
                observations: vec![],
            });

            let rag_result = api::fetch_rag_memories(100).await;
            let obs_result = api::fetch_recent_observations(50).await;

            match (rag_result, obs_result) {
                (Ok(r), Ok(o)) => {
                    state.set(MemoriesState {
                        loading: false,
                        error: None,
                        rag_memories: r.memories,
                        observations: o.observations,
                    });
                }
                (Err(e), _) | (_, Err(e)) => {
                    state.set(MemoriesState {
                        loading: false,
                        error: Some(e),
                        rag_memories: vec![],
                        observations: vec![],
                    });
                }
            }
        }
    });

    let snapshot = state.read().clone();
    let form_snapshot = form.read().clone();
    let modal_type = show_info_modal.read().clone();
    let list_modal_type = show_list_modal.read().clone();

    // Count memories by type
    let type_counts: std::collections::HashMap<String, usize> =
        snapshot
            .rag_memories
            .iter()
            .fold(std::collections::HashMap::new(), |mut acc, mem| {
                *acc.entry(mem.memory_type.clone()).or_insert(0) += 1;
                acc
            });

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("Memories", None::<Route>),
                ],
            }

            ConfigNav { active: ConfigTab::Memories }

            // Info Modal (type description)
            if let Some(mem_type) = modal_type {
                if let Some((title, paragraphs)) = get_type_info(&mem_type) {
                    InfoModal {
                        title: title.to_string(),
                        paragraphs: paragraphs.iter().map(|s| s.to_string()).collect(),
                        on_close: move |_| show_info_modal.set(None),
                    }
                }
            }

            // List Modal (memories of a type)
            if let Some(mem_type) = list_modal_type {
                MemoriesListModal {
                    memory_type: mem_type.clone(),
                    memories: snapshot.rag_memories.clone(),
                    on_close: move |_| show_list_modal.set(None),
                }
            }

            // Add Memory Form
            Panel { title: None::<String>, refresh: None::<String>,
                // Custom title with info button
                div { class: "flex items-center gap-2 mb-3",
                    h3 { class: "text-sm font-semibold text-gray-200", "Add RAG Memory" }
                    button {
                        class: "px-2 py-1 rounded text-sm hover:brightness-110",
                        style: "background-color: #1D6B9A;",
                        onclick: move |_| show_info_modal.set(Some("_add_memory_help".to_string())),
                        InfoIcon {}
                    }
                }
                div { class: "space-y-4",
                    // Memory Type Selection
                    div {
                        label { class: "block text-xs text-gray-400 mb-2", "There is no code-level check (possible) to verify that the content matches the type. Selecting a type category is the subjective choice of the user and only meant to 'classify' the insight on existing memories." }
                        div { class: "flex flex-wrap gap-2",
                            for t in ALL_TYPES.iter() {
                                MemoryTypeButton {
                                    memory_type: t,
                                    is_selected: form_snapshot.memory_type == *t,
                                    is_core: is_core_type(t),
                                    on_select: move |_| {
                                        form.write().memory_type = t.to_string();
                                    },
                                    on_info: move |_| {
                                        show_info_modal.set(Some(t.to_string()));
                                    },
                                }
                            }
                        }
                    }

                    // Content Input
                    div {
                        label { class: "block text-sm text-gray-400 mb-2", "Content" }
                        textarea {
                            class: "w-full bg-gray-800 border border-gray-700 rounded p-3 text-white text-sm focus:border-blue-500 focus:outline-none",
                            rows: "4",
                            placeholder: "Enter the memory content...",
                            value: "{form_snapshot.content}",
                            oninput: move |e| {
                                form.write().content = e.value();
                            }
                        }
                    }

                    // Submit Button and Status
                    div { class: "flex items-center gap-4",
                        button {
                            class: "px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed",
                            disabled: form_snapshot.submitting,
                            onclick: move |_| {
                                let memory_type = form.read().memory_type.clone();
                                let content = form.read().content.clone();

                                // Validate and show helpful error messages
                                if memory_type.is_empty() && content.trim().is_empty() {
                                    form.write().message = Some("Please select a memory type and enter content".to_string());
                                    form.write().is_error = true;
                                    return;
                                }
                                if memory_type.is_empty() {
                                    form.write().message = Some("Please select a memory type (click one of the type buttons above)".to_string());
                                    form.write().is_error = true;
                                    return;
                                }
                                if content.trim().is_empty() {
                                    form.write().message = Some("Please enter some content for the memory".to_string());
                                    form.write().is_error = true;
                                    return;
                                }

                                form.write().submitting = true;
                                form.write().message = None;

                                spawn(async move {
                                    let req = StoreRagRequest {
                                        agent_id: "default".to_string(),
                                        memory_type: memory_type.clone(),
                                        content: content.clone(),
                                    };

                                    match api::store_rag_memory(&req).await {
                                        Ok(_) => {
                                            form.write().submitting = false;
                                            form.write().message = Some(format!("Memory added: {} - {}", memory_type, &content[..content.len().min(50)]));
                                            form.write().is_error = false;
                                            form.write().content = String::new();
                                            // Refresh the list by incrementing counter
                                            refresh_counter.set(refresh_counter() + 1);
                                        }
                                        Err(e) => {
                                            form.write().submitting = false;
                                            form.write().message = Some(e);
                                            form.write().is_error = true;
                                        }
                                    }
                                });
                            },
                            if form_snapshot.submitting {
                                "Saving..."
                            } else {
                                "Add Memory"
                            }
                        }

                        if let Some(msg) = form_snapshot.message.clone() {
                            div {
                                class: if form_snapshot.is_error { "text-sm text-red-400" } else { "text-sm text-green-400" },
                                "{msg}"
                            }
                        }
                    }
                }
            }

            if snapshot.loading {
                div { class: "text-gray-400 text-sm", "Loading memories…" }
            } else if let Some(err) = snapshot.error.clone() {
                div { class: "text-red-400 text-sm", "Failed to load: {err}" }
            } else {
                // List RAG Memories - click a type to view its memories
                Panel { title: Some("List RAG Memories".into()), refresh: None::<String>,
                    div { class: "text-xs text-gray-400 mb-3", "Click a type to view its memories" }
                    div { class: "flex flex-wrap gap-2",
                        // All tab
                        FilterTab {
                            label: "All".to_string(),
                            count: snapshot.rag_memories.len(),
                            is_core: None,
                            on_click: move |_| show_list_modal.set(Some("all".to_string())),
                        }
                        // Type tabs
                        for t in ALL_TYPES.iter() {
                            {
                                let count = type_counts.get(*t).copied().unwrap_or(0);
                                let t_string = t.to_string();
                                let is_core = is_core_type(t);
                                rsx! {
                                    FilterTab {
                                        label: t_string.clone(),
                                        count: count,
                                        is_core: Some(is_core),
                                        on_click: move |_| show_list_modal.set(Some(t.to_string())),
                                    }
                                }
                            }
                        }
                    }
                }

                // Summary Stats
                Panel { title: Some("Memory Summary".into()), refresh: None::<String>,
                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                        div { class: "bg-blue-900/30 rounded p-4 border border-blue-800",
                            div { class: "text-3xl font-bold text-blue-300", "{snapshot.rag_memories.len()}" }
                            div { class: "text-sm text-gray-400", "RAG Memories (LLM Context)" }
                        }
                        div { class: "bg-purple-900/30 rounded p-4 border border-purple-800",
                            div { class: "text-3xl font-bold text-purple-300", "{snapshot.observations.len()}" }
                            div { class: "text-sm text-gray-400", "Manual Observations (Work History)" }
                        }
                    }
                }

                // Manual Observations Section
                Panel { title: Some("Manual Observations".into()), refresh: None::<String>,
                    div { class: "text-sm text-gray-400 mb-3",
                        "Structured work history. Use "
                        span { class: "font-mono bg-gray-800 px-1 rounded", "POST /memory/observations" }
                        " to add observations."
                    }
                    if snapshot.observations.is_empty() {
                        div { class: "text-gray-500 text-sm py-4", "No observations stored." }
                    } else {
                        div { class: "max-h-64 overflow-y-auto",
                            table { class: "w-full text-sm text-left",
                                thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800 sticky top-0 bg-gray-900",
                                    tr {
                                        th { class: "py-2", "Type" }
                                        th { class: "py-2", "Title" }
                                        th { class: "py-2", "Created" }
                                    }
                                }
                                tbody {
                                    for obs in snapshot.observations.iter() {
                                        tr { class: "border-b border-gray-800 last:border-0 hover:bg-gray-800/50",
                                            td { class: "py-2",
                                                span { class: "px-2 py-0.5 rounded text-xs bg-purple-900 text-purple-200", "{obs.entry_type}" }
                                            }
                                            td { class: "py-2 text-white", "{obs.title}" }
                                            td { class: "py-2 text-gray-400 text-xs whitespace-nowrap", "{obs.created_at}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
