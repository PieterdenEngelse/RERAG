//! Agglutinative Languages documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuAgglutinative() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-blue-300", "Agglutinative Languages" }
                    span { class: "text-xs text-gray-400", "Words built by stacking morphemes — one piece, one meaning." }
                }

                p { class: "text-xs text-gray-300 mb-3",
                    "Agglutinative languages form words by stringing together morphemes, each with a single, clear grammatical meaning, usually without changing their form. That makes long words highly transparent: you can read off the grammar piece by piece. It also makes tokenization genuinely hard — a single word can encode what English needs an entire clause to say."
                }

                // ---- Definition ----
                div { class: "mb-4 p-3 bg-gray-800 border border-blue-700 rounded-lg",
                    h2 { class: "text-sm font-bold text-blue-300 mb-2", "The Linguist-Approved Definition" }
                    p { class: "text-xs text-gray-200 italic mb-2",
                        "An agglutinative language forms words by stringing together morphemes, each with a single, clear grammatical meaning, usually without changing their form."
                    }
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-2 mt-2",
                        div { class: "bg-gray-700 rounded p-2",
                            p { class: "text-xs text-yellow-300 font-semibold mb-1", "One morpheme = one function" }
                            p { class: "text-xs text-gray-300", "Each affix carries exactly one grammatical role: plural, tense, case, possession, negation — never blended." }
                        }
                        div { class: "bg-gray-700 rounded p-2",
                            p { class: "text-xs text-yellow-300 font-semibold mb-1", "Morphemes stay stable" }
                            p { class: "text-xs text-gray-300", "Affixes don't fuse or mutate much when combined. Contrast with fusional languages (Latin, Russian) where endings blend several meanings at once." }
                        }
                        div { class: "bg-gray-700 rounded p-2",
                            p { class: "text-xs text-yellow-300 font-semibold mb-1", "Predictable affix order" }
                            p { class: "text-xs text-gray-300", "Morphemes attach in a fixed order, making words long but grammatically transparent." }
                        }
                    }
                }

                // ---- Classic Example ----
                div { class: "mb-4",
                    h2 { class: "text-sm font-bold text-white mb-2", "Classic Example: Turkish" }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-3",
                        div { class: "overflow-x-auto",
                            table { class: "w-full text-xs border-collapse mb-3",
                                thead {
                                    tr { class: "bg-gray-700",
                                        th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Word" }
                                        th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Breakdown" }
                                        th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Meaning" }
                                    }
                                }
                                tbody {
                                    tr {
                                        td { class: "px-2 py-1 border border-gray-700 text-blue-300 font-mono", "ev‑ler‑den" }
                                        td { class: "px-2 py-1 border border-gray-700 text-gray-300 font-mono", "ev + ler + den" }
                                        td { class: "px-2 py-1 border border-gray-700 text-gray-300", "house + plural + from = \"from the houses\"" }
                                    }
                                    tr { class: "bg-gray-800/50",
                                        td { class: "px-2 py-1 border border-gray-700 text-blue-300 font-mono", "git‑me‑yecek‑ti" }
                                        td { class: "px-2 py-1 border border-gray-700 text-gray-300 font-mono", "git + me + yecek + ti" }
                                        td { class: "px-2 py-1 border border-gray-700 text-gray-300", "go + NEG + FUT + PAST = \"was not going to go\"" }
                                    }
                                    tr {
                                        td { class: "px-2 py-1 border border-gray-700 text-blue-300 font-mono", "ev‑ler‑in‑den" }
                                        td { class: "px-2 py-1 border border-gray-700 text-gray-300 font-mono", "ev + ler + in + den" }
                                        td { class: "px-2 py-1 border border-gray-700 text-gray-300", "house + plural + GEN + from = \"from their houses\"" }
                                    }
                                }
                            }
                        }
                        p { class: "text-xs text-gray-400", "Each morpheme is a clean building block. Remove one and the word is still grammatical — just with different meaning." }
                    }
                }

                // ---- Major Language Families ----
                div { class: "mb-4",
                    h2 { class: "text-sm font-bold text-white mb-2", "Major Agglutinative Languages" }
                    div { class: "grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-2",

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "Turkic Family" }
                            ul { class: "text-xs text-gray-300 space-y-0.5 list-none",
                                li { "Turkish, Uzbek, Kazakh, Azerbaijani" }
                                li { "Highly regular morphology" }
                                li { "Vowel harmony governs affix vowels" }
                            }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "Uralic Family" }
                            ul { class: "text-xs text-gray-300 space-y-0.5 list-none",
                                li { "Finnish, Estonian, Hungarian" }
                                li { "Rich case systems (Finnish: 15 cases)" }
                                li { "Postpositions instead of prepositions" }
                            }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "Japonic & Koreanic" }
                            ul { class: "text-xs text-gray-300 space-y-0.5 list-none",
                                li { "Japanese, Korean" }
                                li { "Postpositional particles as separate morphemes" }
                                li { "SOV word order; verb morphology is rich" }
                            }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "Bantu Family" }
                            ul { class: "text-xs text-gray-300 space-y-0.5 list-none",
                                li { "Swahili, Zulu, Xhosa" }
                                li { "Noun-class prefixes propagate across the clause" }
                                li { "Verb prefixes encode subject, object, tense, aspect" }
                            }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "Dravidian Family" }
                            ul { class: "text-xs text-gray-300 space-y-0.5 list-none",
                                li { "Tamil, Telugu, Kannada" }
                                li { "Suffixing morphology" }
                                li { "Case and tense stacked as suffixes" }
                            }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "Other Notable Cases" }
                            ul { class: "text-xs text-gray-300 space-y-0.5 list-none",
                                li { "Quechua (Andean)" }
                                li { "Basque (language isolate)" }
                                li { "Georgian (partially polysynthetic)" }
                            }
                        }
                    }
                }

                // ---- Why this matters for tokenizers ----
                div { class: "mb-4",
                    h2 { class: "text-sm font-bold text-white mb-2", "Why Agglutinative Languages Are Hard for Tokenizers" }
                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-2",

                        div { class: "bg-gray-800 border border-red-800 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-red-300 mb-1", "The Problem with BPE / WordPiece" }
                            p { class: "text-xs text-gray-300 mb-1",
                                "BPE learns merges from training data frequency. In English, common words are already tokens. In Turkish, "
                                span { class: "font-mono text-yellow-300", "ev" }
                                " (house) might appear as "
                                span { class: "font-mono text-yellow-300", "evlerinden" }
                                " — one word, 4 morphemes — and be split into arbitrary byte-pair chunks that don't align with any morpheme boundary."
                            }
                            ul { class: "text-xs text-gray-300 space-y-0.5 list-none",
                                li { "• Token boundaries cross morpheme boundaries" }
                                li { "• Same root appears in thousands of surface forms" }
                                li { "• Vocabulary bloat: rare forms become multi-token sequences" }
                                li { "• Context window wasted on grammatical morphology" }
                            }
                        }

                        div { class: "bg-gray-800 border border-green-800 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-green-300 mb-1", "Morpheme-Aware Solutions" }
                            p { class: "text-xs text-gray-300 mb-1",
                                "Morpheme-aware tokenizers segment on linguistic boundaries — they understand that "
                                span { class: "font-mono text-yellow-300", "ev‑ler‑den" }
                                " is three distinct units."
                            }
                            ul { class: "text-xs text-gray-300 space-y-0.5 list-none",
                                li {
                                    span { class: "text-yellow-400", "Morfessor — " }
                                    "unsupervised morpheme segmentation, language-agnostic"
                                }
                                li {
                                    span { class: "text-yellow-400", "MeCab — " }
                                    "Japanese morphological analyser with POS tagging"
                                }
                                li {
                                    span { class: "text-yellow-400", "KoNLPy — " }
                                    "Korean NLP toolkit with multiple analysers"
                                }
                                li {
                                    span { class: "text-yellow-400", "Zemberek — " }
                                    "Turkish NLP library with morphological analysis"
                                }
                            }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-yellow-300 mb-1", "SentencePiece Helps, Partially" }
                            p { class: "text-xs text-gray-300",
                                "SentencePiece (used in T5, LLaMA, mT5) operates on raw text without pre-tokenization, which helps with agglutinative languages. The Unigram LM variant in particular tends to find morpheme-like boundaries because morpheme-level splits are statistically efficient. It's not perfect, but it's significantly better than pure BPE on Turkish, Finnish, or Swahili."
                            }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-yellow-300 mb-1", "The Token Fertility Problem" }
                            p { class: "text-xs text-gray-300",
                                "\"Token fertility\" = how many tokens a model needs per word. English averages ~1.3 tokens/word with GPT-4 tiktoken. Turkish can hit 3–5 tokens/word for the same meaning. This means agglutinative text consumes 2–4× more context window, raising inference cost and reducing effective context length for these languages."
                            }
                        }
                    }
                }

                // ---- Agglutinative vs Fusional ----
                div { class: "mb-4",
                    h2 { class: "text-sm font-bold text-white mb-2", "Agglutinative vs. Fusional vs. Isolating" }
                    div { class: "overflow-x-auto",
                        table { class: "w-full text-xs border-collapse",
                            thead {
                                tr { class: "bg-gray-700",
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Type" }
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Morpheme:Meaning" }
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Morpheme stability" }
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Examples" }
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Tokenizer challenge" }
                                }
                            }
                            tbody {
                                tr {
                                    td { class: "px-2 py-1 border border-gray-700 text-blue-300", "Agglutinative" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "1:1 (one meaning per morpheme)" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "High (affixes don't mutate)" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Turkish, Finnish, Japanese" }
                                    td { class: "px-2 py-1 border border-gray-700 text-red-300", "High vocab, long words" }
                                }
                                tr { class: "bg-gray-800/50",
                                    td { class: "px-2 py-1 border border-gray-700 text-purple-300", "Fusional" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "1:many (endings blend meanings)" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Low (endings mutate/fuse)" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Latin, Russian, Arabic" }
                                    td { class: "px-2 py-1 border border-gray-700 text-red-300", "Irregular forms, root allomorphy" }
                                }
                                tr {
                                    td { class: "px-2 py-1 border border-gray-700 text-green-300", "Isolating" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "1 morpheme ≈ 1 word" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "N/A (no affixes)" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Mandarin, Vietnamese, Thai" }
                                    td { class: "px-2 py-1 border border-gray-700 text-yellow-300", "Word boundary detection" }
                                }
                            }
                        }
                    }
                }

                // ---- Back link ----
                div { class: "mt-2 pt-2 border-t border-gray-700 flex gap-3",
                    Link { to: Route::DocuTokenizersGeneral {}, class: "btn btn-primary btn-xs", "← Tokenizers General" }
                    Link { to: Route::DocuIndex {}, class: "btn btn-primary btn-xs", "← Back to Index" }
                }
            }
        }
    }
}
