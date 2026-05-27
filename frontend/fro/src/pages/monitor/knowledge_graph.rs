//! Monitor - Knowledge Graph visualization page

use crate::api;
use crate::app::Route;
use crate::components::monitor::NavTabs;
use crate::components::ActiveDropdown;
use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    pub document_count: usize,
    pub chunk_count: usize,
    pub entity_count: usize,
    pub relationship_count: usize,
    pub entity_types: Vec<EntityTypeCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeCount {
    pub entity_type: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub properties: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub label: String,
    pub properties: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[component]
pub fn MonitorKnowledgeGraph() -> Element {
    let mut stats = use_signal(GraphStats::default);
    let mut graph_data = use_signal(GraphData::default);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let mut selected_node_id = use_signal(|| None::<String>);
    let mut query_input = use_signal(String::new);
    let mut search_results = use_signal(Vec::<GraphNode>::new);
    let mut fullscreen = use_signal(|| false);
    let mut show_info = use_signal(|| false);

    // Get dropdown context to close it when entering fullscreen
    let mut active_dropdown = use_context::<Signal<ActiveDropdown>>();

    // Load initial data
    use_future(move || async move {
        loading.set(true);

        match api::fetch_graph_stats().await {
            Ok(s) => stats.set(s),
            Err(e) => error.set(Some(format!("Failed to load stats: {}", e))),
        }

        match api::fetch_graph_sample(50).await {
            Ok(data) => graph_data.set(data),
            Err(e) => {
                if error().is_none() {
                    error.set(Some(format!("Failed to load graph: {}", e)));
                }
            }
        }

        loading.set(false);
    });

    let do_search = move |_| {
        let query = query_input();
        if query.is_empty() {
            return;
        }
        spawn(async move {
            match api::search_graph_entities(&query).await {
                Ok(results) => search_results.set(results),
                Err(e) => error.set(Some(format!("Search failed: {}", e))),
            }
        });
    };

    // Find selected node from graph data or search results
    let selected_node = {
        let id = selected_node_id();
        if let Some(ref id) = id {
            graph_data()
                .nodes
                .iter()
                .find(|n| &n.id == id)
                .cloned()
                .or_else(|| search_results().iter().find(|n| &n.id == id).cloned())
        } else {
            None
        }
    };

    rsx! {
        // Info modal - compact legend with links
        if show_info() {
            div {
                class: "fixed inset-0 z-[10000] flex items-center justify-center bg-black/60",
                onclick: move |_| show_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-5 shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-start gap-8",
                        // Nodes column
                        div {
                            h3 { class: "text-sm font-semibold text-gray-300 mb-2", "Nodes:" }
                            div { class: "space-y-1",
                                a {
                                    href: "#",
                                    class: "flex items-center gap-2 text-sm hover:underline",
                                    onclick: move |e| { e.prevent_default(); show_info.set(false); },
                                    span { class: "w-3 h-3 rounded-full", style: "background-color: #3b82f6;" }
                                    span { class: "text-blue-400", "Doc" }
                                }
                                a {
                                    href: "#",
                                    class: "flex items-center gap-2 text-sm hover:underline",
                                    onclick: move |e| { e.prevent_default(); show_info.set(false); },
                                    span { class: "w-3 h-3 rounded-full", style: "background-color: #22c55e;" }
                                    span { class: "text-green-400", "Chunk" }
                                }
                                a {
                                    href: "#",
                                    class: "flex items-center gap-2 text-sm hover:underline",
                                    onclick: move |e| { e.prevent_default(); show_info.set(false); },
                                    span { class: "w-3 h-3 rounded-full", style: "background-color: #a855f7;" }
                                    span { class: "text-purple-400", "Entity" }
                                }
                            }
                        }
                        // Separator
                        div { class: "text-gray-300 text-2xl", "|" }
                        // Relations column
                        div {
                            h3 { class: "text-sm font-semibold text-gray-300 mb-2", "Relations:" }
                            div { class: "space-y-1",
                                a {
                                    href: "#",
                                    class: "flex items-center gap-2 text-sm hover:underline",
                                    onclick: move |e| { e.prevent_default(); show_info.set(false); },
                                    span { class: "w-4 h-1 rounded", style: "background-color: #f59e0b;" }
                                    span { style: "color: #f59e0b;", "HAS_CHUNK" }
                                }
                                a {
                                    href: "#",
                                    class: "flex items-center gap-2 text-sm hover:underline",
                                    onclick: move |e| { e.prevent_default(); show_info.set(false); },
                                    span { class: "w-4 h-1 rounded", style: "background-color: #10b981;" }
                                    span { style: "color: #10b981;", "MENTIONS" }
                                }
                                a {
                                    href: "#",
                                    class: "flex items-center gap-2 text-sm hover:underline",
                                    onclick: move |e| { e.prevent_default(); show_info.set(false); },
                                    span { class: "w-4 h-1 rounded", style: "background-color: #8b5cf6;" }
                                    span { style: "color: #8b5cf6;", "RELATED_TO" }
                                }
                                a {
                                    href: "#",
                                    class: "flex items-center gap-2 text-sm hover:underline",
                                    onclick: move |e| { e.prevent_default(); show_info.set(false); },
                                    span { class: "w-4 h-1 rounded", style: "background-color: #ec4899;" }
                                    span { style: "color: #ec4899;", "co_occurs" }
                                }
                            }
                        }
                    }
                }
            }
        }

        div { class: "p-6 space-y-6 w-full",
            NavTabs { active: Route::MonitorKnowledgeGraph {} }

            div { class: "mb-4",
                h1 { class: "text-xl font-bold text-gray-100", "Knowledge Graph" }
                p { class: "text-sm text-gray-400", "Visualize entities, documents, and relationships extracted from your documents" }
            }

            if loading() {
                div { class: "flex items-center justify-center py-12",
                    span { class: "loading loading-spinner loading-lg text-primary" }
                }
            } else if let Some(err) = error() {
                div { class: "alert alert-error",
                    span { "{err}" }
                }
            } else {
                // Stats cards
                div { class: "grid grid-cols-2 md:grid-cols-4 gap-4 mb-6",
                    StatCard { label: "Documents", value: stats().document_count }
                    StatCard { label: "Chunks", value: stats().chunk_count }
                    StatCard { label: "Entities", value: stats().entity_count }
                    StatCard { label: "Relationships", value: stats().relationship_count }
                }

                // Entity type breakdown
                if !stats().entity_types.is_empty() {
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 mb-6",
                        h3 { class: "text-sm font-semibold text-gray-200 mb-3", "Entity Types" }
                        div { class: "flex flex-wrap gap-2",
                            for et in stats().entity_types.iter() {
                                span {
                                    class: "px-2 py-1 bg-gray-700 rounded text-xs text-gray-300",
                                    "{et.entity_type}: {et.count}"
                                }
                            }
                        }
                    }
                }

                // Search bar
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 mb-6",
                    h3 { class: "text-sm font-semibold text-gray-200 mb-3", "Search Entities" }
                    div { class: "flex gap-2",
                        input {
                            r#type: "text",
                            class: "input input-sm input-bordered bg-gray-700 text-gray-200 flex-1",
                            placeholder: "Search for entities (e.g., 'Microsoft', 'Paris')...",
                            value: "{query_input}",
                            oninput: move |e| query_input.set(e.value()),
                            onkeypress: move |e| {
                                if e.key() == Key::Enter {
                                    do_search(());
                                }
                            },
                        }
                        button {
                            class: "btn btn-sm btn-primary",
                            onclick: move |_| do_search(()),
                            "Search"
                        }
                    }

                    // Search results
                    if !search_results().is_empty() {
                        div { class: "mt-4 space-y-2",
                            for node in search_results().iter() {
                                {
                                    let node_id = node.id.clone();
                                    let node_type = node.node_type.clone();
                                    let node_label = node.label.clone();
                                    rsx! {
                                        div {
                                            class: "p-2 bg-gray-700 rounded cursor-pointer hover:bg-gray-600",
                                            onclick: move |_| selected_node_id.set(Some(node_id.clone())),
                                            div { class: "flex items-center gap-2",
                                                span {
                                                    class: "px-2 py-0.5 text-xs rounded",
                                                    style: get_node_type_style(&node_type),
                                                    "{node_type}"
                                                }
                                                span { class: "text-gray-200", "{node_label}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Graph visualization - clickable header to toggle fullscreen
                div {
                    class: if fullscreen() {
                        "fixed inset-0 z-[9999] bg-gray-900 p-6 overflow-hidden"
                    } else {
                        "bg-gray-800 border border-gray-700 rounded-lg p-4"
                    },
                    // Header with fullscreen toggle
                    div { class: "flex items-center justify-between mb-3",
                        h3 {
                            class: if fullscreen() {
                                "text-lg font-semibold text-gray-200 cursor-pointer hover:text-primary flex items-center gap-2"
                            } else {
                                "text-sm font-semibold text-gray-200 cursor-pointer hover:text-primary flex items-center gap-2"
                            },
                            onclick: move |_| {
                                // Close any open dropdowns when toggling fullscreen
                                active_dropdown.set(ActiveDropdown(None));
                                fullscreen.set(!fullscreen());
                            },
                            "Graph Visualization"
                            if fullscreen() {
                                span { class: "text-sm text-gray-400 ml-2", "(click title to exit)" }
                            } else {
                                span { class: "text-xs text-gray-400", "(click for fullscreen)" }
                            }
                        }

                    }

                    // Legend - at the top, visible in both modes
                    div {
                        class: if fullscreen() {
                            "mb-4 p-3 bg-gray-800 rounded-lg border border-gray-700"
                        } else {
                            "mb-3 p-2 bg-gray-700/50 rounded-lg"
                        },
                        div { class: "flex flex-wrap gap-6 items-center justify-center",
                            // Info button (fullscreen only) - centered
                            if fullscreen() {
                                button {
                                    class: PARAM_ICON_BUTTON_CLASS,
                                    style: PARAM_ICON_BUTTON_STYLE,
                                    onclick: move |_| show_info.set(true),
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
                            }
                            // Nodes legend
                            div { class: "flex items-center gap-3",
                                span {
                                    class: if fullscreen() { "text-sm text-gray-400 font-medium" } else { "text-xs text-gray-300" },
                                    "Nodes:"
                                }
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: "rounded-full",
                                        style: if fullscreen() { "width: 12px; height: 12px; background-color: #3b82f6;" } else { "width: 10px; height: 10px; background-color: #3b82f6;" }
                                    }
                                    span {
                                        class: if fullscreen() { "text-sm text-gray-300" } else { "text-xs text-gray-400" },
                                        "Doc"
                                    }
                                }
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: "rounded-full",
                                        style: if fullscreen() { "width: 12px; height: 12px; background-color: #22c55e;" } else { "width: 10px; height: 10px; background-color: #22c55e;" }
                                    }
                                    span {
                                        class: if fullscreen() { "text-sm text-gray-300" } else { "text-xs text-gray-400" },
                                        "Chunk"
                                    }
                                }
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: "rounded-full",
                                        style: if fullscreen() { "width: 12px; height: 12px; background-color: #a855f7;" } else { "width: 10px; height: 10px; background-color: #a855f7;" }
                                    }
                                    span {
                                        class: if fullscreen() { "text-sm text-gray-300" } else { "text-xs text-gray-400" },
                                        "Entity"
                                    }
                                }
                            }
                            // Separator
                            span { class: "text-gray-300", "|" }
                            // Relationships legend
                            div { class: "flex items-center gap-2",
                                span {
                                    class: if fullscreen() { "text-sm text-gray-400 font-medium" } else { "text-xs text-gray-300" },
                                    "Relations:"
                                }
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: if fullscreen() { "w-4 h-0.5 rounded" } else { "w-3 h-0.5" },
                                        style: "background-color: #f59e0b;"
                                    }
                                    span {
                                        class: "text-xs",
                                        style: "color: #f59e0b;",
                                        "HAS_CHUNK"
                                    }
                                }
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: if fullscreen() { "w-4 h-0.5 rounded" } else { "w-3 h-0.5" },
                                        style: "background-color: #10b981;"
                                    }
                                    span {
                                        class: "text-xs",
                                        style: "color: #10b981;",
                                        "MENTIONS"
                                    }
                                }
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: if fullscreen() { "w-4 h-0.5 rounded" } else { "w-3 h-0.5" },
                                        style: "background-color: #8b5cf6;"
                                    }
                                    span {
                                        class: "text-xs",
                                        style: "color: #8b5cf6;",
                                        "RELATED_TO"
                                    }
                                }
                                div { class: "flex items-center gap-1",
                                    span {
                                        class: if fullscreen() { "w-4 h-0.5 rounded" } else { "w-3 h-0.5" },
                                        style: "background-color: #ec4899;"
                                    }
                                    span {
                                        class: "text-xs",
                                        style: "color: #ec4899;",
                                        "co_occurs"
                                    }
                                }
                            }
                        }
                    }

                    div { class: "flex gap-4",
                        // Graph canvas - larger size in fullscreen
                        div {
                            class: "flex-1 bg-gray-900 rounded-lg overflow-hidden",
                            style: if fullscreen() {
                                "height: calc(100vh - 120px);"
                            } else {
                                "height: calc(100vh - 400px); min-height: 600px;"
                            },

                            svg {
                                width: "100%",
                                height: "100%",
                                view_box: if fullscreen() { "0 0 1800 1000" } else { "0 0 1200 800" },

                                // Empty state message when no data
                                if graph_data().nodes.is_empty() {
                                    text {
                                        x: if fullscreen() { "900" } else { "600" },
                                        y: if fullscreen() { "400" } else { "300" },
                                        text_anchor: "middle",
                                        fill: "#6b7280",
                                        font_size: "24",
                                        "No graph data available"
                                    }
                                    text {
                                        x: if fullscreen() { "900" } else { "600" },
                                        y: if fullscreen() { "450" } else { "340" },
                                        text_anchor: "middle",
                                        fill: "#4b5563",
                                        font_size: "14",
                                        "Upload documents and enable FalkorDB integration to build the knowledge graph"
                                    }
                                    text {
                                        x: if fullscreen() { "900" } else { "600" },
                                        y: if fullscreen() { "490" } else { "375" },
                                        text_anchor: "middle",
                                        fill: "#4b5563",
                                        font_size: "12",
                                        "Go to Settings → FalkorDB to configure"
                                    }
                                }

                                // Draw edges
                                for edge in graph_data().edges.iter() {
                                    {render_edge_fs(edge, &graph_data().nodes, fullscreen())}
                                }

                                // Draw nodes
                                for (i, node) in graph_data().nodes.iter().enumerate() {
                                    {
                                        let node_id = node.id.clone();
                                        let node_label = node.label.clone();
                                        let is_selected = selected_node_id().as_ref() == Some(&node.id);
                                        let color = get_node_color(&node.node_type);
                                        // Spread nodes across larger canvas - more space in fullscreen
                                        let (cols, spacing_x, spacing_y) = if fullscreen() {
                                            (15, 115, 110)
                                        } else {
                                            (12, 95, 90)
                                        };
                                        let x = (i % cols) * spacing_x + 60;
                                        let y = (i / cols) * spacing_y + 50;
                                        let radius = if is_selected { 24 } else { 18 };
                                        let stroke = if is_selected { "white" } else { "transparent" };
                                        let stroke_width = if is_selected { 2 } else { 0 };
                                        let font_size = if fullscreen() { "12" } else { "10" };
                                        let text_offset = if fullscreen() { 35 } else { 30 };
                                        rsx! {
                                            g {
                                                onclick: move |_| selected_node_id.set(Some(node_id.clone())),
                                                style: "cursor: pointer;",
                                                circle {
                                                    cx: "{x}",
                                                    cy: "{y}",
                                                    r: "{radius}",
                                                    fill: "{color}",
                                                    stroke: "{stroke}",
                                                    stroke_width: "{stroke_width}",
                                                }
                                                text {
                                                    x: "{x}",
                                                    y: "{y + text_offset}",
                                                    text_anchor: "middle",
                                                    fill: "#d1d5db",
                                                    font_size: "{font_size}",
                                                    "{node_label}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Node details panel
                        if let Some(ref node) = selected_node {
                            div { class: "w-64 bg-gray-700 rounded-lg p-4",
                                h4 { class: "text-sm font-semibold text-gray-200 mb-2", "Node Details" }
                                div { class: "space-y-2 text-xs",
                                    div {
                                        span { class: "text-gray-400", "Type: " }
                                        span {
                                            class: "px-2 py-0.5 rounded",
                                            style: get_node_type_style(&node.node_type),
                                            "{node.node_type}"
                                        }
                                    }
                                    div {
                                        span { class: "text-gray-400", "Label: " }
                                        span { class: "text-gray-200", "{node.label}" }
                                    }
                                    div {
                                        span { class: "text-gray-400", "ID: " }
                                        span { class: "text-gray-300 font-mono text-xs", "{node.id}" }
                                    }
                                    if !node.properties.is_empty() {
                                        div { class: "mt-2 pt-2 border-t border-gray-600",
                                            span { class: "text-gray-400", "Properties:" }
                                            for (key, value) in node.properties.iter() {
                                                div { class: "ml-2 mt-1",
                                                    span { class: "text-gray-300", "{key}: " }
                                                    span { class: "text-gray-300", "{value}" }
                                                }
                                            }
                                        }
                                    }
                                }
                                button {
                                    class: "btn btn-xs btn-ghost mt-3 w-full",
                                    onclick: move |_| selected_node_id.set(None),
                                    "Close"
                                }
                            }
                        }
                    }

                }

                // FalkorDB query console link
                div { class: "mt-4 text-sm text-gray-400",
                    "For advanced queries, use the Cypher console on the "
                    a {
                        href: "/config/falkordb",
                        class: "text-blue-400 hover:text-blue-300 underline",
                        "FalkorDB settings page"
                    }
                    "."
                }
            }
        }
    }
}

#[component]
fn StatCard(label: &'static str, value: usize) -> Element {
    rsx! {
        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4",
            div { class: "text-2xl font-bold text-gray-100", "{value}" }
            div { class: "text-xs text-gray-400", "{label}" }
        }
    }
}

fn get_node_type_style(node_type: &str) -> &'static str {
    match node_type {
        "Document" => "background-color: #3b82f6; color: white;",
        "Chunk" => "background-color: #22c55e; color: white;",
        "Entity" => "background-color: #a855f7; color: white;",
        _ => "background-color: #6b7280; color: white;",
    }
}

fn get_node_color(node_type: &str) -> &'static str {
    match node_type {
        "Document" => "#3b82f6",
        "Chunk" => "#22c55e",
        "Entity" => "#a855f7",
        _ => "#6b7280",
    }
}

fn get_edge_color(rel_type: &str) -> &'static str {
    match rel_type {
        "HAS_CHUNK" => "#f59e0b",      // amber
        "MENTIONS" => "#10b981",       // emerald
        "RELATED_TO" => "#8b5cf6",     // violet
        "co_occurs_with" => "#ec4899", // pink
        _ => "#6b7280",                // gray
    }
}

fn render_edge_fs(edge: &GraphEdge, nodes: &[GraphNode], is_fullscreen: bool) -> Element {
    let from_idx = nodes.iter().position(|n| n.id == edge.from);
    let to_idx = nodes.iter().position(|n| n.id == edge.to);

    if let (Some(from_i), Some(to_i)) = (from_idx, to_idx) {
        // Match the node layout - different spacing for fullscreen
        let (cols, spacing_x, spacing_y) = if is_fullscreen {
            (15, 115, 110)
        } else {
            (12, 95, 90)
        };
        let x1 = (from_i % cols) * spacing_x + 60;
        let y1 = (from_i / cols) * spacing_y + 50;
        let x2 = (to_i % cols) * spacing_x + 60;
        let y2 = (to_i / cols) * spacing_y + 50;

        let color = get_edge_color(&edge.label);
        let stroke_width = if is_fullscreen { "2" } else { "1.5" };

        rsx! {
            line {
                x1: "{x1}",
                y1: "{y1}",
                x2: "{x2}",
                y2: "{y2}",
                stroke: "{color}",
                stroke_width: "{stroke_width}",
                stroke_opacity: "0.7",
            }
        }
    } else {
        rsx! {}
    }
}
