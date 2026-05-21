use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
};
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

/// Rig agentic mode explanation modal
const RIG_MODE_INFO_TOOLTIP: &str = r#"TOTAL CALLS
Number of times agentic mode was invoked since the backend started.
Resets on restart.

TOOL CALLS
Total individual tool invocations across all agentic sessions.
One session can trigger multiple tool calls — e.g. the LLM calls
search_documents twice then recall_memory once = 3 tool calls.

Available tools the LLM can call:
  search_documents       full-text + semantic search
  recall_memory          retrieve past conversation memories
  store_memory           persist a fact for future sessions
  search_knowledge_graph entity relationship lookup"#;

/// Context budget modal
const TOKEN_BUDGET_INFO_TOOLTIP: &str = r#"CONTEXT BUDGET BAR
Average % of the model's context window (num_ctx) consumed by the prompt
before any tool results are added. Tool results grow this further each loop.
  Teal   0–60%   enough headroom
  Yellow 60–80%  getting full; tool results may be cut short
  Red    >80%    near limit; earlier context may be dropped by the model

AVG TOKENS / SESSION
Preamble + query token count averaged across the last 100 sessions.

MAX TOKENS SEEN
Highest single-session token count recorded.

AVG SESSION
Mean time for the full Rig tool-calling loop to complete, in milliseconds.

COUNTER
  exact     — token counts from the loaded GGUF vocab (precise)
  heuristic — estimated from character and word counts (~10–15% error)
  mixed     — model changed mid-run; some sessions used each method"#;

/// Rig fallback modal
const RIG_FALLBACK_INFO_TOOLTIP: &str = r#"FALLBACKS
Number of agentic sessions where the Rig loop failed and the system
automatically retried using Classic Hybrid mode. The user received an
answer either way.

Common causes:
  model returned malformed tool-call JSON
  Ollama connection dropped mid-loop
  a tool execution threw an error

Hitting the max iteration cap is NOT a fallback — the loop ends
normally and the last model response is returned directly.

FALLBACK RATE
Fallbacks ÷ Total Calls × 100.
A high rate means the model is not reliably producing tool-call JSON.
Fallback responses include "mode": "agentic_fallback" and a
"fallback_reason" field."#;

#[allow(dead_code)]
const TOOL_PERF_INFO_TOOLTIP: &str = r#"TOOL PERFORMANCE

Tracks every tool invocation made by the agent across all modes.

METRICS
• Tool Executions  — total tool calls since startup (Classic + Rig combined)
• Avg Confidence   — mean confidence score returned by tool selection logic
• Fallback Rate    — % of calls that used a secondary tool because the primary
                     returned no results or failed

TOOL USAGE DISTRIBUTION
Shows the share of calls per tool type. Dominated by search tools in RAG/Hybrid
mode; memory and graph tools appear more when Rig Agentic mode is active.

NOTE
Rig Agentic tool calls (search_documents, recall_memory, store_memory,
search_knowledge_graph) are recorded here alongside Classic pipeline tools.
Check the Rig Agentic Mode section above for Rig-specific fallback stats."#;

#[allow(dead_code)]
const MEMORY_STATS_INFO_TOOLTIP: &str = r#"MEMORY STATISTICS

The agent uses SQLite (agent.db) as its memory store. Three types of memory
are tracked here.

MEMORY TYPES
• Episodes       — one record per agent interaction (query + response + success flag)
                   Written automatically after every /agent call
• RAG Memories   — short-form facts extracted and embedded for vector retrieval
                   Written when the agent calls store_memory or via Rig tool
• Unique Agents  — distinct agent_id values seen in the database
                   Typically "default" unless you run multiple named agents

REFLECTIONS
Self-analysis records generated after a configurable number of episodes.
The agent reviews recent interactions and writes observations about patterns,
failures, and improvements. Appears in "Recent Reflections" below.

STORAGE
All memory is local to agent.db (SQLite). No external service required.
Document embeddings live in the Tantivy index; chunk metadata is referenced
from the knowledge graph via embedding_id pointers."#;

#[allow(dead_code)]
const REFLECTIONS_INFO_TOOLTIP: &str = r#"RECENT REFLECTIONS

After a configurable number of episodes the agent reviews its own recent
interactions and writes a short self-analysis record.

REFLECTION TYPES
  success     — something worked well; pattern worth reinforcing
  failure     — a query failed or returned poor results; root cause noted
  pattern     — recurring behaviour observed across multiple episodes
  improvement — specific change the agent identifies as beneficial

WHAT TRIGGERS A REFLECTION
Reflections are generated automatically by the agent's reflection scheduler.
The interval is controlled by the REFLECTION_INTERVAL env variable (default: 10 episodes).

HOW TO READ THEM
Each reflection shows a type badge, timestamp, and insight text. Failure
reflections are most actionable — they surface gaps in the knowledge base
or queries that consistently produce low-confidence answers."#;

#[allow(dead_code)]
const AGENT_INFO_TOOLTIP: &str = r#"AGENT vs AGENT TOOLS

┌─────────────────────────────────────────────────────────────┐
│                        THE AGENT                            │
├─────────────────────────────────────────────────────────────┤
│ The orchestrator that decides what to do and how            │
│                                                             │
│ • Receives user queries via /agent or /agent/stream         │
│ • Selects a mode: RAG, LLM, Hybrid, RagStrict, Agentic      │
│ • Tracks goals, records episodes, manages memory            │
│ • One agent instance handles all queries                    │
└─────────────────────────────────────────────────────────────┘

TWO DIFFERENT TOOL SETS

Classic mode (RAG / Hybrid / RagStrict)
  Tools are Rust functions called directly in the pipeline:
  • search / vector_search  — document retrieval
  • llm_generate            — Ollama text generation
  • store_memory            — write to agent.db
  • retrieve_memory         — read from agent.db
  • set_goal / update_goal  — goal lifecycle management
  The agent decides which steps to run; it is not the LLM.

Rig Agentic mode (/agent/stream with mode=agentic)
  Tools are JSON-schema definitions handed to the LLM:
  • search_documents        — Tantivy search
  • recall_memory           — retrieve conversation memories
  • store_memory            — persist facts for later sessions
  • search_knowledge_graph  — entity relationship lookup
  The LLM decides which tools to call and in what order.

ANALOGY

  Classic = sous chef following a recipe (predictable steps)
  Agentic = head chef improvising (LLM picks the tools)

KEY DIFFERENCE
• Classic tools: called by Rust code, deterministic order
• Rig tools: called by the LLM, dynamic order, may be skipped"#;

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
    // Rig agentic-mode stats
    rig_stats: Option<api::RigStatsResponse>,
}

#[component]
pub fn MonitorAgentic() -> Element {
    let state = use_signal(|| AgenticState {
        loading: true,
        ..Default::default()
    });

    // Fetch data on mount and periodically
    {
        let mut state = state;
        let mut page_errors = use_context::<Signal<PageErrors>>();
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
                    rig_stats: None,
                };

                // Fetch agent stats
                match api::fetch_agent_stats().await {
                    Ok(stats) => {
                        new_state.agent_stats = Some(stats);
                        page_errors.with_mut(|e| e.clear_error("agentic"));
                    }
                    Err(e) => {
                        let err = format!("Agent stats: {}", e);
                        new_state.error = Some(err.clone());
                        page_errors.with_mut(|errs| errs.set_error("agentic", &err));
                        let _ = api::log_frontend_error("agentic", &err).await;
                    }
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

                // Fetch Rig agentic-mode stats
                if let Ok(rig) = api::fetch_rig_stats().await {
                    new_state.rig_stats = Some(rig);
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
    let rig_stats = snapshot.rig_stats.clone().unwrap_or_default();

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
            }

            Panel { title: Some("Agent Overview".into()), refresh: Some("5s".into()),
                if snapshot.loading {
                    div { class: "text-gray-400 text-sm", "Loading agent stats…" }
                } else {
                    // Active Agents — one card per agent, expands as more are added
                    div { class: "flex flex-wrap gap-3 mb-4",
                        for name in &agent_stats.agent_names {
                            div { class: "rounded p-3 bg-gray-800 border border-teal-700/50 flex items-center gap-2",
                                div { class: "w-2 h-2 rounded-full bg-teal-400 shrink-0" }
                                span { class: "text-sm font-mono text-teal-300", "{name}" }
                                span { class: "text-xs text-gray-500", "agent" }
                            }
                        }
                    }

                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
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
                            info_tooltip: Some(GOAL_INFO_TOOLTIP.into()),
                        }
                    }
                }
            }

            // Rig Agentic Mode Section
            RowHeader {
                title: "Rig Agentic Mode".into(),
            }

            Panel { title: Some("Agentic Sessions".into()), refresh: Some("5s".into()),
                div { class: "grid grid-cols-2 md:grid-cols-4 gap-4",
                    StatCard {
                        title: "Total Calls".into(),
                        value: rig_stats.agentic_calls_total.to_string().into(),
                        unit: None,
                        info_tooltip: Some(RIG_MODE_INFO_TOOLTIP.into()),
                    }
                    StatCard {
                        title: "Fallbacks".into(),
                        value: rig_stats.agentic_fallbacks_total.to_string().into(),
                        unit: None,
                        info_tooltip: Some(RIG_FALLBACK_INFO_TOOLTIP.into()),
                    }
                    StatCard {
                        title: "Tool Calls".into(),
                        value: rig_stats.rig_tool_calls_total.to_string().into(),
                        unit: None,
                        info_tooltip: Some(RIG_MODE_INFO_TOOLTIP.into()),
                    }
                    StatCard {
                        title: "Fallback Rate".into(),
                        value: format!("{:.1}", rig_stats.fallback_rate_pct).into(),
                        unit: Some("%".into()),
                        info_tooltip: Some(RIG_FALLBACK_INFO_TOOLTIP.into()),
                    }
                }

                // Token budget section
                div { class: "mt-4 bg-gray-800/50 rounded p-4 space-y-3",
                    div { class: "flex items-center justify-between mb-1",
                        div { class: "flex items-center gap-2",
                            span { class: "text-xs font-semibold text-gray-300", "Context Budget (avg)" }
                            TokenBudgetInfoButton {}
                        }
                        span { class: "text-xs text-gray-400",
                            "Counter: "
                            span {
                                class: if rig_stats.counter_type.starts_with("exact") {
                                    "text-teal-400"
                                } else if rig_stats.counter_type.starts_with("heuristic") {
                                    "text-yellow-400"
                                } else {
                                    "text-gray-400"
                                },
                                "{rig_stats.counter_type}"
                            }
                        }
                    }

                    // Context utilization bar
                    {
                        let pct = rig_stats.avg_ctx_utilization_pct.min(100.0);
                        let bar_color = if pct > 80.0 {
                            "bg-red-500"
                        } else if pct > 60.0 {
                            "bg-yellow-500"
                        } else {
                            "bg-teal-500"
                        };
                        rsx! {
                            div { class: "space-y-1",
                                div { class: "flex items-center gap-2",
                                    div { class: "flex-1 h-4 bg-gray-700 rounded overflow-hidden",
                                        div {
                                            class: "h-full {bar_color} transition-all",
                                            style: "width: {pct:.1}%"
                                        }
                                    }
                                    span { class: "text-xs text-gray-300 w-12 text-right",
                                        "{pct:.1}%"
                                    }
                                }
                            }
                        }
                    }

                    div { class: "grid grid-cols-3 gap-4 mt-2",
                        div { class: "text-center",
                            div { class: "text-xs text-gray-500", "Avg tokens/session" }
                            div { class: "text-sm font-medium text-gray-200",
                                "{rig_stats.avg_tokens_in:.0}"
                            }
                        }
                        div { class: "text-center",
                            div { class: "text-xs text-gray-500", "Max tokens seen" }
                            div { class: "text-sm font-medium text-gray-200",
                                "{rig_stats.max_tokens_in}"
                            }
                        }
                        div { class: "text-center",
                            div { class: "text-xs text-gray-500", "Avg session" }
                            div { class: "text-sm font-medium text-gray-200",
                                "{rig_stats.avg_session_ms:.0} ms"
                            }
                        }
                    }

                    if rig_stats.token_sample_count == 0 {
                        div { class: "text-xs text-gray-500 italic mt-2",
                            "No agentic sessions recorded yet. Token stats appear after the first agentic query."
                        }
                    } else {
                        div { class: "text-xs text-gray-600 mt-1",
                            "Based on {rig_stats.token_sample_count} session(s)"
                        }
                    }
                }
            }

            // Decision Engine Section
            RowHeader {
                title: "Decision Engine".into(),
            }

            Panel { title: Some("Tool Performance".into()), refresh: Some("10s".into()),
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                    StatCard {
                        title: "Tool Executions".into(),
                        value: tool_stats.tool_executions.to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Avg Confidence".into(),
                        value: format!("{:.1}", tool_stats.avg_confidence).into(),
                        unit: Some("%".into()),
                    }
                    StatCard {
                        title: "Fallback Rate".into(),
                        value: format!("{:.1}", tool_stats.fallback_rate).into(),
                        unit: Some("%".into()),
                    }
                }

                // Tool Usage Distribution
                div { class: "mt-4",
                    RowHeader {
                        title: "Tool Usage Distribution".into(),
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
            }

            Panel { title: Some("Memory Statistics".into()), refresh: Some("10s".into()),
                div { class: "grid grid-cols-1 md:grid-cols-4 gap-4",
                    StatCard {
                        title: "Total Episodes".into(),
                        value: memory_stats.total_episodes.to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "RAG Memories".into(),
                        value: memory_stats.total_rag_memories.to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Unique Agents".into(),
                        value: memory_stats.unique_agents.to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Reflections".into(),
                        value: agent_stats.total_reflections.to_string().into(),
                        unit: None,
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

    rsx! {
        button {
            class: PARAM_ICON_BUTTON_CLASS,
            style: PARAM_ICON_BUTTON_STYLE,
            onclick: move |_| show_tooltip.set(!show_tooltip()),
            title: "Show info",
            svg {
                class: INFO_ICON_SVG_CLASS,
                view_box: "0 0 20 20",
                fill: "none",
                stroke: "currentColor",
                circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
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

/// Info button for Rig token budget / context window explanation
#[component]
fn TokenBudgetInfoButton() -> Element {
    let mut show_tooltip = use_signal(|| false);

    rsx! {
        button {
            class: PARAM_ICON_BUTTON_CLASS,
            style: PARAM_ICON_BUTTON_STYLE,
            onclick: move |_| show_tooltip.set(!show_tooltip()),
            title: "Context budget info",
            svg {
                class: INFO_ICON_SVG_CLASS,
                view_box: "0 0 20 20",
                fill: "none",
                stroke: "currentColor",
                circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
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
                        h2 { class: "text-lg font-semibold text-gray-100", "Context Budget" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_tooltip.set(false),
                            "×"
                        }
                    }
                    div {
                        class: "text-sm text-gray-300 whitespace-pre-line leading-relaxed font-mono",
                        {TOKEN_BUDGET_INFO_TOOLTIP}
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

    rsx! {
        button {
            class: PARAM_ICON_BUTTON_CLASS,
            style: PARAM_ICON_BUTTON_STYLE,
            onclick: move |_| show_tooltip.set(!show_tooltip()),
            title: "What is an Agent?",
            svg {
                class: INFO_ICON_SVG_CLASS,
                view_box: "0 0 20 20",
                fill: "none",
                stroke: "currentColor",
                circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
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
