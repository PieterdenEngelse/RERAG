//! LoRA Export documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuLoraExport() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-6",
            div { class: "max-w-4xl mx-auto",

                div { class: "flex items-center gap-4 mb-6",
                    Link {
                        to: Route::DocuIndex {},
                        class: "text-primary hover:underline",
                        "← Back to Index"
                    }
                }

                    h2 { class: "text-2xl font-bold text-white mb-4", "LoRA Export" }
                    div { class: "space-y-4 text-sm text-gray-200",
                        div { class: "space-y-1",
                            p {
                                "This board controls the entire LoRA snapshot pipeline. It talks to "
                                code { "/training/export_snapshot" }
                                ", the same endpoints that power the CLI scripts under "
                                code { "tools/lora_training/" }
                                "."
                            }
                            p {
                                "Use it when you need a fresh JSONL dataset for fine-tuning or when you want uploads to trigger exports automatically without touching env files."
                            }
                        }
                        div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                            div { class: "space-y-1",
                                strong { "Status card" }
                                p {
                                    "Shows the live job state reported by the backend (running, idle, or last error) plus timestamps from the last run."
                                }
                            }
                            div { class: "space-y-1",
                                strong { "Run Export" }
                                p {
                                    "Immediately launches "
                                    code { "export_docs.py" }
                                    " followed by "
                                    code { "normalize_dataset.py" }
                                    ". Respects whatever filter is configured."
                                }
                            }
                            div { class: "space-y-1",
                                strong { "Auto-export after upload" }
                                p {
                                    "When enabled, every successful document upload batch schedules a LoRA export after the debounce window."
                                }
                            }
                            div { class: "space-y-1",
                                strong { "Filter override" }
                                p {
                                    "Writes to "
                                    code { "LORA_EXPORT_ONLY" }
                                    " in-memory before the scripts run. Provide comma-separated paths relative to "
                                    code { "documents/" }
                                    ". Leave blank to export everything."
                                }
                            }
                        }
                        div { class: "space-y-1 text-xs text-gray-400",
                            p { "Direct API equivalents:" }
                            ul { class: "list-disc ml-5 space-y-1",
                                li { code { "POST /training/export_snapshot" } }
                                li { code { "GET/POST /training/export_snapshot/config" } }
                                li { code { "POST /training/export_snapshot/filter" } }
                            }
                        }
                    }

                div { class: "mt-8 pt-4 border-t border-gray-700",
                    Link {
                        to: Route::DocuIndex {},
                        class: "btn btn-primary btn-sm",
                        "← Back to Index"
                    }
                }
            }
        }
    }
}
