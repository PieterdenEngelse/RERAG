use crate::api;
use crate::api::ToolExecution;
use crate::app::Route;
use crate::components::monitor::*;
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use std::cmp::Ordering;

#[derive(Clone, Default)]
struct ToolsState {
    loading: bool,
    error: Option<String>,
    total_executions: usize,
    avg_confidence: f64,
    fallback_rate: f64,
    recent_executions: Vec<ToolExecution>,
    distribution: Vec<api::ToolUsageEntry>,
    cache_stats: Vec<api::ToolCacheStats>,
    rate_limits: Vec<api::ToolRateLimitStatus>,
    costs: Vec<api::ToolCostEntry>,
    cost_trends: Vec<api::ToolTrend>,
    dependencies: Option<api::ToolDependencyResponse>,
    available_tools: Vec<api::AvailableTool>,
    last_updated: Option<String>,
}

// Tools are now fetched from the backend API via /monitoring/tools/available
// See api::AvailableTool and api::fetch_available_tools()

const SPARKLINE_WIDTH: f64 = 80.0;
const SPARKLINE_HEIGHT: f64 = 22.0;

fn find_cost_sparkline<'a>(trends: &'a [api::ToolTrend], tool_type: &str) -> Option<Vec<f64>> {
    let trend = trends.iter().find(|trend| trend.tool_type == tool_type)?;
    if trend.buckets.is_empty() {
        return None;
    }
    let points: Vec<f64> = trend
        .buckets
        .iter()
        .map(|bucket| bucket.total_cost)
        .collect();
    if points.iter().all(|p| (*p - points[0]).abs() < f64::EPSILON) {
        // Flat line - still return constant values so sparkline renders
        return Some(points);
    }
    Some(points)
}

fn render_sparkline(points: &[f64]) -> Option<VNode> {
    if points.is_empty() {
        return None;
    }

    let max_val = points
        .iter()
        .cloned()
        .fold(f64::MIN, f64::max)
        .max(0.000_001);
    let min_val = points.iter().cloned().fold(f64::MAX, f64::min).min(max_val);

    let norm_points: Vec<(f64, f64)> = points
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            let x = if points.len() == 1 {
                SPARKLINE_WIDTH / 2.0
            } else {
                (idx as f64 / (points.len() - 1) as f64) * SPARKLINE_WIDTH
            };
            let normalized = if (max_val - min_val).abs() < f64::EPSILON {
                0.5
            } else {
                (value - min_val) / (max_val - min_val)
            };
            // invert Y (SVG origin at top)
            let y = SPARKLINE_HEIGHT - (normalized * SPARKLINE_HEIGHT);
            (x, y)
        })
        .collect();

    let path_data = norm_points
        .iter()
        .map(|(x, y)| format!("{:.1},{:.1}", x, y))
        .collect::<Vec<_>>()
        .join(" ");

    Some(
        rsx! {
            svg {
                class: "w-20 h-5 text-blue-400",
                width: "{SPARKLINE_WIDTH}",
                height: "{SPARKLINE_HEIGHT}",
                view_box: format!("0 0 {} {}", SPARKLINE_WIDTH, SPARKLINE_HEIGHT),
                polyline {
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "1.5",
                    points: path_data,
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                }
            }
        }
        .expect("sparkline render failed"),
    )
}

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
                {
                    let mut guard = state.write();
                    guard.loading = true;
                }

                let prev = state.read().clone();
                let mut next = prev.clone();
                next.loading = false;
                let mut errors = Vec::new();

                match api::fetch_tool_stats().await {
                    Ok(stats) => {
                        next.total_executions = stats.tool_executions;
                        next.avg_confidence = stats.avg_confidence;
                        next.fallback_rate = stats.fallback_rate;
                        next.distribution = stats.tool_distribution;
                        next.last_updated = Some(stats.timestamp);
                    }
                    Err(err) => {
                        errors.push(format!("stats: {}", err));
                    }
                }

                match api::fetch_tool_executions(50).await {
                    Ok(executions) => {
                        next.recent_executions = executions.executions;
                    }
                    Err(err) => errors.push(format!("executions: {}", err)),
                }

                match api::fetch_tool_cache_stats().await {
                    Ok(cache) => {
                        next.cache_stats = cache.caches;
                    }
                    Err(err) => errors.push(format!("cache: {}", err)),
                }

                match api::fetch_tool_rate_limits().await {
                    Ok(rate_limits) => {
                        next.rate_limits = rate_limits.statuses;
                    }
                    Err(err) => errors.push(format!("rate limits: {}", err)),
                }

                match api::fetch_tool_costs().await {
                    Ok(costs) => {
                        next.costs = costs.costs;
                    }
                    Err(err) => errors.push(format!("costs: {}", err)),
                }

                match api::fetch_tool_trends("day").await {
                    Ok(trends) => next.cost_trends = trends.trends,
                    Err(err) => errors.push(format!("trends: {}", err)),
                }

                match api::fetch_tool_dependencies().await {
                    Ok(graph) => next.dependencies = Some(graph),
                    Err(err) => errors.push(format!("dependencies: {}", err)),
                }

                match api::fetch_available_tools().await {
                    Ok(available) => next.available_tools = available.tools,
                    Err(err) => errors.push(format!("available tools: {}", err)),
                }

                next.error = if errors.is_empty() {
                    None
                } else {
                    Some(errors.join(" | "))
                };

                state.set(next);
                TimeoutFuture::new(5_000).await;
            }
        });
    }

    let snapshot = state.read().clone();
    let tool_count = if snapshot.available_tools.is_empty() { 18 } else { snapshot.available_tools.len() };
    let active_tool_count = snapshot.available_tools.iter().filter(|t| t.status == "active").count();
    let enabled_caches = snapshot.cache_stats.iter().filter(|c| c.enabled).count();
    let total_cache_entries: usize = snapshot.cache_stats.iter().map(|c| c.current_entries).sum();
    let avg_cache_hit_rate = if snapshot.cache_stats.is_empty() {
        0.0
    } else {
        snapshot.cache_stats.iter().map(|c| c.hit_rate).sum::<f64>()
            / snapshot.cache_stats.len() as f64
    };
    let avg_cache_hit_rate_pct = avg_cache_hit_rate * 100.0;

    let enabled_rate_limits = snapshot.rate_limits.iter().filter(|r| r.enabled).count();
    let avg_rate_utilization = if snapshot.rate_limits.is_empty() {
        0.0
    } else {
        snapshot
            .rate_limits
            .iter()
            .map(|r| r.utilization)
            .sum::<f64>()
            / snapshot.rate_limits.len() as f64
    };
    let avg_rate_utilization_pct = avg_rate_utilization * 100.0;

    let total_tool_cost: f32 = snapshot.costs.iter().map(|c| c.total_cost).sum();
    let total_cost_executions: usize = snapshot.costs.iter().map(|c| c.executions).sum();
    let top_cost_entry = snapshot.costs.iter().max_by(|a, b| {
        a.total_cost
            .partial_cmp(&b.total_cost)
            .unwrap_or(Ordering::Equal)
    });
    let top_cost_tool_label = top_cost_entry
        .map(|entry| format!("{} (${:.2})", entry.tool_type, entry.total_cost))
        .unwrap_or_else(|| "n/a".into());
    let top_cost_sparkline = top_cost_entry
        .and_then(|entry| find_cost_sparkline(&snapshot.cost_trends, &entry.tool_type));

    let dependency_nodes = snapshot
        .dependencies
        .as_ref()
        .map(|d| d.graph.nodes.len())
        .unwrap_or(0);
    let dependency_edges = snapshot
        .dependencies
        .as_ref()
        .map(|d| d.graph.edges.len())
        .unwrap_or(0);

    let mut top_dependency_edges: Vec<api::ToolDependencyEdge> = snapshot
        .dependencies
        .as_ref()
        .map(|deps| deps.graph.edges.clone())
        .unwrap_or_default();
    top_dependency_edges.sort_by(|a, b| b.count.cmp(&a.count));
    top_dependency_edges.truncate(6);

    let mut top_dependency_nodes: Vec<api::ToolDependencyNode> = snapshot
        .dependencies
        .as_ref()
        .map(|deps| deps.graph.nodes.clone())
        .unwrap_or_default();
    top_dependency_nodes.sort_by(|a, b| b.executions.cmp(&a.executions));
    top_dependency_nodes.truncate(6);

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
                description: Some(format!("{active_tool_count} active / {tool_count} total tools in the agent tool registry").into()),
            }

            div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
                if snapshot.available_tools.is_empty() {
                    div { class: "col-span-3 text-gray-400 text-sm py-4", "Loading tools..." }
                } else {
                    for tool in snapshot.available_tools.iter() {
                        {
                            let tool_name = tool.name.clone();
                            let tool_icon = tool.icon.clone();
                            let tool_desc = tool.description.clone();
                            let tool_status = tool.status.clone();
                            let tool_category = tool.category.clone();
                            rsx! {
                                div {
                                    class: "bg-gray-800 border border-gray-700 rounded-lg p-4 hover:border-gray-500 transition-colors cursor-pointer",
                                    onclick: {
                                        let name = tool_name.clone();
                                        move |_| {
                                            selected_tool.set(Some(name.clone()));
                                            show_tool_info.set(true);
                                        }
                                    },
                                    div { class: "flex items-start justify-between mb-3",
                                        div { class: "flex items-center gap-3",
                                            span { class: "text-2xl", "{tool_icon}" }
                                            div {
                                                h3 { class: "text-white font-semibold", "{tool_name}" }
                                                p { class: "text-gray-400 text-xs", "{tool_desc}" }
                                            }
                                        }
                                        span {
                                            class: match tool_status.as_str() {
                                                "active" => "w-2 h-2 rounded-full bg-green-500",
                                                "placeholder" => "w-2 h-2 rounded-full bg-yellow-500",
                                                _ => "w-2 h-2 rounded-full bg-red-500",
                                            },
                                        }
                                    }
                                    div { class: "flex items-center justify-between text-xs text-gray-500",
                                        span {
                                            class: match tool_status.as_str() {
                                                "active" => "text-green-400",
                                                "placeholder" => "text-yellow-400",
                                                _ => "text-red-400",
                                            },
                                            match tool_status.as_str() {
                                                "active" => "● Ready",
                                                "placeholder" => "○ Placeholder",
                                                _ => "✕ Disabled",
                                            }
                                        }
                                        span { class: "text-gray-600", "{tool_category}" }
                                    }
                                }
                            }
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
                        div { class: "text-2xl font-bold text-white", "{tool_count}" }
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

            // Usage Distribution
            RowHeader {
                title: "Usage Distribution".into(),
                description: Some("Share of tool invocations across agent episodes".into()),
            }

            Panel { title: Some("Tool Usage".into()), refresh: Some("5s".into()),
                if snapshot.distribution.is_empty() {
                    div { class: "text-gray-400 text-sm py-4",
                        "No tool usage distribution available yet."
                    }
                } else {
                    div { class: "space-y-3",
                        for entry in snapshot.distribution.iter() {
                            div { class: "bg-gray-800/60 rounded p-3",
                                div { class: "flex items-center justify-between text-sm",
                                    span { class: "text-white font-semibold", "{entry.tool_name}" }
                                    span { class: "text-gray-400 font-mono",
                                        {
                                            format!("{:.1}% ({} execs)", entry.percentage, entry.count)
                                        }
                                    }
                                }
                                div { class: "w-full h-2 bg-gray-900 rounded mt-2 overflow-hidden",
                                    div {
                                        class: "h-full bg-blue-500 rounded",
                                        style: format!(
                                            "width: {:.2}%;",
                                            entry.percentage.min(100.0).max(0.0)
                                        ),
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Cache stats
            RowHeader {
                title: "Cache Performance".into(),
                description: Some("Per-tool cache layers, hit rate, and resource usage".into()),
            }

            Panel { title: Some("Cache Layers".into()), refresh: Some("5s".into()),
                if snapshot.cache_stats.is_empty() {
                    div { class: "text-gray-400 text-sm py-4",
                        "Cache metrics not reported yet. Ensure /monitoring/tools/cache is reachable."
                    }
                } else {
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4 mb-4",
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Active caches" }
                            div { class: "text-2xl font-bold text-white", "{enabled_caches}" }
                        }
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Entries tracked" }
                            div { class: "text-2xl font-bold text-blue-300", "{total_cache_entries}" }
                        }
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Avg hit rate" }
                            div { class: "text-2xl font-bold text-green-400",
                                {
                                    format!("{avg_cache_hit_rate_pct:.1}%")
                                }
                            }
                        }
                    }

                    div { class: "overflow-x-auto",
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800 text-xs",
                                tr {
                                    th { class: "py-2 px-2", "Tool" }
                                    th { class: "py-2 px-2", "Status" }
                                    th { class: "py-2 px-2", "TTL" }
                                    th { class: "py-2 px-2", "Entries" }
                                    th { class: "py-2 px-2", "Hit Rate" }
                                }
                            }
                            tbody {
                                for stats in snapshot.cache_stats.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0 hover:bg-gray-800/30",
                                        td { class: "py-2 px-2 text-white font-medium", "{stats.tool_type}" }
                                        td { class: "py-2 px-2",
                                            span {
                                                class: if stats.enabled { "px-2 py-0.5 rounded text-xs bg-green-900/50 text-green-300" } else { "px-2 py-0.5 rounded text-xs bg-gray-700 text-gray-300" },
                                                if stats.enabled { "Active" } else { "Disabled" }
                                            }
                                        }
                                        td { class: "py-2 px-2 text-gray-400",
                                            if stats.ttl_secs == 0 {
                                                "—"
                                            } else {
                                                {
                                                    format!("{}s", stats.ttl_secs)
                                                }
                                            }
                                        }
                                        td { class: "py-2 px-2 text-gray-400",
                                            {
                                                format!("{}/{}", stats.current_entries, stats.max_entries)
                                            }
                                        }
                                        td { class: "py-2 px-2",
                                            div { class: "flex items-center gap-2",
                                                div { class: "w-32 h-2 bg-gray-900 rounded overflow-hidden",
                                                    div {
                                                        class: "h-full bg-green-500",
                                                        style: format!("width: {:.2}%;", (stats.hit_rate * 100.0).min(100.0).max(0.0)),
                                                    }
                                                }
                                                span { class: "text-gray-300 text-xs font-mono",
                                                    {
                                                    format!("{:.1}%", stats.hit_rate * 100.0)
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

            // Rate limits
            RowHeader {
                title: "Rate Limit Status".into(),
                description: Some("Per-tool token buckets and utilization".into()),
            }

            Panel { title: Some("Rate Limiters".into()), refresh: Some("5s".into()),
                if snapshot.rate_limits.is_empty() {
                    div { class: "text-gray-400 text-sm py-4",
                        "No rate limiter data available. Check /monitoring/tools/rate-limits."
                    }
                } else {
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4 mb-4",
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Enabled limiters" }
                            div { class: "text-2xl font-bold text-white", "{enabled_rate_limits}" }
                        }
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Avg utilization" }
                            div { class: "text-2xl font-bold text-yellow-300",
                                {
                                    format!("{avg_rate_utilization_pct:.1}%")
                                }
                            }
                        }
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Telemetry" }
                            div { class: "text-sm text-gray-200", "Updates every 5s" }
                        }
                    }

                    div { class: "overflow-x-auto",
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800 text-xs",
                                tr {
                                    th { class: "py-2 px-2", "Tool" }
                                    th { class: "py-2 px-2", "Enabled" }
                                    th { class: "py-2 px-2", "Window" }
                                    th { class: "py-2 px-2", "Tokens" }
                                    th { class: "py-2 px-2", "Utilization" }
                                }
                            }
                            tbody {
                                for status in snapshot.rate_limits.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0 hover:bg-gray-800/30",
                                        td { class: "py-2 px-2 text-white font-medium", "{status.tool_type}" }
                                        td { class: "py-2 px-2",
                                            span {
                                                class: if status.enabled { "px-2 py-0.5 rounded text-xs bg-green-900/50 text-green-300" } else { "px-2 py-0.5 rounded text-xs bg-gray-700 text-gray-300" },
                                                if status.enabled { "On" } else { "Off" }
                                            }
                                        }
                                        td { class: "py-2 px-2 text-gray-400",
                                            {
                                                format!("{} req / {}s", status.max_requests, status.window_secs)
                                            }
                                        }
                                        td { class: "py-2 px-2 text-gray-400",
                                            {
                                                format!("{:.0}/{:.0}", status.tokens_available, status.tokens_max)
                                            }
                                        }
                                        td { class: "py-2 px-2",
                                            div { class: "flex items-center gap-2",
                                                div { class: "w-32 h-2 bg-gray-900 rounded overflow-hidden",
                                                    div {
                                                        class: format!("h-full {}", if status.utilization > 0.8 { "bg-red-500" } else if status.utilization > 0.6 { "bg-yellow-400" } else { "bg-green-500" }),
                                                        style: format!("width: {:.2}%;", (status.utilization * 100.0).min(100.0).max(0.0)),
                                                    }
                                                }
                                                span { class: "text-gray-300 text-xs font-mono",
                                                    {
                                                        format!("{:.1}%", status.utilization * 100.0)
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

            // Cost tracking
            RowHeader {
                title: "Cost Tracking".into(),
                description: Some("Execution-cost accounting per tool".into()),
            }

            Panel { title: Some("Tool Costs".into()), refresh: Some("5s".into()),
                if snapshot.costs.is_empty() {
                    div { class: "text-gray-400 text-sm py-4",
                        "No cost metadata recorded yet. Ensure cost metadata is emitted from tool executor."
                    }
                } else {
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4 mb-4",
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Total cost" }
                            div { class: "text-2xl font-bold text-white",
                                {
                                    format!("${:.2}", total_tool_cost)
                                }
                            }
                        }
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Executions tracked" }
                            div { class: "text-2xl font-bold text-blue-300", "{total_cost_executions}" }
                        }
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Top spender" }
                            div { class: "text-base font-semibold text-orange-300", "{top_cost_tool_label}" }
                            if let Some(ref points) = top_cost_sparkline {
                                if let Some(svg) = render_sparkline(points.as_slice()) {
                                    div { class: "mt-2 text-blue-300", {svg} }
                                }
                            }
                        }
                    }

                    div { class: "overflow-x-auto",
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800 text-xs",
                                tr {
                                    th { class: "py-2 px-2", "Tool" }
                                    th { class: "py-2 px-2", "Total Cost" }
                                    th { class: "py-2 px-2", "Executions" }
                                    th { class: "py-2 px-2", "Avg Cost" }
                                    th { class: "py-2 px-2", "Trend (24h)" }
                                    th { class: "py-2 px-2", "Last Updated" }
                                }
                            }
                            tbody {
                                for entry in snapshot.costs.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0 hover:bg-gray-800/30",
                                        td { class: "py-2 px-2 text-white font-medium", "{entry.tool_type}" }
                                        td { class: "py-2 px-2 text-gray-300",
                                            {
                                                format!("${:.2}", entry.total_cost)
                                            }
                                        }
                                        td { class: "py-2 px-2 text-gray-400", "{entry.executions}" }
                                        td { class: "py-2 px-2 text-gray-300",
                                            {
                                                format!("${:.4}", entry.avg_cost)
                                            }
                                        }
                                        td { class: "py-2 px-2",
                                            if let Some(points) = find_cost_sparkline(&snapshot.cost_trends, &entry.tool_type) {
                                                if let Some(svg) = render_sparkline(points.as_slice()) {
                                                    div { class: "text-blue-300", {svg} }
                                                } else {
                                                    span { class: "text-gray-500 text-xs", "—" }
                                                }
                                            } else {
                                                span { class: "text-gray-500 text-xs", "—" }
                                            }
                                        }
                                        td { class: "py-2 px-2 text-gray-500 text-xs", "{entry.last_updated}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Dependency graph
            RowHeader {
                title: "Tool Dependency Graph".into(),
                description: Some("Sequence edges observed in ToolChain executions".into()),
            }

            Panel { title: Some("Execution Graph".into()), refresh: Some("5s".into()),
                if let Some(_) = snapshot.dependencies {
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4 mb-4",
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Tools observed" }
                            div { class: "text-2xl font-bold text-white", "{dependency_nodes}" }
                        }
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Edges recorded" }
                            div { class: "text-2xl font-bold text-blue-300", "{dependency_edges}" }
                        }
                        div { class: "bg-gray-700/50 rounded p-4",
                            div { class: "text-xs text-gray-400", "Graph refresh" }
                            div { class: "text-base text-gray-200", "5s polling" }
                        }
                    }

                    div { class: "grid grid-cols-1 lg:grid-cols-2 gap-4",
                        // Top tools
                        div { class: "bg-gray-800/60 rounded p-4",
                            h3 { class: "text-sm font-semibold text-gray-200 mb-3", "Most active tools" }
                            if top_dependency_nodes.is_empty() {
                                div { class: "text-gray-400 text-xs", "No dependency data yet." }
                            } else {
                                table { class: "w-full text-sm",
                                    thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800 text-xs",
                                        tr {
                                            th { class: "py-2 px-2", "Tool" }
                                            th { class: "py-2 px-2", "Executions" }
                                        }
                                    }
                                    tbody {
                                        for node in top_dependency_nodes.iter() {
                                            tr { class: "border-b border-gray-800 last:border-0",
                                                td { class: "py-2 px-2 text-white", "{node.tool_type}" }
                                                td { class: "py-2 px-2 text-gray-300", "{node.executions}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Top edges
                        div { class: "bg-gray-800/60 rounded p-4",
                            h3 { class: "text-sm font-semibold text-gray-200 mb-3", "Top tool-to-tool hops" }
                            if top_dependency_edges.is_empty() {
                                div { class: "text-gray-400 text-xs", "Edges will appear once agents chain multiple tools." }
                            } else {
                                table { class: "w-full text-sm",
                                    thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800 text-xs",
                                        tr {
                                            th { class: "py-2 px-2", "From" }
                                            th { class: "py-2 px-2", "→" }
                                            th { class: "py-2 px-2", "To" }
                                            th { class: "py-2 px-2", "Count" }
                                        }
                                    }
                                    tbody {
                                        for edge in top_dependency_edges.iter() {
                                            tr { class: "border-b border-gray-800 last:border-0",
                                                td { class: "py-2 px-2 text-white", "{edge.from}" }
                                                td { class: "py-2 px-2 text-center text-gray-500", "→" }
                                                td { class: "py-2 px-2 text-white", "{edge.to}" }
                                                td { class: "py-2 px-2 text-gray-300", "{edge.count}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "text-gray-400 text-sm py-4",
                        "Dependency graph is warming up. Allow a multi-tool reasoning run or check /monitoring/tools/dependencies."
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

                        // Find the tool info from API data
                        {
                            let tool = snapshot.available_tools.iter().find(|t| t.name == tool_name);
                            if let Some(t) = tool {
                                let t_icon = t.icon.clone();
                                let t_name = t.name.clone();
                                let t_desc = t.description.clone();
                                let t_status = t.status.clone();
                                let t_category = t.category.clone();
                                rsx! {
                                    div { class: "flex items-center justify-between mb-4",
                                        div { class: "flex items-center gap-3",
                                            span { class: "text-3xl", "{t_icon}" }
                                            h2 { class: "text-lg font-semibold text-gray-100", "{t_name}" }
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
                                            p { class: "text-sm text-gray-300", "{t_desc}" }
                                        }

                                        div {
                                            div { class: "text-xs text-gray-500 uppercase tracking-wide mb-1", "Category" }
                                            span { class: "px-2 py-1 rounded text-xs bg-gray-700 text-gray-300 capitalize", "{t_category}" }
                                        }

                                        div {
                                            div { class: "text-xs text-gray-500 uppercase tracking-wide mb-1", "Status" }
                                            span {
                                                class: match t_status.as_str() {
                                                    "active" => "px-2 py-1 rounded text-xs bg-green-900/50 text-green-300",
                                                    "placeholder" => "px-2 py-1 rounded text-xs bg-yellow-900/50 text-yellow-300",
                                                    _ => "px-2 py-1 rounded text-xs bg-red-900/50 text-red-300",
                                                },
                                                match t_status.as_str() {
                                                    "active" => "Active - Fully implemented",
                                                    "placeholder" => "Placeholder - Requires external API/setup",
                                                    _ => "Disabled",
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
