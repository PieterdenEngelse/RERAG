//! NER (Named Entity Recognition) configuration page.

use crate::pages::hardware::components::{info_modal, InfoIcon};
use crate::pages::hardware::constants::{PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE};
use crate::{
    api,
    app::Route,
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
};
use dioxus::prelude::*;
use dioxus_router::Link;

const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_COLUMN_CLASS: &str = "param-column-spacing";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";
const PARAM_NUMBER_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
const PARAM_CHECKBOX_CLASS: &str = "checkbox checkbox-xs onnx-checkbox";
const PARAM_SELECT_CLASS: &str = "select select-xs select-bordered bg-gray-700 text-gray-200 w-36";
const PARAM_TEXT_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 w-64 font-mono";

#[component]
pub fn ConfigNer() -> Element {
    // ═══════════════════════════════════════════════════════════════
    // CONTROL
    // ═══════════════════════════════════════════════════════════════
    let mut extraction_enabled = use_signal(|| true);
    let mut type_allowlist = use_signal(|| "PERSON,ORGANIZATION,LOCATION,PRODUCT".to_string());

    // ═══════════════════════════════════════════════════════════════
    // QUALITY
    // ═══════════════════════════════════════════════════════════════
    let mut confidence_threshold = use_signal(|| 0.85f64);
    let mut type_thresholds =
        use_signal(|| r#"{"PERSON":0.75,"ORGANIZATION":0.95,"PRODUCT":0.95}"#.to_string());
    let mut fuzzy_threshold = use_signal(|| 0.8f64);

    // ═══════════════════════════════════════════════════════════════
    // FILTERING
    // ═══════════════════════════════════════════════════════════════
    let mut min_length = use_signal(|| 2usize);
    let mut max_length = use_signal(|| 100usize);
    let mut dedup_case_insensitive = use_signal(|| true);
    let mut nesting_strategy = use_signal(|| "KeepLongest".to_string());

    // ═══════════════════════════════════════════════════════════════
    // PERFORMANCE
    // ═══════════════════════════════════════════════════════════════
    let mut batch_size = use_signal(|| 4usize);
    let mut quantization_enabled = use_signal(|| false);
    let mut model_cache_enabled = use_signal(|| true);

    // ═══════════════════════════════════════════════════════════════
    // INTEGRATION
    // ═══════════════════════════════════════════════════════════════
    let mut graph_storage_enabled = use_signal(|| true);

    // ═══════════════════════════════════════════════════════════════
    // Save / load state
    // ═══════════════════════════════════════════════════════════════
    let mut saving = use_signal(|| false);
    let mut save_message = use_signal(|| Option::<String>::None);

    // ═══════════════════════════════════════════════════════════════
    // Restart state
    // ═══════════════════════════════════════════════════════════════
    let mut restarting = use_signal(|| false);
    let mut restart_msg = use_signal(|| Option::<String>::None);

    // Load config on mount
    use_effect(move || {
        spawn(async move {
            if let Ok(resp) = api::fetch_ner_config().await {
                let c = resp.config;
                extraction_enabled.set(c.extraction_enabled);
                type_allowlist.set(c.type_allowlist);
                confidence_threshold.set(c.confidence_threshold);
                type_thresholds.set(c.type_thresholds);
                fuzzy_threshold.set(c.fuzzy_threshold);
                min_length.set(c.min_length);
                max_length.set(c.max_length);
                dedup_case_insensitive.set(c.dedup_case_insensitive);
                nesting_strategy.set(c.nesting_strategy);
                batch_size.set(c.batch_size);
                quantization_enabled.set(c.quantization_enabled);
                model_cache_enabled.set(c.model_cache_enabled);
                graph_storage_enabled.set(c.graph_storage_enabled);
            }
        });
    });

    // Save handler
    let save_config = move |_| {
        spawn(async move {
            saving.set(true);
            save_message.set(None);
            let request = api::NerConfigRequest {
                extraction_enabled: Some(extraction_enabled()),
                type_allowlist: Some(type_allowlist()),
                confidence_threshold: Some(confidence_threshold()),
                type_thresholds: Some(type_thresholds()),
                fuzzy_threshold: Some(fuzzy_threshold()),
                min_length: Some(min_length()),
                max_length: Some(max_length()),
                dedup_case_insensitive: Some(dedup_case_insensitive()),
                nesting_strategy: Some(nesting_strategy()),
                batch_size: Some(batch_size()),
                quantization_enabled: Some(quantization_enabled()),
                model_cache_enabled: Some(model_cache_enabled()),
                graph_storage_enabled: Some(graph_storage_enabled()),
            };
            match api::update_ner_config(request).await {
                Ok(resp) => save_message.set(Some(resp.message)),
                Err(e) => save_message.set(Some(format!("Error: {e}"))),
            }
            saving.set(false);
        });
    };

    // ═══════════════════════════════════════════════════════════════
    // Info modal signals
    // ═══════════════════════════════════════════════════════════════
    let mut show_extraction_enabled_info = use_signal(|| false);
    let mut show_type_allowlist_info = use_signal(|| false);
    let mut show_confidence_threshold_info = use_signal(|| false);
    let mut show_type_thresholds_info = use_signal(|| false);
    let mut show_fuzzy_threshold_info = use_signal(|| false);
    let mut show_min_length_info = use_signal(|| false);
    let mut show_max_length_info = use_signal(|| false);
    let mut show_dedup_info = use_signal(|| false);
    let mut show_nesting_info = use_signal(|| false);
    let mut show_batch_size_info = use_signal(|| false);
    let mut show_quantization_info = use_signal(|| false);
    let mut show_model_cache_info = use_signal(|| false);
    let mut show_graph_storage_info = use_signal(|| false);

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                    BreadcrumbItem::new("NER", Some(Route::ConfigNer {})),
                ],
            }

            ConfigNav { active: ConfigTab::Ner }

            // ═══════════════════════════════════════════════════════════════
            // HEADER TILE
            // ═══════════════════════════════════════════════════════════════
            Panel { title: None, refresh: None,
                div { class: "flex items-center gap-4 flex-wrap",
                    span { class: "text-base text-gray-100 font-semibold", "Named Entity Extraction (NER)" }
                    Link {
                        to: Route::DocuEntitiesProduction {},
                        class: PARAM_ICON_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        InfoIcon {}
                    }
                    span { class: "text-xs text-cyan-400", "Changes require a backend restart to take effect." }
                    button {
                        class: "btn btn-xs text-white",
                        style: "background-color: #7C2A02; border-color: #7C2A02;",
                        disabled: saving(),
                        onclick: save_config,
                        if saving() { "Saving…" } else { "Save Configuration" }
                    }
                    if let Some(msg) = save_message() {
                        span { class: "text-xs text-gray-400", "{msg}" }
                    }
                    button {
                        class: "btn btn-xs text-white",
                        style: "background-color: #374151; border-color: #374151;",
                        disabled: restarting(),
                        onclick: move |_| {
                            restarting.set(true);
                            restart_msg.set(None);
                            spawn(async move {
                                match api::restart_backend().await {
                                    Ok(()) => restart_msg.set(Some("Restart initiated.".to_string())),
                                    Err(e) => restart_msg.set(Some(format!("Error: {e}"))),
                                }
                                restarting.set(false);
                            });
                        },
                        if restarting() { "Restarting…" } else { "Restart backend" }
                    }
                    if let Some(msg) = restart_msg() {
                        span { class: "text-xs text-gray-400", "{msg}" }
                    }
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // CONTROL + QUALITY
            // ═══════════════════════════════════════════════════════════════
            Panel { title: None, refresh: None,
                div { class: "flex flex-wrap gap-8",

                    // ── CONTROL ──────────────────────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-64",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Control" }
                        div { class: "flex flex-wrap gap-8",
                            div { class: PARAM_COLUMN_CLASS,

                                // ENTITY_EXTRACTION_ENABLED
                                div { class: PARAM_BLOCK_CLASS,
                                    div { class: "flex items-center gap-3",
                                        input {
                                            r#type: "checkbox",
                                            class: PARAM_CHECKBOX_CLASS,
                                            checked: extraction_enabled(),
                                            onchange: move |e| extraction_enabled.set(e.checked()),
                                        }
                                        label { class: PARAM_LABEL_CLASS, "ENTITY_EXTRACTION_ENABLED" }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_extraction_enabled_info.set(true),
                                            title: "Master toggle",
                                            InfoIcon {}
                                        }
                                    }
                                }

                                // ENTITY_CONTROL_TYPE_ALLOWLIST
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "ENTITY_CONTROL_TYPE_ALLOWLIST" }
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "text",
                                            class: PARAM_TEXT_INPUT_CLASS,
                                            value: "{type_allowlist()}",
                                            placeholder: "PERSON,ORGANIZATION,LOCATION,PRODUCT",
                                            oninput: move |e| type_allowlist.set(e.value()),
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_type_allowlist_info.set(true),
                                            title: "Entity type allowlist",
                                            InfoIcon {}
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // ── QUALITY ──────────────────────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-64",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Quality" }
                        div { class: "flex flex-wrap gap-8",
                            div { class: PARAM_COLUMN_CLASS,

                                // ENTITY_QUALITY_CONFIDENCE_THRESHOLD
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "ENTITY_QUALITY_CONFIDENCE_THRESHOLD" }
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "number",
                                            min: "0",
                                            max: "1",
                                            step: "0.05",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: "{confidence_threshold()}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<f64>() {
                                                    confidence_threshold.set(v.clamp(0.0, 1.0));
                                                }
                                            },
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_confidence_threshold_info.set(true),
                                            title: "Confidence threshold",
                                            InfoIcon {}
                                        }
                                    }
                                }

                                // ENTITY_QUALITY_TYPE_THRESHOLDS
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "ENTITY_QUALITY_TYPE_THRESHOLDS" }
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "text",
                                            class: PARAM_TEXT_INPUT_CLASS,
                                            value: "{type_thresholds()}",
                                            placeholder: "Per-type JSON thresholds",
                                            oninput: move |e| type_thresholds.set(e.value()),
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_type_thresholds_info.set(true),
                                            title: "Per-type thresholds",
                                            InfoIcon {}
                                        }
                                    }
                                }

                                // ENTITY_QUALITY_FUZZY_THRESHOLD
                                div { class: PARAM_BLOCK_CLASS,
                                    label { class: PARAM_LABEL_CLASS, "ENTITY_QUALITY_FUZZY_THRESHOLD" }
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "number",
                                            min: "0",
                                            max: "1",
                                            step: "0.05",
                                            class: PARAM_NUMBER_INPUT_CLASS,
                                            value: "{fuzzy_threshold()}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<f64>() {
                                                    fuzzy_threshold.set(v.clamp(0.0, 1.0));
                                                }
                                            },
                                        }
                                        button {
                                            class: PARAM_ICON_BUTTON_CLASS,
                                            style: PARAM_ICON_BUTTON_STYLE,
                                            onclick: move |_| show_fuzzy_threshold_info.set(true),
                                            title: "Fuzzy match threshold",
                                            InfoIcon {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // FILTERING + PERFORMANCE + INTEGRATION
            // ═══════════════════════════════════════════════════════════════
            Panel { title: None, refresh: None,
                div { class: "flex flex-wrap gap-8",

                    // ── FILTERING ─────────────────────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-56",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Filtering" }
                        div { class: PARAM_COLUMN_CLASS,

                            // ENTITY_FILTER_MIN_LENGTH
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "ENTITY_FILTER_MIN_LENGTH" }
                                div { class: "flex items-center gap-2",
                                    input {
                                        r#type: "number",
                                        min: "1",
                                        max: "50",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: "{min_length()}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<usize>() {
                                                min_length.set(v);
                                            }
                                        },
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_min_length_info.set(true),
                                        title: "Minimum entity length",
                                        InfoIcon {}
                                    }
                                }
                            }

                            // ENTITY_FILTER_MAX_LENGTH
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "ENTITY_FILTER_MAX_LENGTH" }
                                div { class: "flex items-center gap-2",
                                    input {
                                        r#type: "number",
                                        min: "10",
                                        max: "500",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: "{max_length()}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<usize>() {
                                                max_length.set(v);
                                            }
                                        },
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_max_length_info.set(true),
                                        title: "Maximum entity length",
                                        InfoIcon {}
                                    }
                                }
                            }

                            // ENTITY_FILTER_DEDUPLICATE_CASE_INSENSITIVE
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-3",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: dedup_case_insensitive(),
                                        onchange: move |e| dedup_case_insensitive.set(e.checked()),
                                    }
                                    label { class: PARAM_LABEL_CLASS, "ENTITY_FILTER_DEDUPLICATE_CASE_INSENSITIVE" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_dedup_info.set(true),
                                        title: "Case-insensitive deduplication",
                                        InfoIcon {}
                                    }
                                }
                            }

                            // ENTITY_FILTER_NESTING_STRATEGY
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "ENTITY_FILTER_NESTING_STRATEGY" }
                                div { class: "flex items-center gap-2",
                                    select {
                                        class: PARAM_SELECT_CLASS,
                                        value: "{nesting_strategy()}",
                                        onchange: move |e| nesting_strategy.set(e.value()),
                                        option { value: "KeepLongest", "KeepLongest" }
                                        option { value: "KeepAll", "KeepAll" }
                                        option { value: "KeepShortest", "KeepShortest" }
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_nesting_info.set(true),
                                        title: "Nesting strategy",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                    }

                    // ── PERFORMANCE ───────────────────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-56",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Performance" }
                        div { class: PARAM_COLUMN_CLASS,

                            // ENTITY_PERFORMANCE_BATCH_SIZE
                            div { class: PARAM_BLOCK_CLASS,
                                label { class: PARAM_LABEL_CLASS, "ENTITY_PERFORMANCE_BATCH_SIZE" }
                                div { class: "flex items-center gap-2",
                                    input {
                                        r#type: "number",
                                        min: "1",
                                        max: "64",
                                        class: PARAM_NUMBER_INPUT_CLASS,
                                        value: "{batch_size()}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<usize>() {
                                                batch_size.set(v);
                                            }
                                        },
                                    }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_batch_size_info.set(true),
                                        title: "Inference batch size",
                                        InfoIcon {}
                                    }
                                }
                            }

                            // ENTITY_PERFORMANCE_QUANTIZATION_ENABLED
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-3",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: quantization_enabled(),
                                        onchange: move |e| quantization_enabled.set(e.checked()),
                                    }
                                    label { class: PARAM_LABEL_CLASS, "ENTITY_PERFORMANCE_QUANTIZATION_ENABLED" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_quantization_info.set(true),
                                        title: "Model quantization",
                                        InfoIcon {}
                                    }
                                }
                            }

                            // ENTITY_PERFORMANCE_MODEL_CACHE_ENABLED
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-3",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: model_cache_enabled(),
                                        onchange: move |e| model_cache_enabled.set(e.checked()),
                                    }
                                    label { class: PARAM_LABEL_CLASS, "ENTITY_PERFORMANCE_MODEL_CACHE_ENABLED" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_model_cache_info.set(true),
                                        title: "Model cache",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                    }

                    // ── INTEGRATION ───────────────────────────────────────
                    div { class: "rounded border border-gray-600 p-4 flex-1 min-w-48",
                        span { class: "text-sm text-gray-300 font-semibold mb-3 block", "Integration" }
                        div { class: PARAM_COLUMN_CLASS,

                            // ENTITY_INTEGRATION_GRAPH_STORAGE_ENABLED
                            div { class: PARAM_BLOCK_CLASS,
                                div { class: "flex items-center gap-3",
                                    input {
                                        r#type: "checkbox",
                                        class: PARAM_CHECKBOX_CLASS,
                                        checked: graph_storage_enabled(),
                                        onchange: move |e| graph_storage_enabled.set(e.checked()),
                                    }
                                    label { class: PARAM_LABEL_CLASS, "ENTITY_INTEGRATION_GRAPH_STORAGE_ENABLED" }
                                    button {
                                        class: PARAM_ICON_BUTTON_CLASS,
                                        style: PARAM_ICON_BUTTON_STYLE,
                                        onclick: move |_| show_graph_storage_info.set(true),
                                        title: "Knowledge graph storage",
                                        InfoIcon {}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // ENV VAR REFERENCE TILE
            // ═══════════════════════════════════════════════════════════════
            Panel { title: None, refresh: None,
                div { class: "flex flex-col gap-2",
                    span { class: "text-sm text-gray-300 font-semibold", "Current .env values" }
                    span { class: "text-xs text-gray-500 italic mb-1",
                        "Read-only — edit .env and restart the backend to apply."
                    }
                    div { class: "text-xs font-mono text-gray-400 space-y-1 bg-gray-900 rounded p-3 border border-gray-700",
                        div { class: "text-gray-500", "# Control" }
                        div { "ENTITY_EXTRACTION_ENABLED={extraction_enabled()}" }
                        div { "ENTITY_CONTROL_TYPE_ALLOWLIST={type_allowlist()}" }
                        div { class: "text-gray-500 mt-1", "# Quality" }
                        div { "ENTITY_QUALITY_CONFIDENCE_THRESHOLD={confidence_threshold()}" }
                        div { "ENTITY_QUALITY_TYPE_THRESHOLDS='{type_thresholds()}'" }
                        div { "ENTITY_QUALITY_FUZZY_THRESHOLD={fuzzy_threshold()}" }
                        div { class: "text-gray-500 mt-1", "# Filtering" }
                        div { "ENTITY_FILTER_MIN_LENGTH={min_length()}" }
                        div { "ENTITY_FILTER_MAX_LENGTH={max_length()}" }
                        div { "ENTITY_FILTER_DEDUPLICATE_CASE_INSENSITIVE={dedup_case_insensitive()}" }
                        div { "ENTITY_FILTER_NESTING_STRATEGY={nesting_strategy()}" }
                        div { class: "text-gray-500 mt-1", "# Performance" }
                        div { "ENTITY_PERFORMANCE_BATCH_SIZE={batch_size()}" }
                        div { "ENTITY_PERFORMANCE_QUANTIZATION_ENABLED={quantization_enabled()}" }
                        div { "ENTITY_PERFORMANCE_MODEL_CACHE_ENABLED={model_cache_enabled()}" }
                        div { class: "text-gray-500 mt-1", "# Integration" }
                        div { "ENTITY_INTEGRATION_GRAPH_STORAGE_ENABLED={graph_storage_enabled()}" }
                    }
                }
            }
        }

        // ═══════════════════════════════════════════════════════════════
        // INFO MODALS
        // ═══════════════════════════════════════════════════════════════

        if show_extraction_enabled_info() {
            { info_modal("ENTITY_EXTRACTION_ENABLED", show_extraction_enabled_info, vec![
                "Master toggle for the entire NER pipeline.",
                "When false, no entity extraction runs — documents are indexed without entity annotations and the knowledge graph receives no entity data from ingestion.",
                "Default: true.",
            ]) }
        }
        if show_type_allowlist_info() {
            { info_modal("ENTITY_CONTROL_TYPE_ALLOWLIST", show_type_allowlist_info, vec![
                "Comma-separated list of entity types to extract. Only entities matching these types are kept; all others are discarded before quality filtering.",
                "Standard CoNLL/OntoNotes types: PERSON, ORGANIZATION, LOCATION, GPE, PRODUCT, EVENT, DATE, TIME, MONEY, PERCENT, LAW, LANGUAGE.",
                "Narrowing this list reduces noise in the knowledge graph and speeds up post-processing — if you only care about PERSON and ORGANIZATION, drop the rest.",
                "Default: PERSON,ORGANIZATION,LOCATION,PRODUCT.",
            ]) }
        }
        if show_confidence_threshold_info() {
            { info_modal("ENTITY_QUALITY_CONFIDENCE_THRESHOLD", show_confidence_threshold_info, vec![
                "Global minimum model confidence score (0–1) for an entity span to be accepted.",
                "Entities where the NER model's token-level confidence falls below this threshold are dropped before any further processing.",
                "Higher values (e.g. 0.90+) produce cleaner output with fewer false positives but may miss low-confidence but correct mentions.",
                "Lower values (e.g. 0.70) increase recall at the cost of precision — useful when downstream deduplication or graph inference can correct errors.",
                "Per-type overrides in ENTITY_QUALITY_TYPE_THRESHOLDS take precedence over this global value.",
                "Default: 0.85.",
            ]) }
        }
        if show_type_thresholds_info() {
            { info_modal("ENTITY_QUALITY_TYPE_THRESHOLDS", show_type_thresholds_info, vec![
                "JSON object mapping entity type names to per-type confidence thresholds.",
                "When a type appears in this map, its threshold overrides ENTITY_QUALITY_CONFIDENCE_THRESHOLD for that type only.",
                "Example: {\"PERSON\":0.75,\"ORGANIZATION\":0.95,\"PRODUCT\":0.95}",
                "PERSON is set lower (0.75) because names are often shorter and ambiguous, so a looser threshold recovers more correct mentions.",
                "ORGANIZATION and PRODUCT are set higher (0.95) because false positives for these types are more disruptive in knowledge graph queries.",
                "Types not listed in this map fall back to ENTITY_QUALITY_CONFIDENCE_THRESHOLD.",
            ]) }
        }
        if show_fuzzy_threshold_info() {
            { info_modal("ENTITY_QUALITY_FUZZY_THRESHOLD", show_fuzzy_threshold_info, vec![
                "Minimum fuzzy-match similarity score (0–1) used during deduplication and entity resolution.",
                "When two extracted entity strings are compared for sameness (e.g. 'Apple Inc' vs 'Apple'), the fuzzy score must meet this threshold for them to be merged.",
                "Lower values merge more aggressively — fewer unique nodes in the graph but higher risk of incorrectly collapsing distinct entities.",
                "Higher values keep entities separate unless they are nearly identical — safer but may leave near-duplicate nodes (e.g. different capitalizations of the same name).",
                "Default: 0.80.",
            ]) }
        }
        if show_min_length_info() {
            { info_modal("ENTITY_FILTER_MIN_LENGTH", show_min_length_info, vec![
                "Minimum character length of an entity span to be accepted.",
                "Single-character spans ('A', 'I') are almost always noise — this filter removes them before quality scoring.",
                "Raise this if you see many short false positives (abbreviations, initials) that pass the confidence threshold.",
                "Default: 2 characters.",
            ]) }
        }
        if show_max_length_info() {
            { info_modal("ENTITY_FILTER_MAX_LENGTH", show_max_length_info, vec![
                "Maximum character length of an entity span to be accepted.",
                "Very long spans (e.g. 150+ characters) are usually chunking errors where the model merged an entire clause into a single entity.",
                "Filtering them keeps the knowledge graph free of malformed nodes.",
                "Default: 100 characters.",
            ]) }
        }
        if show_dedup_info() {
            { info_modal("ENTITY_FILTER_DEDUPLICATE_CASE_INSENSITIVE", show_dedup_info, vec![
                "When true, deduplicate entity mentions case-insensitively within the same document.",
                "\"Apple\", \"APPLE\", and \"apple\" are treated as the same mention and collapsed to a single canonical form before graph storage.",
                "Disable only if case carries semantic meaning in your corpus (e.g. programming languages where 'Go' and 'go' are intentionally different).",
                "Default: true.",
            ]) }
        }
        if show_nesting_info() {
            { info_modal("ENTITY_FILTER_NESTING_STRATEGY", show_nesting_info, vec![
                "Strategy for resolving overlapping or nested entity spans.",
                "KeepLongest (default): When two spans overlap, keep only the longer one. 'New York City' wins over 'New York'. Best for most use cases.",
                "KeepAll: Retain all spans including nested ones. 'New York' and 'New York City' both survive. Useful if you need fine-grained entity graphs.",
                "KeepShortest: Keep only the innermost span. Rarely useful — mainly when you need atomic tokens rather than full named entities.",
            ]) }
        }
        if show_batch_size_info() {
            { info_modal("ENTITY_PERFORMANCE_BATCH_SIZE", show_batch_size_info, vec![
                "Number of text segments sent to the NER model in a single inference call.",
                "Larger batches improve GPU/CPU utilization by amortizing model loading overhead across more inputs.",
                "Larger batches also use more memory — on CPU-only deployments, keep this at 4–8.",
                "On GPU with 8+ GB VRAM, values of 16–32 are typical.",
                "Default: 4.",
            ]) }
        }
        if show_quantization_info() {
            { info_modal("ENTITY_PERFORMANCE_QUANTIZATION_ENABLED", show_quantization_info, vec![
                "Load the NER model in quantized form (INT8 or similar) to reduce memory footprint and speed up CPU inference.",
                "Quantization typically reduces model size by 2–4× with a small accuracy penalty (usually <1% F1 on standard benchmarks).",
                "Recommended when running on CPU with limited RAM, or when co-deploying with other models that compete for memory.",
                "Requires a quantized model variant to be available — the standard float32 model is used when this is false.",
                "Default: false.",
            ]) }
        }
        if show_model_cache_info() {
            { info_modal("ENTITY_PERFORMANCE_MODEL_CACHE_ENABLED", show_model_cache_info, vec![
                "Keep the NER model loaded in memory between extraction calls.",
                "When true, the model is loaded once at startup and held resident — subsequent calls pay only inference cost, not load cost.",
                "When false, the model is loaded and unloaded per batch. Frees memory between ingestion bursts but adds significant latency to the first call after idle.",
                "Disable only if you have severe memory pressure and ingestion is infrequent.",
                "Default: true.",
            ]) }
        }
        if show_graph_storage_info() {
            { info_modal("ENTITY_INTEGRATION_GRAPH_STORAGE_ENABLED", show_graph_storage_info, vec![
                "When true, extracted entities and their relationships are persisted to the Neo4j knowledge graph.",
                "Each entity becomes a graph node; co-occurrence and document provenance edges link entities to source documents and to each other.",
                "Disable if Neo4j is unavailable or if you want NER output for downstream use without graph persistence.",
                "Requires NEO4J_ENABLED=true and a running Neo4j instance — entity storage is silently skipped if the graph is unreachable.",
                "Default: true.",
            ]) }
        }
    }
}
