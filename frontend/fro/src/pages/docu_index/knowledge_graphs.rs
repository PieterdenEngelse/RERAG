//! Knowledge Graphs documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuKnowledgeGraphs() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "Knowledge Graphs" }
                    span { class: "text-xs text-gray-400", "Structured information as a network of entities and relationships." }
                }

                div { class: "grid grid-cols-3 gap-2",

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "What Is a Knowledge Graph?" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "A knowledge graph is a structured representation of information as a network of entities and relationships."
                        }
                        p { class: "text-xs text-gray-300",
                            "Nodes represent entities (people, places, concepts, documents). Edges represent relationships between them (works at, located in, mentions, depends on)."
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Why Knowledge Graphs for RAG?" }
                        p { class: "text-xs text-gray-300 mb-1", "Standard RAG retrieves isolated chunks. Knowledge graphs add structure:" }
                        div { class: "text-xs text-gray-300 space-y-0.5",
                            p { span { class: "text-white font-medium", "Multi-hop reasoning " } "— follow relationships across documents" }
                            p { span { class: "text-white font-medium", "Entity disambiguation " } "— distinguish same-name entities by context" }
                            p { span { class: "text-white font-medium", "Cross-document connections " } "— link related chunks via shared entities" }
                            p { span { class: "text-white font-medium", "Structural context " } "— understand how concepts relate, not just what they are" }
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "In the AG System" }
                        p { class: "text-xs text-gray-300 mb-1", "AG uses a two-tier graph architecture:" }
                        div { class: "text-xs text-gray-300 space-y-1",
                            p {
                                span { class: "text-white font-medium", "Neo4j " }
                                "— ingestion-time graph building. Extracts entities, builds relationships, stores the full knowledge graph."
                            }
                            p {
                                span { class: "text-white font-medium", "Petgraph " }
                                "— runtime graph queries. Loads an exported JSON snapshot from Neo4j into RAM for fast, in-process traversal."
                            }
                        }
                        p { class: "text-xs text-gray-500 mt-1",
                            "Neo4j never runs at query time. All runtime graph traversal goes through petgraph — nanoseconds with no network overhead."
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
