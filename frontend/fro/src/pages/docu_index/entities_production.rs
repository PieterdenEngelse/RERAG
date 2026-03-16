//! Entities Production documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuEntitiesProduction() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4", "Entities Production" }
                    p { class: "text-lg text-gray-200 mb-4",
                        "Entities aren't \"generated\" automatically by a knowledge graph system\u{2014}they are "
                        strong { "created from your data" }
                        ". But the way they are produced depends on the pipeline you build."
                    }

                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "How Entities Are Produced" }

                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "1. Entities come from your source data"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Any concrete, real-world thing in your dataset becomes an entity."
                    }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "A row in a database \u{2192} entity" }
                        li { "A JSON object \u{2192} entity" }
                        li { "A document \u{2192} entity" }
                        li { "A user profile \u{2192} entity" }
                        li { "A product \u{2192} entity" }
                        li { "A location \u{2192} entity" }
                    }

                    h4 { class: "text-lg font-semibold text-white mt-6 mb-2",
                        "2. Entities can be extracted from text"
                    }
                    p { class: "text-sm text-gray-300 mb-2",
                        "If you run NLP or an ONNX model for NER (Named Entity Recognition), you can detect entities inside text."
                    }
                    p { class: "text-sm text-gray-300 mb-2", "Example sentence:" }
                    div { class: "bg-gray-700 rounded p-3 my-2 text-sm text-gray-200 italic",
                        "Microsoft acquired GitHub in 2018."
                    }
                    p { class: "text-sm text-gray-300 mt-2 mb-2", "NER model outputs:" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li {
                            span { class: "text-blue-300", "Microsoft" }
                            " \u{2192} Organization entity"
                        }
                        li {
                            span { class: "text-blue-300", "GitHub" }
                            " \u{2192} Organization entity"
                        }
                        li {
                            span { class: "text-blue-300", "2018" }
                            " \u{2192} Date entity"
                        }
                    }

                    h3 { class: "text-xl font-bold text-white mt-8 mb-3", "Summary Table" }
                    table { class: "table table-sm w-full text-gray-300 mb-4",
                        thead {
                            tr {
                                th { class: "text-gray-200", "Source" }
                                th { class: "text-gray-200", "How Entities Are Produced" }
                            }
                        }
                        tbody {
                            tr {
                                td { "Structured data" }
                                td { "Each row/object becomes an entity node" }
                            }
                            tr {
                                td { "Text (NER)" }
                                td { "ONNX/NLP models extract named entities" }
                            }
                            tr {
                                td { "Manual modeling" }
                                td { "You define entity types and create nodes" }
                            }
                            tr {
                                td { "Embeddings + clustering" }
                                td { "Semantic groups become entity nodes" }
                            }
                            tr {
                                td { "Graph inference" }
                                td { "Patterns in relationships reveal new entities" }
                            }
                        }
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
