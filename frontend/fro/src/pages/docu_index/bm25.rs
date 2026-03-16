//! BM25 documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuBm25() -> Element {
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

                    h2 { class: "text-lg font-semibold text-blue-300 mb-4", "BM25 (Best Matching 25)" }
                    p { class: "text-gray-200 leading-relaxed",
                        "BM25 is the ranking algorithm Tantivy uses to score how relevant a document is to a search query. It combines three factors:"
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1", "Term Frequency (TF)" }
                    p { class: "text-gray-200 leading-relaxed",
                        "How often the search term appears in a document. More occurrences means more relevant, but with diminishing returns. The 10th mention of \"error\" adds less score than the 2nd."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1",
                        "Inverse Document Frequency (IDF)"
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "How rare the term is across all documents. A word like \"the\" appears everywhere so it\u{2019}s nearly worthless. A word like \"segfault\" appears in few documents so it\u{2019}s highly discriminating."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1",
                        "Document Length Normalization"
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Shorter documents that contain the term get boosted over longer ones. If a 50-word chunk mentions \"error\" twice, that\u{2019}s more significant than a 5000-word document mentioning it twice."
                    }
                    h4 { class: "text-sm font-semibold text-green-300 pt-4 mb-1", "In AG" }
                    p { class: "text-gray-200 leading-relaxed",
                        "Tantivy computes BM25 across all segments and returns documents sorted by score. BM25 scores get converted to reciprocal rank scores (1.0 / (60.0 + rank + 1.0)) before being fused with vector similarity scores by the hybrid searcher."
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
