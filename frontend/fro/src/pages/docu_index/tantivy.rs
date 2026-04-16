//! Tantivy documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuTantivy() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-blue-300", "Tantivy in AG" }
                    span { class: "text-xs text-gray-400", "Full-text search engine — the symbolic memory side of hybrid search." }
                }

                div { class: "grid grid-cols-3 gap-2",
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "What is Tantivy?" }
                            p { class: "text-xs text-gray-200",
                                "Full-text search engine library written in Rust (think Lucene but native Rust). Serves as the structured retrieval layer — the \"symbolic memory\" side of hybrid search."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "How data gets in" }
                            p { class: "text-xs text-gray-200",
                                "File → extract_text → chunker splits into chunks → each chunk gets a chunk_id (filename#0, filename#1) → add_document stores it as a Tantivy doc with three fields: doc_id, title, content. Simultaneously, the chunk is embedded into a vector and stored in the vector store."
                            }
                        }
                    }
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "The inverted index" }
                            p { class: "text-xs text-gray-200",
                                "Tantivy doesn't store documents as-is. For every term (word), it maintains a list of which documents contain it. Searching \"error\" doesn't scan all documents — it jumps directly to matching ones. This is what makes BM25 keyword search fast at scale."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Segments" }
                            p { class: "text-xs text-gray-200",
                                "Every time you commit new documents, Tantivy writes them into a segment. Over time segments accumulate. The LogMergePolicy auto-compacts small segments into larger ones to prevent segment bloat."
                            }
                        }
                    }
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "How search works" }
                            p { class: "text-xs text-gray-200",
                                "Retriever.search() creates a QueryParser over title and content fields, parses the query, and uses TopDocs::with_limit(10) to get the top 10 BM25-ranked results. Results go through L1 (in-retriever query cache) → L2 (LRU memory) → L3 (Redis) caching layers."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "The hybrid part" }
                            p { class: "text-xs text-gray-200",
                                "Tantivy's BM25 keyword results get combined with vector cosine similarity results via Reciprocal Rank Fusion — giving both exact keyword matching and semantic similarity in one query."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "On your hardware" }
                            p { class: "text-xs text-gray-200",
                                "Tantivy is CPU-efficient. Index lives on disk at "
                                code { class: "text-xs text-gray-400", "~/.local/share/ag/index/tantivy/" }
                                ". In-memory footprint is mainly the segment readers, not the full index."
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
