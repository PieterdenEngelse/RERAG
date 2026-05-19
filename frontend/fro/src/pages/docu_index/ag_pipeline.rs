//! AG Pipeline documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuAgPipeline() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "AG Pipeline" }
                    span { class: "text-xs text-gray-400", "Documents flow through a multi-stage pipeline from upload to agentic retrieval." }
                }

                div { class: "grid grid-cols-3 gap-2",

                    // Col 1: diagram
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Flow" }
                        div { class: "font-mono text-xs text-blue-300 whitespace-pre leading-tight",
                            "  Document Upload
        │
        ▼
     Parsing
        │
        ▼
    Chunking
        │
        ▼
   Embedding
        │
        ▼
    Indexing
        │
        ▼
 Graph Building
        │
        ▼
   Retrieval
        │
        ▼
     Agent"
                        }
                    }

                    // Col 2: stages 1-4
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-0.5", "1. Document Upload" }
                            p { class: "text-xs text-gray-300",
                                "User uploads via " code { class: "text-green-300", "/upload" } ". Supported: PDF, TXT, MD, and other text-based files."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-0.5", "2. Parsing" }
                            p { class: "text-xs text-gray-300",
                                code { class: "text-green-300", "parser.rs" } " extracts raw text. Handles PDF, plain text, Markdown via MIME detection."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-0.5", "3. Chunking" }
                            p { class: "text-xs text-gray-300",
                                code { class: "text-green-300", "chunker.rs" } " splits text into semantically meaningful chunks. Supports fixed-size, sentence-aware, and adaptive strategies."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-0.5", "4. Embedding" }
                            p { class: "text-xs text-gray-300",
                                code { class: "text-green-300", "embedder.rs" } " generates dense vector representations via the ONNX model. Each chunk becomes a fixed-length vector capturing semantic meaning."
                            }
                        }
                    }

                    // Col 3: stages 5-8
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-0.5", "5. Indexing" }
                            p { class: "text-xs text-gray-300",
                                code { class: "text-green-300", "index.rs" } " stores each chunk in two places: Tantivy (full-text keyword search) and the vector store (semantic similarity search)."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-0.5", "6. Graph Building" }
                            p { class: "text-xs text-gray-300",
                                code { class: "text-green-300", "knowledge_builder.rs" } " creates Document, Chunk, and Entity nodes in FalkorDB. Builds HAS_CHUNK, MENTIONS, RELATED_TO relationships."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-0.5", "7. Retrieval" }
                            p { class: "text-xs text-gray-300",
                                code { class: "text-green-300", "retriever.rs" } " combines Tantivy full-text + vector similarity search. Hybrid with configurable weighting and L1/L2/L3 caching."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-0.5", "8. Agent" }
                            p { class: "text-xs text-gray-300",
                                code { class: "text-green-300", "agent.rs" } " / " code { class: "text-green-300", "agentic.rs" } " orchestrates the LLM with tools, constructs prompts, calls Ollama, manages the agentic decision loop."
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
