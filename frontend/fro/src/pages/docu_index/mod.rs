//! Documentation index subpages

mod ag_pipeline;
mod agglutinative;
mod bias;
mod bm25;
mod bpe_unigram;
mod canonicalization;
mod detr_layout;
mod embeddings;
mod entities_production;
mod file_watcher;
mod io_uring;
mod knowledge_graphs;
mod lora_export;
mod onnx;
mod onnx_params;
mod rig;
mod rkyv;
mod tantivy;
mod threads;
mod tokenizers_general;

pub use ag_pipeline::DocuAgPipeline;
pub use agglutinative::DocuAgglutinative;
pub use bias::DocuBias;
pub use bm25::DocuBm25;
pub use bpe_unigram::DocuBpeUnigram;
pub use canonicalization::DocuCanonicalization;
pub use detr_layout::DocuDetrLayout;
pub use embeddings::DocuEmbeddings;
pub use entities_production::DocuEntitiesProduction;
pub use file_watcher::DocuFileWatcher;
pub use io_uring::DocuIoUring;
pub use knowledge_graphs::DocuKnowledgeGraphs;
pub use lora_export::DocuLoraExport;
pub use onnx::DocuOnnx;
pub use onnx_params::DocuOnnxParams;
pub use rig::DocuRig;
pub use rkyv::DocuRkyv;
pub use tantivy::DocuTantivy;
pub use threads::DocuThreads;
pub use tokenizers_general::DocuTokenizersGeneral;
// Documentation - Index page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuIndex() -> Element {
    rsx! {
        div { id: "top", class: "min-h-screen bg-gray-900 p-6",
            div { class: "w-full",

                a {
                    href: "/docu",
                    class: "text-primary hover:underline mb-4 inline-block",
                    "← Back to Documentation"
                }

                h1 { class: "text-3xl font-bold mb-6 text-white", "📇 Index" }

                div { class: "space-y-2",
                    Link {
                        to: Route::DocuAgPipeline {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "AG Pipeline"
                    }
                    Link {
                        to: Route::DocuAgglutinative {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Agglutinative Languages"
                    }
                    Link {
                        to: Route::DocuBias {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Bias"
                    }
                    Link {
                        to: Route::DocuBm25 {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "BM25"
                    }
                    Link {
                        to: Route::DocuBpeUnigram {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "BPE Unigram"
                    }
                    Link {
                        to: Route::DocuCanonicalization {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Canonicalization"
                    }
                    Link {
                        to: Route::DocuDetrLayout {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "DETR-style image-based layout model"
                    }
                    Link {
                        to: Route::DocuEmbeddings {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Embeddings"
                    }
                    Link {
                        to: Route::DocuEntitiesProduction {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Entities Production"
                    }
                    Link {
                        to: Route::DocuFileWatcher {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "File Watcher"
                    }
                    Link {
                        to: Route::DocuIoUring {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "io_uring"
                    }
                    Link {
                        to: Route::DocuKnowledgeGraphs {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Knowledge Graphs"
                    }
                    Link {
                        to: Route::DocuLoraExport {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "LoRA Export"
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
                        to: Route::DocuRig {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Rig (Agentic Framework)"
                    }
                    Link {
                        to: Route::DocuRkyv {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "rkyv"
                    }
                    Link {
                        to: Route::DocuTantivy {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Tantivy"
                    }
                    Link {
                        to: Route::DocuThreads {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Threads"
                    }
                    Link {
                        to: Route::DocuTokenizersGeneral {},
                        class: "text-primary hover:underline text-lg font-semibold block",
                        "Tokenizers General"
                    }
                }
            }
        }
    }
}
