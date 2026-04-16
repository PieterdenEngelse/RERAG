//! Threads documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuThreads() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "Threads in Rust: Tokio, Rayon & spawn_blocking" }
                }

                div { class: "grid grid-cols-3 gap-2",

                    // Col 1: intro + I/O bound
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "What is a Thread?" }
                            p { class: "text-xs text-gray-300 mb-0.5",
                                "The smallest unit of execution a CPU can schedule — a single sequence of instructions inside a program."
                            }
                            ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                                li { "Managed independently by the OS scheduler" }
                                li { "Multiple threads in the same process share memory" }
                                li { "Modern systems support multithreading for performance" }
                            }
                        }
                        div { class: "bg-blue-900/30 border border-blue-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-blue-300 mb-1", "I/O-Bound (Waiting)" }
                            p { class: "text-xs text-gray-300 mb-1", "CPU sits idle while waiting for external resources." }
                            ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                                li { "HTTP requests" }
                                li { "Database queries" }
                                li { "File reads/writes" }
                                li { "Redis cache" }
                            }
                            p { class: "text-xs text-blue-300 mt-1", "→ Use Tokio (async)" }
                        }
                    }

                    // Col 2: CPU bound + status
                    div { class: "space-y-2",
                        div { class: "bg-green-900/30 border border-green-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "CPU-Bound (Computing)" }
                            p { class: "text-xs text-gray-300 mb-1", "CPU is actively working, no waiting." }
                            ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                                li { "Embedding generation" }
                                li { "Text chunking" }
                                li { "Vector similarity" }
                                li { "Reranking" }
                            }
                            p { class: "text-xs text-green-300 mt-1", "→ Use Rayon or spawn_blocking" }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 text-xs text-gray-200 space-y-0.5",
                            p { "✅ Tokio 1.47 — HTTP, Redis, async tasks" }
                            p { "✅ Rayon 1.10 — retriever.rs, batch.rs, product_quantization.rs" }
                            p { "✅ spawn_blocking — embedder.rs for ONNX inference" }
                            p { class: "text-gray-400 mt-0.5", "Correctly uses async for I/O and parallel threads for CPU work." }
                        }
                    }

                    // Col 3: table
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "When to Use What" }
                        table { class: "table table-xs w-full text-gray-300",
                            thead {
                                tr {
                                    th { class: "text-gray-400 text-xs", "Task" }
                                    th { class: "text-gray-400 text-xs", "Tool" }
                                    th { class: "text-gray-400 text-xs", "Why" }
                                }
                            }
                            tbody {
                                tr { td { class: "text-xs", "HTTP request" } td { class: "text-xs text-blue-300", "tokio::spawn" } td { class: "text-xs", "I/O wait" } }
                                tr { td { class: "text-xs", "Database query" } td { class: "text-xs text-blue-300", "async/await" } td { class: "text-xs", "I/O wait" } }
                                tr { td { class: "text-xs", "Batch embeddings" } td { class: "text-xs text-green-300", "rayon par_iter" } td { class: "text-xs", "CPU parallel" } }
                                tr { td { class: "text-xs", "Single embedding" } td { class: "text-xs text-yellow-300", "spawn_blocking" } td { class: "text-xs", "CPU in async" } }
                                tr { td { class: "text-xs", "Vector search" } td { class: "text-xs text-green-300", "rayon" } td { class: "text-xs", "CPU parallel" } }
                                tr { td { class: "text-xs", "File read" } td { class: "text-xs text-blue-300", "tokio::fs" } td { class: "text-xs", "I/O wait" } }
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
