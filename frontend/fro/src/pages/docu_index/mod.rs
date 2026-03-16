//! Documentation index subpages

mod embeddings;
mod knowledge_graphs;
mod onnx;
mod onnx_params;
mod io_uring;
mod bias;
mod threads;
mod entities_production;
mod ag_pipeline;
mod lora_export;
mod neo4j;
mod tantivy;
mod bm25;

pub use embeddings::DocuEmbeddings;
pub use knowledge_graphs::DocuKnowledgeGraphs;
pub use onnx::DocuOnnx;
pub use onnx_params::DocuOnnxParams;
pub use io_uring::DocuIoUring;
pub use bias::DocuBias;
pub use threads::DocuThreads;
pub use entities_production::DocuEntitiesProduction;
pub use ag_pipeline::DocuAgPipeline;
pub use lora_export::DocuLoraExport;
pub use neo4j::DocuNeo4j;
pub use tantivy::DocuTantivy;
pub use bm25::DocuBm25;
// Documentation - Index page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuIndex() -> Element {
    rsx! {
        div { id: "top", class: "min-h-screen bg-gray-900 p-6",
            div { class: "max-w-4xl mx-auto",

                a {
                    href: "/docu",
                    class: "text-primary hover:underline mb-4 inline-block",
                    "← Back to Documentation"
                }

                h1 { class: "text-3xl font-bold mb-6 text-white", "📇 Index" }

                div { class: "space-y-2",
                    Link {
                        to: Route::DocuEmbeddings {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Embeddings"
                    }
                    Link {
                        to: Route::DocuKnowledgeGraphs {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Knowledge Graphs"
                    }
                    Link {
                        to: Route::DocuOnnx {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "ONNX"
                    }
                    Link {
                        to: Route::DocuOnnxParams {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "ONNX Parameters"
                    }
                    Link {
                        to: Route::DocuIoUring {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "io_uring"
                    }
                    Link {
                        to: Route::DocuBias {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Bias"
                    }
                    Link {
                        to: Route::DocuThreads {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Threads"
                    }
                    Link {
                        to: Route::DocuEntitiesProduction {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Entities Production"
                    }
                    Link {
                        to: Route::DocuAgPipeline {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "AG Pipeline"
                    }
                    Link {
                        to: Route::DocuLoraExport {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "LoRA Export"
                    }
                    Link {
                        to: Route::DocuNeo4j {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Neo4j"
                    }
                    Link {
                        to: Route::DocuTantivy {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Tantivy"
                    }
                    Link {
                        to: Route::DocuBm25 {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "BM25"
                    }
                }
            }
        }
    }
}
