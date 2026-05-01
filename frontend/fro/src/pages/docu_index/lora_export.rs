//! LoRA Export documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuLoraExport() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full space-y-3",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "LoRA Export" }
                    span { class: "text-xs text-gray-400",
                        "Controls the LoRA snapshot pipeline via "
                        code { class: "text-xs", "/training/export_snapshot" }
                        "."
                    }
                }

                // What LoRA is
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 space-y-3",
                    h2 { class: "text-sm font-bold text-white", "What LoRA is" }
                    p { class: "text-xs text-gray-300 leading-relaxed",
                        "LoRA stands for "
                        span { class: "text-white font-semibold", "Low-Rank Adaptation" }
                        ". It is a fine-tuning technique: a way to update a pre-trained language model so it learns from new data without retraining the whole model. Full fine-tuning would update all parameters — billions of numbers — which is slow and expensive. LoRA is efficient: it freezes the original model entirely and inserts small trainable "
                        span { class: "text-white font-semibold", "adapter matrices" }
                        " at key layers. Only those adapters are trained. They are \"low-rank\" — each adapter is two small matrices whose product approximates the full weight update — so they are fast to train and small to store or swap."
                    }
                    p { class: "text-xs text-gray-300 leading-relaxed",
                        "The adapters capture what is specific to your domain. The base model's general language ability is preserved unchanged."
                    }
                }

                // What it does in ag
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 space-y-3",
                    h2 { class: "text-sm font-bold text-white", "What LoRA export does in ag" }
                    p { class: "text-xs text-gray-300 leading-relaxed",
                        "When you upload documents, they go through the Text Ingestion Pipeline — parsed, chunked, embedded, indexed. That makes them "
                        span { class: "text-white font-semibold", "searchable" }
                        " (RAG path). LoRA export is the parallel "
                        span { class: "text-white font-semibold", "training path" }
                        ": it takes those same indexed chunks and generates synthetic question-answer pairs from each one. The LLM reads the chunk and writes questions a reader might ask, plus the answers. Those pairs become supervised training examples."
                    }
                    div { class: "grid grid-cols-1 md:grid-cols-4 gap-2 text-xs",
                        div { class: "bg-gray-900 rounded p-2 text-center",
                            div { class: "text-gray-400 mb-1", "① corpus" }
                            div { class: "text-gray-200", "indexed document chunks" }
                        }
                        div { class: "bg-gray-900 rounded p-2 text-center",
                            div { class: "text-gray-400 mb-1", "② generate" }
                            div { class: "text-gray-200", "synthetic Q&A pairs per chunk" }
                        }
                        div { class: "bg-gray-900 rounded p-2 text-center",
                            div { class: "text-gray-400 mb-1", "③ export" }
                            div { class: "text-gray-200", "JSONL training examples" }
                        }
                        div { class: "bg-gray-900 rounded p-2 text-center",
                            div { class: "text-gray-400 mb-1", "④ fine-tune" }
                            div { class: "text-gray-200", "LoRA adapters trained on your corpus" }
                        }
                    }
                    p { class: "text-xs text-gray-300 leading-relaxed",
                        "The result is a model that has "
                        span { class: "text-white font-semibold", "internalized" }
                        " knowledge from your corpus as weights — not as retrieved context. It knows your domain without needing to look things up at inference time."
                    }
                }

                // RAG vs fine-tuning
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 space-y-2",
                    h2 { class: "text-sm font-bold text-white", "RAG vs fine-tuning vs both" }
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-2 text-xs",
                        div { class: "bg-gray-900 rounded p-2 space-y-1",
                            div { class: "text-teal-300 font-semibold", "RAG only" }
                            p { class: "text-gray-300", "Base model stays generic. Your documents are retrieved at inference time and injected into the prompt. Knowledge is always current but costs tokens and a retrieval hop per query." }
                        }
                        div { class: "bg-gray-900 rounded p-2 space-y-1",
                            div { class: "text-purple-300 font-semibold", "Fine-tuning only" }
                            p { class: "text-gray-300", "Knowledge baked into weights. No retrieval overhead. But the model only knows what was in the corpus at training time — it cannot incorporate documents added later." }
                        }
                        div { class: "bg-gray-900 rounded p-2 space-y-1",
                            div { class: "text-green-300 font-semibold", "Fine-tuned + RAG" }
                            p { class: "text-gray-300", "Baked-in domain fluency from fine-tuning plus up-to-date retrieval from RAG. The model reasons in your domain naturally and still has access to the latest documents." }
                        }
                    }
                }

                // Controls reference
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4",
                    h2 { class: "text-sm font-bold text-white mb-2", "Export controls reference" }
                    div { class: "grid grid-cols-3 gap-2",

                        div { class: "space-y-2",
                            div { class: "bg-gray-900 rounded p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "Status card" }
                                p { class: "text-xs text-gray-300",
                                    "Shows the live job state reported by the backend (running, idle, or last error) plus timestamps from the last run."
                                }
                            }
                            div { class: "bg-gray-900 rounded p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "Run Export" }
                                p { class: "text-xs text-gray-300",
                                    "Immediately launches " code { class: "text-green-300", "export_docs.py" } " followed by " code { class: "text-green-300", "normalize_dataset.py" } ". Respects whatever filter is configured."
                                }
                            }
                        }

                        div { class: "space-y-2",
                            div { class: "bg-gray-900 rounded p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "Auto-export after upload" }
                                p { class: "text-xs text-gray-300",
                                    "When enabled, every successful document upload batch schedules a LoRA export after the debounce window."
                                }
                            }
                            div { class: "bg-gray-900 rounded p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "Filter override" }
                                p { class: "text-xs text-gray-300",
                                    "Writes to " code { class: "text-green-300", "LORA_EXPORT_ONLY" } " in-memory before the scripts run. Provide comma-separated paths relative to " code { class: "text-green-300", "documents/" } ". Leave blank to export everything."
                                }
                            }
                        }

                        div { class: "bg-gray-900 rounded p-2",
                            h3 { class: "text-xs font-bold text-white mb-1", "Direct API equivalents" }
                            p { class: "text-xs text-gray-400 mb-1",
                                "Talks to the same endpoints that power the CLI scripts under " code { class: "text-xs", "tools/lora_training/" } "."
                            }
                            ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                                li { code { class: "text-green-300", "POST /training/export_snapshot" } }
                                li { code { class: "text-green-300", "GET/POST /training/export_snapshot/config" } }
                                li { code { class: "text-green-300", "POST /training/export_snapshot/filter" } }
                            }
                        }
                    }
                }

                div { class: "mt-2 pt-2 border-t border-gray-700",
                    Link { to: Route::DocuIndex {}, class: "btn btn-primary btn-xs", "← Back to Index" }
                }
            }
        }
    }
}
