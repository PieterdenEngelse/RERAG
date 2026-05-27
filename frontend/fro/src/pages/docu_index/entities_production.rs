//! Entities Production documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuEntitiesProduction() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "Entities Production" }
                    span { class: "text-xs text-gray-400", "Entities are created from your data — the pipeline determines how." }
                }

                div { class: "grid grid-cols-3 gap-2",

                    // Col 1: source data
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "1. From uploaded documents" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "AG runs NER at ingestion time — as each document is chunked and indexed, "
                            code { class: "text-green-300", "ner_extractor.rs" }
                            " scans the text and extracts named entities automatically."
                        }
                        p { class: "text-xs text-gray-400 mb-1", "Supported input formats:" }
                        ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                            li { "PDF, TXT, Markdown" }
                            li { "HTML, XML, JSON" }
                            li { "Office: DOCX, XLSX, CSV, ODT, ODS" }
                            li { "Source code: .rs .py .js .ts .go .java .cs .cpp .c .rb .php .sh .sql .yaml .toml" }
                            li { "Special files: Dockerfile, Makefile, .gitignore, README" }
                        }
                        p { class: "text-xs text-gray-300 mt-1",
                            "No separate dataset needed — your documents are the input."
                        }
                    }

                    // Col 2: NER extraction
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "2. How the model works" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Uses " span { class: "text-white", "dslim/bert-base-NER" } " — a pre-trained BERT model fine-tuned on CoNLL-2003, loaded from "
                            code { class: "text-green-300", "~/ag/models/ner/model.onnx" }
                            ". Runs on each chunk at index time."
                        }
                        div { class: "bg-gray-700 rounded p-2 my-1 text-xs text-gray-200 italic", "Microsoft acquired GitHub in 2018." }
                        p { class: "text-xs text-gray-400 mb-0.5", "Model outputs:" }
                        ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                            li { span { class: "text-blue-300", "Microsoft" } " → ORG" }
                            li { span { class: "text-blue-300", "GitHub" } " → ORG" }
                            li { span { class: "text-blue-300", "2018" } " → MISC" }
                        }
                        p { class: "text-xs text-gray-300 mt-1", "Labels: PERSON, ORG, LOC, MISC" }
                    }

                    // Col 3: pipeline summary
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Pipeline Summary" }
                        div { class: "text-xs text-gray-300 space-y-1",
                            p {
                                span { class: "text-gray-400", "Input: " }
                                "uploaded document (PDF, TXT, MD, DOCX, XLSX, CSV, ODT, ODS, …)"
                            }
                            p {
                                span { class: "text-gray-400", "Chunked by: " }
                                code { class: "text-green-300", "chunker.rs" }
                            }
                            p {
                                span { class: "text-gray-400", "NER run by: " }
                                code { class: "text-green-300", "ner_extractor.rs" }
                                " on each chunk text"
                            }
                            p {
                                span { class: "text-gray-400", "Model: " }
                                "dslim/bert-base-NER via ONNX Runtime"
                            }
                            p {
                                span { class: "text-gray-400", "Stored to: " }
                                "FalkorDB as Entity nodes with MENTIONS edges to Chunk nodes"
                            }
                            p {
                                span { class: "text-gray-400", "Threshold: " }
                                "confidence ≥ 0.7, length ≥ 2 chars"
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
