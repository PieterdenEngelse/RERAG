//! Embeddings documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuEmbeddings() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "Embeddings" }
                    span { class: "text-xs text-gray-400", "Dense numerical vectors where geometry becomes meaning." }
                }

                div { class: "grid grid-cols-3 gap-2",

                    // Col 1: definition
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "What Is an Embedding?" }
                            p { class: "text-xs text-gray-300 mb-1",
                                "An embedding maps any tokenizable unit — from a character or subword to a full sentence, document, image, or user profile — into a "
                                strong { "dense, fixed‑length numerical vector" }
                                ". Distances and directions in this vector space encode semantic, syntactic, or structural relationships, so similar units lie close together and meaningful transformations correspond to consistent geometric patterns."
                            }
                            div { class: "bg-gray-700 rounded p-2 text-center",
                                code { class: "text-sm text-blue-300", "v ∈ ℝⁿ" }
                            }
                            p { class: "text-xs text-gray-400 mt-1",
                                "v is a vector with n real-valued components, living in an n-dimensional real vector space. "
                                "Form: " code { class: "text-blue-300 text-xs", "v = (v₁, v₂, ..., vₙ)" }
                                ". Every embedding from a given model has the same dimensionality n."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "When Are Embeddings Used?" }
                            ul { class: "text-xs text-gray-300 list-decimal ml-3 space-y-0.5",
                                li { "Document indexing — on upload/add" }
                                li { "Search queries — every search" }
                                li { "RAG retrieval — when the AI answers questions" }
                                li { "Similarity matching — comparing docs/chunks" }
                                li { "Agent memory storage — storing memories" }
                                li { "Agent memory retrieval — recalling interactions" }
                            }
                        }
                    }

                    // Col 2: why this matters
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Why This Matters" }
                            p { class: "text-xs text-gray-300 mb-1", "Because embeddings live in a structured vector space:" }
                            div { class: "text-xs text-gray-300 space-y-0.5",
                                p { strong { "Distances " } "reflect similarity (closer = more similar meanings)" }
                                p { strong { "Directions " } "encode relationships (king − man + woman ≈ queen)" }
                                p { strong { "Clustering " } "groups related concepts" }
                                p { strong { "Search " } "becomes geometric nearest-neighbor lookup" }
                                p { strong { "Classification " } "becomes linear separation in high-dimensional space" }
                            }
                            p { class: "text-xs text-gray-400 mt-1",
                                "This turns messy human concepts into mathematically structured objects that models can reason about."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "ONNX: Embedding Generator" }
                            p { class: "text-xs text-gray-300 mb-0.5", "Typical workflow:" }
                            ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                                li { "Load an ONNX model (sentence transformer, MiniLM, etc.)" }
                                li { "Pass input text/image through it" }
                                li { "Model outputs an embedding vector: v ∈ ℝⁿ" }
                            }
                            p { class: "text-xs text-gray-400 mt-0.5", "ONNX is the embedding generator." }
                        }
                    }

                    // Col 3: FalkorDB + how they work together
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "FalkorDB: Embedding Store" }
                            p { class: "text-xs text-gray-300 mb-0.5", "FalkorDB has native vector properties, indexes, and similarity search (cosine, Euclidean). Store embeddings on nodes:" }
                            div { class: "bg-gray-700 rounded p-1.5 font-mono text-xs text-green-300 leading-tight",
                                "CREATE (d:Document {{"
                                br {}
                                "  id: \"doc1\","
                                br {}
                                "  embedding: [0.12, -0.87, ...]"
                                br {}
                                "}})"
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "How They Work Together" }
                            div { class: "text-xs text-gray-300 space-y-1",
                                div {
                                    p { strong { "Step 1 — Generate (ONNX)" } }
                                    p { class: "text-gray-400 ml-2", "Input text → ONNX model → vector v ∈ ℝⁿ" }
                                }
                                div {
                                    p { strong { "Step 2 — Store (FalkorDB)" } }
                                    p { class: "text-gray-400 ml-2", "Attach vector to node: Document, User, Product…" }
                                }
                                div {
                                    p { strong { "Step 3 — Query (FalkorDB)" } }
                                    ul { class: "text-gray-400 list-disc ml-4 space-y-0.5",
                                        li { "Nearest-neighbor search" }
                                        li { "Rank by similarity" }
                                        li { "Mix embeddings with graph structure (FalkorDB's strength)" }
                                    }
                                }
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
