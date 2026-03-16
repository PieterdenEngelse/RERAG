//! Embeddings documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuEmbeddings() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4", "What Embeddings Are" }

                    p { class: "text-lg text-gray-200 mb-4",
                        "An "
                        strong { "embedding" }
                        " is a way of representing complex objects\u{2014}like words, sentences, images, or even users\u{2014}as "
                        strong { "dense, fixed-length numerical vectors" }
                        ". The key idea is that "
                        strong { "geometry becomes meaning" }
                        ": distances and directions between vectors correspond to semantic relationships between the original objects."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Formal Definition" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Formally, an embedding takes an input object x and maps it to a vector:"
                    }
                    div { class: "bg-gray-700 rounded p-4 my-4 text-center",
                        code { class: "text-lg text-blue-300", "v \u{2208} \u{211d}\u{207f}" }
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "This expression captures several important ideas:"
                    }
                    ul { class: "space-y-2 text-sm text-gray-300 ml-4 list-disc",
                        li {
                            "The symbol "
                            strong { "\u{2208}" }
                            " means \"is an element of\" or \"belongs to.\""
                        }
                        li {
                            strong { "\u{211d}" }
                            " is the set of all real numbers."
                        }
                        li {
                            strong { "\u{211d}\u{207f}" }
                            " is the set of all n-dimensional vectors of real numbers."
                        }
                    }
                    p { class: "text-sm text-gray-300 mt-4 mb-2",
                        "So the statement "
                        strong { "v \u{2208} \u{211d}\u{207f}" }
                        " means:"
                    }
                    div { class: "bg-gray-700 rounded p-4 my-4 border-l-4 border-blue-500",
                        p { class: "text-gray-200 italic",
                            "The embedding v is a vector with n real-valued components, living in an n-dimensional real vector space."
                        }
                    }
                    p { class: "text-sm text-gray-300 mb-2", "A vector in this space looks like:" }
                    div { class: "bg-gray-700 rounded p-4 my-4 text-center",
                        code { class: "text-lg text-blue-300", "v = (v\u{2081}, v\u{2082}, ..., v\u{2099})" }
                    }
                    p { class: "text-sm text-gray-300",
                        "Each v\u{1d62} is a real number. Every embedding produced by a given model has the same dimensionality n, which ensures that all embeddings live in a shared geometric space where comparisons make sense."
                    }

                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "Why This Matters" }
                    p { class: "text-sm text-gray-300 mb-4",
                        "Because embeddings live in a structured vector space:"
                    }
                    ul { class: "space-y-3 text-sm text-gray-300 ml-4",
                        li {
                            strong { "Distances" }
                            " reflect similarity (closer vectors \u{2192} more similar meanings)"
                        }
                        li {
                            strong { "Directions" }
                            " encode relationships (e.g., the famous analogy: king - man + woman \u{2248} queen)"
                        }
                        li {
                            strong { "Clustering" }
                            " groups related concepts"
                        }
                        li {
                            strong { "Search" }
                            " becomes geometric nearest-neighbor lookup"
                        }
                        li {
                            strong { "Classification" }
                            " becomes linear separation in high-dimensional space"
                        }
                    }
                    p { class: "text-sm text-gray-300 mt-4",
                        "This is why embeddings are so powerful: they turn messy, symbolic, human-level concepts into "
                        strong { "mathematically structured objects" }
                        " that models can reason about."
                    }

                    h3 { class: "text-xl font-bold text-white mt-8 mb-3",
                        "How ONNX and Neo4j Relate Through Embeddings"
                    }

                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "1. ONNX: The Model Runtime That Produces Embeddings"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "ONNX (Open Neural Network Exchange) is a model format + runtime. You use it to run models locally or in production without being tied to a specific framework."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Typical workflow:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Load an ONNX model (e.g., a sentence transformer, CLIP, MiniLM, etc.)" }
                        li { "Pass input text/image through it" }
                        li { "The model outputs an embedding vector: v \u{2208} \u{211d}\u{207f}" }
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "This vector is your semantic representation. So "
                        strong { "ONNX is the embedding generator" }
                        "."
                    }

                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "2. Neo4j: The Graph Database That Stores and Queries Embeddings"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Neo4j is a graph database, but it also has:"
                    }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Native vector properties" }
                        li { "Native vector indexes" }
                        li { "Native vector similarity search (cosine, dot, Euclidean)" }
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-2",
                        "This means you can store embeddings directly on nodes:"
                    }
                    div { class: "bg-gray-700 rounded p-3 my-2 font-mono text-xs text-green-300",
                        "CREATE (d:Document {{"
                        br {}
                        "  id: \"doc1\","
                        br {}
                        "  text: \"...\","
                        br {}
                        "  embedding: [0.12, -0.87, 0.003, ...]"
                        br {}
                        "}})"
                    }

                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "3. How They Work Together"
                    }
                    div { class: "space-y-3 text-sm text-gray-300",
                        div {
                            strong { "Step 1 \u{2014} Generate embeddings with ONNX" }
                            p { class: "ml-4", "Input: text, sentence, image, user profile, etc." }
                            p { class: "ml-4", "Output: vector (v \u{2208} \u{211d}\u{207f})" }
                        }
                        div {
                            strong { "Step 2 \u{2014} Store embeddings in Neo4j" }
                            p { class: "ml-4",
                                "Attach the vector to a node: Document, User, Product, Concept, etc."
                            }
                        }
                        div {
                            strong { "Step 3 \u{2014} Query Neo4j using vector similarity" }
                            ul { class: "ml-4 list-disc",
                                li { "Find nearest neighbors" }
                                li { "Rank by similarity" }
                                li { "Combine vector similarity with graph structure" }
                                li { "Mix embeddings with relationships (this is where Neo4j shines)" }
                            }
                        }
                    }

                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "When Are Embeddings Used?" }
                    ul { class: "space-y-2 text-sm text-gray-300 ml-4 list-decimal",
                        li { "Document indexing - When you upload/add documents to RAG" }
                        li { "Search queries - Every time you search" }
                        li { "RAG retrieval - When the AI answers questions" }
                        li { "Similarity matching - Comparing documents/chunks" }
                        li { "Agent memory storage - When agents store memories" }
                        li { "Agent memory retrieval - When agents recall past interactions" }
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
