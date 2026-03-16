//! Knowledge Graphs documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuKnowledgeGraphs() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4", "Knowledge Graphs" }
                    p { class: "text-lg text-gray-200 mb-4",
                        "A knowledge graph is a structured representation of information as a network of entities and relationships."
                    }
                    p { class: "text-sm text-gray-300 mb-4",
                        "In a knowledge graph, nodes represent entities (people, places, concepts, documents) and edges represent relationships between them (works at, located in, mentions, depends on)."
                    }
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Why Knowledge Graphs for RAG?" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Standard RAG retrieves isolated chunks. Knowledge graphs add structure:"
                    }
                    ul { class: "space-y-2 text-sm text-gray-300 ml-4 list-disc",
                        li {
                            strong { "Multi-hop reasoning" }
                            " \u{2014} follow relationships across documents"
                        }
                        li {
                            strong { "Entity disambiguation" }
                            " \u{2014} distinguish same-name entities by context"
                        }
                        li {
                            strong { "Cross-document connections" }
                            " \u{2014} link related chunks via shared entities"
                        }
                        li {
                            strong { "Structural context" }
                            " \u{2014} understand how concepts relate, not just what they are"
                        }
                    }
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "In the AG System" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "AG uses a two-tier graph architecture:"
                    }
                    ul { class: "space-y-2 text-sm text-gray-300 ml-4 list-disc",
                        li {
                            strong { "Neo4j" }
                            " \u{2014} ingestion-time graph building. Extracts entities, builds relationships, stores the full knowledge graph."
                        }
                        li {
                            strong { "Petgraph" }
                            " \u{2014} runtime graph queries. Loads an exported JSON snapshot from Neo4j into RAM for fast, in-process traversal."
                        }
                    }
                    p { class: "text-sm text-gray-300 mt-3",
                        "Neo4j never runs at query time. All runtime graph traversal goes through petgraph, which is nanoseconds with no network overhead."
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
