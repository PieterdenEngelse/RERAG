use crate::api;
use crate::api::ToolExecution;
use crate::app::Route;
use crate::components::monitor::*;
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[derive(Clone, Default)]
struct ToolsState {
    loading: bool,
    error: Option<String>,
    total_executions: usize,
    avg_confidence: f64,
    fallback_rate: f64,
    recent_executions: Vec<ToolExecution>,
}

/// Tool info for display
struct ToolInfo {
    name: &'static str,
    description: &'static str,
    status: &'static str,
    icon: &'static str,
}

const TOOLS: &[ToolInfo] = &[
    // Core Tools
    ToolInfo {
        name: "Calculator",
        description: "Mathematical calculations and arithmetic operations",
        status: "active",
        icon: "🧮",
    },
    ToolInfo {
        name: "WebSearch",
        description: "Search the web for information",
        status: "active",
        icon: "🔍",
    },
    ToolInfo {
        name: "URLFetch",
        description: "Fetch and parse content from URLs",
        status: "active",
        icon: "🌐",
    },
    ToolInfo {
        name: "SemanticSearch",
        description: "Search indexed documents using semantic similarity",
        status: "active",
        icon: "📚",
    },
    ToolInfo {
        name: "DatabaseQuery",
        description: "Execute read-only SQL queries",
        status: "active",
        icon: "🗄️",
    },
    ToolInfo {
        name: "CodeExecution",
        description: "Execute Python or Bash code snippets",
        status: "active",
        icon: "💻",
    },
    ToolInfo {
        name: "ImageGeneration",
        description: "Generate images from text descriptions",
        status: "placeholder",
        icon: "🎨",
    },
    // Agent Tools
    ToolInfo {
        name: "Summarizer",
        description: "Summarize text, documents, or search results",
        status: "active",
        icon: "📝",
    },
    ToolInfo {
        name: "QueryRewriter",
        description: "Improve queries by fixing typos and expanding abbreviations",
        status: "active",
        icon: "🔄",
    },
    ToolInfo {
        name: "Classifier",
        description: "Categorize content, detect intent, and extract tags",
        status: "active",
        icon: "🏷️",
    },
    ToolInfo {
        name: "FileAnalyzer",
        description: "Analyze file contents and extract metadata",
        status: "active",
        icon: "📊",
    },
    ToolInfo {
        name: "Notification",
        description: "Send alerts and notifications via webhooks",
        status: "active",
        icon: "🔔",
    },
];

#[component]
pub fn MonitorTools() -> Element {
    let state = use_signal(ToolsState::default);
    let mut show_tool_info = use_signal(|| false);
    let mut selected_tool = use_signal(|| None::<String>);

    // Fetch tool execution data
    {
        let mut state = state.clone();
        use_future(move || async move {
            loop {
                state.write().loading = true;
                
                let stats_result = api::fetch_tool_stats().await;
                let executions_result = api::fetch_tool_executions(50).await;
                
                match (stats_result, executions_result) {
                    (Ok(stats), Ok(executions)) => {
                        state.set(ToolsState {
                            loading: false,
                            error: None,
                            total_executions: stats.tool_executions,
                            avg_confidence: stats.avg_confidence,
                            fallback_rate: stats.fallback_rate,
                            recent_executions: executions.executions,
                        });
                    }
                    (Err(e), _) | (_, Err(e)) => {
                        let prev = state.read().clone();
                        state.set(ToolsState {
                            loading: false,
                            error: Some(e),
                            ..prev
                        });
                    }
                }

                TimeoutFuture::new(5_000).await;
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
                    BreadcrumbItem::new("Tools", None::<Route>),
                ],
            }

            NavTabs { active: Route::MonitorTools {} }

            // Page header
            Panel { title: Some("Agent Tools".into()), refresh: None::<String>,
                div { class: "text-sm text-gray-300 space-y-2",
                    p { "Agent tools extend the capabilities of the AI system. Each tool provides specialized functionality that can be invoked during agent reasoning." }
                    div { class: "flex flex-wrap gap-4 mt-3 text-xs",
                        div { class: "flex items-center gap-1",
                            span { class: "w-2 h-2 rounded-full bg-green-500" }
                            span { "Active" }
                        }
                        div { class: "flex items-center gap-1",
                            span { class: "w-2 h-2 rounded-full bg-yellow-500" }
                            span { "Placeholder" }
                        }
                        div { class: "flex items-center gap-1",
                            span { class: "w-2 h-2 rounded-full bg-red-500" }
                            span { "Disabled" }
                        }
                    }
                }
            }

            // Tools Grid
            RowHeader {
                title: "Available Tools".into(),
                description: Some("12 tools registered in the agent tool registry".into()),
            }

            div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
                for tool in TOOLS.iter() {
                    div {
                        class: "bg-gray-800 border border-gray-700 rounded-lg p-4 hover:border-gray-500 transition-colors cursor-pointer",
                        onclick: {
                            let tool_name = tool.name.to_string();
                            move |_| {
                                selected_tool.set(Some(tool_name.clone()));
                                show_tool_info.set(true);
                            }
                        },
                        div { class: "flex items-start justify-between mb-3",
                            div { class: "flex items-center gap-3",
                                span { class: "text-2xl", "{tool.icon}" }
                                div {
                                    h3 { class: "text-white font-semibold", "{tool.name}" }
                                    p { class: "text-gray-400 text-xs", "{tool.description}" }
                                }
                            }
                            span {
                                class: match tool.status {
                                    "active" => "w-2 h-2 rounded-full bg-green-500",
                                    "placeholder" => "w-2 h-2 rounded-full bg-yellow-500",
                                    _ => "w-2 h-2 rounded-full bg-red-500",
                                },
                            }
                        }
                        div { class: "flex items-center justify-between text-xs text-gray-500",
                            span {
                                class: match tool.status {
                                    "active" => "text-green-400",
                                    "placeholder" => "text-yellow-400",
                                    _ => "text-red-400",
                                },
                                match tool.status {
                                    "active" => "● Ready",
                                    "placeholder" => "○ Placeholder",
                                    _ => "✕ Disabled",
                                }
                            }
                            span { "Click for details" }
                        }
                    }
                }
            }

            // Tool Execution Log
            {
                let desc = format!("Tool invocations from agent reasoning sessions ({} total)", snapshot.total_executions);
                rsx! {
                    RowHeader {
                        title: "Recent Executions".into(),
                        description: Some(desc.into()),
                    }
                }
            }

            Panel { title: Some("Execution Log".into()), refresh: Some("5s".into()),
                if snapshot.loading {
                    div { class: "text-gray-400 text-sm py-4", "Loading executions..." }
                } else if let Some(ref err) = snapshot.error {
                    div { class: "text-red-400 text-sm py-4", "Error: {err}" }
                } else if snapshot.recent_executions.is_empty() {
                    div { class: "text-gray-400 text-sm py-8 text-center",
                        div { class: "mb-2", "No tool executions recorded yet." }
                        div { class: "text-xs",
                            "Tools are invoked automatically during agent reasoning when needed."
                        }
                    }
                } else {
                    div { class: "overflow-x-auto",
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800 text-xs",
                                tr {
                                    th { class: "py-2 px-2", "Time" }
                                    th { class: "py-2 px-2", "Tool" }
                                    th { class: "py-2 px-2", "Query" }
                                    th { class: "py-2 px-2", "Status" }
                                    th { class: "py-2 px-2", "Latency" }
                                    th { class: "py-2 px-2", "Confidence" }
                                }
                            }
                            tbody {
                                for exec in snapshot.recent_executions.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0 hover:bg-gray-800/50",
                                        td { class: "py-2 px-2 text-gray-400 text-xs whitespace-nowrap",
                                            {
                                                // Format timestamp: extract time part
                                                let ts = &exec.timestamp;
                                                if ts.len() >= 19 {
                                                    format!("{} {}", &ts[5..10], &ts[11..16])
                                                } else {
                                                    ts.clone()
                                                }
                                            }
                                        }
                                        td { class: "py-2 px-2",
                                            span { class: "px-2 py-0.5 rounded text-xs bg-blue-900 text-blue-200", "{exec.tool_type}" }
                                        }
                                        td { class: "py-2 px-2 text-white max-w-[250px] truncate", title: "{exec.query}", "{exec.query}" }
                                        td { class: "py-2 px-2",
                                            if exec.success {
                                                span { class: "text-green-400", "✓ Success" }
                                            } else {
                                                span { class: "text-red-400", "✗ Failed" }
                                            }
                                        }
                                        td { class: "py-2 px-2 text-gray-400", "{exec.execution_time_ms}ms" }
                                        td { class: "py-2 px-2 text-gray-400", "{exec.confidence:.2}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Tool Statistics
            RowHeader {
                title: "Tool Statistics".into(),
                description: Some("Aggregated metrics for tool usage".into()),
            }

            Panel { title: Some("Usage Metrics".into()), refresh: Some("5s".into()),
                div { class: "grid grid-cols-2 md:grid-cols-4 gap-4",
                    div { class: "bg-gray-700/50 rounded p-4 text-center",
                        div { class: "text-2xl font-bold text-white", "12" }
                        div { class: "text-xs text-gray-400", "Total Tools" }
                    }
                    div { class: "bg-gray-700/50 rounded p-4 text-center",
                        div { class: "text-2xl font-bold text-blue-400", "{snapshot.total_executions}" }
                        div { class: "text-xs text-gray-400", "Executions" }
                    }
                    div { class: "bg-gray-700/50 rounded p-4 text-center",
                        div { class: "text-2xl font-bold text-green-400", "{snapshot.avg_confidence:.2}" }
                        div { class: "text-xs text-gray-400", "Avg Confidence" }
                    }
                    div { class: "bg-gray-700/50 rounded p-4 text-center",
                        div {
                            class: format!("text-2xl font-bold {}", if snapshot.fallback_rate > 0.1 { "text-red-400" } else { "text-green-400" }),
                            { format!("{:.1}%", snapshot.fallback_rate * 100.0) }
                        }
                        div { class: "text-xs text-gray-400", "Failure Rate" }
                    }
                }
            }

            // Tool Categories
            Panel { title: Some("Tool Categories".into()), refresh: None::<String>,
                div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 text-sm",
                    div { class: "bg-gray-700/50 rounded p-3",
                        div { class: "flex items-center gap-2 mb-2",
                            span { class: "text-purple-400 font-semibold", "Computation" }
                        }
                        div { class: "text-gray-400 text-xs space-y-1",
                            div { "Calculator - Math" }
                            div { "CodeExecution - Run code" }
                        }
                    }
                    div { class: "bg-gray-700/50 rounded p-3",
                        div { class: "flex items-center gap-2 mb-2",
                            span { class: "text-blue-400 font-semibold", "Information" }
                        }
                        div { class: "text-gray-400 text-xs space-y-1",
                            div { "WebSearch - Web queries" }
                            div { "URLFetch - Fetch URLs" }
                            div { "SemanticSearch - Docs" }
                            div { "DatabaseQuery - SQL" }
                        }
                    }
                    div { class: "bg-gray-700/50 rounded p-3",
                        div { class: "flex items-center gap-2 mb-2",
                            span { class: "text-green-400 font-semibold", "Generation" }
                        }
                        div { class: "text-gray-400 text-xs space-y-1",
                            div { "ImageGeneration - Images" }
                        }
                    }
                    div { class: "bg-gray-700/50 rounded p-3",
                        div { class: "flex items-center gap-2 mb-2",
                            span { class: "text-yellow-400 font-semibold", "Agents" }
                        }
                        div { class: "text-gray-400 text-xs space-y-1",
                            div { "Summarizer - Summaries" }
                            div { "QueryRewriter - Improve queries" }
                            div { "Classifier - Categorize" }
                            div { "FileAnalyzer - Analyze files" }
                            div { "Notification - Alerts" }
                        }
                    }
                }
            }
        }

        // Tool Info Modal
        if show_tool_info() {
            if let Some(tool_name) = selected_tool() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_tool_info.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-lg shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        
                        // Find the tool info
                        {
                            let tool = TOOLS.iter().find(|t| t.name == tool_name);
                            if let Some(t) = tool {
                                rsx! {
                                    div { class: "flex items-center justify-between mb-4",
                                        div { class: "flex items-center gap-3",
                                            span { class: "text-3xl", "{t.icon}" }
                                            h2 { class: "text-lg font-semibold text-gray-100", "{t.name}" }
                                        }
                                        button {
                                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                            onclick: move |_| show_tool_info.set(false),
                                            "×"
                                        }
                                    }
                                    
                                    div { class: "space-y-4",
                                        div {
                                            div { class: "text-xs text-gray-500 uppercase tracking-wide mb-1", "Description" }
                                            p { class: "text-sm text-gray-300", "{t.description}" }
                                        }
                                        
                                        div {
                                            div { class: "text-xs text-gray-500 uppercase tracking-wide mb-1", "Status" }
                                            span {
                                                class: match t.status {
                                                    "active" => "px-2 py-1 rounded text-xs bg-green-900/50 text-green-300",
                                                    "placeholder" => "px-2 py-1 rounded text-xs bg-yellow-900/50 text-yellow-300",
                                                    _ => "px-2 py-1 rounded text-xs bg-red-900/50 text-red-300",
                                                },
                                                match t.status {
                                                    "active" => "Active - Fully implemented",
                                                    "placeholder" => "Placeholder - API not configured",
                                                    _ => "Disabled",
                                                }
                                            }
                                        }
                                        
                                        div {
                                            div { class: "text-xs text-gray-500 uppercase tracking-wide mb-1", "Usage" }
                                            div { class: "text-xs text-gray-400 bg-gray-700/50 rounded p-2 font-mono",
                                                match t.name {
                                                    "Calculator" => "Input: \"5 + 3\" or \"100 * 2\"",
                                                    "WebSearch" => "Input: \"latest AI research papers\"",
                                                    "URLFetch" => "Input: \"https://example.com\"",
                                                    "SemanticSearch" => "Input: \"find documents about Rust\"",
                                                    "DatabaseQuery" => "Input: \"SELECT * FROM users LIMIT 10\"",
                                                    "CodeExecution" => "Input: \"```python\\nprint(2+2)\\n```\"",
                                                    "ImageGeneration" => "Input: \"A sunset over mountains\"",
                                                    "Summarizer" => "Input: \"Summarize this article about climate change...\"",
                                                    "QueryRewriter" => "Input: \"hw to fix nulpointer exeption\"",
                                                    "Classifier" => "Input: \"Is this email spam or legitimate?\"",
                                                    "FileAnalyzer" => "Input: \"Analyze /path/to/document.pdf\"",
                                                    "Notification" => "Input: \"Alert: Build failed on main branch\"",
                                                    _ => "No usage example available",
                                                }
                                            }
                                        }
                                    }
                                    
                                    button {
                                        class: "btn btn-primary btn-sm mt-4 w-full",
                                        onclick: move |_| show_tool_info.set(false),
                                        "Close"
                                    }
                                }
                            } else {
                                rsx! {
                                    div { class: "text-gray-400", "Tool not found" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
