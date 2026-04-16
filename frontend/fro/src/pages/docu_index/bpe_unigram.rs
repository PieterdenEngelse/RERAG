//! BPE / BBPE / Unigram tokenization documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuBpeUnigram() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-blue-300", "BPE / BBPE / Unigram Tokenizers" }
                    span { class: "text-xs text-gray-400", "Three core subword algorithms — and why BBPE dominates modern LLMs." }
                }

                p { class: "text-xs text-gray-300 mb-3",
                    "Classic BPE is the foundational algorithm. Byte-Level BPE (BBPE) is its dominant modern variant — used by GPT-2, GPT-3, LLaMA-2, Mistral, and Gemma. Unigram is a probabilistic alternative favoured by multilingual models. All three affect compression, vocabulary size, and downstream model behaviour differently."
                }

                // Three algorithm cards
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-2 mb-2",

                    // BPE card
                    div { class: "bg-gray-800 border border-blue-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-blue-300 mb-0.5", "BPE — Byte-Pair Encoding" }
                        p { class: "text-xs text-orange-400 font-semibold mb-1", "Foundational algorithm (pre-LLM era)" }
                        p { class: "text-xs text-gray-400 mb-1 italic",
                            "Core idea: Start with characters → repeatedly merge the most frequent adjacent pairs → build larger subwords."
                        }
                        div { class: "grid grid-cols-2 gap-1",
                            div {
                                p { class: "text-xs text-yellow-400 font-semibold mb-0.5", "Mechanics" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Greedy, deterministic merging" }
                                    li { "• Vocabulary grows bottom-up from characters" }
                                    li { "• Each merge is irreversible" }
                                    li { "• Longest valid merge sequence at encode time" }
                                }
                            }
                            div {
                                p { class: "text-xs text-green-400 font-semibold mb-0.5", "Strengths" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Stable, predictable segmentation" }
                                    li { "• Great for English and similar languages" }
                                    li { "• Easy to implement and reason about" }
                                }
                                p { class: "text-xs text-red-400 font-semibold mt-1 mb-0.5", "Weaknesses" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Unknown tokens for unseen scripts" }
                                    li { "• Greedy merges can lock in suboptimal patterns" }
                                    li { "• Not zero-shot multilingual" }
                                }
                            }
                        }
                        p { class: "text-xs text-gray-400 mt-1 italic",
                            "Mental model: A compression algorithm that keeps gluing frequent pairs until you hit your vocab budget."
                        }
                    }

                    // BBPE card
                    div { class: "bg-gray-800 border border-cyan-600 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-cyan-300 mb-0.5", "BBPE — Byte-Level BPE" }
                        p { class: "text-xs text-green-400 font-semibold mb-1", "Dominant variant in modern LLMs" }
                        p { class: "text-xs text-gray-400 mb-1 italic",
                            "Core idea: Run BPE over raw bytes (0–255) instead of Unicode characters — every possible input is representable."
                        }
                        div { class: "grid grid-cols-2 gap-1",
                            div {
                                p { class: "text-xs text-yellow-400 font-semibold mb-0.5", "Mechanics" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Base vocabulary is 256 raw bytes" }
                                    li { "• BPE merges run on byte sequences" }
                                    li { "• No UNK token — any byte is encodable" }
                                    li { "• Deterministic, same as classic BPE" }
                                }
                            }
                            div {
                                p { class: "text-xs text-green-400 font-semibold mb-0.5", "Strengths" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Zero unknown tokens" }
                                    li { "• Works across all languages and scripts" }
                                    li { "• Handles noisy text (ASR, OCR, code)" }
                                    li { "• Used by GPT-2/3/4, LLaMA-2, Mistral, Gemma" }
                                }
                                p { class: "text-xs text-red-400 font-semibold mt-1 mb-0.5", "Weaknesses" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Rare scripts inflate token counts" }
                                    li { "• Still greedy — no probabilistic paths" }
                                }
                            }
                        }
                        p { class: "text-xs text-gray-400 mt-1 italic",
                            "Mental model: BPE applied to the byte stream — every file is just numbers 0–255, so nothing is ever unknown."
                        }
                    }

                    // Unigram card
                    div { class: "bg-gray-800 border border-purple-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-purple-300 mb-1", "Unigram — SentencePiece Unigram Model" }
                        p { class: "text-xs text-gray-400 mb-1 italic",
                            "Core idea: Start with a large candidate vocabulary → iteratively prune tokens that reduce likelihood → keep the best subset."
                        }
                        div { class: "grid grid-cols-2 gap-1",
                            div {
                                p { class: "text-xs text-yellow-400 font-semibold mb-0.5", "Mechanics" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Probabilistic model over segmentations" }
                                    li { "• Training removes low-utility tokens" }
                                    li { "• Viterbi decoding for best segmentation" }
                                    li { "• Can sample alternative segmentations" }
                                }
                            }
                            div {
                                p { class: "text-xs text-green-400 font-semibold mb-0.5", "Strengths" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Multiple segmentation paths considered" }
                                    li { "• More compact, semantically coherent subwords" }
                                    li { "• Handles multilingual and morphologically rich languages better" }
                                    li { "• Plays nicely with noise-based training (T5)" }
                                }
                                p { class: "text-xs text-red-400 font-semibold mt-1 mb-0.5", "Weaknesses" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Slower to train and encode (Viterbi)" }
                                    li { "• Sampling can introduce variability" }
                                    li { "• Harder to reason about prune history" }
                                }
                            }
                        }
                        p { class: "text-xs text-gray-400 mt-1 italic",
                            "Mental model: A probabilistic model that keeps the tokens that best explain the corpus, not the ones that happen to be frequent pairs."
                        }
                    }
                }

                // Side-by-side comparison table
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 mb-2",
                    h3 { class: "text-sm font-bold text-green-300 mb-1", "Side-by-side Comparison" }
                    div { class: "overflow-x-auto",
                        table { class: "text-xs w-full",
                            thead {
                                tr { class: "text-gray-400 border-b border-gray-700",
                                    th { class: "text-left py-1 pr-4 font-semibold", "Aspect" }
                                    th { class: "text-left py-1 pr-4 font-semibold text-blue-300", "BPE" }
                                    th { class: "text-left py-1 pr-4 font-semibold text-cyan-300", "BBPE" }
                                    th { class: "text-left py-1 font-semibold text-purple-300", "Unigram" }
                                }
                            }
                            tbody { class: "text-gray-200",
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-4 text-gray-400", "Base units" }
                                    td { class: "py-1 pr-4", "Unicode chars" }
                                    td { class: "py-1 pr-4", "Raw bytes (0–255)" }
                                    td { class: "py-1", "Unicode chars" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-4 text-gray-400", "Training" }
                                    td { class: "py-1 pr-4", "Greedy merges" }
                                    td { class: "py-1 pr-4", "Greedy merges on bytes" }
                                    td { class: "py-1", "Probabilistic pruning" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-4 text-gray-400", "Encoding" }
                                    td { class: "py-1 pr-4", "Deterministic longest-match" }
                                    td { class: "py-1 pr-4", "Deterministic longest-match" }
                                    td { class: "py-1", "Viterbi (optimal or sampled)" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-4 text-gray-400", "Unknown tokens" }
                                    td { class: "py-1 pr-4", "Possible" }
                                    td { class: "py-1 pr-4 text-green-400", "None — ever" }
                                    td { class: "py-1", "Possible" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-4 text-gray-400", "Multilingual" }
                                    td { class: "py-1 pr-4", "Decent" }
                                    td { class: "py-1 pr-4", "Good (byte fallback)" }
                                    td { class: "py-1", "Often best" }
                                }
                                tr { class: "border-b border-gray-700/50",
                                    td { class: "py-1 pr-4 text-gray-400", "Flexibility" }
                                    td { class: "py-1 pr-4", "Low" }
                                    td { class: "py-1 pr-4", "Low" }
                                    td { class: "py-1", "High" }
                                }
                                tr {
                                    td { class: "py-1 pr-4 text-gray-400", "Used by" }
                                    td { class: "py-1 pr-4", "Original NMT work" }
                                    td { class: "py-1 pr-4", "GPT-2/3/4, LLaMA-2, Mistral, Gemma" }
                                    td { class: "py-1", "T5, ALBERT, multilingual models" }
                                }
                            }
                        }
                    }
                }

                // When to choose + RAG implications side by side
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-2 mb-2",

                    // When to choose
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "When to Choose Which" }
                        div { class: "grid grid-cols-3 gap-2",
                            div {
                                p { class: "text-xs text-blue-300 font-semibold mb-0.5", "Choose BPE if:" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Reproducing a legacy system" }
                                    li { "• Single-language, clean corpus" }
                                    li { "• Studying the algorithm foundation" }
                                }
                            }
                            div {
                                p { class: "text-xs text-cyan-300 font-semibold mb-0.5", "Choose BBPE if:" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Building a modern GPT-style LLM" }
                                    li { "• Noisy text: code, ASR, OCR" }
                                    li { "• No unknown tokens is a requirement" }
                                    li { "• Extending GPT-2/LLaMA tokenizers" }
                                }
                            }
                            div {
                                p { class: "text-xs text-purple-300 font-semibold mb-0.5", "Choose Unigram if:" }
                                ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                    li { "• Multilingual or morphologically rich text" }
                                    li { "• Cleaner, more compact vocabularies" }
                                    li { "• Sampling-based augmentation" }
                                    li { "• T5-style encoder-decoder models" }
                                }
                            }
                        }
                    }

                    // RAG implications
                    div { class: "bg-gray-800 border border-yellow-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-yellow-300 mb-1", "Practical Implications for Your Rust RAG Pipeline" }
                        p { class: "text-xs text-gray-400 mb-1", "Given your goals (summary-first retrieval, clustering, memory-efficient graph projections):" }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li {
                                span { class: "text-cyan-300 font-semibold", "BBPE " }
                                "is what your backing LLMs (LLaMA, Mistral) actually use — understanding it is the most practical path. Stable byte-level boundaries → reproducible embeddings and no OOV surprises."
                            }
                            li {
                                span { class: "text-purple-300 font-semibold", "Unigram " }
                                "gives slightly better compression and more semantically coherent subwords → can improve embedding quality and reduce RAM footprint for large corpora."
                            }
                            li {
                                span { class: "text-blue-300 font-semibold", "Classic BPE " }
                                "is mainly useful as a conceptual stepping stone — few modern production systems use it directly."
                            }
                        }
                        p { class: "text-xs text-gray-400 mt-1",
                            "For multilingual or domain-heavy corpora, Unigram tends to produce cleaner clusters; for everything else BBPE is the safe default."
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
