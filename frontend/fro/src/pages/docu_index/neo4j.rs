//! Neo4j documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuNeo4j() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4", "Neo4j in RAG: GraphRAG" }
                    p { class: "text-sm text-cyan-400 mb-4",
                        "How Neo4j enhances retrieval-augmented generation"
                    }

                    div { class: "grid grid-cols-1 lg:grid-cols-3 gap-3",
                        div { class: "space-y-2",
                            div { class: "p-2 rounded bg-red-900/20 border border-red-700/30",
                                h3 { class: "text-xs font-semibold text-red-300 mb-1",
                                    "\u{274c} The Problem with Traditional RAG"
                                }
                                p { class: "text-[11px] text-gray-300",
                                    "Vector search alone misses connections between concepts. Query about \"rate limiting\" finds rate limit docs, but misses related monitoring docs that share entities."
                                }
                            }
                            div { class: "p-2 rounded bg-green-900/20 border border-green-700/30",
                                h3 { class: "text-xs font-semibold text-green-300 mb-1",
                                    "\u{2705} Neo4j Solution: Entity-Aware Retrieval"
                                }
                                p { class: "text-[11px] text-gray-300",
                                    "Neo4j stores entities and relationships extracted from documents. When you search, it finds additional relevant chunks by following entity connections."
                                }
                            }
                        }

                        div { class: "space-y-1.5",
                            h3 { class: "text-xs font-semibold text-cyan-300 mb-1", "\u{1f4ca} Use Cases" }
                            div { class: "p-1.5 rounded bg-gray-700/50 border border-gray-600",
                                span { class: "text-xs font-medium text-white", "1. Entity-Based Expansion" }
                                p { class: "text-[10px] text-gray-400",
                                    "Chunks sharing entities are linked even if text differs"
                                }
                            }
                            div { class: "p-1.5 rounded bg-gray-700/50 border border-gray-600",
                                span { class: "text-xs font-medium text-white", "2. Multi-Hop Reasoning" }
                                p { class: "text-[10px] text-gray-400",
                                    "Traverse 2+ relationships: RateLimiter \u{2192} Prometheus \u{2192} Grafana"
                                }
                            }
                            div { class: "p-1.5 rounded bg-gray-700/50 border border-gray-600",
                                span { class: "text-xs font-medium text-white", "3. Cross-Document Queries" }
                                p { class: "text-[10px] text-gray-400",
                                    "Find connections across different files via shared entities"
                                }
                            }
                            div { class: "p-1.5 rounded bg-gray-700/50 border border-gray-600",
                                span { class: "text-xs font-medium text-white", "4. Agent Memory" }
                                p { class: "text-[10px] text-gray-400",
                                    "Store past interactions as graph, learn from successful queries"
                                }
                            }
                        }

                        div { class: "space-y-2",
                            h3 { class: "text-xs font-semibold text-cyan-300 mb-1",
                                "\u{1f504} RAG Flow with Neo4j"
                            }
                            div { class: "p-2 rounded bg-gray-700 font-mono text-[10px] leading-tight",
                                pre { class: "text-gray-300",
                                    "Query: \"How do I configure rate limiting?\"\n         \u{2502}\n         \u{25bc}\n\u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}   \u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}\n\u{2502} Vector Search   \u{2502}   \u{2502} Graph Search    \u{2502}\n\u{2502} (Tantivy)       \u{2502}   \u{2502} (Neo4j)         \u{2502}\n\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}   \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n         \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n                   \u{25bc}\n        \u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}\n        \u{2502} Graph Expansion   \u{2502}\n        \u{2502} + Merged Context  \u{2502}\n        \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n                  \u{25bc}\n           Better Answer"
                                }
                            }
                            h3 { class: "text-xs font-semibold text-cyan-300 mt-2 mb-1",
                                "\u{2699}\u{fe0f} Configuration"
                            }
                            div { class: "grid grid-cols-2 gap-1 text-[10px] font-mono",
                                div { class: "px-1.5 py-0.5 rounded bg-gray-700",
                                    span { class: "text-cyan-400", "NEO4J_ENABLED" }
                                    span { class: "text-gray-500", "=true" }
                                }
                                div { class: "px-1.5 py-0.5 rounded bg-gray-700",
                                    span { class: "text-cyan-400", "GRAPH_EXPANSION_MAX_HOPS" }
                                    span { class: "text-gray-500", "=2" }
                                }
                                div { class: "px-1.5 py-0.5 rounded bg-gray-700",
                                    span { class: "text-cyan-400", "GRAPH_EXPANSION_MAX_CHUNKS" }
                                    span { class: "text-gray-500", "=10" }
                                }
                                div { class: "px-1.5 py-0.5 rounded bg-gray-700",
                                    span { class: "text-cyan-400", "GRAPH_ENTITY_WEIGHT" }
                                    span { class: "text-gray-500", "=0.7" }
                                }
                            }
                        }
                    }

                    p { class: "text-[11px] text-gray-500 mt-4",
                        "Access Neo4j Browser at "
                        a {
                            class: "text-cyan-400 hover:underline",
                            href: "http://localhost:7474",
                            target: "_blank",
                            "localhost:7474"
                        }
                        ". Configure in Settings \u{2192} Neo4j or via environment variables."
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
