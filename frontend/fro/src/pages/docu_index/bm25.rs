//! BM25 documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuBm25() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-blue-300", "BM25 — Best Matching 25" }
                    span { class: "text-xs text-gray-400", "Ranking algorithm used by Tantivy." }
                }

                div { class: "grid grid-cols-4 gap-2",
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "Term Frequency (TF)" }
                        p { class: "text-xs text-gray-200",
                            "How often the search term appears in a document. More occurrences = more relevant, but with diminishing returns. The 10th mention of \"error\" adds less score than the 2nd."
                        }
                    }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "Inverse Document Frequency (IDF)" }
                        p { class: "text-xs text-gray-200",
                            "How rare the term is across all documents. \"the\" appears everywhere — nearly worthless. \"segfault\" appears in few documents — highly discriminating."
                        }
                    }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "Document Length Normalization" }
                        p { class: "text-xs text-gray-200",
                            "Shorter documents that contain the term get boosted. A 50-word chunk mentioning \"error\" twice is more significant than a 5000-word doc mentioning it twice."
                        }
                    }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "In AG" }
                        p { class: "text-xs text-gray-200",
                            "Tantivy computes BM25 across all segments and returns documents sorted by score. BM25 scores get converted to reciprocal rank scores "
                            code { class: "text-xs text-gray-400", "(1.0 / (60.0 + rank + 1.0))" }
                            " before being fused with vector similarity scores by the hybrid searcher."
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
