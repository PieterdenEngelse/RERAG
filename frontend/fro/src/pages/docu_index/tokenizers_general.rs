//! Tokenizers General documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuTokenizersGeneral() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-blue-300", "Tokenizers — General" }
                    span { class: "text-xs text-gray-400", "The universal adapter between raw human data and symbolic computation." }
                }

                p { class: "text-xs text-gray-300 mb-3",
                    "Tokenizers are basically the interface layer between raw human data and any model that expects discrete symbols. That opens up a whole ecosystem of use cases."
                }

                // ---- Family Tree ----
                div { class: "mb-4",
                    h2 { class: "text-sm font-bold text-white mb-1", "The Tokenizer Family Tree" }
                    p { class: "text-xs text-gray-400 mb-2",
                        "BPE and WordPiece are siblings in a much larger taxonomy. Modern LLMs draw from several distinct families, each designed around different goals: compression efficiency, linguistic structure, byte-level universality, or robustness to noise."
                    }

                    div { class: "grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-2 mb-3",

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "1. Character-level" }
                            p { class: "text-xs text-gray-400 mb-1", "One token per character. Smallest possible vocabulary." }
                            ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                li { span { class: "text-yellow-400", "Vocab: " } "≈100–300 chars" }
                                li { span { class: "text-green-400", "Pro: " } "No OOV issues" }
                                li { span { class: "text-red-400", "Con: " } "Very long sequences" }
                            }
                            p { class: "text-xs text-gray-500 mt-1", "Char-CNN, DeepSpeech, DNA models" }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "2. Word-level" }
                            p { class: "text-xs text-gray-400 mb-1", "One token per word. The pre-2015 default." }
                            ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                li { span { class: "text-yellow-400", "Vocab: " } "50k–500k" }
                                li { span { class: "text-green-400", "Pro: " } "Fast, intuitive" }
                                li { span { class: "text-red-400", "Con: " } "OOV and huge embedding matrices" }
                            }
                            p { class: "text-xs text-gray-500 mt-1", "Early word2vec pipelines, classic NLP" }
                        }

                        div { class: "bg-gray-800 border border-blue-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-blue-300 mb-1", "3. Subword — modern core" }
                            p { class: "text-xs text-gray-400 mb-1", "Best compression / vocab tradeoff. Four main algorithms:" }
                            ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                li {
                                    span { class: "text-yellow-400", "BPE — " }
                                    Link { to: Route::DocuBpeUnigram {}, class: "text-primary hover:underline", "greedy pair merges" }
                                }
                                li { span { class: "text-yellow-400", "WordPiece — " } "merge maximising corpus likelihood" }
                                li { span { class: "text-yellow-400", "Unigram LM — " } "probabilistic; prunes a candidate set" }
                                li { span { class: "text-yellow-400", "SentencePiece — " } "framework: BPE or Unigram on raw bytes" }
                            }
                            p { class: "text-xs text-gray-500 mt-1", "BERT, GPT-2, T5, ALBERT, LLaMA-1" }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "4. Byte-level" }
                            p { class: "text-xs text-gray-400 mb-1", "Operates directly on bytes (0–255)." }
                            ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                li { span { class: "text-green-400", "Pro: " } "No Unicode normalisation issues" }
                                li { span { class: "text-green-400", "Pro: " } "Handles emojis, accents, noise" }
                                li { span { class: "text-red-400", "Con: " } "Longer sequences than subword" }
                            }
                            p { class: "text-xs text-gray-500 mt-1", "GPT-2/3/4 tiktoken, LLaMA-2 BPE" }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "5. Morpheme-aware" }
                            p { class: "text-xs text-gray-400 mb-1", "Linguistically informed segmentation." }
                            ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                li {
                                    span { class: "text-green-400", "Pro: " }
                                    "Great for "
                                    Link { to: Route::DocuAgglutinative {}, class: "text-primary hover:underline", "agglutinative languages" }
                                    " (like Turkish)"
                                }
                                li { span { class: "text-red-400", "Con: " } "Hard to generalise across languages" }
                            }
                            p { class: "text-xs text-gray-500 mt-1", "MeCab (Japanese), Morfessor, KoNLPy (Korean)" }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "6. Hybrid (rules + stats)" }
                            p { class: "text-xs text-gray-400 mb-1", "Combines rule-based exceptions with statistical segmentation." }
                            p { class: "text-xs text-gray-500 mt-1", "spaCy tokenizer, Moses (MT pipelines)" }
                        }

                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-purple-300 mb-1", "7. Whitespace / rule-based" }
                            p { class: "text-xs text-gray-400 mb-1", "Simple splitting with heuristics. Classical NLP baseline." }
                            p { class: "text-xs text-gray-500 mt-1", "NLTK word_tokenize, regex tokenizers" }
                        }

                        div { class: "bg-gray-800 border border-orange-700 rounded-lg p-2",
                            h3 { class: "text-xs font-bold text-orange-300 mb-1", "8. Neural (emerging)" }
                            p { class: "text-xs text-gray-400 mb-1", "Learned end-to-end with the model — or tokenizer-free entirely." }
                            ul { class: "text-xs text-gray-200 space-y-0.5 list-none",
                                li { span { class: "text-yellow-400", "Neural segmentation — " } "subword boundaries via a neural net" }
                                li { span { class: "text-yellow-400", "Tokenizer-free — " } "model trains directly on bytes or chars" }
                            }
                            p { class: "text-xs text-gray-500 mt-1", "CANINE, ByT5, Charformer, Meta byte-level prototypes" }
                        }
                    }

                    div { class: "overflow-x-auto",
                        table { class: "w-full text-xs border-collapse",
                            thead {
                                tr { class: "bg-gray-700",
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Family" }
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Granularity" }
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Learning strategy" }
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Pros" }
                                    th { class: "text-left text-gray-200 px-2 py-1 font-semibold border border-gray-600", "Cons" }
                                }
                            }
                            tbody {
                                tr {
                                    td { class: "px-2 py-1 border border-gray-700 text-purple-300", "Character" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "1 char" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "None" }
                                    td { class: "px-2 py-1 border border-gray-700 text-green-300", "No OOV" }
                                    td { class: "px-2 py-1 border border-gray-700 text-red-300", "Long sequences" }
                                }
                                tr { class: "bg-gray-800/50",
                                    td { class: "px-2 py-1 border border-gray-700 text-purple-300", "Word" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "1 word" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Frequency" }
                                    td { class: "px-2 py-1 border border-gray-700 text-green-300", "Fast" }
                                    td { class: "px-2 py-1 border border-gray-700 text-red-300", "Huge vocab, OOV" }
                                }
                                tr {
                                    td { class: "px-2 py-1 border border-gray-700 text-blue-300", "Subword" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "2–10 chars" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Statistical" }
                                    td { class: "px-2 py-1 border border-gray-700 text-green-300", "Best tradeoff" }
                                    td { class: "px-2 py-1 border border-gray-700 text-red-300", "Training complexity" }
                                }
                                tr { class: "bg-gray-800/50",
                                    td { class: "px-2 py-1 border border-gray-700 text-purple-300", "Byte-level" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "1 byte" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "None / statistical" }
                                    td { class: "px-2 py-1 border border-gray-700 text-green-300", "Universal" }
                                    td { class: "px-2 py-1 border border-gray-700 text-red-300", "Longer sequences" }
                                }
                                tr {
                                    td { class: "px-2 py-1 border border-gray-700 text-purple-300", "Morpheme" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Linguistic units" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Rules + stats" }
                                    td { class: "px-2 py-1 border border-gray-700 text-green-300", "Complex languages" }
                                    td { class: "px-2 py-1 border border-gray-700 text-red-300", "Hard to generalise" }
                                }
                                tr { class: "bg-gray-800/50",
                                    td { class: "px-2 py-1 border border-gray-700 text-orange-300", "Neural" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Variable" }
                                    td { class: "px-2 py-1 border border-gray-700 text-gray-300", "Learned end-to-end" }
                                    td { class: "px-2 py-1 border border-gray-700 text-green-300", "Potentially optimal" }
                                    td { class: "px-2 py-1 border border-gray-700 text-red-300", "Very new, experimental" }
                                }
                            }
                        }
                    }
                }

                // ---- Use Cases ----
                div { class: "mb-2 mt-1",
                    h2 { class: "text-sm font-bold text-white", "Use Cases Beyond LLMs" }
                    p { class: "text-xs text-gray-400", "Because tokenizers are the universal adapter between raw data and symbolic computation, they appear across many non-LLM contexts." }
                }

                div { class: "grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-2",

                    // 1. Compression & Storage
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "1. Compression & Storage Optimization" }
                        p { class: "text-xs text-gray-300 mb-1", "Tokenizers act as domain-specific compressors." }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li {
                                span { class: "text-yellow-400", "Lossless text compression — " }
                                Link {
                                    to: Route::DocuBpeUnigram {},
                                    class: "text-primary hover:underline",
                                    "BPE/Unigram tokenizers"
                                }
                                " often outperform gzip on natural language because they exploit linguistic structure. Useful for: log storage, telemetry, chat archives, dataset deduplication."
                            }
                            li {
                                span { class: "text-yellow-400", "Embedding store compression — " }
                                "Storing tokenized text instead of raw strings reduces RAM/SSD footprint in vector DBs or graph stores."
                            }
                            li {
                                span { class: "text-yellow-400", "On-device model deployment — " }
                                "Smaller vocabularies → smaller embedding matrices → smaller models."
                            }
                        }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-blue-900 text-blue-300", "Subword" }
                        }
                    }

                    // 2. Data Cleaning
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "2. Data Cleaning & Normalization Pipelines" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Tokenizers are often the first "
                            Link {
                                to: Route::DocuCanonicalization {},
                                class: "text-primary hover:underline",
                                "canonicalization"
                            }
                            " step in any ML or analytics pipeline."
                        }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li { "• Normalizing Unicode, accents, punctuation" }
                            li { "• Splitting mixed-language text" }
                            li { "• Detecting malformed sequences" }
                            li { "• Identifying out-of-vocabulary patterns (useful for anomaly detection)" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "This is why tokenizers show up in ETL pipelines even when no LLM is involved." }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Byte-level" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-gray-700 text-gray-300", "Hybrid" }
                        }
                    }

                    // 3. Security & Filtering
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "3. Security & Filtering" }
                        p { class: "text-xs text-gray-300 mb-1", "Tokenization is a surprisingly strong tool for security." }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li {
                                span { class: "text-yellow-400", "Prompt injection detection — " }
                                "Certain attack patterns produce distinctive token sequences (e.g., repeated control tokens, weird Unicode)."
                            }
                            li {
                                span { class: "text-yellow-400", "PII detection — " }
                                "Phone numbers, emails, IBANs, etc. map to predictable token patterns."
                            }
                            li {
                                span { class: "text-yellow-400", "Malware / code anomaly detection — " }
                                "Tokenizers for code reveal suspicious patterns in source code or logs."
                            }
                        }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Byte-level" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-gray-700 text-gray-300", "Rule-based" }
                        }
                    }

                    // 4. Search & IR
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "4. Search & IR Beyond RAG" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Tokenizers are core to classical IR (Boolean retrieval, vector-space models, TF-IDF, BM25, inverted indexes). Modern tokenizers (BPE, WordPiece, SentencePiece) break words into subwords to handle morphology and OOV words. Classical IR tokenizers were word-level, not subword-level."
                        }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li {
                                span { class: "text-yellow-400", "BM25 / sparse retrieval — " }
                                "Tokenization defines the term space. Modern hybrid search uses LLM tokenizers to unify sparse + dense retrieval."
                            }
                            li {
                                span { class: "text-yellow-400", "Query rewriting — " }
                                "Token-level stats help detect synonyms, multi-word expressions, and segmentation errors."
                            }
                            li {
                                span { class: "text-yellow-400", "Index compression — " }
                                "Token IDs compress better than raw strings in inverted indexes."
                            }
                        }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Word-level" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-blue-900 text-blue-300", "Subword" }
                        }
                    }

                    // 5. Code Intelligence
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "5. Code Intelligence" }
                        p { class: "text-xs text-gray-300 mb-1", "For code, tokenizers are essential even outside LLMs." }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li { "• Static analysis" }
                            li { "• AST reconstruction" }
                            li { "• Code similarity detection" }
                            li { "• Clone detection" }
                            li { "• Vulnerability pattern mining" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "Many code tools use tokenization as a preprocessing step before graph-based or symbolic analysis." }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-blue-900 text-blue-300", "Subword" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-gray-700 text-gray-300", "Hybrid" }
                        }
                    }

                    // 6. Speech, OCR, Multimodal
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "6. Speech, OCR, and Multimodal Pipelines" }
                        p { class: "text-xs text-gray-300 mb-1", "Tokenizers bridge raw signals → text → model input." }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li {
                                span { class: "text-yellow-400", "ASR (Automatic Speech Recognition) post-processing — " }
                                "Tokenizers help segment raw ASR output into meaningful units."
                            }
                            li {
                                span { class: "text-yellow-400", "OCR cleanup — " }
                                "Tokenization reveals segmentation errors, ligature issues, or hallucinated characters."
                            }
                            li {
                                span { class: "text-yellow-400", "Multimodal alignment — " }
                                "Vision-language models align image regions to token sequences."
                            }
                        }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Byte-level" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Character-level" }
                        }
                    }

                    // 7. Dataset Analytics
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "7. Dataset Analytics & Quality Control" }
                        p { class: "text-xs text-gray-300 mb-1", "Tokenizers give you a structured view of your corpus." }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li { "• Token distribution analysis" }
                            li { "• Detecting domain drift" }
                            li { "• Identifying rare or harmful patterns" }
                            li { "• Measuring dataset entropy" }
                            li { "• Detecting duplicates via token-level hashing" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "Crucial for training data curation." }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-blue-900 text-blue-300", "Subword" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Word-level" }
                        }
                    }

                    // 8. Model Evaluation
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "8. Model Evaluation & Benchmarking" }
                        p { class: "text-xs text-gray-300 mb-1", "Tokenization affects evaluation metrics." }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li { "• Perplexity is computed over tokens" }
                            li { "• Tokenization affects BLEU/ROUGE scores" }
                            li { "• Token boundaries influence error analysis" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "Researchers often run multiple tokenizers to compare model behavior." }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-blue-900 text-blue-300", "Subword" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Byte-level" }
                        }
                    }

                    // 9. Custom DSLs
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "9. Custom DSLs & Domain-Specific Models" }
                        p { class: "text-xs text-gray-300 mb-1", "Tokenizers define the \"language\" of:" }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li { "• SQL variants" }
                            li { "• Robotics command languages" }
                            li { "• Game scripting languages" }
                            li { "• Financial transaction logs" }
                            li { "• Medical coding systems (ICD, SNOMED)" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "A tokenizer becomes the grammar boundary for the domain." }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-gray-700 text-gray-300", "Rule-based" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Character-level" }
                        }
                    }

                    // 10. Privacy & Federated Learning
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "10. Privacy & Federated Learning" }
                        p { class: "text-xs text-gray-300 mb-1", "Tokenizers help with:" }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li {
                                span { class: "text-yellow-400", "Local differential privacy — " }
                                "Token-level noise injection."
                            }
                            li {
                                span { class: "text-yellow-400", "Federated text learning — " }
                                "Tokenization ensures consistent vocab across devices."
                            }
                            li {
                                span { class: "text-yellow-400", "Sensitive token masking — " }
                                "Before sending data to a server or model."
                            }
                        }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-blue-900 text-blue-300", "Subword" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Byte-level" }
                        }
                    }

                    // 11. Token-Level Feature Engineering
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "11. Token-Level Feature Engineering" }
                        p { class: "text-xs text-gray-300 mb-1", "Even outside deep learning:" }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li { "• Logistic regression on token n-grams" }
                            li { "• Token-level TF-IDF" }
                            li { "• Token-based clustering" }
                            li { "• Topic modeling (LDA, NMF)" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "Tokenizers are the foundation of classical NLP." }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Word-level" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-blue-900 text-blue-300", "Subword" }
                        }
                    }

                    // 12. Graph Construction
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-green-300 mb-1", "12. Graph Construction" }
                        p { class: "text-xs text-gray-300 mb-1", "Tokenizers help build:" }
                        ul { class: "text-xs text-gray-200 space-y-1 list-none",
                            li { "• Token co-occurrence graphs" }
                            li { "• Token-sentence bipartite graphs" }
                            li { "• Token-topic graphs" }
                            li { "• Token-embedding similarity graphs" }
                        }
                        p { class: "text-xs text-gray-400 mt-1", "Useful for GraphRAG, clustering, and summary-first retrieval." }
                        div { class: "flex flex-wrap gap-1 mt-2",
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-purple-900 text-purple-300", "Word-level" }
                            span { class: "inline-block px-1 rounded text-[10px] font-semibold bg-blue-900 text-blue-300", "Subword" }
                        }
                    }
                }

                div { class: "mt-3 p-2 bg-gray-800 border border-blue-700 rounded-lg",
                    p { class: "text-xs text-blue-200 font-semibold", "The meta-point" }
                    p { class: "text-xs text-gray-300 mt-1",
                        "Tokenizers are not just \"LLM plumbing.\" They're the universal adapter between raw human data and symbolic computation."
                    }
                }

                div { class: "mt-2 pt-2 border-t border-gray-700",
                    Link { to: Route::DocuIndex {}, class: "btn btn-primary btn-xs", "← Back to Index" }
                }
            }
        }
    }
}
