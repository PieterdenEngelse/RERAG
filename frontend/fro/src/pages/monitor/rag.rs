use crate::api;
use crate::api::{ChunkingStatsSnapshot, RagMemoryItem};
use crate::app::Route;
use crate::components::monitor::*;
use dioxus::prelude::*;

#[derive(Clone, Default)]
struct RagState {
    loading: bool,
    error: Option<String>,
    rag_memories: Vec<RagMemoryItem>,
    chunking_history: Vec<ChunkingStatsSnapshot>,
}

/// Check if a chunking entry has issues
fn has_issues(entry: &ChunkingStatsSnapshot) -> bool {
    // Issue: zero chunks produced
    if entry.chunks == 0 {
        return true;
    }
    
    if let Some(ref detection) = entry.detection {
        // Issue: fallback detection method
        if detection.detection_method == "fallback" {
            return true;
        }
        // Issue: extension-only detection (less reliable)
        if detection.detection_method == "extension" {
            return true;
        }
        // Issue: unknown format
        if detection.detected_format.to_lowercase() == "unknown" {
            return true;
        }
    } else {
        // Issue: no detection info at all
        return true;
    }
    
    false
}

#[component]
pub fn MonitorRag() -> Element {
    let state = use_signal(RagState::default);
    let mut show_issues_only = use_signal(|| true); // Default ON
    let mut show_issues_info = use_signal(|| false); // Issues info modal
    let mut show_strategies_info = use_signal(|| false); // Strategies info modal

    {
        let mut state = state.clone();
        use_future(move || async move {
            state.set(RagState {
                loading: true,
                error: None,
                rag_memories: vec![],
                chunking_history: vec![],
            });

            let rag_result = api::fetch_rag_memories(50).await;
            let chunking_result = api::fetch_chunking_stats(20).await;

            match (rag_result, chunking_result) {
                (Ok(r), Ok(c)) => {
                    state.set(RagState {
                        loading: false,
                        error: None,
                        rag_memories: r.memories,
                        chunking_history: c.snapshots,
                    });
                }
                (Ok(r), Err(_)) => {
                    // Chunking stats might not be available yet
                    state.set(RagState {
                        loading: false,
                        error: None,
                        rag_memories: r.memories,
                        chunking_history: vec![],
                    });
                }
                (Err(e), _) => {
                    state.set(RagState {
                        loading: false,
                        error: Some(e),
                        rag_memories: vec![],
                        chunking_history: vec![],
                    });
                }
            }
        });
    }

    let snapshot = state.read().clone();

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("RAG", None::<Route>),
                ],
            }

            NavTabs { active: Route::MonitorRag {} }

            // Page header
            Panel { title: Some("RAG Context Store".into()), refresh: None::<String>,
                div { class: "text-sm text-gray-300 space-y-2",
                    p { "Retrieval-Augmented Generation (RAG) is a passive retrieval system. Data is indexed and retrieved on-demand to provide context for LLM inference." }
                    div { class: "flex flex-wrap gap-4 mt-3 text-xs",
                        div { class: "flex items-center gap-2",
                            span { class: "w-2 h-2 rounded-full bg-blue-500" }
                            span { class: "text-gray-400", "No autonomy — responds to queries" }
                        }
                        div { class: "flex items-center gap-2",
                            span { class: "w-2 h-2 rounded-full bg-blue-500" }
                            span { class: "text-gray-400", "Stateless retrieval — same query = same results" }
                        }
                        div { class: "flex items-center gap-2",
                            span { class: "w-2 h-2 rounded-full bg-blue-500" }
                            span { class: "text-gray-400", "Document-centric — chunks, embeddings, vectors" }
                        }
                    }
                }
            }

            if snapshot.loading {
                div { class: "text-gray-400 text-sm", "Loading…" }
            } else if let Some(err) = snapshot.error.clone() {
                div { class: "text-red-400 text-sm", "Failed to load: {err}" }
            } else {
                // Stats row
                div { class: "grid grid-cols-1 md:grid-cols-4 gap-4",
                    StatCard {
                        title: "Total Memories".into(),
                        value: snapshot.rag_memories.len().to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Conversations".into(),
                        value: snapshot.rag_memories.iter().filter(|m| m.memory_type == "conversation").count().to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Facts".into(),
                        value: snapshot.rag_memories.iter().filter(|m| m.memory_type == "fact").count().to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Notes".into(),
                        value: snapshot.rag_memories.iter().filter(|m| m.memory_type == "note").count().to_string().into(),
                        unit: None,
                    }
                }

                // ═══════════════════════════════════════════════════════════
                // DETECTION OBSERVABILITY SECTION
                // ═══════════════════════════════════════════════════════════
                RowHeader {
                    title: "Detection Observability".into(),
                    description: Some("Raw inputs (MIME type, extension) vs derived conclusions (format, strategy). The gap between observation and conclusion is where detection failures hide.".into()),
                }

                Panel { title: Some("Chunking Detection Log".into()), refresh: None::<String>,
                    // Toggle for issues-only filter
                    div { class: "flex items-center justify-between mb-3 pb-3 border-b border-gray-700",
                        div { class: "flex items-center gap-3",
                            label { class: "flex items-center gap-2 cursor-pointer",
                                input {
                                    r#type: "checkbox",
                                    class: "toggle toggle-sm !border !border-white",
                                    style: format!(
                                        "border: 1px solid white; background-color: {};",
                                        if show_issues_only() { "" } else { "#d1d5db" }
                                    ),
                                    checked: show_issues_only(),
                                    onchange: move |_| show_issues_only.set(!show_issues_only()),
                                }
                                span { class: "text-sm font-medium text-white", "Show Issues Only" }
                            }
                            // Info button for issue criteria
                            button {
                                class: "w-5 h-5 min-w-5 min-h-5 shrink-0 rounded flex items-center justify-center cursor-pointer hover:opacity-80",
                                style: "background-color: #1D6B9A; border: 1px solid #1D6B9A;",
                                onclick: move |_| show_issues_info.set(true),
                                title: "Issue detection criteria",
                                svg {
                                    class: "w-4 h-4 text-white",
                                    view_box: "0 0 20 20",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1.5",
                                    circle { cx: "10", cy: "10", r: "9" }
                                    line { x1: "10", y1: "8", x2: "10", y2: "14" }
                                    circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                }
                            }
                        }
                        // Right side: strategies info button + issue count badge
                        div { class: "flex items-center gap-3",
                            // Strategies info button
                            button {
                                class: "flex items-center gap-1 px-2 py-1 rounded text-xs bg-blue-900/50 text-blue-300 hover:bg-blue-800/50 cursor-pointer",
                                onclick: move |_| show_strategies_info.set(true),
                                title: "View chunking strategies",
                                svg {
                                    class: "w-3 h-3",
                                    view_box: "0 0 20 20",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1.5",
                                    circle { cx: "10", cy: "10", r: "9" }
                                    line { x1: "10", y1: "8", x2: "10", y2: "14" }
                                    circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                }
                                "Strategies"
                            }
                            // Issue count badge
                            {
                                let issue_count = snapshot.chunking_history.iter().filter(|e| has_issues(e)).count();
                                let total_count = snapshot.chunking_history.len();
                                if issue_count > 0 {
                                    rsx! {
                                        span { class: "px-2 py-1 rounded text-xs bg-red-900/50 text-red-300",
                                            "{issue_count} issue(s) / {total_count} total"
                                        }
                                    }
                                } else {
                                    rsx! {
                                        span { class: "px-2 py-1 rounded text-xs bg-green-900/50 text-green-300",
                                            "✓ No issues ({total_count} entries)"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if snapshot.chunking_history.is_empty() {
                        div { class: "text-gray-400 text-sm py-4",
                            "No chunking operations recorded yet. Upload documents to see detection observability."
                        }
                    } else {
                        // Filter entries based on toggle
                        {
                            let filtered_entries: Vec<_> = if show_issues_only() {
                                snapshot.chunking_history.iter().filter(|e| has_issues(e)).collect()
                            } else {
                                snapshot.chunking_history.iter().collect()
                            };
                            
                            if filtered_entries.is_empty() && show_issues_only() {
                                rsx! {
                                    div { class: "text-green-400 text-sm py-4 text-center",
                                        "✓ No issues detected. All chunking operations completed successfully."
                                    }
                                }
                            } else {
                                rsx! {
                                    div { class: "overflow-x-auto",
                                        table { class: "w-full text-sm text-left",
                                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800 text-xs",
                                                tr {
                                                    th { class: "py-2 px-2", "Date/Time" }
                                                    th { class: "py-2 px-2", "File" }
                                                    th { class: "py-2 px-2", "Raw: MIME" }
                                                    th { class: "py-2 px-2", "Raw: Ext" }
                                                    th { class: "py-2 px-2", "→ Format" }
                                                    th { class: "py-2 px-2", "→ Strategy" }
                                                    th { class: "py-2 px-2", "Method" }
                                                    th { class: "py-2 px-2", "Chunks" }
                                                    th { class: "py-2 px-2", "Tokens" }
                                                    th { class: "py-2 px-2", "Duration" }
                                                }
                                            }
                                            tbody {
                                                for entry in filtered_entries.iter() {
                                        tr { class: "border-b border-gray-800 last:border-0 hover:bg-gray-800/50",
                                            // Date/Time column
                                            td { class: "py-2 px-2 text-gray-400 text-xs whitespace-nowrap",
                                                {
                                                    // Parse and format the timestamp
                                                    let ts = &entry.recorded_at;
                                                    // Try to extract just date and time (format: YYYY-MM-DD HH:MM:SS)
                                                    if ts.len() >= 19 {
                                                        let date_part = &ts[5..10]; // MM-DD
                                                        let time_part = &ts[11..16]; // HH:MM
                                                        format!("{} {}", date_part, time_part)
                                                    } else {
                                                        ts.clone()
                                                    }
                                                }
                                            }
                                            td { class: "py-2 px-2 text-white font-mono text-xs max-w-[150px] truncate", "{entry.file}" }
                                            // Raw inputs
                                            td { class: "py-2 px-2",
                                                if let Some(ref detection) = entry.detection {
                                                    if let Some(ref mime) = detection.mime_type {
                                                        span { class: "px-1.5 py-0.5 rounded text-xs bg-green-900/50 text-green-300 font-mono", "{mime}" }
                                                    } else {
                                                        span { class: "text-gray-600 text-xs", "—" }
                                                    }
                                                } else {
                                                    span { class: "text-gray-600 text-xs", "—" }
                                                }
                                            }
                                            td { class: "py-2 px-2",
                                                if let Some(ref detection) = entry.detection {
                                                    if let Some(ref ext) = detection.extension {
                                                        span { class: "px-1.5 py-0.5 rounded text-xs bg-gray-700 text-gray-300 font-mono", ".{ext}" }
                                                    } else {
                                                        span { class: "text-gray-600 text-xs", "—" }
                                                    }
                                                } else {
                                                    span { class: "text-gray-600 text-xs", "—" }
                                                }
                                            }
                                            // Derived conclusions
                                            td { class: "py-2 px-2",
                                                if let Some(ref detection) = entry.detection {
                                                    span { class: "px-1.5 py-0.5 rounded text-xs bg-blue-900 text-blue-200", "{detection.detected_format}" }
                                                } else {
                                                    span { class: "text-gray-600 text-xs", "—" }
                                                }
                                            }
                                            td { class: "py-2 px-2",
                                                if let Some(ref detection) = entry.detection {
                                                    span { class: "px-1.5 py-0.5 rounded text-xs bg-purple-900 text-purple-200", "{detection.chosen_strategy}" }
                                                } else {
                                                    span { class: "px-1.5 py-0.5 rounded text-xs bg-gray-700 text-gray-300", "{entry.chunker_mode}" }
                                                }
                                            }
                                            td { class: "py-2 px-2",
                                                if let Some(ref detection) = entry.detection {
                                                    span { class: "text-xs",
                                                        class: if detection.detection_method == "magic_bytes" { "text-green-400" } else if detection.detection_method == "extension" { "text-yellow-400" } else { "text-orange-400" },
                                                        "{detection.detection_method}"
                                                    }
                                                } else {
                                                    span { class: "text-gray-600 text-xs", "—" }
                                                }
                                            }
                                            // Outcome metrics
                                            td { class: "py-2 px-2 text-white", "{entry.chunks}" }
                                            td { class: "py-2 px-2 text-gray-400", "{entry.tokens}" }
                                            td { class: "py-2 px-2 text-gray-400 text-xs", "{entry.duration_ms}ms" }
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

                // Detection method legend
                Panel { title: Some("Detection Methods".into()), refresh: None::<String>,
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4 text-sm",
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "w-2 h-2 rounded-full bg-green-500" }
                                span { class: "text-green-300 font-semibold", "magic_bytes" }
                            }
                            div { class: "text-gray-400 text-xs", "Most reliable. Inspects file header bytes to determine actual content type regardless of extension." }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "w-2 h-2 rounded-full bg-yellow-500" }
                                span { class: "text-yellow-300 font-semibold", "extension" }
                            }
                            div { class: "text-gray-400 text-xs", "Fallback when magic bytes don't match. Can be spoofed or incorrect if file was renamed." }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "w-2 h-2 rounded-full bg-orange-500" }
                                span { class: "text-orange-300 font-semibold", "heuristic" }
                            }
                            div { class: "text-gray-400 text-xs", "Last resort. Analyzes content patterns when no metadata available. Least reliable." }
                        }
                    }
                }

                // Main memories table
                Panel { title: Some("Stored Context".into()), refresh: None::<String>,
                    if snapshot.rag_memories.is_empty() {
                        div { class: "text-gray-400 text-sm py-8 text-center",
                            div { class: "mb-2", "No RAG context stored yet." }
                            div { class: "text-xs",
                                "Use "
                                span { class: "font-mono bg-gray-800 px-1 rounded", "POST /memory/store_rag" }
                                " to add context for retrieval."
                            }
                        }
                    } else {
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800",
                                tr {
                                    th { class: "py-2", "Type" }
                                    th { class: "py-2", "Content" }
                                    th { class: "py-2", "Source" }
                                    th { class: "py-2", "Indexed" }
                                }
                            }
                            tbody {
                                for mem in snapshot.rag_memories.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0 hover:bg-gray-800/50",
                                        td { class: "py-2",
                                            span { class: "px-2 py-0.5 rounded text-xs bg-blue-900 text-blue-200", "{mem.memory_type}" }
                                        }
                                        td { class: "py-2 text-white max-w-md truncate", "{mem.content}" }
                                        td { class: "py-2 text-gray-400 text-xs", "{mem.agent_id}" }
                                        td { class: "py-2 text-gray-400 text-xs", "{mem.timestamp}" }
                                    }
                                }
                            }
                        }
                    }
                }

                // Memory types info
                Panel { title: Some("Memory Types".into()), refresh: None::<String>,
                    div { class: "grid grid-cols-2 md:grid-cols-4 gap-4 text-sm",
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-blue-300 font-semibold mb-1", "conversation" }
                            div { class: "text-gray-400 text-xs", "Past exchanges and dialogue history" }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-blue-300 font-semibold mb-1", "note" }
                            div { class: "text-gray-400 text-xs", "User-added notes and annotations" }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-blue-300 font-semibold mb-1", "fact" }
                            div { class: "text-gray-400 text-xs", "Factual information and data" }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-blue-300 font-semibold mb-1", "preference" }
                            div { class: "text-gray-400 text-xs", "User preferences and settings" }
                        }
                    }
                }
            }
        }

        // Issues Info Modal
        if show_issues_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_issues_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-md shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "Issue Detection Criteria" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_issues_info.set(false),
                            "×"
                        }
                    }
                    div { class: "text-sm text-gray-300 space-y-3",
                        p { class: "text-gray-400 mb-4",
                            "Entries are flagged as issues when any of the following conditions are detected:"
                        }
                        ul { class: "space-y-2",
                            li { class: "flex items-start gap-2",
                                span { class: "text-red-400 mt-0.5", "•" }
                                div {
                                    span { class: "text-white font-medium", "Zero chunks produced" }
                                    span { class: "text-gray-400", " — File was processed but generated no searchable content" }
                                }
                            }
                            li { class: "flex items-start gap-2",
                                span { class: "text-orange-400 mt-0.5", "•" }
                                div {
                                    span { class: "text-white font-medium", "Fallback detection method" }
                                    span { class: "text-gray-400", " — Last resort detection, least reliable" }
                                }
                            }
                            li { class: "flex items-start gap-2",
                                span { class: "text-yellow-400 mt-0.5", "•" }
                                div {
                                    span { class: "text-white font-medium", "Extension-only detection" }
                                    span { class: "text-gray-400", " — Less reliable than magic bytes, can be spoofed" }
                                }
                            }
                            li { class: "flex items-start gap-2",
                                span { class: "text-red-400 mt-0.5", "•" }
                                div {
                                    span { class: "text-white font-medium", "Unknown format detected" }
                                    span { class: "text-gray-400", " — Could not identify the file format" }
                                }
                            }
                            li { class: "flex items-start gap-2",
                                span { class: "text-red-400 mt-0.5", "•" }
                                div {
                                    span { class: "text-white font-medium", "No detection info available" }
                                    span { class: "text-gray-400", " — Detection metadata missing entirely" }
                                }
                            }
                        }
                    }
                    button {
                        class: "btn btn-primary btn-sm mt-4 w-full",
                        onclick: move |_| show_issues_info.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // Strategies Info Modal
        if show_strategies_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_strategies_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl shadow-xl max-h-[85vh] overflow-y-auto",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "Chunking Strategies" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_strategies_info.set(false),
                            "×"
                        }
                    }
                    p { class: "text-sm text-gray-400 mb-4",
                        "The system automatically selects the optimal chunking strategy based on detected file format:"
                    }
                    div { class: "space-y-4",
                        // character_split
                        div { class: "bg-gray-700/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "px-2 py-0.5 rounded text-xs bg-purple-900 text-purple-200 font-mono", "character_split" }
                                span { class: "text-gray-400 text-xs", "for PDF" }
                            }
                            p { class: "text-sm text-gray-300 mb-2", "Splits content by character count with overlap. Best for PDFs where structure is lost during extraction." }
                            div { class: "text-xs text-gray-500 font-mono bg-gray-800 rounded p-2",
                                "Example: \"Lorem ipsum dolor sit amet...\" → [chunk1: 500 chars] [chunk2: 500 chars]"
                            }
                        }
                        // paragraph_split
                        div { class: "bg-gray-700/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "px-2 py-0.5 rounded text-xs bg-purple-900 text-purple-200 font-mono", "paragraph_split" }
                                span { class: "text-gray-400 text-xs", "for Plain Text" }
                            }
                            p { class: "text-sm text-gray-300 mb-2", "Splits on double newlines to preserve paragraph boundaries. Keeps related sentences together." }
                            div { class: "text-xs text-gray-500 font-mono bg-gray-800 rounded p-2",
                                "Example: \"Para 1...\\n\\nPara 2...\" → [\"Para 1...\"] [\"Para 2...\"]"
                            }
                        }
                        // header_aware
                        div { class: "bg-gray-700/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "px-2 py-0.5 rounded text-xs bg-purple-900 text-purple-200 font-mono", "header_aware" }
                                span { class: "text-gray-400 text-xs", "for Markdown" }
                            }
                            p { class: "text-sm text-gray-300 mb-2", "Respects Markdown header hierarchy (# ## ###). Each section becomes a chunk with its header." }
                            div { class: "text-xs text-gray-500 font-mono bg-gray-800 rounded p-2",
                                "Example: \"# Title\\n## Section\\nContent\" → [\"# Title\"] [\"## Section\\nContent\"]"
                            }
                        }
                        // tag_aware
                        div { class: "bg-gray-700/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "px-2 py-0.5 rounded text-xs bg-purple-900 text-purple-200 font-mono", "tag_aware" }
                                span { class: "text-gray-400 text-xs", "for HTML / XML" }
                            }
                            p { class: "text-sm text-gray-300 mb-2", "Preserves tag structure. Splits at block-level elements while keeping nested content intact." }
                            div { class: "text-xs text-gray-500 font-mono bg-gray-800 rounded p-2",
                                "Example: \"<div><p>Text</p></div>\" → [\"<div><p>Text</p></div>\"]"
                            }
                        }
                        // structure_aware
                        div { class: "bg-gray-700/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "px-2 py-0.5 rounded text-xs bg-purple-900 text-purple-200 font-mono", "structure_aware" }
                                span { class: "text-gray-400 text-xs", "for JSON" }
                            }
                            p { class: "text-sm text-gray-300 mb-2", "Respects JSON object/array boundaries. Each top-level element or nested object becomes a chunk." }
                            div { class: "text-xs text-gray-500 font-mono bg-gray-800 rounded p-2",
                                r#"Example: [{"a":1}, {"b":2}] → [obj1] [obj2]"#
                            }
                        }
                        // ast_based
                        div { class: "bg-gray-700/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "px-2 py-0.5 rounded text-xs bg-purple-900 text-purple-200 font-mono", "ast_based" }
                                span { class: "text-gray-400 text-xs", "for Source Code" }
                            }
                            p { class: "text-sm text-gray-300 mb-2", "Parses code into AST and chunks by functions/classes. Keeps complete code units together." }
                            div { class: "text-xs text-gray-500 font-mono bg-gray-800 rounded p-2",
                                "Example: fn foo() + fn bar() → [fn foo()] [fn bar()]"
                            }
                        }
                        // skip
                        div { class: "bg-gray-700/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "px-2 py-0.5 rounded text-xs bg-gray-600 text-gray-300 font-mono", "skip" }
                                span { class: "text-gray-400 text-xs", "for Binary files" }
                            }
                            p { class: "text-sm text-gray-300 mb-2", "Binary files (images, executables) are skipped as they cannot be meaningfully chunked for text search." }
                            div { class: "text-xs text-gray-500 font-mono bg-gray-800 rounded p-2",
                                "Example: image.png, app.exe → [skipped]"
                            }
                        }
                        // fallback_paragraph
                        div { class: "bg-gray-700/50 rounded p-3",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "px-2 py-0.5 rounded text-xs bg-orange-900 text-orange-200 font-mono", "fallback_paragraph" }
                                span { class: "text-gray-400 text-xs", "for Unknown formats" }
                            }
                            p { class: "text-sm text-gray-300 mb-2", "Last resort for unrecognized formats. Uses basic paragraph splitting with conservative chunk sizes." }
                            div { class: "text-xs text-gray-500 font-mono bg-gray-800 rounded p-2",
                                "Example: unknown.xyz → [paragraph chunks with overlap]"
                            }
                        }
                    }
                    button {
                        class: "btn btn-primary btn-sm mt-4 w-full",
                        onclick: move |_| show_strategies_info.set(false),
                        "Got it!"
                    }
                }
            }
        }
    }
}
