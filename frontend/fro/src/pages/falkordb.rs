//! FalkorDB Knowledge Graph configuration & status page.
//!
//! FalkorDB replaced Neo4j as the graph store. Key model differences this page
//! reflects:
//!   - FalkorDB is a Redis module — connection is a `redis://` URI, not Bolt.
//!   - Auth is a single optional Redis password; there is no separate username.
//!   - "database" is a graph *key name* passed to `GRAPH.QUERY`.
//!
//! Settings are editable: Save persists them to a `.env.graph` file on the
//! backend, which overrides environment variables. A restart (or Reconnect for
//! connection changes) applies them.
//!
//! Every field carries its own info button (ag is a learning platform — make
//! the invisible visible). Field help is keyed by name through `field_info`.
//!
//! The page separates two graphs deliberately:
//!   - FalkorDB        — the persistent ingestion store, written during indexing.
//!   - Petgraph runtime — the in-process graph that every retrieval-time query
//!     actually reads. FalkorDB is never on the read path.

use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use crate::{
    api,
    components::config_nav::{ConfigNav, ConfigTab},
};
use dioxus::prelude::*;

// Styling constants — shared with the other config pages.
const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";
const CARD_CLASS: &str = "rounded border border-gray-600 p-4 w-fit";
const CARD_TITLE_CLASS: &str = "text-sm text-gray-300 font-semibold";
const ACTION_BTN_STYLE: &str = "background-color: #1D6B9A; border-color: #1D6B9A; color: white;";
const TEXT_INPUT_CLASS: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 w-44";
const NUM_INPUT_CLASS: &str = "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
const CHECKBOX_CLASS: &str = "checkbox checkbox-xs";
const FIELD_ROW_CLASS: &str = "flex items-end gap-2";

/// Info "i" icon — matches the canonical info-button spec.
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

/// Info button toggling a plain bool modal signal (card / action level).
fn info_btn(mut toggle: Signal<bool>) -> Element {
    rsx! {
        button {
            class: PARAM_ICON_BUTTON_CLASS,
            style: PARAM_ICON_BUTTON_STYLE,
            onclick: move |_| toggle.set(true),
            InfoIcon {}
        }
    }
}

/// Info button for a single field — opens the keyed field modal.
fn field_info_btn(key: &'static str, mut field_info: Signal<Option<&'static str>>) -> Element {
    rsx! {
        button {
            class: PARAM_ICON_BUTTON_CLASS,
            style: PARAM_ICON_BUTTON_STYLE,
            onclick: move |_| field_info.set(Some(key)),
            InfoIcon {}
        }
    }
}

/// Generic info modal — title + paragraphs, ✕ to close.
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

/// Per-field help text, keyed by field name.
fn field_info_content(key: &str) -> (&'static str, Vec<&'static str>) {
    match key {
        // ── Connection ──────────────────────────────────────────────
        "enabled" => ("enabled", vec![
            "Master switch for the FalkorDB knowledge graph. When off, ag skips graph ingestion and graph-aware retrieval entirely.",
            "Turning it on does not start FalkorDB itself — it tells ag to connect to and use it. Saved as FALKOR_ENABLED.",
        ]),
        "uri" => ("uri", vec![
            "The FalkorDB connection address. FalkorDB is a Redis module, so this is a Redis URI — redis://host:port — not a Bolt URI.",
            "Default redis://localhost:6380. Saved as FALKOR_URI.",
        ]),
        "password" => ("password", vec![
            "The optional Redis password for FalkorDB. There is no separate username — auth is password-only.",
            "Write-only: leave blank to keep the current password. Anything typed here is saved as FALKOR_PASSWORD into .env.graph.",
        ]),
        "database" => ("graph key", vec![
            "The graph key name passed to GRAPH.QUERY — FalkorDB's equivalent of a database name. All of ag's nodes and relationships live under this one key.",
            "Default \"ag\". Saved as FALKOR_DATABASE.",
        ]),
        "max_connections" => ("max_connections", vec![
            "Size of the connection pool ag keeps open to FalkorDB. Higher values allow more concurrent graph operations.",
            "Default 10, range 1–100. Saved as FALKOR_MAX_CONNECTIONS.",
        ]),
        "timeout_ms" => ("timeout_ms", vec![
            "How long ag waits when opening a connection to FalkorDB before giving up, in milliseconds.",
            "Default 5000, range 1000–60000. Saved as FALKOR_CONNECTION_TIMEOUT_MS.",
        ]),
        "command_timeout" => ("command_timeout_ms", vec![
            "How long a single FalkorDB query may run before it is aborted, in milliseconds. This is the command/response timeout — distinct from timeout_ms, which only covers opening the connection.",
            "Enforced server-side: FalkorDB returns a timeout error for a query that exceeds it. It applies to row-returning queries (graph search, the Cypher console), not to ingestion writes.",
            "0 = no timeout (default). Saved as FALKOR_COMMAND_TIMEOUT_MS.",
        ]),
        // ── FalkorDB Store stats ────────────────────────────────────
        "stat_total_nodes" => ("total_nodes", vec![
            "Total nodes (Documents, Chunks, Entities, Concepts, …) in the persistent FalkorDB graph.",
        ]),
        "stat_relationships" => ("relationships", vec![
            "Total relationships (edges) in the FalkorDB graph — HAS_CHUNK, MENTIONS, RELATED_TO and so on.",
        ]),
        "stat_documents" => ("documents", vec![
            "Number of Document nodes in FalkorDB — one per ingested source file.",
        ]),
        "stat_chunks" => ("chunks", vec![
            "Number of Chunk nodes in FalkorDB. Each document is split into chunks; entities are extracted per chunk.",
        ]),
        "stat_entities" => ("entities", vec![
            "Number of distinct Entity nodes in FalkorDB — the people, organisations, concepts and so on extracted from chunks.",
        ]),
        // ── Petgraph Runtime stats ──────────────────────────────────
        "stat_node_count" => ("node_count", vec![
            "Number of nodes in the in-process petgraph runtime — the in-RAM graph every search query reads.",
            "Populated by Export → Runtime, not by ingestion directly.",
        ]),
        "stat_edge_count" => ("edge_count", vec![
            "Number of edges in the in-process petgraph runtime.",
        ]),
        "stat_built" => ("built", vec![
            "Whether the petgraph runtime currently holds a graph. \"No — empty\" means no Export has run, so graph-aware retrieval has nothing to walk.",
            "Use Export → Runtime to build it from the FalkorDB graph.",
        ]),
        // ── Graph Expansion ─────────────────────────────────────────
        "expansion_enabled" => ("expansion_enabled", vec![
            "Turns graph-aware retrieval on or off. When on, a search not only matches text but also walks the graph to pull in related chunks.",
            "Saved as GRAPH_EXPANSION_ENABLED.",
        ]),
        "max_hops" => ("max_hops", vec![
            "How many relationship steps retrieval may walk out from a matched chunk. 1 = direct neighbours only; higher reaches further but adds noise and latency.",
            "Default 2. Saved as GRAPH_EXPANSION_MAX_HOPS.",
        ]),
        "max_chunks" => ("max_chunks", vec![
            "The cap on how many extra chunks graph expansion may add to a result set, however many the traversal finds.",
            "Default 10. Saved as GRAPH_EXPANSION_MAX_CHUNKS.",
        ]),
        "entity_weight" => ("entity_weight", vec![
            "How strongly retrieval favours chunks connected through shared named entities (people, organisations, places).",
            "0–100%. Default 70%. Saved as GRAPH_ENTITY_WEIGHT.",
        ]),
        "concept_weight" => ("concept_weight", vec![
            "How strongly retrieval favours chunks connected through shared concepts — broader topics, as opposed to named entities.",
            "0–100%. Default 50%. Saved as GRAPH_CONCEPT_WEIGHT.",
        ]),
        "min_strength" => ("min_strength", vec![
            "The minimum strength a graph edge must have to be followed during expansion. Raising it drops weak, noisy links.",
            "0–100%. Default 30%. Saved as GRAPH_MIN_RELATIONSHIP_STRENGTH.",
        ]),
        // ── Entity Extraction ───────────────────────────────────────
        "extraction_enabled" => ("extraction_enabled", vec![
            "Turns entity extraction on or off during indexing. When off, documents stay searchable but no entities or relationships are added to the graph.",
            "Saved as ENTITY_EXTRACTION_ENABLED.",
        ]),
        "confidence" => ("confidence", vec![
            "The minimum confidence an extracted entity must reach to be kept. Raising it yields fewer but cleaner entities.",
            "0–100%. Default 50%. Saved as ENTITY_EXTRACTION_CONFIDENCE_THRESHOLD.",
        ]),
        "fuzzy" => ("fuzzy_match", vec![
            "How similar two entity mentions must be to be linked as the same entity — e.g. \"IBM\" and \"I.B.M.\". Higher means stricter matching.",
            "0–100%. Default 80%. Saved as ENTITY_LINKING_FUZZY_THRESHOLD.",
        ]),
        _ => ("Field", vec!["No description available."]),
    }
}

/// Modal showing help for one field (looked up by key).
fn field_modal(key: &str, mut field_info: Signal<Option<&'static str>>) -> Element {
    let (title, paragraphs) = field_info_content(key);
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| field_info.set(None),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-lg max-h-[90vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3",
                    h2 { class: "text-base font-semibold text-gray-100", "{title}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| field_info.set(None),
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

/// Read-only labelled value with a field info button (used for stats).
fn ro_field(
    label: &str,
    key: &'static str,
    value: String,
    field_info: Signal<Option<&'static str>>,
) -> Element {
    rsx! {
        div { class: PARAM_BLOCK_CLASS,
            label { class: PARAM_LABEL_CLASS, "{label}" }
            div { class: FIELD_ROW_CLASS,
                span { class: "text-gray-200", "{value}" }
                {field_info_btn(key, field_info)}
            }
        }
    }
}

/// Labelled text input bound to a String signal, with a field info button.
fn text_field(
    label: &str,
    key: &'static str,
    mut sig: Signal<String>,
    field_info: Signal<Option<&'static str>>,
) -> Element {
    rsx! {
        div { class: PARAM_BLOCK_CLASS,
            label { class: PARAM_LABEL_CLASS, "{label}" }
            div { class: FIELD_ROW_CLASS,
                input {
                    r#type: "text",
                    class: TEXT_INPUT_CLASS,
                    value: "{sig}",
                    oninput: move |evt| sig.set(evt.value()),
                }
                {field_info_btn(key, field_info)}
            }
        }
    }
}

/// Labelled numeric input bound to a u32 signal, clamped to [min, max].
fn num_field(
    label: &str,
    key: &'static str,
    mut sig: Signal<u32>,
    min: u32,
    max: u32,
    field_info: Signal<Option<&'static str>>,
) -> Element {
    rsx! {
        div { class: PARAM_BLOCK_CLASS,
            label { class: PARAM_LABEL_CLASS, "{label}" }
            div { class: FIELD_ROW_CLASS,
                input {
                    r#type: "number",
                    class: NUM_INPUT_CLASS,
                    value: "{sig}",
                    onchange: move |evt| {
                        if let Ok(v) = evt.value().parse::<u32>() {
                            sig.set(v.clamp(min, max));
                        }
                    },
                }
                {field_info_btn(key, field_info)}
            }
        }
    }
}

/// Labelled checkbox bound to a bool signal, with a field info button.
fn check_field(
    label: &str,
    key: &'static str,
    mut sig: Signal<bool>,
    field_info: Signal<Option<&'static str>>,
) -> Element {
    rsx! {
        div { class: PARAM_BLOCK_CLASS,
            label { class: PARAM_LABEL_CLASS, "{label}" }
            div { class: FIELD_ROW_CLASS,
                input {
                    r#type: "checkbox",
                    class: CHECKBOX_CLASS,
                    checked: sig(),
                    onchange: move |_| {
                        let v = sig();
                        sig.set(!v);
                    },
                }
                {field_info_btn(key, field_info)}
            }
        }
    }
}

/// Format a Cypher result cell for display.
fn fmt_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "—".to_string(),
        other => other.to_string(),
    }
}

#[component]
pub fn ConfigFalkorDb() -> Element {
    // Loaded config snapshot — used for stats + the feature-compiled flag.
    let mut cfg = use_signal(|| Option::<api::FalkorConfigResponse>::None);
    let mut pg_stats = use_signal(|| Option::<api::PetgraphRuntimeStats>::None);
    let mut connected = use_signal(|| false);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| Option::<String>::None);

    // Editable form fields (weights/thresholds held as 0-100 percentages).
    let mut f_enabled = use_signal(|| false);
    let mut f_uri = use_signal(String::new);
    let mut f_password = use_signal(String::new);
    let mut f_database = use_signal(String::new);
    let mut f_max_connections = use_signal(|| 10u32);
    let mut f_timeout = use_signal(|| 5000u32);
    let mut f_command_timeout = use_signal(|| 0u32);
    let mut f_expansion_enabled = use_signal(|| true);
    let mut f_max_hops = use_signal(|| 2u32);
    let mut f_max_chunks = use_signal(|| 10u32);
    let mut f_entity_weight = use_signal(|| 70u32);
    let mut f_concept_weight = use_signal(|| 50u32);
    let mut f_min_strength = use_signal(|| 30u32);
    let mut f_extraction_enabled = use_signal(|| true);
    let mut f_confidence = use_signal(|| 50u32);
    let mut f_fuzzy = use_signal(|| 80u32);

    // Action state. Save / Rebuild / Reconnect / Export share the feedback line.
    let mut saving = use_signal(|| false);
    let mut action_msg = use_signal(|| Option::<String>::None);
    let mut action_err = use_signal(|| Option::<String>::None);
    let mut rebuilding = use_signal(|| false);
    let mut reconnecting = use_signal(|| false);
    let mut exporting = use_signal(|| false);
    let mut test_msg = use_signal(|| Option::<String>::None);
    let mut test_err = use_signal(|| Option::<String>::None);

    // Cypher console.
    let mut cypher_input = use_signal(String::new);
    let mut cypher_running = use_signal(|| false);
    let mut cypher_result = use_signal(|| Option::<api::CypherQueryResponse>::None);
    let mut cypher_error = use_signal(|| Option::<String>::None);

    // Per-field info modal — holds the key of the open field, or None.
    let field_info = use_signal(|| Option::<&'static str>::None);

    // Card / action modal toggles.
    let mut show_help = use_signal(|| false);
    let mut show_schema = use_signal(|| false);
    // Flipped inside info_btn / info_modal helpers — passed by value, not `mut`.
    let show_connection_info = use_signal(|| false);
    let show_falkor_stats_info = use_signal(|| false);
    let show_petgraph_info = use_signal(|| false);
    let show_expansion_info = use_signal(|| false);
    let show_extraction_info = use_signal(|| false);
    let show_cypher_info = use_signal(|| false);
    let show_save_info = use_signal(|| false);
    let show_test_info = use_signal(|| false);
    let show_rebuild_info = use_signal(|| false);
    let show_reconnect_info = use_signal(|| false);
    let show_export_info = use_signal(|| false);

    // ── Load config + petgraph stats on mount ───────────────────────
    use_effect(move || {
        spawn(async move {
            match api::fetch_falkor_config().await {
                Ok(c) => {
                    connected.set(c.connected);
                    // Populate the editable form from the loaded config.
                    f_enabled.set(c.enabled);
                    f_uri.set(c.uri.clone());
                    f_database.set(c.database.clone());
                    f_max_connections.set(c.max_connections as u32);
                    f_timeout.set(c.connection_timeout_ms as u32);
                    f_command_timeout.set(c.command_timeout_ms as u32);
                    f_expansion_enabled.set(c.expansion_enabled);
                    f_max_hops.set(c.max_hops as u32);
                    f_max_chunks.set(c.max_chunks as u32);
                    f_entity_weight.set((c.entity_weight * 100.0).round() as u32);
                    f_concept_weight.set((c.concept_weight * 100.0).round() as u32);
                    f_min_strength.set((c.min_relationship_strength * 100.0).round() as u32);
                    f_extraction_enabled.set(c.extraction_enabled);
                    f_confidence.set((c.confidence_threshold * 100.0).round() as u32);
                    f_fuzzy.set((c.fuzzy_threshold * 100.0).round() as u32);
                    cfg.set(Some(c));
                    loading.set(false);
                }
                Err(e) => {
                    error.set(Some(format!("Failed to load FalkorDB config: {}", e)));
                    loading.set(false);
                }
            }
            if let Ok(s) = api::fetch_petgraph_stats().await {
                pg_stats.set(Some(s));
            }
        });
    });

    // ── Handlers ─────────────────────────────────────────────────────
    // Refreshes the loaded snapshot (stats + connected) without touching the
    // editable form — the user may have unsaved edits.
    let refresh_config = move || {
        spawn(async move {
            if let Ok(c) = api::fetch_falkor_config().await {
                connected.set(c.connected);
                cfg.set(Some(c));
            }
        });
    };

    let on_save = move |_| {
        saving.set(true);
        action_msg.set(Some("Saving…".to_string()));
        action_err.set(None);
        let payload = api::FalkorConfigSave {
            enabled: f_enabled(),
            uri: f_uri(),
            password: {
                let p = f_password();
                if p.is_empty() {
                    None
                } else {
                    Some(p)
                }
            },
            database: f_database(),
            max_connections: f_max_connections() as usize,
            connection_timeout_ms: f_timeout() as u64,
            command_timeout_ms: f_command_timeout() as u64,
            expansion_enabled: f_expansion_enabled(),
            max_hops: f_max_hops() as usize,
            max_chunks: f_max_chunks() as usize,
            entity_weight: f_entity_weight() as f32 / 100.0,
            concept_weight: f_concept_weight() as f32 / 100.0,
            min_relationship_strength: f_min_strength() as f32 / 100.0,
            extraction_enabled: f_extraction_enabled(),
            confidence_threshold: f_confidence() as f32 / 100.0,
            fuzzy_threshold: f_fuzzy() as f32 / 100.0,
        };
        spawn(async move {
            match api::save_falkor_config(&payload).await {
                Ok(r) => {
                    action_msg.set(Some(r.message));
                    action_err.set(None);
                    // Password is write-only — clear the field after a save.
                    f_password.set(String::new());
                }
                Err(e) => {
                    action_msg.set(None);
                    action_err.set(Some(format!("Save failed: {}", e)));
                }
            }
            saving.set(false);
        });
    };

    let on_test = move |_| {
        test_msg.set(Some("Testing connection…".to_string()));
        test_err.set(None);
        spawn(async move {
            match api::test_falkor_connection().await {
                Ok(r) => {
                    connected.set(r.connected);
                    if r.connected {
                        test_msg.set(Some("Connected to FalkorDB".to_string()));
                        test_err.set(None);
                    } else {
                        test_msg.set(None);
                        test_err.set(Some(r.message));
                    }
                }
                Err(e) => {
                    test_msg.set(None);
                    test_err.set(Some(format!("Request failed: {}", e)));
                }
            }
        });
    };

    let on_reconnect = move |_| {
        reconnecting.set(true);
        action_msg.set(Some("Reconnecting to FalkorDB…".to_string()));
        action_err.set(None);
        spawn(async move {
            match api::reconnect_graph().await {
                Ok(_) => {
                    action_msg.set(Some("Reconnected to FalkorDB".to_string()));
                    action_err.set(None);
                    refresh_config();
                }
                Err(e) => {
                    action_msg.set(None);
                    action_err.set(Some(format!("Reconnect failed: {}", e)));
                }
            }
            reconnecting.set(false);
        });
    };

    let on_rebuild = move |_| {
        rebuilding.set(true);
        action_msg.set(Some("Rebuilding knowledge graph…".to_string()));
        action_err.set(None);
        spawn(async move {
            match api::rebuild_knowledge_graph().await {
                Ok(r) => {
                    action_msg.set(Some(format!(
                        "Rebuilt: {} docs, {} chunks processed",
                        r.documents_processed, r.chunks_processed
                    )));
                    if !r.errors.is_empty() {
                        action_err.set(Some(format!("Warnings: {}", r.errors.join(", "))));
                    }
                    refresh_config();
                }
                Err(e) => {
                    action_msg.set(None);
                    action_err.set(Some(format!("Rebuild failed: {}", e)));
                }
            }
            rebuilding.set(false);
        });
    };

    let on_export = move |_| {
        exporting.set(true);
        action_msg.set(Some("Exporting graph to petgraph runtime…".to_string()));
        action_err.set(None);
        spawn(async move {
            match api::export_graph().await {
                Ok(r) => {
                    action_msg.set(Some(format!(
                        "Exported {} nodes, {} relationships → petgraph reloaded",
                        r.nodes, r.relationships
                    )));
                    action_err.set(None);
                    if let Ok(s) = api::fetch_petgraph_stats().await {
                        pg_stats.set(Some(s));
                    }
                }
                Err(e) => {
                    action_msg.set(None);
                    action_err.set(Some(format!("Export failed: {}", e)));
                }
            }
            exporting.set(false);
        });
    };

    let on_run_cypher = move |_| {
        let q = cypher_input().trim().to_string();
        if q.is_empty() {
            return;
        }
        cypher_running.set(true);
        cypher_error.set(None);
        spawn(async move {
            match api::run_cypher_query(&q).await {
                Ok(r) => {
                    cypher_result.set(Some(r));
                    cypher_error.set(None);
                }
                Err(e) => {
                    cypher_result.set(None);
                    cypher_error.set(Some(e));
                }
            }
            cypher_running.set(false);
        });
    };

    rsx! {
        div { class: "p-6 space-y-6 w-full",
            ConfigNav { active: ConfigTab::FalkorDb }

            if loading() {
                div { class: "flex items-center justify-center py-8",
                    span { class: "loading loading-spinner loading-lg text-primary" }
                }
            } else if let Some(err) = error() {
                div { class: "alert alert-error", span { "{err}" } }
            } else if let Some(c) = cfg() {
                // ── Main panel ───────────────────────────────────────
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 shadow",
                    // Header: title + status + actions
                    div { class: "flex items-start justify-between mb-3 flex-wrap gap-3",
                        div { class: "flex items-center gap-3 flex-wrap",
                            h3 { class: "text-sm font-semibold text-gray-200", "FalkorDB Knowledge Graph" }
                            {info_btn(show_help)}
                            span { class: "text-xs font-semibold text-gray-400", "Status:" }
                            span {
                                class: if connected() { "text-xs font-semibold text-green-400" } else { "text-xs font-semibold text-gray-400" },
                                if connected() { "Reachable" } else { "Unreachable" }
                            }
                        }
                        div { class: "flex flex-col items-end gap-1",
                            div { class: "flex items-center gap-2",
                                div { class: "flex items-center gap-1",
                                    button {
                                        class: "btn btn-sm",
                                        style: ACTION_BTN_STYLE,
                                        onclick: on_save,
                                        disabled: saving(),
                                        if saving() { "Saving…" } else { "Save" }
                                    }
                                    {info_btn(show_save_info)}
                                }
                                div { class: "flex items-center gap-1",
                                    button {
                                        class: "btn btn-sm",
                                        style: ACTION_BTN_STYLE,
                                        onclick: on_rebuild,
                                        disabled: !connected() || rebuilding(),
                                        if rebuilding() { "Rebuilding…" } else { "Rebuild" }
                                    }
                                    {info_btn(show_rebuild_info)}
                                }
                                div { class: "flex items-center gap-1",
                                    button {
                                        class: "btn btn-sm",
                                        style: ACTION_BTN_STYLE,
                                        onclick: on_reconnect,
                                        disabled: reconnecting(),
                                        if reconnecting() { "Reconnecting…" } else { "Reconnect" }
                                    }
                                    {info_btn(show_reconnect_info)}
                                }
                            }
                            span { class: "text-xs text-gray-400 italic",
                                "Save writes .env.graph — restart ag to apply"
                            }
                        }
                    }

                    // Shared action feedback
                    if action_msg().is_some() || action_err().is_some() {
                        div { class: "mb-3 text-xs",
                            if let Some(m) = action_msg() {
                                div { class: "text-green-400", "{m}" }
                            }
                            if let Some(e) = action_err() {
                                div { class: "text-red-400", "{e}" }
                            }
                        }
                    }

                    // Feature-not-compiled warning
                    if !c.feature_compiled {
                        div { class: "alert alert-warning mb-3 py-2",
                            div {
                                h3 { class: "font-bold text-sm", "Graph feature not compiled" }
                                p { class: "text-xs",
                                    "Build with: "
                                    code { class: "bg-gray-800 px-1 rounded", "cargo build --features graph" }
                                }
                            }
                        }
                    }

                    // Cards
                    div { class: "flex flex-wrap gap-4 items-stretch",

                        // ── Connection ───────────────────────────────
                        div { class: CARD_CLASS,
                            div { class: "flex items-center gap-2 mb-3",
                                span { class: CARD_TITLE_CLASS, "Connection" }
                                {info_btn(show_connection_info)}
                            }
                            div { class: "grid grid-cols-2 gap-x-8 gap-y-2",
                                {check_field("enabled", "enabled", f_enabled, field_info)}
                                {text_field("uri", "uri", f_uri, field_info)}
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "password" }
                                    div { class: FIELD_ROW_CLASS,
                                        input {
                                            r#type: "password",
                                            class: TEXT_INPUT_CLASS,
                                            placeholder: "blank = keep current",
                                            value: "{f_password}",
                                            oninput: move |evt| f_password.set(evt.value()),
                                        }
                                        {field_info_btn("password", field_info)}
                                    }
                                }
                                {text_field("graph key", "database", f_database, field_info)}
                                {num_field("max_connections", "max_connections", f_max_connections, 1, 100, field_info)}
                                {num_field("timeout_ms", "timeout_ms", f_timeout, 1000, 60000, field_info)}
                                {num_field("command_timeout_ms", "command_timeout", f_command_timeout, 0, 600000, field_info)}
                            }
                            div { class: "mt-3 flex items-start gap-3",
                                button {
                                    class: "btn btn-sm",
                                    style: ACTION_BTN_STYLE,
                                    onclick: on_test,
                                    "Test"
                                }
                                {info_btn(show_test_info)}
                                div { class: "flex-1 min-h-[2rem] pt-1",
                                    if let Some(m) = test_msg() {
                                        div { class: "text-xs text-green-400 break-words", "{m}" }
                                    } else if let Some(e) = test_err() {
                                        div { class: "text-xs text-red-400 break-words", "{e}" }
                                    }
                                }
                            }
                        }

                        // ── FalkorDB stats (ingestion store) ─────────
                        div { class: CARD_CLASS,
                            div { class: "flex items-center gap-2 mb-1",
                                span { class: CARD_TITLE_CLASS, "FalkorDB Store" }
                                {info_btn(show_falkor_stats_info)}
                            }
                            p { class: "text-xs text-gray-400 mb-3", "Persistent graph, written during ingestion" }
                            if let Some(s) = c.stats.clone() {
                                div { class: "grid grid-cols-2 gap-x-8 gap-y-2",
                                    {ro_field("total_nodes", "stat_total_nodes", s.total_nodes.to_string(), field_info)}
                                    {ro_field("relationships", "stat_relationships", s.total_relationships.to_string(), field_info)}
                                    {ro_field("documents", "stat_documents", s.documents.to_string(), field_info)}
                                    {ro_field("chunks", "stat_chunks", s.chunks.to_string(), field_info)}
                                    {ro_field("entities", "stat_entities", s.entities.to_string(), field_info)}
                                }
                            } else {
                                p { class: "text-xs text-gray-400", "Connect to FalkorDB to see store stats." }
                            }
                        }

                        // ── Petgraph runtime (query graph) ───────────
                        div { class: CARD_CLASS,
                            div { class: "flex items-center gap-2 mb-1",
                                span { class: CARD_TITLE_CLASS, "Petgraph Runtime" }
                                {info_btn(show_petgraph_info)}
                            }
                            p { class: "text-xs text-gray-400 mb-3", "In-process graph — every search query reads this" }
                            if let Some(s) = pg_stats() {
                                div { class: "grid grid-cols-2 gap-x-8 gap-y-2",
                                    {ro_field("node_count", "stat_node_count", s.node_count.to_string(), field_info)}
                                    {ro_field("edge_count", "stat_edge_count", s.edge_count.to_string(), field_info)}
                                    {ro_field("built", "stat_built", if s.is_empty { "No — empty".into() } else { "Yes".into() }, field_info)}
                                }
                            } else {
                                p { class: "text-xs text-gray-400", "Runtime stats unavailable." }
                            }
                            div { class: "mt-3 flex items-center gap-2",
                                button {
                                    class: "btn btn-sm",
                                    style: ACTION_BTN_STYLE,
                                    onclick: on_export,
                                    disabled: !connected() || exporting(),
                                    if exporting() { "Exporting…" } else { "Export → Runtime" }
                                }
                                {info_btn(show_export_info)}
                            }
                        }

                        // ── Graph expansion ─────────────────────────
                        div { class: CARD_CLASS,
                            div { class: "flex items-center gap-2 mb-3",
                                span { class: CARD_TITLE_CLASS, "Graph Expansion" }
                                {info_btn(show_expansion_info)}
                            }
                            div { class: "grid grid-cols-2 gap-x-8 gap-y-2",
                                {check_field("expansion_enabled", "expansion_enabled", f_expansion_enabled, field_info)}
                                {num_field("max_hops", "max_hops", f_max_hops, 1, 10, field_info)}
                                {num_field("max_chunks", "max_chunks", f_max_chunks, 1, 100, field_info)}
                                {num_field("entity_weight %", "entity_weight", f_entity_weight, 0, 100, field_info)}
                                {num_field("concept_weight %", "concept_weight", f_concept_weight, 0, 100, field_info)}
                                {num_field("min_strength %", "min_strength", f_min_strength, 0, 100, field_info)}
                            }
                        }

                        // ── Entity extraction ───────────────────────
                        div { class: CARD_CLASS,
                            div { class: "flex items-center gap-2 mb-3",
                                span { class: CARD_TITLE_CLASS, "Entity Extraction" }
                                {info_btn(show_extraction_info)}
                            }
                            div { class: "grid grid-cols-2 gap-x-8 gap-y-2",
                                {check_field("extraction_enabled", "extraction_enabled", f_extraction_enabled, field_info)}
                                {num_field("confidence %", "confidence", f_confidence, 0, 100, field_info)}
                                {num_field("fuzzy_match %", "fuzzy", f_fuzzy, 0, 100, field_info)}
                            }
                            div { class: "mt-3",
                                span { class: "text-gray-400 text-xs", "Entity types" }
                                div { class: "flex flex-wrap gap-1 mt-1",
                                    for t in ["PERSON", "ORGANIZATION", "LOCATION", "CONCEPT", "TECHNOLOGY", "EVENT"] {
                                        span { class: "badge badge-outline badge-xs", "{t}" }
                                    }
                                }
                            }
                        }

                        // ── Schema preview ──────────────────────────
                        div { class: CARD_CLASS,
                            div { class: "flex items-center justify-between gap-4 mb-3",
                                span { class: CARD_TITLE_CLASS, "Graph Schema" }
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

                    // ── Cypher console ──────────────────────────────
                    div { class: "mt-4 rounded border border-gray-600 p-4 w-full",
                        div { class: "flex items-center gap-2 mb-1",
                            span { class: CARD_TITLE_CLASS, "Cypher Console" }
                            {info_btn(show_cypher_info)}
                        }
                        p { class: "text-xs text-gray-400 mb-2",
                            "Read-only — FalkorDB rejects writes (CREATE/MERGE/DELETE/SET) on this endpoint."
                        }
                        // Example queries
                        div { class: "flex flex-wrap gap-2 mb-2",
                            for example in [
                                "MATCH (n) RETURN labels(n)[0] AS label, count(*) AS count ORDER BY count DESC",
                                "MATCH (e:Entity) RETURN e.name, e.entity_type, e.mention_count ORDER BY e.mention_count DESC LIMIT 10",
                                "MATCH (d:Document)-[:HAS_CHUNK]->(c:Chunk) RETURN d.title, count(c) AS chunks ORDER BY chunks DESC LIMIT 10",
                            ] {
                                button {
                                    class: "btn btn-xs btn-outline",
                                    onclick: move |_| cypher_input.set(example.to_string()),
                                    title: "{example}",
                                    "Example"
                                }
                            }
                        }
                        textarea {
                            class: "textarea textarea-bordered bg-gray-700 text-gray-200 w-full font-mono text-xs",
                            rows: "3",
                            placeholder: "MATCH (e:Entity) RETURN e.name LIMIT 10",
                            value: "{cypher_input}",
                            oninput: move |evt| cypher_input.set(evt.value()),
                        }
                        div { class: "mt-2 flex items-center gap-3",
                            button {
                                class: "btn btn-sm",
                                style: ACTION_BTN_STYLE,
                                onclick: on_run_cypher,
                                disabled: !connected() || cypher_running() || cypher_input().trim().is_empty(),
                                if cypher_running() { "Running…" } else { "Run" }
                            }
                            if !connected() {
                                span { class: "text-xs text-gray-400", "Connect to FalkorDB to run queries." }
                            }
                        }
                        if let Some(e) = cypher_error() {
                            div { class: "mt-2 text-xs text-red-400 break-words", "{e}" }
                        }
                        if let Some(r) = cypher_result() {
                            div { class: "mt-2",
                                div { class: "text-xs text-gray-400 mb-1", "{r.row_count} row(s)" }
                                if r.rows.is_empty() {
                                    div { class: "text-xs text-gray-400", "Query returned no rows." }
                                } else {
                                    div { class: "overflow-x-auto",
                                        table { class: "table table-xs",
                                            tbody {
                                                for row in r.rows.iter() {
                                                    tr {
                                                        for cell in row.iter() {
                                                            td { class: "font-mono text-xs text-gray-200 align-top",
                                                                {fmt_cell(cell)}
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
                }

                // ── Per-field info modal ────────────────────────────
                if let Some(key) = field_info() {
                    {field_modal(key, field_info)}
                }

                // ── Card / action modals ────────────────────────────
                if show_connection_info() {
                    {info_modal("Connection", show_connection_info, vec![
                        "How the app reaches FalkorDB. FalkorDB is a Redis module, so the URI uses the Redis protocol (redis://host:port) — not Neo4j's Bolt protocol.",
                        "Auth is a single optional Redis password; there is no separate username. The \"graph key\" is the key name passed to GRAPH.QUERY — FalkorDB's equivalent of a database name.",
                        "Editing these and clicking Save writes .env.graph. Connection changes take effect on the next restart, or immediately when you click Reconnect.",
                    ])}
                }
                if show_falkor_stats_info() {
                    {info_modal("FalkorDB Store", show_falkor_stats_info, vec![
                        "Counts of nodes and relationships in the persistent FalkorDB graph.",
                        "FalkorDB is written during document ingestion and graph rebuilds. It is the durable store — but it is not read during normal search queries.",
                        "Stats appear only when the app is connected to FalkorDB.",
                    ])}
                }
                if show_petgraph_info() {
                    {info_modal("Petgraph Runtime", show_petgraph_info, vec![
                        "The petgraph runtime is an in-process graph held in RAM. Every retrieval-time graph expansion reads this — never FalkorDB directly.",
                        "It is populated by exporting the FalkorDB graph to a JSON snapshot and loading that into memory. This keeps the read path fast and independent of the graph database.",
                        "If FalkorDB is down, search still works: the runtime graph keeps serving from the last export.",
                    ])}
                }
                if show_expansion_info() {
                    {info_modal("Graph Expansion", show_expansion_info, vec![
                        "Controls how retrieval walks the graph to find related chunks: max_hops limits traversal depth, max_chunks caps how many extra chunks are added.",
                        "entity_weight and concept_weight bias expansion toward shared entities vs. shared concepts; min_strength drops weak relationships.",
                        "Edit and Save to persist. These take effect on the next ingestion or after a restart.",
                    ])}
                }
                if show_extraction_info() {
                    {info_modal("Entity Extraction", show_extraction_info, vec![
                        "Controls entity extraction during indexing. confidence drops low-confidence entities; fuzzy_match links near-duplicate mentions (e.g. \"IBM\" and \"I.B.M.\").",
                        "Extracted entities and their relationships are written into the FalkorDB graph.",
                        "Edit and Save to persist. These take effect on the next ingestion or graph rebuild.",
                    ])}
                }
                if show_cypher_info() {
                    {info_modal("Cypher Console", show_cypher_info, vec![
                        "Run ad-hoc OpenCypher queries against the FalkorDB graph and see the rows back.",
                        "Queries run via GRAPH.RO_QUERY — FalkorDB enforces read-only server-side, so CREATE / MERGE / DELETE / SET are rejected. You cannot change the graph from here.",
                        "Use the Example buttons for starting points, or write your own to explore what ingestion produced.",
                    ])}
                }
                if show_save_info() {
                    {info_modal("Save", show_save_info, vec![
                        "Writes every setting on this page to a .env.graph file next to the app.",
                        ".env.graph OVERRIDES environment variables — a Save is treated as your most recent intent. Delete the file to revert to ag.env/.env.",
                        "Restart ag to apply everything; connection changes also apply when you click Reconnect. The password field is write-only — leave it blank to keep the current password.",
                    ])}
                }
                if show_test_info() {
                    {info_modal("Test", show_test_info, vec![
                        "Runs a health check against FalkorDB using the current connection.",
                        "Connectivity check only — it does not change any data. Note it tests the live connection, not unsaved edits in the form.",
                    ])}
                }
                if show_rebuild_info() {
                    {info_modal("Rebuild", show_rebuild_info, vec![
                        "Rebuilds the FalkorDB knowledge graph from every indexed document — re-running entity extraction and relationship linking.",
                        "Can take time on a large corpus. Use after ingesting documents or changing extraction settings.",
                    ])}
                }
                if show_reconnect_info() {
                    {info_modal("Reconnect", show_reconnect_info, vec![
                        "Re-establishes the connection to FalkorDB and re-initializes the graph schema.",
                        "Picks up saved connection changes (URI, password, graph key) without restarting ag. Also useful after restarting the FalkorDB service or container.",
                    ])}
                }
                if show_export_info() {
                    {info_modal("Export → Runtime", show_export_info, vec![
                        "Exports the FalkorDB graph to a JSON snapshot and reloads the in-process petgraph runtime from it.",
                        "This is the bridge between the durable store and the fast read path: ingestion writes FalkorDB, export refreshes the runtime graph that queries actually use.",
                    ])}
                }

                // Help modal — What is GraphRAG?
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
                                p { "GraphRAG pairs ordinary vector/keyword retrieval with a knowledge graph, so the app can follow relationships between entities — not just match text." }
                                h3 { class: "font-semibold text-white mt-2", "Two graphs, two jobs:" }
                                ul { class: "list-disc list-inside space-y-1 text-gray-400",
                                    li { "FalkorDB — the durable store. Written during ingestion; holds documents, chunks, entities and their relationships." }
                                    li { "Petgraph runtime — an in-memory copy. Every search query reads this; FalkorDB is never on the read path." }
                                }
                                h3 { class: "font-semibold text-white mt-2", "How a document flows through:" }
                                ol { class: "list-decimal list-inside space-y-1 text-gray-400",
                                    li { "Documents are chunked and indexed for retrieval." }
                                    li { "Entities are extracted from chunks and linked into the FalkorDB graph." }
                                    li { "The graph is exported to a snapshot and loaded into the petgraph runtime." }
                                    li { "Queries do vector/keyword search AND walk the runtime graph, then fuse the results." }
                                }
                            }
                        }
                    }
                }

                // Full schema modal
                if show_schema() {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| show_schema.set(false),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[95vw] max-w-4xl max-h-[90vh] overflow-y-auto shadow-xl",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-center justify-between mb-3",
                                h2 { class: "text-base font-semibold text-gray-100", "FalkorDB Graph Schema" }
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
(Episode)-[:LED_TO]->(Reflection)

// Timestamps are epoch-ms integers (generated app-side, not by the DB)."
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
