// src/pages/train.rs
// Training Data Collection & Custom Model Management Page

use crate::api::{self, TrainingFeedbackRequest, TrainingStats};
use crate::app::PageErrors;
use dioxus::prelude::*;

#[component]
pub fn Train() -> Element {
    // State
    let mut stats = use_signal(TrainingStats::default);
    let mut collection_enabled = use_signal(|| false);
    let mut loading = use_signal(|| true);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut success_msg = use_signal(|| Option::<String>::None);
    let mut exporting = use_signal(|| false);
    let mut clearing = use_signal(|| false);

    // Manual feedback form state
    let mut feedback_query = use_signal(String::new);
    let mut feedback_response = use_signal(String::new);
    let mut feedback_context = use_signal(String::new);
    let mut feedback_score = use_signal(|| 4u8);
    let mut submitting_feedback = use_signal(|| false);

    // Get global page errors context
    let mut page_errors = use_context::<Signal<PageErrors>>();

    // Load stats on mount
    use_effect(move || {
        spawn(async move {
            page_errors.with_mut(|e| e.clear_error("train"));
            match api::get_training_stats().await {
                Ok(resp) => {
                    stats.set(resp.stats);
                    collection_enabled.set(resp.collection_enabled);
                    loading.set(false);
                    page_errors.with_mut(|e| e.clear_error("train"));
                }
                Err(e) => {
                    let err = format!("Failed to load stats: {}", e);
                    error_msg.set(Some(err.clone()));
                    loading.set(false);
                    page_errors.with_mut(|errs| errs.set_error("train", &err));
                    let _ = api::log_frontend_error("train", &err).await;
                }
            }
        });
    });

    // Refresh stats
    let refresh_stats = move |_| {
        spawn(async move {
            loading.set(true);
            match api::get_training_stats().await {
                Ok(resp) => {
                    stats.set(resp.stats);
                    collection_enabled.set(resp.collection_enabled);
                    error_msg.set(None);
                }
                Err(e) => {
                    error_msg.set(Some(format!("Failed to refresh: {}", e)));
                }
            }
            loading.set(false);
        });
    };

    // Export training data
    let export_data = move |_| {
        spawn(async move {
            exporting.set(true);
            error_msg.set(None);
            success_msg.set(None);

            match api::export_training_data().await {
                Ok(resp) => {
                    success_msg.set(Some(format!(
                        "Exported {} examples to {}",
                        resp.exported_count, resp.output_path
                    )));
                }
                Err(e) => {
                    error_msg.set(Some(format!("Export failed: {}", e)));
                }
            }
            exporting.set(false);
        });
    };

    // Clear training data
    let clear_data = move |_| {
        spawn(async move {
            clearing.set(true);
            error_msg.set(None);
            success_msg.set(None);

            match api::clear_training_data().await {
                Ok(_) => {
                    success_msg.set(Some("Training data cleared".to_string()));
                    // Refresh stats
                    if let Ok(resp) = api::get_training_stats().await {
                        stats.set(resp.stats);
                    }
                }
                Err(e) => {
                    error_msg.set(Some(format!("Clear failed: {}", e)));
                }
            }
            clearing.set(false);
        });
    };

    // Submit manual feedback
    let submit_feedback = move |_| {
        let query = feedback_query();
        let response = feedback_response();
        let context = feedback_context();
        let score = feedback_score();

        if query.trim().is_empty() || response.trim().is_empty() {
            error_msg.set(Some("Query and response are required".to_string()));
            return;
        }

        spawn(async move {
            submitting_feedback.set(true);
            error_msg.set(None);
            success_msg.set(None);

            let feedback = TrainingFeedbackRequest {
                query: query.clone(),
                response: response.clone(),
                context: if context.trim().is_empty() {
                    None
                } else {
                    Some(context.clone())
                },
                quality_score: score,
                conversation_id: None,
                mode: Some("manual".to_string()),
                model: None,
            };

            match api::submit_training_feedback(feedback).await {
                Ok(resp) => {
                    if resp.status == "collected" {
                        success_msg.set(Some("Feedback submitted!".to_string()));
                        // Clear form
                        feedback_query.set(String::new());
                        feedback_response.set(String::new());
                        feedback_context.set(String::new());
                        feedback_score.set(4);
                        // Refresh stats
                        if let Ok(stats_resp) = api::get_training_stats().await {
                            stats.set(stats_resp.stats);
                        }
                    } else {
                        error_msg.set(Some(resp.message));
                    }
                }
                Err(e) => {
                    error_msg.set(Some(format!("Submit failed: {}", e)));
                }
            }
            submitting_feedback.set(false);
        });
    };

    // Calculate progress percentage
    let progress_pct = (stats().usable_count as f32 / 500.0 * 100.0).min(100.0);

    rsx! {
        div {
            class: "min-h-screen bg-base-200 p-4",
            "data-theme": "dark",

            // Header
            div {
                class: "max-w-4xl mx-auto mb-6",
                h1 {
                    class: "text-2xl font-bold text-white mb-2",
                    "🎓 Training Data Collection"
                }
                p {
                    class: "text-base-content/70",
                    "Collect high-quality examples to fine-tune a custom model for your RAG system."
                }
            }

            // Error/Success messages
            if let Some(err) = error_msg() {
                div {
                    class: "max-w-4xl mx-auto mb-4 alert alert-error",
                    span { "{err}" }
                    button {
                        class: "btn btn-ghost btn-xs",
                        onclick: move |_| error_msg.set(None),
                        "✕"
                    }
                }
            }
            if let Some(msg) = success_msg() {
                div {
                    class: "max-w-4xl mx-auto mb-4 alert alert-success",
                    span { "{msg}" }
                    button {
                        class: "btn btn-ghost btn-xs",
                        onclick: move |_| success_msg.set(None),
                        "✕"
                    }
                }
            }

            // Main content
            div {
                class: "max-w-4xl mx-auto grid gap-4",

                // Stats Card
                div {
                    class: "card bg-base-100 shadow-xl",
                    div {
                        class: "card-body",

                        div {
                            class: "flex justify-between items-center mb-4",
                            h2 { class: "card-title", "📊 Collection Progress" }
                            div {
                                class: "flex gap-2",
                                button {
                                    class: "btn btn-ghost btn-sm",
                                    onclick: refresh_stats,
                                    disabled: loading(),
                                    if loading() { "⏳" } else { "🔄" }
                                    " Refresh"
                                }
                            }
                        }

                        // Status badge
                        div {
                            class: "mb-4",
                            if collection_enabled() {
                                span {
                                    class: "badge badge-success gap-1",
                                    "✓ Collection Enabled"
                                }
                            } else {
                                span {
                                    class: "badge badge-warning gap-1",
                                    "⚠ Collection Disabled"
                                }
                                p {
                                    class: "text-xs text-base-content/60 mt-1",
                                    "Set TRAINING_DATA_ENABLED=true to enable automatic collection"
                                }
                            }
                        }

                        // Progress bar
                        div {
                            class: "mb-4",
                            div {
                                class: "flex justify-between text-sm mb-1",
                                span { "Progress to training-ready (500 examples)" }
                                span { "{stats().usable_count} / 500" }
                            }
                            progress {
                                class: "progress progress-primary w-full",
                                value: "{progress_pct}",
                                max: "100"
                            }
                            if stats().ready_for_export {
                                p {
                                    class: "text-success text-sm mt-1",
                                    "✓ Ready for export and training!"
                                }
                            }
                        }

                        // Stats grid
                        div {
                            class: "grid grid-cols-2 md:grid-cols-4 gap-4",

                            div {
                                class: "stat bg-base-200 rounded-lg p-3",
                                div { class: "stat-title text-xs", "Total" }
                                div { class: "stat-value text-lg", "{stats().total_examples}" }
                            }

                            div {
                                class: "stat bg-base-200 rounded-lg p-3",
                                div { class: "stat-title text-xs", "Usable (≥3)" }
                                div { class: "stat-value text-lg text-success", "{stats().usable_count}" }
                            }

                            div {
                                class: "stat bg-base-200 rounded-lg p-3",
                                div { class: "stat-title text-xs", "High Quality (≥4)" }
                                div { class: "stat-value text-lg text-primary", "{stats().high_quality_count}" }
                            }

                            div {
                                class: "stat bg-base-200 rounded-lg p-3",
                                div { class: "stat-title text-xs", "Avg Quality" }
                                div { class: "stat-value text-lg", "{stats().average_quality:.1}" }
                            }
                        }

                        // Action buttons
                        div {
                            class: "card-actions justify-end mt-4",
                            button {
                                class: "btn btn-outline btn-error btn-sm",
                                onclick: clear_data,
                                disabled: clearing() || stats().total_examples == 0,
                                if clearing() { "Clearing..." } else { "🗑️ Clear All" }
                            }
                            button {
                                class: "btn btn-primary btn-sm",
                                onclick: export_data,
                                disabled: exporting() || stats().usable_count == 0,
                                if exporting() { "Exporting..." } else { "📦 Export for Unsloth" }
                            }
                        }
                    }
                }

                // Manual Feedback Card
                div {
                    class: "card bg-base-100 shadow-xl",
                    div {
                        class: "card-body",

                        div {
                            class: "flex justify-between items-center mb-4",
                            h2 { class: "card-title", "✍️ Add Training Example (Manual)" }
                            button {
                                class: "btn btn-success btn-lg text-lg",
                                onclick: submit_feedback,
                                disabled: submitting_feedback(),
                                if submitting_feedback() { "Saving..." } else { "💾 Save Example" }
                            }
                        }

                        // Query input
                        div {
                            class: "form-control mb-3",
                            label {
                                class: "label",
                                span { class: "label-text", "Query (Instruction)" }
                            }
                            textarea {
                                class: "textarea textarea-bordered h-20",
                                style: "margin-left: 1cm;",
                                placeholder: "What question was asked?",
                                value: "{feedback_query}",
                                oninput: move |evt| feedback_query.set(evt.value())
                            }
                        }

                        // Context input
                        div {
                            class: "form-control mb-3",
                            label {
                                class: "label",
                                span { class: "label-text", "Context (Optional - RAG retrieved content)" }
                            }
                            textarea {
                                class: "textarea textarea-bordered h-20",
                                style: "margin-left: 1cm;",
                                placeholder: "What context was provided?",
                                value: "{feedback_context}",
                                oninput: move |evt| feedback_context.set(evt.value())
                            }
                        }

                        // Response input
                        div {
                            class: "form-control mb-3",
                            label {
                                class: "label",
                                span { class: "label-text", "Response (Output)" }
                            }
                            textarea {
                                class: "textarea textarea-bordered h-24",
                                style: "margin-left: 1cm;",
                                placeholder: "What was the ideal response?",
                                value: "{feedback_response}",
                                oninput: move |evt| feedback_response.set(evt.value())
                            }
                        }

                        // Quality score
                        div {
                            class: "form-control mb-4",
                            label {
                                class: "label",
                                span { class: "label-text", "Quality Score" }
                            }
                            div {
                                class: "flex gap-2",
                                for score in 1..=5 {
                                    button {
                                        class: if feedback_score() == score {
                                            "btn btn-primary btn-sm"
                                        } else {
                                            "btn btn-outline btn-sm"
                                        },
                                        onclick: move |_| feedback_score.set(score),
                                        match score {
                                            1 => "😞 1",
                                            2 => "😕 2",
                                            3 => "😐 3",
                                            4 => "🙂 4",
                                            5 => "😊 5",
                                            _ => ""
                                        }
                                    }
                                }
                            }
                        }


                    }
                }

                // Instructions Card
                div {
                    class: "card bg-base-100 shadow-xl",
                    div {
                        class: "card-body",

                        h2 { class: "card-title mb-4", "📖 How to Train a Custom Model" }

                        div {
                            class: "steps steps-vertical",

                            div {
                                class: "step step-primary",
                                div {
                                    class: "text-left ml-4",
                                    p { class: "font-medium", "1. Collect Training Data" }
                                    p { class: "text-sm text-base-content/70", "Use the RAG system and rate responses, or add examples manually above. Aim for 500+ high-quality examples." }
                                }
                            }

                            div {
                                class: if stats().ready_for_export { "step step-primary" } else { "step" },
                                div {
                                    class: "text-left ml-4",
                                    p { class: "font-medium", "2. Export Data" }
                                    p { class: "text-sm text-base-content/70", "Click 'Export for Unsloth' to create a JSONL file in Alpaca format." }
                                }
                            }

                            div {
                                class: "step",
                                div {
                                    class: "text-left ml-4",
                                    p { class: "font-medium", "3. Fine-tune with Unsloth" }
                                    p { class: "text-sm text-base-content/70", "Upload to Google Colab (free T4 GPU) and run the training notebook." }
                                }
                            }

                            div {
                                class: "step",
                                div {
                                    class: "text-left ml-4",
                                    p { class: "font-medium", "4. Deploy to Ollama" }
                                    p { class: "text-sm text-base-content/70", "Download the GGUF file and import with: ollama create ag-custom -f Modelfile" }
                                }
                            }

                            div {
                                class: "step",
                                div {
                                    class: "text-left ml-4",
                                    p { class: "font-medium", "5. Enable Custom Model" }
                                    p { class: "text-sm text-base-content/70", "Set CUSTOM_MODEL_ENABLED=true and CUSTOM_MODEL_NAME=ag-custom" }
                                }
                            }
                        }

                        // Links
                        div {
                            class: "mt-4 flex gap-2 flex-wrap",
                            a {
                                class: "btn btn-outline btn-sm",
                                href: "https://colab.research.google.com/",
                                target: "_blank",
                                "🔗 Google Colab"
                            }
                            a {
                                class: "btn btn-outline btn-sm",
                                href: "https://unsloth.ai/docs",
                                target: "_blank",
                                "📚 Unsloth Docs"
                            }
                        }
                    }
                }
            }
        }
    }
}
