//! LoRA Export documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuLoraExport() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "LoRA Export" }
                    span { class: "text-xs text-gray-400",
                        "Controls the LoRA snapshot pipeline via "
                        code { class: "text-xs", "/training/export_snapshot" }
                        "."
                    }
                }

                div { class: "grid grid-cols-3 gap-2",

                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Status card" }
                            p { class: "text-xs text-gray-300",
                                "Shows the live job state reported by the backend (running, idle, or last error) plus timestamps from the last run."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Run Export" }
                            p { class: "text-xs text-gray-300",
                                "Immediately launches " code { class: "text-green-300", "export_docs.py" } " followed by " code { class: "text-green-300", "normalize_dataset.py" } ". Respects whatever filter is configured."
                            }
                        }
                    }

                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Auto-export after upload" }
                            p { class: "text-xs text-gray-300",
                                "When enabled, every successful document upload batch schedules a LoRA export after the debounce window."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Filter override" }
                            p { class: "text-xs text-gray-300",
                                "Writes to " code { class: "text-green-300", "LORA_EXPORT_ONLY" } " in-memory before the scripts run. Provide comma-separated paths relative to " code { class: "text-green-300", "documents/" } ". Leave blank to export everything."
                            }
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Direct API Equivalents" }
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

                div { class: "mt-2 pt-2 border-t border-gray-700",
                    Link { to: Route::DocuIndex {}, class: "btn btn-primary btn-xs", "← Back to Index" }
                }
            }
        }
    }
}
