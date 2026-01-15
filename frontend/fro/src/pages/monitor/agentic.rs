use crate::{api, app::Route, components::monitor::*};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

/// Episode definition tooltip content
const EPISODE_INFO_TOOLTIP: &str = r#"EPISODE DEFINITION

An episode is a recorded unit of agent activity capturing a single user-agent interaction.

┌─────────────────────────────────┐
│ Field              │ Description │
├─────────────────────────────────┤
│ id                 │ Unique UUID │
│ agent_id           │ Agent name  │
│ query              │ User input  │
│ response           │ Agent answer│
│ context_chunks_used│ Chunks found│
│ success            │ true/false  │
│ created_at         │ Timestamp   │
└─────────────────────────────────┘

SUCCESS vs FAILURE
An episode is marked successful when the agent finds relevant chunks in the knowledge base to answer the query. It's marked failed when no relevant chunks are found and the agent returns a fallback response like "I couldn't find relevant information in the knowledge base."

MAXIMUM THROUGHPUT
• Rate limit: 10 QPS (queries/second)
• Theoretical calculation: 10 requests/second × 60 seconds × 60 minutes = 36,000 episodes/hour
• Practical max: hundreds to low thousands/hour (depends on LLM processing time and hardware)

PURPOSE
• Monitor agent performance
• Calculate success rates
• Debug specific queries
• Identify knowledge gaps"#;

/// Goal definition tooltip content
const GOAL_INFO_TOOLTIP: &str = r#"GOAL DEFINITION

A goal is explicitly created by an external caller - it's not automatically detected or inferred by the system.

HOW TO CREATE A GOAL

Via API:
curl -X POST http://127.0.0.1:3010/agent/goals \
  -H "Content-Type: application/json" \
  -d '{"goal": "Your objective here"}'

┌─────────────────────────────────┐
│ Field        │ Description     │
├─────────────────────────────────┤
│ id           │ Unique UUID     │
│ agent_id     │ Agent name      │
│ goal         │ Objective text  │
│ status       │ active/completed/failed │
│ created_at   │ When created    │
│ completed_at │ When finished   │
└─────────────────────────────────┘

GOAL vs EPISODE
• Episode: Automatic - created on every /agent query
• Goal: Manual - explicitly created via API

GOAL LIFECYCLE
1. Created: User/system registers an objective
2. Active: Goal is being tracked
3. Completed: Goal achieved (via API call)
4. Failed: Goal could not be achieved

CREATION METHODS EXPLAINED

1. API Endpoint (POST /agent/goals)
External interface for outside clients (frontend, CLI, other services).
Flow: Client sends HTTP POST → Actix handler validates → Calls domain logic → Response returned
Use for: External clients, UI, cross-service communication

2. Programmatic (agent_memory.set_goal(...))
Internal Rust function calls within the same process.
Use for: Background tasks, tests, internal orchestration, goal chaining, bypassing HTTP overhead

The API endpoint wraps the programmatic method internally.

AUTOMATIC GOAL DETECTION (Future Enhancement)

1. LLM decides — Most flexible. Send message to LLM with system prompt to classify intent and extract goals. Returns structured data (goal/question/chat).

2. Keyword heuristics — Simpler and faster. Check for words like "find", "search", "look up" or phrases like "can you get". Fast but brittle.

3. Hybrid — Practical middle ground. Use heuristics for obvious cases, fall back to LLM for ambiguous ones. Saves LLM calls while handling edge cases.

With local LLM and limited resources, heuristics with LLM fallback keeps things snappy for obvious requests while handling edge cases.

NOTE: Currently, goals must be explicitly created. Automatic detection is not yet implemented."#;

/// Agent vs Tools explanation tooltip
const AGENT_INFO_TOOLTIP: &str = r#"AGENT vs AGENT TOOLS

┌─────────────────────────────────────────────────────────────┐
│                        THE AGENT                            │
├─────────────────────────────────────────────────────────────┤
│ The brain/orchestrator that decides what to do              │
│                                                             │
│ • Receives user queries                                     │
│ • Decides which mode to use (RAG, LLM, Hybrid)              │
│ • Coordinates the workflow: retrieve → reason → respond     │
│ • Tracks goals and manages memory                           │
│ • There is ONE agent that handles all queries               │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                      AGENT TOOLS                            │
├─────────────────────────────────────────────────────────────┤
│ The capabilities/actions the agent can use                  │
│                                                             │
│ • search         - Search documents in Tantivy              │
│ • vector_search  - Semantic similarity search               │
│ • llm_generate   - Call Ollama for text generation          │
│ • store_memory   - Save to agent memory                     │
│ • retrieve_memory- Recall from memory                       │
│ • set_goal       - Create a new goal                        │
│ • update_goal    - Update goal status                       │
└─────────────────────────────────────────────────────────────┘

ANALOGY

  Agent = A chef (1 person)
  Tools = Kitchen equipment (knives, pans, oven, mixer...)

The chef (agent) decides what to cook and which tools to use.
The tools don't make decisions - they just execute when the
chef uses them.

KEY DIFFERENCE

• Agent: Makes decisions, orchestrates workflow (1 instance)
• Tools: Execute specific actions when called (many types)"#;

#[derive(Clone, Default)]
struct AgenticState {
    loading: bool,
    error: Option<String>,
    // Agent stats from API
    agent_stats: Option<api::AgentStatsResponse>,
    // Goals from API
    goals: Option<api::GoalsResponse>,
    // Episodes from API
    episodes: Option<api::EpisodesResponse>,
    // Reflections from API
    reflections: Option<api::ReflectionsResponse>,
    // Memory stats from API
    memory_stats: Option<api::MemoryStatsResponse>,
    // Tool stats from API
    tool_stats: Option<api::ToolStatsResponse>,
}

#[component]
pub fn MonitorAgentic() -> Element {
    let state = use_signal(|| AgenticState {
        loading: true,
        ..Default::default()
    });

    // Fetch data on mount and periodically
    {
        let mut state = state.clone();
        use_future(move || async move {
            loop {
                let mut new_state = AgenticState {
                    loading: false,
                    error: None,
                    agent_stats: None,
                    goals: None,
                    episodes: None,
                    reflections: None,
                    memory_stats: None,
                    tool_stats: None,
                };

                // Fetch agent stats
                match api::fetch_agent_stats().await {
                    Ok(stats) => new_state.agent_stats = Some(stats),
                    Err(e) => new_state.error = Some(format!("Agent stats: {}", e)),
                }

                // Fetch goals
                if let Ok(goals) = api::fetch_goals().await {
                    new_state.goals = Some(goals);
                }

                // Fetch episodes
                if let Ok(episodes) = api::fetch_recent_episodes(10).await {
                    new_state.episodes = Some(episodes);
                }

                // Fetch reflections
                if let Ok(reflections) = api::fetch_reflections(5).await {
                    new_state.reflections = Some(reflections);
                }

                // Fetch memory stats
                if let Ok(memory) = api::fetch_memory_stats().await {
                    new_state.memory_stats = Some(memory);
                }

                // Fetch tool stats
                if let Ok(tools) = api::fetch_tool_stats().await {
                    new_state.tool_stats = Some(tools);
                }

                state.set(new_state);

                // Refresh every 5 seconds
                TimeoutFuture::new(5_000).await;
            }
        });
    }

    let snapshot = state.read().clone();

    // Extract values with defaults
    let agent_stats = snapshot.agent_stats.clone().unwrap_or_default();
    let goals = snapshot.goals.clone().unwrap_or_default();
    let episodes = snapshot.episodes.clone().unwrap_or_default();
    let reflections = snapshot.reflections.clone().unwrap_or_default();
    let memory_stats = snapshot.memory_stats.clone().unwrap_or_default();
    let tool_stats = snapshot.tool_stats.clone().unwrap_or_default();

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Agentic", None),
                ],
            }

            NavTabs { active: Route::MonitorAgentic {} }

            // Error display
            if let Some(err) = snapshot.error.clone() {
                div { class: "bg-red-900/50 border border-red-500 rounded p-3 text-red-200 text-sm",
                    "Error loading data: {err}"
                }
            }

            // Agent Activity Section
            RowHeader {
                title: "Agent Activity".into(),
                description: Some("Real-time agent behavior and performance metrics".into()),
            }

            Panel { title: Some("Agent Overview".into()), refresh: Some("5s".into()),
                if snapshot.loading {
                    div { class: "text-gray-400 text-sm", "Loading agent stats…" }
                } else {
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                        // Active Agents with info button
                        div { class: "rounded p-4 bg-gray-800 border border-gray-700",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-2xl font-bold text-gray-100",
                                    "{agent_stats.active_agents}"
                                }
                                span { class: "text-sm font-semibold text-gray-200",
                                    "Active Agent"
                                }
                                AgentInfoButton {}
                            }
                            div { class: "text-xs text-gray-400",
                                "The orchestrator that coordinates retrieval, reasoning, and response"
                            }
                        }
                        // Episodes/hr with Success Rate below
                        div { class: "rounded p-4 bg-gray-800 border border-gray-700",
                            div { class: "flex items-center gap-2 mb-2",
                                span { class: "text-sm font-semibold text-gray-200",
                                    "{agent_stats.episodes_last_hour} Episodes/hr"
                                }
                                EpisodeInfoButton {}
                            }
                            div { class: "text-xs text-gray-400",
                                "Success Rate: "
                                span { class: "text-gray-200 font-medium",
                                    "{agent_stats.success_rate:.1}%"
                                }
                            }
                        }
                        StatCard {
                            title: "Active Goals".into(),
                            value: agent_stats.active_goals.to_string().into(),
                            unit: None,
                            description: None,
                            info_tooltip: Some(GOAL_INFO_TOOLTIP.into()),
                        }
                    }
                }
            }

            // Decision Engine Section
            RowHeader {
                title: "Decision Engine".into(),
                description: Some("Tool selection and reasoning metrics".into()),
            }

            Panel { title: Some("Tool Performance".into()), refresh: Some("10s".into()),
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                    StatCard {
                        title: "Tool Executions".into(),
                        value: tool_stats.tool_executions.to_string().into(),
                        unit: None,
                        description: Some("Total tool calls".into()),
                    }
                    StatCard {
                        title: "Avg Confidence".into(),
                        value: format!("{:.1}", tool_stats.avg_confidence).into(),
                        unit: Some("%".into()),
                        description: Some("Decision confidence score".into()),
                    }
                    StatCard {
                        title: "Fallback Rate".into(),
                        value: format!("{:.1}", tool_stats.fallback_rate).into(),
                        unit: Some("%".into()),
                        description: Some("Secondary tool usage".into()),
                    }
                }

                // Tool Usage Distribution
                div { class: "mt-4",
                    RowHeader {
                        title: "Tool Usage Distribution".into(),
                        description: Some("Which tools are being selected".into()),
                    }
                    div { class: "bg-gray-800/50 rounded p-4 space-y-2",
                        for tool in tool_stats.tool_distribution.iter() {
                            div { class: "flex items-center gap-3",
                                span { class: "text-xs text-gray-400 w-32", "{tool.tool_name}" }
                                div { class: "flex-1 h-4 bg-gray-700 rounded overflow-hidden",
                                    div {
                                        class: "h-full bg-teal-500",
                                        style: "width: {tool.percentage}%"
                                    }
                                }
                                span { class: "text-xs text-gray-300 w-12 text-right", "{tool.percentage:.0}%" }
                            }
                        }
                        if tool_stats.tool_distribution.is_empty() {
                            div { class: "text-gray-500 text-sm italic", "No tool usage data yet" }
                        }
                    }
                }
            }

            // Memory Health Section
            RowHeader {
                title: "Agent Memory".into(),
                description: Some("Episodic memory and storage health".into()),
            }

            Panel { title: Some("Memory Statistics".into()), refresh: Some("10s".into()),
                div { class: "grid grid-cols-1 md:grid-cols-4 gap-4",
                    StatCard {
                        title: "Total Episodes".into(),
                        value: memory_stats.total_episodes.to_string().into(),
                        unit: None,
                        description: Some("Stored interactions".into()),
                    }
                    StatCard {
                        title: "RAG Memories".into(),
                        value: memory_stats.total_rag_memories.to_string().into(),
                        unit: None,
                        description: Some("Vector-embedded memories".into()),
                    }
                    StatCard {
                        title: "Unique Agents".into(),
                        value: memory_stats.unique_agents.to_string().into(),
                        unit: None,
                        description: Some("Distinct agent IDs".into()),
                    }
                    StatCard {
                        title: "Reflections".into(),
                        value: agent_stats.total_reflections.to_string().into(),
                        unit: None,
                        description: Some("Self-analysis records".into()),
                    }
                }
            }

            // Goals Section
            Panel { title: Some("Goal Tracking".into()), refresh: Some("10s".into()),
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                    // Active Goals
                    div { class: "bg-gray-800/50 rounded p-4",
                        div { class: "text-xs text-gray-400 mb-2", "Active" }
                        div { class: "text-2xl font-bold text-teal-400", "{goals.active}" }
                    }
                    // Completed Goals
                    div { class: "bg-gray-800/50 rounded p-4",
                        div { class: "text-xs text-gray-400 mb-2", "Completed" }
                        div { class: "text-2xl font-bold text-green-400", "{goals.completed}" }
                    }
                    // Failed Goals
                    div { class: "bg-gray-800/50 rounded p-4",
                        div { class: "text-xs text-gray-400 mb-2", "Failed" }
                        div { class: "text-2xl font-bold text-red-400", "{goals.failed}" }
                    }
                }

                // Goals list
                if !goals.goals.is_empty() {
                    div { class: "mt-4",
                        div { class: "text-xs text-gray-400 mb-2", "Recent Goals" }
                        div { class: "space-y-2",
                            for goal in goals.goals.iter().take(5) {
                                div { class: "bg-gray-900/50 rounded p-2 flex items-center gap-3",
                                    span {
                                        class: match goal.status.as_str() {
                                            "active" => "w-2 h-2 rounded-full bg-teal-400",
                                            "completed" => "w-2 h-2 rounded-full bg-green-400",
                                            "failed" => "w-2 h-2 rounded-full bg-red-400",
                                            _ => "w-2 h-2 rounded-full bg-gray-400",
                                        }
                                    }
                                    span { class: "text-sm text-gray-200 flex-1", "{goal.goal}" }
                                    span { class: "text-xs text-gray-500", "{goal.status}" }
                                }
                            }
                        }
                    }
                }
            }

            // Recent Reflections Section
            Panel { title: Some("Recent Reflections".into()), refresh: Some("30s".into()),
                if reflections.reflections.is_empty() {
                    div { class: "text-gray-500 text-sm italic",
                        "No reflections recorded yet. Reflections appear after agent interactions."
                    }
                } else {
                    div { class: "space-y-2",
                        for reflection in reflections.reflections.iter() {
                            div { class: "bg-gray-900/50 rounded p-3",
                                div { class: "flex items-center gap-2 mb-1",
                                    span {
                                        class: match reflection.reflection_type.as_str() {
                                            "success" => "text-xs px-2 py-0.5 rounded bg-green-900/50 text-green-300",
                                            "failure" => "text-xs px-2 py-0.5 rounded bg-red-900/50 text-red-300",
                                            "pattern" => "text-xs px-2 py-0.5 rounded bg-blue-900/50 text-blue-300",
                                            "improvement" => "text-xs px-2 py-0.5 rounded bg-purple-900/50 text-purple-300",
                                            _ => "text-xs px-2 py-0.5 rounded bg-gray-700 text-gray-300",
                                        },
                                        "{reflection.reflection_type}"
                                    }
                                    span { class: "text-xs text-gray-500", "{format_timestamp(reflection.created_at)}" }
                                }
                                div { class: "text-sm text-gray-200", "{reflection.insight}" }
                            }
                        }
                    }
                }
            }

            // Recent Episodes Section
            Panel { title: Some("Recent Episodes".into()), refresh: Some("5s".into()),
                if episodes.episodes.is_empty() {
                    div { class: "text-gray-500 text-sm italic",
                        "No episodes recorded yet. Episodes appear after agent queries."
                    }
                } else {
                    div { class: "space-y-2",
                        for episode in episodes.episodes.iter().take(5) {
                            div { class: "bg-gray-900/50 rounded p-3",
                                div { class: "flex items-center gap-2 mb-1",
                                    span {
                                        class: if episode.success {
                                            "text-xs px-2 py-0.5 rounded bg-green-900/50 text-green-300"
                                        } else {
                                            "text-xs px-2 py-0.5 rounded bg-red-900/50 text-red-300"
                                        },
                                        if episode.success { "✓ success" } else { "✗ failed" }
                                    }
                                    span { class: "text-xs text-gray-500", "{format_timestamp(episode.created_at)}" }
                                    span { class: "text-xs text-gray-600", "• {episode.context_chunks_used} chunks" }
                                }
                                div { class: "text-sm text-gray-200 truncate", "Q: {episode.query}" }
                                div { class: "text-xs text-gray-400 truncate mt-1", "A: {truncate_text(&episode.response, 100)}" }
                            }
                        }
                    }
                    if episodes.total > 5 {
                        div { class: "text-xs text-gray-500 mt-2 text-center",
                            "Showing 5 of {episodes.total} episodes"
                        }
                    }
                }
            }
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

/// Info button for Episodes with tooltip
#[component]
fn EpisodeInfoButton() -> Element {
    let mut show_tooltip = use_signal(|| false);

    const INFO_BUTTON_CLASS: &str =
        "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded border border-blue-500/40 bg-blue-500/10 flex items-center justify-center cursor-pointer hover:bg-blue-500/20";

    rsx! {
        button {
            class: INFO_BUTTON_CLASS,
            onclick: move |_| show_tooltip.set(!show_tooltip()),
            title: "Show info",
            svg {
                class: "w-3 h-3 text-blue-400",
                view_box: "0 0 20 20",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                circle { cx: "10", cy: "10", r: "9" }
                line { x1: "10", y1: "8", x2: "10", y2: "14" }
                circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
            }
        }

        if *show_tooltip.read() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_tooltip.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[95vh] overflow-y-auto shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "Episode Info" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_tooltip.set(false),
                            "×"
                        }
                    }
                    div {
                        class: "text-sm text-gray-300 whitespace-pre-line leading-relaxed",
                        {EPISODE_INFO_TOOLTIP}
                    }
                }
            }
        }
    }
}

/// Info button for Agent vs Tools explanation
#[component]
fn AgentInfoButton() -> Element {
    let mut show_tooltip = use_signal(|| false);

    const INFO_BUTTON_CLASS: &str =
        "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded border border-blue-500/40 bg-blue-500/10 flex items-center justify-center cursor-pointer hover:bg-blue-500/20";

    rsx! {
        button {
            class: INFO_BUTTON_CLASS,
            onclick: move |_| show_tooltip.set(!show_tooltip()),
            title: "What is an Agent?",
            svg {
                class: "w-3 h-3 text-blue-400",
                view_box: "0 0 20 20",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                circle { cx: "10", cy: "10", r: "9" }
                line { x1: "10", y1: "8", x2: "10", y2: "14" }
                circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
            }
        }

        if *show_tooltip.read() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_tooltip.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-2xl max-h-[95vh] overflow-y-auto shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "Agent vs Agent Tools" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_tooltip.set(false),
                            "×"
                        }
                    }
                    div {
                        class: "text-sm text-gray-300 whitespace-pre-line leading-relaxed font-mono",
                        {AGENT_INFO_TOOLTIP}
                    }
                }
            }
        }
    }
}
