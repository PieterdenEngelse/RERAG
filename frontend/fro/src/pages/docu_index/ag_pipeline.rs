//! AG Pipeline documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuAgPipeline() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4", "AG Pipeline" }
                    p { class: "text-lg text-gray-200 mb-6",
                        "The AG system processes documents through a multi-stage pipeline, from upload to agentic retrieval."
                    }

                    div { class: "bg-gray-700 rounded p-4 my-4 font-mono text-xs text-blue-300 whitespace-pre",
                        "  Document Upload\n        \u{2502}\n        \u{25bc}\n     Parsing\n        \u{2502}\n        \u{25bc}\n    Chunking\n        \u{2502}\n        \u{25bc}\n   Embedding\n        \u{2502}\n        \u{25bc}\n    Indexing\n        \u{2502}\n        \u{25bc}\n Graph Building\n        \u{2502}\n        \u{25bc}\n   Retrieval\n        \u{2502}\n        \u{25bc}\n     Agent"
                    }

                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "1. Document Upload" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "User uploads a file via the "
                        code { class: "text-green-300", "/upload" }
                        " endpoint. Supported formats include PDF, TXT, MD, and other text-based files."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "2. Parsing" }
                    p { class: "text-sm text-gray-300 mb-2",
                        code { class: "text-green-300", "parser.rs" }
                        " extracts raw text from the uploaded file. Handles PDF, plain text, Markdown, and other formats using MIME detection."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "3. Chunking" }
                    p { class: "text-sm text-gray-300 mb-2",
                        code { class: "text-green-300", "chunker.rs" }
                        " splits the extracted text into smaller, semantically meaningful chunks. Supports multiple chunking strategies including fixed-size, sentence-aware, and adaptive chunking."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "4. Embedding" }
                    p { class: "text-sm text-gray-300 mb-2",
                        code { class: "text-green-300", "embedder.rs" }
                        " generates dense vector representations for each chunk using the ONNX embedding model. Each chunk becomes a fixed-length numerical vector that captures its semantic meaning."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "5. Indexing" }
                    p { class: "text-sm text-gray-300 mb-2",
                        code { class: "text-green-300", "index.rs" }
                        " stores each chunk in two places: the Tantivy full-text search index for keyword search, and the vector store for semantic similarity search."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "6. Graph Building" }
                    p { class: "text-sm text-gray-300 mb-2",
                        code { class: "text-green-300", "knowledge_builder.rs" }
                        " creates Document, Chunk, and Entity nodes in the Neo4j knowledge graph. Extracts entities from chunk content and builds relationships between them (HAS_CHUNK, MENTIONS, RELATED_TO)."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "7. Retrieval" }
                    p { class: "text-sm text-gray-300 mb-2",
                        code { class: "text-green-300", "retriever.rs" }
                        " handles search queries by combining Tantivy full-text search with vector similarity search. Supports hybrid search with configurable weighting and multi-layer caching (L1/L2/L3)."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "8. Agent" }
                    p { class: "text-sm text-gray-300 mb-2",
                        code { class: "text-green-300", "agent.rs" }
                        " / "
                        code { class: "text-green-300", "agentic.rs" }
                        " orchestrates the LLM with tools. Takes the retrieved context, constructs prompts, calls Ollama for inference, and manages the agentic decision loop including tool composition and memory."
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
