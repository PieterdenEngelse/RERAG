//! Threads documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuThreads() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4",
                        "Threads in Rust: Tokio, Rayon & spawn_blocking"
                    }

                    div { class: "bg-gray-900 rounded p-4 mb-6",
                        p { class: "text-sm text-gray-200 mb-3",
                            "A thread is the smallest unit of execution that a CPU can schedule and run. It represents a single sequence of instructions inside a program."
                        }
                        ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-1",
                            li { "Threads are managed independently by the OS scheduler" }
                            li { "Multiple threads in the same process share memory and resources" }
                            li { "Modern systems support multithreading for better responsiveness and performance" }
                        }
                    }

                    div { class: "grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6",
                        div { class: "bg-blue-900/30 border border-blue-700 rounded p-4",
                            h4 { class: "font-bold text-blue-300 mb-2", "I/O-Bound (Waiting)" }
                            p { class: "text-sm text-gray-300 mb-2",
                                "CPU sits idle while waiting for external resources."
                            }
                            ul { class: "text-sm text-gray-300 list-disc ml-4 space-y-1",
                                li { "HTTP requests" }
                                li { "Database queries" }
                                li { "File reads/writes" }
                                li { "Redis cache" }
                            }
                            p { class: "text-xs text-blue-300 mt-2", "\u{2192} Use Tokio (async)" }
                        }
                        div { class: "bg-green-900/30 border border-green-700 rounded p-4",
                            h4 { class: "font-bold text-green-300 mb-2", "CPU-Bound (Computing)" }
                            p { class: "text-sm text-gray-300 mb-2",
                                "CPU is actively working, no waiting."
                            }
                            ul { class: "text-sm text-gray-300 list-disc ml-4 space-y-1",
                                li { "Embedding generation" }
                                li { "Text chunking" }
                                li { "Vector similarity" }
                                li { "Reranking" }
                            }
                            p { class: "text-xs text-green-300 mt-2",
                                "\u{2192} Use Rayon or spawn_blocking"
                            }
                        }
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "When to Use What" }
                    table { class: "table table-sm w-full text-gray-300 mb-4",
                        thead {
                            tr {
                                th { class: "text-gray-200", "Task" }
                                th { class: "text-gray-200", "Tool" }
                                th { class: "text-gray-200", "Why" }
                            }
                        }
                        tbody {
                            tr {
                                td { "HTTP request" }
                                td { class: "text-blue-300", "tokio::spawn" }
                                td { "I/O wait" }
                            }
                            tr {
                                td { "Database query" }
                                td { class: "text-blue-300", "async/await" }
                                td { "I/O wait" }
                            }
                            tr {
                                td { "Batch embeddings" }
                                td { class: "text-green-300", "rayon par_iter" }
                                td { "CPU parallel" }
                            }
                            tr {
                                td { "Single embedding" }
                                td { class: "text-yellow-300", "spawn_blocking" }
                                td { "CPU in async" }
                            }
                            tr {
                                td { "Vector search" }
                                td { class: "text-green-300", "rayon" }
                                td { "CPU parallel" }
                            }
                            tr {
                                td { "File read" }
                                td { class: "text-blue-300", "tokio::fs" }
                                td { "I/O wait" }
                            }
                        }
                    }

                    div { class: "bg-gray-700 rounded p-4 text-sm text-gray-200",
                        p { class: "mb-2", "\u{2705} Tokio 1.47 with full features - HTTP, Redis, async tasks" }
                        p { class: "mb-2", "\u{2705} Rayon 1.10 - retriever.rs, batch.rs, product_quantization.rs" }
                        p { class: "mb-2", "\u{2705} spawn_blocking - embedder.rs for ONNX inference" }
                        p { "Your RAG system correctly uses async for I/O and parallel threads for CPU work." }
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
