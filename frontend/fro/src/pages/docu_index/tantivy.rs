//! Tantivy documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuTantivy() -> Element {
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

                    h2 { class: "text-lg font-semibold text-blue-300 mb-4", "Tantivy in AG" }
                    p { class: "text-gray-200 leading-relaxed",
                        "Tantivy is a full-text search engine library written in Rust (think Lucene but native Rust). In this system it serves as the structured retrieval layer\u{2014}the \"symbolic memory\" side of the hybrid search."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1", "How data gets in" }
                    p { class: "text-gray-200 leading-relaxed",
                        "When a file lands in ~/ag/documents/, the indexing pipeline runs: file \u{2192} extract_text \u{2192} chunker splits into chunks \u{2192} each chunk gets a chunk_id (filename#0, filename#1, etc.) \u{2192} add_document stores it as a Tantivy document with three fields: doc_id, title, and content. Simultaneously, the chunk gets embedded into a vector and stored in the in-memory vector store."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1", "The inverted index" }
                    p { class: "text-gray-200 leading-relaxed",
                        "Tantivy doesn\u{2019}t store documents as-is. It builds an inverted index\u{2014}for every term (word), it maintains a list of which documents contain that term. So searching \"error\" doesn\u{2019}t scan all documents; it jumps directly to the matching ones. This is what makes "
                        a {
                            class: "text-blue-300 underline decoration-dotted",
                            href: "#bm25",
                            "BM25"
                        }
                        " keyword search fast even at scale."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1", "Segments" }
                    p { class: "text-gray-200 leading-relaxed",
                        "Every time you commit new documents, Tantivy writes them into a segment. Over time, segments accumulate. The LogMergePolicy auto-compacts small segments into larger ones to prevent segment bloat."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1", "How search works" }
                    p { class: "text-gray-200 leading-relaxed",
                        "Retriever.search() creates a QueryParser over the title and content fields, parses the query string, and uses TopDocs::with_limit(10) to get the top 10 BM25-ranked results. Results go through L1 (in-retriever query cache) \u{2192} L2 (LRU memory) \u{2192} L3 (Redis) caching layers."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1", "The hybrid part" }
                    p { class: "text-gray-200 leading-relaxed",
                        "Tantivy\u{2019}s BM25 keyword results get combined with vector cosine similarity results via Reciprocal Rank Fusion, giving both exact keyword matching and semantic similarity in one query."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1", "On your hardware" }
                    p { class: "text-gray-200 leading-relaxed",
                        "Tantivy is CPU-efficient and the index lives on disk at ~/.local/share/ag/index/tantivy/. The in-memory footprint is mainly the segment readers, not the full index."
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
