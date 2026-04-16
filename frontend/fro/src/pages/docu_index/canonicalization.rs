//! Canonicalization documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuCanonicalization() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuTokenizersGeneral {}, class: "text-primary hover:underline text-sm shrink-0", "← Tokenizers General" }
                    h1 { class: "text-lg font-bold text-blue-300", "Canonicalization" }
                    span { class: "text-xs text-gray-400", "Taking messy real-world input and converting it into a single standardized form." }
                }

                p { class: "text-xs text-gray-300 mb-1",
                    "Canonicalization is the process of taking messy, variable, real-world input and converting it into a single, standardized, machine-friendly form before any downstream processing happens."
                }
                p { class: "text-xs text-gray-400 mb-3",
                    "In other words: different-looking inputs that mean the same thing should be turned into the same representation. Tokenization is one canonicalization step, but not the only one."
                }

                div { class: "grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-2 mb-2",

                    // What it means
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "What Canonicalization Means in ML/Analytics" }
                        p { class: "text-xs text-gray-300 mb-1", "Canonicalization is about reducing entropy in your data. Raw text is full of variation:" }
                        ul { class: "text-xs text-gray-200 space-y-0.5 font-mono list-none",
                            li { "\"U.S.A.\"  vs  \"USA\"" }
                            li { "\"colour\"  vs  \"color\"" }
                            li { "\"I'm\"  vs  \"I am\"" }
                            li { "\"résumé\"  vs  \"resume\"" }
                            li { "\" extra spaces \"" }
                            li { "Unicode lookalikes (Cyrillic \"а\" vs Latin \"a\")" }
                        }
                        p { class: "text-xs text-gray-400 mt-1",
                            "Canonicalization collapses these into a consistent, predictable form so your model doesn't waste capacity learning irrelevant variation."
                        }
                    }

                    // Typical steps
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "Typical Canonicalization Steps (Before Tokenization)" }
                        ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                            li { "• Unicode normalization (NFC/NFKC)" }
                            li { "• Lowercasing (unless casing matters)" }
                            li { "• Whitespace normalization" }
                            li { "• Punctuation normalization" }
                            li { "• Accent stripping (optional)" }
                            li { "• Stopword removal (analytics, not LLMs)" }
                            li { "• Stemming/lemmatization (analytics, not LLMs)" }
                            li { "• URL/email/user-handle masking" }
                            li { "• Number normalization (\"1,000\" → \"1000\")" }
                            li { "• Emoji/markup normalization" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "Tokenizers often implicitly do some of this — SentencePiece normalizes Unicode by default." }
                    }

                    // Why tokenization is canonicalization
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "Why Tokenization Is a Canonicalization Step" }
                        p { class: "text-xs text-gray-300 mb-1", "Because tokenizers:" }
                        ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                            li { "• Enforce a single segmentation of text" }
                            li { "• Map many surface forms to the same subword" }
                            li { "• Collapse rare variants into shared tokens" }
                            li { "• Remove ambiguity in whitespace and punctuation" }
                            li { "• Convert text into a stable, model-ready representation" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "For example, BPE might turn:" }
                        p { class: "text-xs text-gray-300 font-mono mt-0.5", "\"running\", \"runs\", \"run\", \"runner\"" }
                        p { class: "text-xs text-gray-400 mt-0.5", "into:" }
                        p { class: "text-xs text-gray-300 font-mono mt-0.5",
                            "run + ning" br {} "run + s" br {} "run" br {} "run + ner"
                        }
                        p { class: "text-xs text-gray-400 mt-1", "Different surface forms → shared base units." }
                    }
                }

                // RAG implications — full width
                div { class: "bg-gray-800 border border-yellow-700 rounded-lg p-2 mb-2",
                    h3 { class: "text-sm font-bold text-yellow-300 mb-1", "In RAG, Canonicalization Affects" }
                    div { class: "grid grid-cols-2 md:grid-cols-3 gap-2",
                        div {
                            p { class: "text-xs text-yellow-400 font-semibold mb-0.5", "Embedding stability" }
                            p { class: "text-xs text-gray-200", "Same meaning → same tokens → similar vectors." }
                        }
                        div {
                            p { class: "text-xs text-yellow-400 font-semibold mb-0.5", "Clustering quality" }
                            p { class: "text-xs text-gray-200", "Less noise in token distributions." }
                        }
                        div {
                            p { class: "text-xs text-yellow-400 font-semibold mb-0.5", "Graph construction" }
                            p { class: "text-xs text-gray-200", "Cleaner nodes, fewer near-duplicate entities." }
                        }
                        div {
                            p { class: "text-xs text-yellow-400 font-semibold mb-0.5", "Summary-first retrieval" }
                            p { class: "text-xs text-gray-200", "Consistent token boundaries improve summarizer behavior." }
                        }
                        div {
                            p { class: "text-xs text-yellow-400 font-semibold mb-0.5", "Memory footprint" }
                            p { class: "text-xs text-gray-200", "Less vocabulary fragmentation." }
                        }
                        div {
                            p { class: "text-xs text-red-400 font-semibold mb-0.5", "If sloppy..." }
                            p { class: "text-xs text-gray-200", "Near-duplicates in your graph, noisier clusters, and drifting summaries." }
                        }
                    }
                }

                div { class: "mt-2 pt-2 border-t border-gray-700",
                    Link { to: Route::DocuTokenizersGeneral {}, class: "btn btn-primary btn-xs", "← Back to Tokenizers General" }
                }
            }
        }
    }
}
