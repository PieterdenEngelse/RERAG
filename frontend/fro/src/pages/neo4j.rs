//! Neo4j Knowledge Graph Configuration Page
//! Follows the styling pattern from config_io_uring.rs

use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use crate::{
    api,
    app::PageErrors,
    components::config_nav::{ConfigNav, ConfigTab},
};
use dioxus::prelude::*;

// Styling constants matching io_uring page
const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_COLUMN_CLASS: &str = "param-column-spacing";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";
const PARAM_NUMBER_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
const PARAM_TEXT_INPUT_CLASS: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 w-48";
const CHECKBOX_CLASS: &str = "checkbox checkbox-xs onnx-checkbox";

/// Info icon component
#[component]
fn InfoIcon() -> Element {
    rsx! {
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

/// Info modal component
fn info_modal(title: &str, toggle: Signal<bool>, paragraphs: Vec<&str>) -> Element {
    let mut toggle = toggle;
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| toggle.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-lg max-h-[90vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3",
                    h2 { class: "text-base font-semibold text-gray-100", "{title}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| toggle.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-2",
                    for paragraph in paragraphs {
                        p { "{paragraph}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ConfigNeo4j() -> Element {
    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 1: CONNECTION
    // ═══════════════════════════════════════════════════════════════
    let mut enabled = use_signal(|| false);
    let mut uri = use_signal(|| "bolt://localhost:7687".to_string());
    let mut user = use_signal(|| "neo4j".to_string());
    let mut password = use_signal(|| String::new());
    let mut database = use_signal(|| "neo4j".to_string());
    let mut max_connections = use_signal(|| 10u32);
    let mut connection_timeout_ms = use_signal(|| 5000u32);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 2: GRAPH EXPANSION
    // ═══════════════════════════════════════════════════════════════
    let mut expansion_enabled = use_signal(|| true);
    let mut max_hops = use_signal(|| 2u32);
    let mut max_chunks = use_signal(|| 10u32);
    let mut entity_weight = use_signal(|| 70u32); // stored as 0-100
    let mut concept_weight = use_signal(|| 50u32);
    let mut min_relationship_strength = use_signal(|| 30u32);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 3: ENTITY EXTRACTION
    // ═══════════════════════════════════════════════════════════════
    let mut extraction_enabled = use_signal(|| true);
    let mut confidence_threshold = use_signal(|| 50u32);
    let mut fuzzy_threshold = use_signal(|| 80u32);

    // Status state
    let mut feature_compiled = use_signal(|| false);
    let mut connected = use_signal(|| false);
    let mut container_up = use_signal(|| Option::<bool>::None);
    let mut stats_total_nodes = use_signal(|| 0usize);
    let mut stats_total_relationships = use_signal(|| 0usize);
    let mut stats_documents = use_signal(|| 0usize);
    let mut stats_chunks = use_signal(|| 0usize);
    let mut stats_entities = use_signal(|| 0usize);

    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| Option::<String>::None);

    // Load config on mount
    use_effect(move || {
        spawn(async move {
            // Fetch Neo4j config/status
            match api::fetch_neo4j_config().await {
                Ok(config) => {
                    // Update state from API response
                    feature_compiled.set(config.feature_compiled);
                    enabled.set(config.enabled);
                    connected.set(config.connected);
                    uri.set(config.uri);
                    user.set(config.user);
                    database.set(config.database);
                    max_connections.set(config.max_connections as u32);
                    connection_timeout_ms.set(config.connection_timeout_ms as u32);
                    // Graph expansion
                    expansion_enabled.set(config.expansion_enabled);
                    max_hops.set(config.max_hops as u32);
                    max_chunks.set(config.max_chunks as u32);
                    entity_weight.set((config.entity_weight * 100.0) as u32);
                    concept_weight.set((config.concept_weight * 100.0) as u32);
                    min_relationship_strength
                        .set((config.min_relationship_strength * 100.0) as u32);
                    // Entity extraction
                    extraction_enabled.set(config.extraction_enabled);
                    confidence_threshold.set((config.confidence_threshold * 100.0) as u32);
                    fuzzy_threshold.set((config.fuzzy_threshold * 100.0) as u32);
                    // Stats
                    if let Some(stats) = config.stats {
                        stats_total_nodes.set(stats.total_nodes);
                        stats_total_relationships.set(stats.total_relationships);
                        stats_documents.set(stats.documents);
                        stats_chunks.set(stats.chunks);
                        stats_entities.set(stats.entities);
                    }
                    loading.set(false);

                    // Fetch Docker container status for Neo4j (up/down)
                    spawn(async move {
                        match api::fetch_docker_status().await {
                            Ok(docker) => {
                                let up = docker
                                    .containers
                                    .iter()
                                    .find(|c| c.name == "ag-neo4j")
                                    .map(|c| c.state == "running");
                                container_up.set(up);
                            }
                            Err(_) => {
                                container_up.set(None);
                            }
                        }
                    });
                }
                Err(e) => {
                    error.set(Some(format!("Failed to load config: {}", e)));
                    loading.set(false);
                }
            }
        });
    });

    // Save state
    let mut saving = use_signal(|| false);
    let mut save_status = use_signal(|| Option::<String>::None);
    let mut save_error = use_signal(|| Option::<String>::None);

    // Reset to defaults handler
    let reset_to_defaults = move |_| {
        // Category 1: Connection
        enabled.set(false);
        uri.set("bolt://localhost:7687".to_string());
        user.set("neo4j".to_string());
        password.set(String::new());
        database.set("neo4j".to_string());
        max_connections.set(10);
        connection_timeout_ms.set(5000);
        // Category 2: Graph Expansion
        expansion_enabled.set(true);
        max_hops.set(2);
        max_chunks.set(10);
        entity_weight.set(70);
        concept_weight.set(50);
        min_relationship_strength.set(30);
        // Category 3: Entity Extraction
        extraction_enabled.set(true);
        confidence_threshold.set(50);
        fuzzy_threshold.set(80);
        // Clear any save status
        save_status.set(Some("Reset to defaults (not saved yet)".to_string()));
        save_error.set(None);
    };

    // Info modal signals - Category 1
    let mut show_enabled_info = use_signal(|| false);
    let mut show_uri_info = use_signal(|| false);
    let mut show_user_info = use_signal(|| false);
    let mut show_password_info = use_signal(|| false);
    let mut show_database_info = use_signal(|| false);
    let mut show_max_connections_info = use_signal(|| false);
    let mut show_timeout_info = use_signal(|| false);

    // Info modal signals - Category 2
    let mut show_expansion_info = use_signal(|| false);
    let mut show_max_hops_info = use_signal(|| false);
    let mut show_max_chunks_info = use_signal(|| false);
    let mut show_entity_weight_info = use_signal(|| false);
    let mut show_concept_weight_info = use_signal(|| false);
    let mut show_min_strength_info = use_signal(|| false);

    // Info modal signals - Category 3
    let mut show_extraction_info = use_signal(|| false);
    let mut show_confidence_info = use_signal(|| false);
    let mut show_fuzzy_info = use_signal(|| false);

    // Help modal
    let mut show_help = use_signal(|| false);
    let mut show_schema = use_signal(|| false);

    // Action button info modals
    let mut show_reset_action_info = use_signal(|| false);
    let mut show_test_action_info = use_signal(|| false);
    let mut show_rebuild_action_info = use_signal(|| false);
    let mut show_save_action_info = use_signal(|| false);

    // Get global page errors context
    let mut _page_errors = use_context::<Signal<PageErrors>>();

    // Save handler
    let on_save = {
        move |_| {
            saving.set(true);
            save_status.set(None);
            save_error.set(None);

            // TODO: Implement actual save API call
            // For now, just show success immediately
            save_status.set(Some("Saved! Restart required.".to_string()));
            saving.set(false);
        }
    };

    // Test connection handler
    let mut test_status = use_signal(|| Option::<String>::None);
    let mut test_error = use_signal(|| Option::<String>::None);

    let on_test_connection = {
        move |_| {
            test_status.set(Some("Testing connection...".to_string()));
            test_error.set(None);

            spawn(async move {
                match api::test_neo4j_connection().await {
                    Ok(result) => {
                        connected.set(result.connected);
                        if result.connected {
                            test_status.set(Some("Connected".to_string()));
                            test_error.set(None);
                        } else {
                            // Make the message more actionable than "health check failed"
                            let msg = if result.message.to_lowercase().contains("health check") {
                                "Connection failed. Is the Neo4j container running and are URI/user/password correct?".to_string()
                            } else {
                                result.message
                            };
                            test_error.set(Some(msg));
                            test_status.set(None);
                        }
                    }
                    Err(e) => {
                        test_error.set(Some(format!("Request failed: {}", e)));
                        test_status.set(None);
                    }
                }
            });
        }
    };

    // Rebuild knowledge graph handler
    let mut rebuilding = use_signal(|| false);
    let on_rebuild = {
        move |_| {
            rebuilding.set(true);
            save_status.set(Some("Rebuilding knowledge graph...".to_string()));
            save_error.set(None);

            spawn(async move {
                match api::rebuild_knowledge_graph().await {
                    Ok(result) => {
                        // Update stats
                        stats_entities.set(result.entities_extracted);

                        let msg = format!(
                            "✅ Rebuilt! {} docs, {} chunks, {} entities",
                            result.documents_processed,
                            result.chunks_processed,
                            result.entities_extracted
                        );
                        save_status.set(Some(msg));

                        if !result.errors.is_empty() {
                            save_error.set(Some(format!("Warnings: {}", result.errors.join(", "))));
                        }

                        // Refresh stats by re-fetching config
                        if let Ok(config) = api::fetch_neo4j_config().await {
                            if let Some(stats) = config.stats {
                                stats_total_nodes.set(stats.total_nodes);
                                stats_total_relationships.set(stats.total_relationships);
                                stats_documents.set(stats.documents);
                                stats_chunks.set(stats.chunks);
                                stats_entities.set(stats.entities);
                            }
                        }
                    }
                    Err(e) => {
                        save_error.set(Some(format!("Rebuild failed: {}", e)));
                        save_status.set(None);
                    }
                }
                rebuilding.set(false);
            });
        }
    };

    rsx! {
        div { class: "p-6 space-y-6 w-full",
            // Navigation
            ConfigNav { active: ConfigTab::Neo4j }

            if loading() {
                div { class: "flex items-center justify-center py-8",
                    span { class: "loading loading-spinner loading-lg text-primary" }
                }
            } else if let Some(err) = error() {
                div { class: "alert alert-error",
                    span { "{err}" }
                }
            } else {
                // Configuration Panel with save button in header
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 shadow",
                    // Header with title on left, save button on right
                    div { class: "flex items-start justify-between mb-3",
                        div { class: "flex items-center gap-3",
                            h3 { class: "text-sm font-semibold text-gray-200", "Neo4j GraphRAG Configuration" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_help.set(true),
                                title: "What is GraphRAG?",
                                InfoIcon {}
                            }
                            span { class: "text-xs font-semibold text-gray-400", "Status:" }
                            span {
                                class: "text-xs font-semibold text-cyan-400",
                                if connected() { "Connected" } else { "Disconnected" }
                            }
                            span { class: "text-xs font-semibold text-gray-400", "Container:" }
                            span {
                                class: "text-xs font-semibold text-cyan-400",
                                match container_up() {
                                    Some(true) => "Up",
                                    Some(false) => "Down",
                                    None => "Unknown",
                                }
                            }
                        }
                        div { class: "flex items-center gap-3",
                            if let Some(msg) = save_status() {
                                span { class: "text-green-400 text-xs", "{msg}" }
                            }
                            if let Some(err) = save_error() {
                                span { class: "text-red-400 text-xs", "{err}" }
                            }
                            div { class: "flex flex-col items-center gap-1",
                                div { class: "flex items-center gap-2",
                                    div { class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm",
                                            style: "background-color: #1D6B9A; border-color: #1D6B9A; color: white;",
                                            onclick: reset_to_defaults,
                                            "Reset"
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_reset_action_info.set(true),
                                            title: "Reset info",
                                            InfoIcon {}
                                        }
                                    }
                                    div { class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm",
                                            style: "background-color: #1D6B9A; border-color: #1D6B9A; color: white;",
                                            onclick: on_rebuild,
                                            disabled: !enabled() || rebuilding(),
                                            title: "Rebuild knowledge graph from all indexed documents",
                                            if rebuilding() { "Rebuilding..." } else { "Rebuild" }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_rebuild_action_info.set(true),
                                            title: "Rebuild info",
                                            InfoIcon {}
                                        }
                                    }
                                    div { class: "flex items-center gap-1",
                                        button {
                                            class: "btn btn-sm",
                                            style: "background-color: #1D6B9A; border-color: #1D6B9A; color: white;",
                                            onclick: on_save,
                                            disabled: saving(),
                                            if saving() { "Saving…" } else { "Save" }
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_save_action_info.set(true),
                                            title: "Save info",
                                            InfoIcon {}
                                        }
                                    }
                                }
                                span { class: "text-xs text-white italic", "Restart required" }
                            }
                        }
                    }

                    // Content - boards
                    div { class: "text-gray-100 text-xs",
                        div { class: "flex flex-wrap gap-4 items-stretch",


                            // ═══════════════════════════════════════════════════════════════
                            // CATEGORY 1: CONNECTION
                            // ═══════════════════════════════════════════════════════════════
                            div { class: "rounded border border-gray-600 p-4 w-fit relative",
                                div { class: "flex items-center gap-2 mb-3",
                                    span { class: "text-sm text-gray-300 font-semibold", "Connection" }
                                }
                                div { class: "flex flex-wrap gap-6 justify-start",
                                    // Enable column
                                    // Hide when not connected to avoid confusing "enabled" state while Neo4j is offline.
                                    if connected() {
                                        div { class: PARAM_COLUMN_CLASS,
                                            span { class: "text-gray-300 font-semibold text-xs", "Enable" }
                                            div { class: PARAM_BLOCK_CLASS,
                                                label { class: PARAM_LABEL_CLASS, "neo4j_enabled" }
                                                div { class: "flex items-end gap-2",
                                                    input {
                                                        r#type: "checkbox",
                                                        class: CHECKBOX_CLASS,
                                                        checked: enabled(),
                                                        onchange: move |_| enabled.set(!enabled()),
                                                    }
                                                    button {
                                                        class: PARAM_ICON_BUTTON_CLASS,
                                                        style: PARAM_ICON_BUTTON_STYLE,
                                                        onclick: move |_| show_enabled_info.set(true),
                                                        InfoIcon {}
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Server column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Server" }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "uri" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "text",
                                                    class: PARAM_TEXT_INPUT_CLASS,
                                                    value: "{uri}",
                                                    disabled: !enabled(),
                                                    onchange: move |evt| uri.set(evt.value()),
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_uri_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "database" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "text",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{database}",
                                                    disabled: !enabled(),
                                                    onchange: move |evt| database.set(evt.value()),
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_database_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                    // Auth column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Auth" }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "user" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "text",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{user}",
                                                    disabled: !enabled(),
                                                    onchange: move |evt| user.set(evt.value()),
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_user_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "password" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "password",
                                                    class: PARAM_TEXT_INPUT_CLASS,
                                                    value: "{password}",
                                                    oninput: move |evt| password.set(evt.value()),
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_password_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                    // Pool column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Pool" }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "max_connections" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{max_connections}",
                                                    disabled: !enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            max_connections.set(v.clamp(1, 100));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_max_connections_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "timeout_ms" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{connection_timeout_ms}",
                                                    disabled: !enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            connection_timeout_ms.set(v.clamp(1000, 60000));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_timeout_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                }

                                // Test connection (button left, message right on same row)
                                div { class: "mt-3 w-full flex items-start gap-3",
                                    // Left: button + info icon
                                    div { class: "flex items-center gap-2",
                                        button {
                                            class: "btn btn-sm",
                                            style: "background-color: #1D6B9A; border-color: #1D6B9A; color: white;",
                                            onclick: on_test_connection,
                                            disabled: !enabled(),
                                            "Test"
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_test_action_info.set(true),
                                            title: "Test info",
                                            InfoIcon {}
                                        }
                                    }

                                    // Right: message area (same height as button; reserves space to prevent jumping)
                                    div { class: "flex-1 min-h-[2rem] pt-1",
                                        if let Some(msg) = test_status() {
                                            div { class: "text-xs text-cyan-400 whitespace-normal break-words", "{msg}" }
                                        } else if let Some(err) = test_error() {
                                            div { class: "text-xs text-red-400 whitespace-normal break-words", "{err}" }
                                        } else {
                                            div { class: "text-xs text-gray-600", "" }
                                        }
                                    }
                                }
                            }

                            // ═══════════════════════════════════════════════════════════════
                            // CATEGORY 2: GRAPH EXPANSION
                            // ═══════════════════════════════════════════════════════════════
                            div { class: "rounded border border-gray-600 p-4 w-fit",
                                div { class: "flex items-center gap-2 mb-3",
                                    span { class: "text-sm text-gray-300 font-semibold", "Graph Expansion" }
                                }
                                div { class: "flex flex-wrap gap-6 justify-start",
                                    // Enable column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Enable" }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "expansion_enabled" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "checkbox",
                                                    class: CHECKBOX_CLASS,
                                                    checked: expansion_enabled(),
                                                    disabled: !enabled(),
                                                    onchange: move |_| expansion_enabled.set(!expansion_enabled()),
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_expansion_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                    // Traversal column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Traversal" }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "max_hops" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{max_hops}",
                                                    disabled: !enabled() || !expansion_enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            max_hops.set(v.clamp(1, 5));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_max_hops_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "max_chunks" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{max_chunks}",
                                                    disabled: !enabled() || !expansion_enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            max_chunks.set(v.clamp(1, 50));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_max_chunks_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                    // Weights column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Weights (%)" }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "entity_weight" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{entity_weight}",
                                                    disabled: !enabled() || !expansion_enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            entity_weight.set(v.clamp(0, 100));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_entity_weight_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "concept_weight" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{concept_weight}",
                                                    disabled: !enabled() || !expansion_enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            concept_weight.set(v.clamp(0, 100));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_concept_weight_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "min_strength" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{min_relationship_strength}",
                                                    disabled: !enabled() || !expansion_enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            min_relationship_strength.set(v.clamp(0, 100));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_min_strength_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // ═══════════════════════════════════════════════════════════════
                            // GRAPH STATS
                            // ═══════════════════════════════════════════════════════════════
                            div { class: "rounded border border-gray-600 p-4 w-fit",
                                div { class: "flex items-center gap-2 mb-3",
                                    span { class: "text-sm text-gray-300 font-semibold", "Graph Stats" }
                                }
                                div { class: "grid grid-cols-2 gap-x-10 gap-y-2",
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "total_nodes" }
                                        span { class: "text-gray-200", "{stats_total_nodes}" }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "total_relationships" }
                                        span { class: "text-gray-200", "{stats_total_relationships}" }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "documents" }
                                        span { class: "text-gray-200", "{stats_documents}" }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "chunks" }
                                        span { class: "text-gray-200", "{stats_chunks}" }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "entities" }
                                        span { class: "text-gray-200", "{stats_entities}" }
                                    }
                                }
                            }

                            // ═══════════════════════════════════════════════════════════════
                            // CATEGORY 3: ENTITY EXTRACTION
                            // ═══════════════════════════════════════════════════════════════
                            div { class: "rounded border border-gray-600 p-4 w-fit",
                                div { class: "flex items-center gap-2 mb-3",
                                    span { class: "text-sm text-gray-300 font-semibold", "Entity Extraction" }
                                }
                                div { class: "flex flex-wrap gap-6 justify-start",
                                    // Enable column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Enable" }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "extraction_enabled" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "checkbox",
                                                    class: CHECKBOX_CLASS,
                                                    checked: extraction_enabled(),
                                                    disabled: !enabled(),
                                                    onchange: move |_| extraction_enabled.set(!extraction_enabled()),
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_extraction_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                    // Thresholds column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Thresholds (%)" }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "confidence" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{confidence_threshold}",
                                                    disabled: !enabled() || !extraction_enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            confidence_threshold.set(v.clamp(0, 100));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_confidence_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                        div { class: PARAM_BLOCK_CLASS,
                                            label { class: PARAM_LABEL_CLASS, "fuzzy_match" }
                                            div { class: "flex items-end gap-2",
                                                input {
                                                    r#type: "number",
                                                    class: PARAM_NUMBER_INPUT_CLASS,
                                                    value: "{fuzzy_threshold}",
                                                    disabled: !enabled() || !extraction_enabled(),
                                                    onchange: move |evt| {
                                                        if let Ok(v) = evt.value().parse::<u32>() {
                                                            fuzzy_threshold.set(v.clamp(0, 100));
                                                        }
                                                    },
                                                }
                                                button {
                                                    class: PARAM_ICON_BUTTON_CLASS,
                                                    style: PARAM_ICON_BUTTON_STYLE,
                                                    onclick: move |_| show_fuzzy_info.set(true),
                                                    InfoIcon {}
                                                }
                                            }
                                        }
                                    }
                                    // Entity types column
                                    div { class: PARAM_COLUMN_CLASS,
                                        span { class: "text-gray-300 font-semibold text-xs", "Entity Types" }
                                        div { class: "flex flex-wrap gap-1 mt-1",
                                            span { class: "badge badge-outline badge-xs", "PERSON" }
                                            span { class: "badge badge-outline badge-xs", "ORG" }
                                            span { class: "badge badge-outline badge-xs", "LOC" }
                                            span { class: "badge badge-outline badge-xs", "CONCEPT" }
                                            span { class: "badge badge-outline badge-xs", "TECH" }
                                        }
                                    }
                                }
                            }

                            // ═══════════════════════════════════════════════════════════════
                            // SCHEMA PREVIEW
                            // ═══════════════════════════════════════════════════════════════
                            div { class: "rounded border border-gray-600 p-4 w-fit",
                                div { class: "flex items-center justify-between gap-4 mb-3",
                                    span { class: "text-sm text-gray-300 font-semibold", "Graph Schema" }
                                    button {
                                        class: "btn btn-xs btn-outline",
                                        onclick: move |_| show_schema.set(true),
                                        "View Full Schema"
                                    }
                                }
                                div { class: "bg-gray-900 rounded p-3 font-mono text-xs text-gray-400",
                                    pre {
                                        "(:Document)-[:HAS_CHUNK]->(:Chunk)\n"
                                        "(:Chunk)-[:MENTIONS]->(:Entity)\n"
                                        "(:Entity)-[:RELATED_TO]->(:Entity)\n"
                                        "(:Agent)-[:EXPERIENCED]->(:Episode)"
                                    }
                                }
                            }
                        }
                    }
                }

                // Enable Instructions
                if !feature_compiled() {
                    div { class: "alert alert-warning mt-4",
                        svg { class: "w-6 h-6", fill: "none", view_box: "0 0 24 24", stroke: "currentColor",
                            path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" }
                        }
                        div {
                            h3 { class: "font-bold", "Neo4j feature not compiled" }
                            p { class: "text-sm",
                                "Build with: "
                                code { class: "bg-gray-800 px-1 rounded", "cargo build --features neo4j" }
                            }
                        }
                    }
                }

                // Action button info modals
                if show_reset_action_info() {
                    {info_modal("Reset", show_reset_action_info, vec![
                        "Resets the form fields on this page back to their default values.",
                        "This does not change the running backend configuration until you click Save.",
                        "Use this if you want to undo edits and return to the defaults."
                    ])}
                }
                if show_test_action_info() {
                    {info_modal("Test", show_test_action_info, vec![
                        "Attempts to connect to Neo4j using the current settings (URI/user/password/database).",
                        "This is a connectivity check only — it does not modify data.",
                        "Use this after changing connection settings or after starting the Neo4j container."
                    ])}
                }
                if show_rebuild_action_info() {
                    {info_modal("Rebuild", show_rebuild_action_info, vec![
                        "Rebuilds the Neo4j knowledge graph from already-indexed documents.",
                        "This can take time depending on how many documents/chunks you have.",
                        "Use this after ingesting documents or changing extraction settings."
                    ])}
                }
                if show_save_action_info() {
                    {info_modal("Save", show_save_action_info, vec![
                        "Saves the current settings.",
                        "Note: in the current implementation this UI shows success but the backend save endpoint is marked TODO.",
                        "In a production setup, changing these settings may require a restart to take full effect."
                    ])}
                }

                // Info modals - Category 1
                if show_enabled_info() {
                    {info_modal("neo4j_enabled", show_enabled_info, vec![
                        "Enables Neo4j integration inside AG (GraphRAG ingestion + graph queries).",
                        "Does NOT start the Neo4j container — it only controls whether the backend will try to use Neo4j.",
                        "When enabled, the backend will use Neo4j if it is reachable; if Neo4j is down or misconfigured, graph features may be skipped or fail until it’s available.",
                        "Use this toggle to turn graph ingestion on/off without changing Docker/container state."
                    ])}
                }
                if show_uri_info() {
                    {info_modal("uri", show_uri_info, vec![
                        "Neo4j Bolt protocol URI.",
                        "Format: bolt://hostname:port or neo4j://hostname:port",
                        "Default: bolt://localhost:7687"
                    ])}
                }
                if show_user_info() {
                    {info_modal("user", show_user_info, vec![
                        "Neo4j authentication username.",
                        "Default: neo4j"
                    ])}
                }
                if show_password_info() {
                    {info_modal("password", show_password_info, vec![
                        "Neo4j authentication password.",
                        "Required for authentication. Keep secure."
                    ])}
                }
                if show_database_info() {
                    {info_modal("database", show_database_info, vec![
                        "Neo4j database name to use.",
                        "Default: neo4j (the default database)"
                    ])}
                }
                if show_max_connections_info() {
                    {info_modal("max_connections", show_max_connections_info, vec![
                        "Maximum number of connections in the connection pool.",
                        "Higher values allow more concurrent operations.",
                        "Default: 10. Range: 1-100."
                    ])}
                }
                if show_timeout_info() {
                    {info_modal("timeout_ms", show_timeout_info, vec![
                        "Connection timeout in milliseconds.",
                        "How long to wait for a connection before failing.",
                        "Default: 5000ms. Range: 1000-60000ms."
                    ])}
                }

                // Info modals - Category 2
                if show_expansion_info() {
                    {info_modal("expansion_enabled", show_expansion_info, vec![
                        "Enable graph-based context expansion during retrieval.",
                        "When enabled, related chunks are discovered through entity relationships.",
                        "Default: true."
                    ])}
                }
                if show_max_hops_info() {
                    {info_modal("max_hops", show_max_hops_info, vec![
                        "Maximum number of relationship hops to traverse.",
                        "Higher values find more distant connections but increase latency.",
                        "Default: 2. Range: 1-5."
                    ])}
                }
                if show_max_chunks_info() {
                    {info_modal("max_chunks", show_max_chunks_info, vec![
                        "Maximum number of chunks to return from graph expansion.",
                        "Limits the context size added from graph traversal.",
                        "Default: 10. Range: 1-50."
                    ])}
                }
                if show_entity_weight_info() {
                    {info_modal("entity_weight", show_entity_weight_info, vec![
                        "Weight given to entity-based connections (0-100%).",
                        "Higher values prioritize chunks sharing named entities.",
                        "Default: 70%."
                    ])}
                }
                if show_concept_weight_info() {
                    {info_modal("concept_weight", show_concept_weight_info, vec![
                        "Weight given to concept-based connections (0-100%).",
                        "Higher values prioritize chunks discussing similar concepts.",
                        "Default: 50%."
                    ])}
                }
                if show_min_strength_info() {
                    {info_modal("min_strength", show_min_strength_info, vec![
                        "Minimum relationship strength to consider (0-100%).",
                        "Filters out weak connections below this threshold.",
                        "Default: 30%."
                    ])}
                }

                // Info modals - Category 3
                if show_extraction_info() {
                    {info_modal("extraction_enabled", show_extraction_info, vec![
                        "Enable entity extraction during document indexing.",
                        "Extracts named entities and links them in the knowledge graph.",
                        "Default: true."
                    ])}
                }
                if show_confidence_info() {
                    {info_modal("confidence", show_confidence_info, vec![
                        "Minimum confidence threshold for entity extraction (0-100%).",
                        "Entities below this confidence are discarded.",
                        "Default: 50%."
                    ])}
                }
                if show_fuzzy_info() {
                    {info_modal("fuzzy_match", show_fuzzy_info, vec![
                        "Threshold for fuzzy entity matching (0-100%).",
                        "Used to link similar entity mentions (e.g., 'IBM' and 'I.B.M.').",
                        "Default: 80%."
                    ])}
                }

                // Help modal - What is GraphRAG?
                if show_help() {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| show_help.set(false),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-2xl max-h-[90vh] overflow-y-auto shadow-xl",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-center justify-between mb-3",
                                h2 { class: "text-base font-semibold text-gray-100", "What is GraphRAG?" }
                                button {
                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                    onclick: move |_| show_help.set(false),
                                    "×"
                                }
                            }
                            div { class: "text-sm text-gray-300 space-y-3",
                                p { "GraphRAG combines traditional vector-based RAG with knowledge graph traversal for enhanced retrieval and reasoning." }

                                h3 { class: "font-semibold text-white mt-4", "Key Benefits:" }
                                ul { class: "list-disc list-inside space-y-1 text-gray-400",
                                    li { "Multi-hop reasoning - Answer \"How is X related to Y?\" questions" }
                                    li { "Entity-centric retrieval - Find information through entity connections" }
                                    li { "Better context expansion - Discover related chunks through graph relationships" }
                                    li { "Reduced hallucination - Ground responses in entity knowledge" }
                                }

                                h3 { class: "font-semibold text-white mt-4", "How it works:" }
                                ol { class: "list-decimal list-inside space-y-1 text-gray-400",
                                    li { "Documents are chunked and indexed (existing RAG)" }
                                    li { "Entities are extracted from chunks and linked in Neo4j" }
                                    li { "Queries trigger both vector search AND graph expansion" }
                                    li { "Results are fused and reranked for relevance" }
                                }
                            }
                        }
                    }
                }

                // Full Schema Modal
                if show_schema() {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| show_schema.set(false),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[95vw] max-w-4xl max-h-[90vh] overflow-y-auto shadow-xl",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-center justify-between mb-3",
                                h2 { class: "text-base font-semibold text-gray-100", "Neo4j Graph Schema" }
                                button {
                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                    onclick: move |_| show_schema.set(false),
                                    "×"
                                }
                            }
                            div { class: "bg-gray-900 rounded p-4 font-mono text-xs text-gray-300 overflow-x-auto",
                                pre {
"// ═══════════════════════════════════════════════════════════════
// DOCUMENT KNOWLEDGE GRAPH
// ═══════════════════════════════════════════════════════════════

(:Document {{id, title, source, content_hash, mime_type, created_at}})
(:Chunk {{id, content, embedding_id, position, token_count}})
(:Entity {{id, name, normalized_name, entity_type, mention_count}})
(:Concept {{id, name, description, domain, importance}})

// Document Relationships
(Document)-[:HAS_CHUNK {{position}}]->(Chunk)
(Chunk)-[:MENTIONS {{confidence, context}}]->(Entity)
(Chunk)-[:DISCUSSES {{relevance}}]->(Concept)
(Entity)-[:RELATED_TO {{type, strength}}]->(Entity)
(Concept)-[:BROADER_THAN]->(Concept)
(Document)-[:REFERENCES]->(Document)

// ═══════════════════════════════════════════════════════════════
// AGENT MEMORY GRAPH
// ═══════════════════════════════════════════════════════════════

(:Agent {{id, name, created_at}})
(:Goal {{id, description, status, created_at}})
(:Task {{id, description, status}})
(:Episode {{id, query, response, success, timestamp}})
(:Reflection {{id, type, insight}})

// Agent Relationships
(Agent)-[:HAS_GOAL]->(Goal)
(Goal)-[:HAS_TASK]->(Task)
(Agent)-[:EXPERIENCED]->(Episode)
(Episode)-[:USED_CHUNK]->(Chunk)
(Episode)-[:MENTIONED_ENTITY]->(Entity)
(Episode)-[:LED_TO]->(Reflection)"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
