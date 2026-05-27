// ~/ag/frontend/fro/src/pages/terms.rs  v1.0
// Config page: manage custom entity recognition terms

use crate::components::config_nav::{ConfigNav, ConfigTab};
use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

const BACKEND: &str = "http://localhost:3010";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TermEntry {
    category: String,
    term: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
struct TermsApiResponse {
    status: String,
    terms: Vec<TermEntry>,
    file_path: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ExtractedEntity {
    text: String,
    #[serde(rename = "type")]
    entity_type: String,
    confidence: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
struct ExtractResponse {
    status: String,
    entity_count: usize,
    entities: Vec<ExtractedEntity>,
}

const CATEGORIES: &[(&str, &str)] = &[
    ("MED", "Medical"),
    ("TECH", "Technology"),
    ("ORG", "Organization"),
    ("LOC", "Location"),
    ("PERSON", "Person"),
    ("PRODUCT", "Product"),
    ("EVENT", "Event"),
];

fn category_color(cat: &str) -> &'static str {
    match cat {
        "MED" => "background-color:#991B1B;",
        "TECH" => "background-color:#1E40AF;",
        "ORG" => "background-color:#065F46;",
        "LOC" => "background-color:#92400E;",
        "PERSON" => "background-color:#5B21B6;",
        "PRODUCT" => "background-color:#0E7490;",
        "EVENT" => "background-color:#9D174D;",
        _ => "background-color:#374151;",
    }
}

#[component]
pub fn ConfigTerms() -> Element {
    let mut terms = use_signal(Vec::<TermEntry>::new);
    let mut file_path = use_signal(String::new);
    let mut loading = use_signal(|| true);
    let mut saving = use_signal(|| false);
    let mut status_msg = use_signal(|| Option::<String>::None);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut new_term = use_signal(String::new);
    let mut new_category = use_signal(|| "EVENT".to_string());
    let mut filter_cat = use_signal(|| "ALL".to_string());
    let mut show_info = use_signal(|| false);

    // Test section state
    let mut test_text = use_signal(String::new);
    let mut test_results = use_signal(|| Option::<Vec<ExtractedEntity>>::None);
    let mut testing = use_signal(|| false);
    let mut test_error = use_signal(|| Option::<String>::None);

    // Load terms on mount
    let _load = use_resource(move || async move {
        loading.set(true);
        error_msg.set(None);
        match gloo_net::http::Request::get(&format!("{}/config/entity_terms", BACKEND))
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(data) = resp.json::<TermsApiResponse>().await {
                    terms.set(data.terms);
                    file_path.set(data.file_path);
                } else {
                    error_msg.set(Some("Failed to parse response".into()));
                }
            }
            Err(e) => {
                error_msg.set(Some(format!("Load failed: {}", e)));
            }
        }
        loading.set(false);
    });

    // Add term
    let on_add = move |_: MouseEvent| {
        let t = new_term().trim().to_string();
        if t.is_empty() {
            return;
        }
        let cat = new_category();
        let mut current = terms();
        let exists = current
            .iter()
            .any(|e| e.category == cat && e.term.to_lowercase() == t.to_lowercase());
        if !exists {
            current.push(TermEntry {
                category: cat,
                term: t,
            });
            terms.set(current);
            new_term.set(String::new());
            status_msg.set(Some("Term added — click Save to persist".into()));
        } else {
            status_msg.set(Some("Term already exists in this category".into()));
        }
    };

    // Save to backend
    let on_save = move |_: MouseEvent| {
        let current_terms = terms();
        spawn(async move {
            saving.set(true);
            error_msg.set(None);
            status_msg.set(None);
            let payload = serde_json::json!({ "terms": current_terms });
            match gloo_net::http::Request::post(&format!("{}/config/entity_terms", BACKEND))
                .header("Content-Type", "application/json")
                .body(payload.to_string())
                .unwrap()
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.ok() {
                        status_msg.set(Some(
                            "Terms saved. Restart ag.service or re-index for changes to take effect.".into()
                        ));
                    } else {
                        error_msg.set(Some("Save failed — check backend logs".into()));
                    }
                }
                Err(e) => {
                    error_msg.set(Some(format!("Save failed: {}", e)));
                }
            }
            saving.set(false);
        });
    };

    // Filtered view
    let filtered: Vec<(usize, TermEntry)> = terms()
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            let f = filter_cat();
            f == "ALL" || e.category == f
        })
        .map(|(i, e)| (i, e.clone()))
        .collect();

    rsx! {
        div { class: "p-4 max-w-5xl mx-auto",
            ConfigNav { active: ConfigTab::Terms }

            div { class: "mt-6",
                // Header + info button
                div { class: "flex items-center gap-2 mb-3",
                    h3 { class: "text-xl font-bold text-gray-100",
                        "Entity Recognition Terms"
                    }
                    button {
                        class: PARAM_ICON_BUTTON_CLASS,
                        style: PARAM_ICON_BUTTON_STYLE,
                        onclick: move |_| show_info.set(!show_info()),
                        svg {
                            class: INFO_ICON_SVG_CLASS,
                            view_box: "0 0 20 20",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.5",
                            circle { cx: "10", cy: "10", r: "9" }
                            line { x1: "10", y1: "8", x2: "10", y2: "14" }
                            circle {
                                cx: "10", cy: "6.3", r: "1",
                                fill: "currentColor", stroke: "none",
                            }
                        }
                    }
                }

                // Info modal
                if show_info() {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| show_info.set(false),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-xl max-h-[90vh] overflow-y-auto shadow-xl",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-center justify-between mb-4",
                                h2 { class: "text-lg font-semibold text-gray-100",
                                    "About Entity Terms"
                                }
                                button {
                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                    onclick: move |_| show_info.set(false),
                                    "×"
                                }
                            }
                            div { class: "text-sm text-gray-300 space-y-3",
                                p {
                                    "Custom terms let the entity extractor recognize domain-specific vocabulary that the built-in rules don't cover."
                                }
                                p {
                                    "When documents are indexed, the extractor scans each chunk for these terms and tags matching text with the chosen category."
                                }
                                p {
                                    "Example: adding \"Renaissance\" as EVENT means any uploaded history document containing that phrase will have it tagged as an Event entity in the knowledge graph."
                                }
                                p {
                                    "After saving, restart ag.service or trigger a re-index for existing documents to pick up the new terms."
                                }
                                p { class: "text-gray-400 text-xs",
                                    "File: {file_path}"
                                }
                            }
                        }
                    }
                }

                p { class: "text-sm text-gray-400 mb-4",
                    "Add domain-specific terms the entity extractor should recognize during indexing."
                }

                // Status messages
                if let Some(msg) = status_msg() {
                    div {
                        class: "text-sm text-green-400 mb-3 px-3 py-2 rounded",
                        style: "background-color:#064E3B;",
                        "{msg}"
                    }
                }
                if let Some(msg) = error_msg() {
                    div {
                        class: "text-sm text-red-400 mb-3 px-3 py-2 rounded",
                        style: "background-color:#7F1D1D;",
                        "{msg}"
                    }
                }

                // Add term form
                div {
                    class: "flex flex-wrap items-end gap-2 mb-4 p-3 rounded",
                    style: "background-color:#1F2937;",
                    div {
                        label { class: "block text-xs text-gray-400 mb-1", "Category" }
                        select {
                            class: "select select-xs bg-gray-700 text-gray-200",
                            value: "{new_category}",
                            onchange: move |evt| new_category.set(evt.value()),
                            for &(code, label) in CATEGORIES.iter() {
                                option { value: code, "{label} ({code})" }
                            }
                        }
                    }
                    div { class: "flex-1 min-w-[200px]",
                        label { class: "block text-xs text-gray-400 mb-1", "Term" }
                        input {
                            class: "input input-xs input-bordered bg-gray-700 text-gray-200 w-full",
                            r#type: "text",
                            placeholder: "e.g. Battle of Waterloo, Treaty of Versailles, Renaissance",
                            value: "{new_term}",
                            oninput: move |evt| new_term.set(evt.value()),
                        }
                    }
                    button {
                        class: "btn btn-xs btn-primary",
                        onclick: on_add,
                        "Add"
                    }
                }

                // Filter + count + save bar
                div { class: "flex flex-wrap items-center gap-3 mb-3",
                    select {
                        class: "select select-xs bg-gray-700 text-gray-200",
                        value: "{filter_cat}",
                        onchange: move |evt| filter_cat.set(evt.value()),
                        option { value: "ALL", "All categories" }
                        for &(code, label) in CATEGORIES.iter() {
                            option { value: code, "{label}" }
                        }
                    }
                    span { class: "text-xs text-gray-300",
                        "{filtered.len()} of {terms().len()} terms"
                    }
                    div { class: "ml-auto",
                        button {
                            class: "btn btn-xs btn-success",
                            disabled: saving(),
                            onclick: on_save,
                            if saving() { "Saving..." } else { "Save to disk" }
                        }
                    }
                }

                // Terms table
                if loading() {
                    p { class: "text-gray-400 text-sm", "Loading terms..." }
                } else if filtered.is_empty() {
                    p { class: "text-gray-300 text-sm italic",
                        "No terms yet. Add your first term above."
                    }
                } else {
                    div {
                        class: "overflow-x-auto rounded",
                        style: "max-height:60vh; overflow-y:auto;",
                        table { class: "table table-xs w-full",
                            thead {
                                tr { class: "text-gray-400",
                                    th { style: "width:100px;", "Category" }
                                    th { "Term" }
                                    th { style: "width:50px;", "" }
                                }
                            }
                            tbody {
                                for (idx, entry) in filtered.iter() {
                                    {
                                        let delete_idx = *idx;
                                        let cat_style = category_color(&entry.category);
                                        let cat_label = entry.category.clone();
                                        let term_label = entry.term.clone();
                                        rsx! {
                                            tr { class: "hover:bg-gray-800",
                                                td {
                                                    span {
                                                        class: "text-xs px-2 py-0.5 rounded text-white font-mono",
                                                        style: cat_style,
                                                        "{cat_label}"
                                                    }
                                                }
                                                td { class: "text-gray-200 text-sm",
                                                    "{term_label}"
                                                }
                                                td {
                                                    button {
                                                        class: "btn btn-xs btn-ghost text-red-400 hover:text-red-300",
                                                        onclick: move |_| {
                                                            let mut current = terms();
                                                            if delete_idx < current.len() {
                                                                current.remove(delete_idx);
                                                                terms.set(current);
                                                                status_msg.set(Some(
                                                                    "Term removed — click Save to persist".into()
                                                                ));
                                                            }
                                                        },
                                                        "×"
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

                // ── Test your terms ──
                div { class: "mt-6 p-3 rounded",
                    style: "background-color:#1F2937;",
                    h4 { class: "text-sm font-semibold text-gray-200 mb-2",
                        "Test your terms"
                    }
                    p { class: "text-xs text-gray-400 mb-2",
                        "Paste text to see which entities the extractor finds. Uses the currently saved terms on disk."
                    }
                    textarea {
                        class: "textarea textarea-bordered bg-gray-700 text-gray-200 w-full text-sm",
                        rows: "3",
                        placeholder: "e.g. Napoleon Bonaparte fought at the Battle of Waterloo in 1815.",
                        value: "{test_text}",
                        oninput: move |evt| test_text.set(evt.value()),
                    }
                    div { class: "flex items-center gap-2 mt-2",
                        button {
                            class: "btn btn-xs btn-info",
                            disabled: testing() || test_text().trim().is_empty(),
                            onclick: move |_| {
                                let txt = test_text();
                                spawn(async move {
                                    testing.set(true);
                                    test_error.set(None);
                                    test_results.set(None);
                                    let payload = serde_json::json!({ "text": txt });
                                    match gloo_net::http::Request::post(
                                        &format!("{}/extract_entities", BACKEND),
                                    )
                                    .header("Content-Type", "application/json")
                                    .body(payload.to_string())
                                    .unwrap()
                                    .send()
                                    .await
                                    {
                                        Ok(resp) => {
                                            if let Ok(data) = resp.json::<ExtractResponse>().await {
                                                test_results.set(Some(data.entities));
                                            } else {
                                                test_error.set(Some("Failed to parse response".into()));
                                            }
                                        }
                                        Err(e) => {
                                            test_error.set(Some(format!("Request failed: {}", e)));
                                        }
                                    }
                                    testing.set(false);
                                });
                            },
                            if testing() { "Testing..." } else { "Extract entities" }
                        }
                        if let Some(results) = test_results() {
                            span { class: "text-xs text-gray-400",
                                "{results.len()} entities found"
                            }
                        }
                    }

                    // Test error
                    if let Some(msg) = test_error() {
                        div { class: "text-sm text-red-400 mt-2 px-2 py-1 rounded",
                            style: "background-color:#7F1D1D;",
                            "{msg}"
                        }
                    }

                    // Test results
                    if let Some(results) = test_results() {
                        if results.is_empty() {
                            p { class: "text-xs text-gray-300 mt-2 italic",
                                "No entities found. Try adding more terms above and saving first."
                            }
                        } else {
                            div { class: "mt-2 flex flex-wrap gap-2",
                                for ent in results.iter() {
                                    {
                                        let cs = category_color(&ent.entity_type);
                                        let lbl = ent.entity_type.clone();
                                        let txt = ent.text.clone();
                                        let conf = (ent.confidence * 100.0) as u32;
                                        rsx! {
                                            span {
                                                class: "inline-flex items-center gap-1 px-2 py-1 rounded text-xs text-white",
                                                style: cs,
                                                span { class: "font-mono text-[10px] opacity-70", "{lbl}" }
                                                span { class: "font-medium", "{txt}" }
                                                span { class: "opacity-50 text-[10px]", "{conf}%" }
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
