//! ONNX documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuOnnx() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "ONNX — Open Neural Network Exchange" }
                    span { class: "text-xs text-gray-400", "Train anywhere, run anywhere." }
                }

                div { class: "grid grid-cols-3 gap-2",

                    // Col 1
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "The Problem It Solves" }
                            p { class: "text-xs text-gray-300 mb-1",
                                "Before ONNX, a PyTorch model only ran in PyTorch. A TensorFlow model only ran in TF. Run PyTorch on mobile? Rewrite it. TF in Rust? Good luck. Optimize for Intel/NVIDIA/AMD? Different format for each."
                            }
                            p { class: "text-xs text-gray-300",
                                "After ONNX: export from PyTorch, TF, Keras, or JAX into one .onnx file. That file runs anywhere: ONNX Runtime, TensorRT, OpenVINO, CoreML, DirectML, WebNN."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "What's Inside an ONNX File" }
                            div { class: "text-xs text-gray-300 space-y-0.5",
                                p { span { class: "text-gray-400", "Graph: " } "computation structure — ops (MatMul, Add, ReLU, Softmax) and data flow between them." }
                                p { span { class: "text-gray-400", "Weights: " } "learned parameters — layer weights and biases stored as tensors." }
                                p { span { class: "text-gray-400", "Metadata: " } "opset version, producer, model version." }
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Summary" }
                            ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                                li { "Open Neural Network Exchange — Microsoft + Meta" }
                                li { "File extension: .onnx" }
                                li { "Main benefit: train anywhere, run anywhere" }
                                li { "Rust crate: ort" }
                                li { "Best for: inference, embeddings, cross-platform" }
                            }
                        }
                    }

                    // Col 2
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Execution Providers" }
                            p { class: "text-xs text-gray-300 mb-1", "ONNX Runtime runs the same model on different hardware through execution providers." }
                            div { class: "text-xs text-gray-300 space-y-0.5",
                                p { span { class: "text-gray-400", "CPU — " } "default, works everywhere" }
                                p { span { class: "text-gray-400", "CUDA — " } "NVIDIA GPUs" }
                                p { span { class: "text-gray-400", "TensorRT — " } "optimized NVIDIA" }
                                p { span { class: "text-gray-400", "ROCm — " } "AMD GPUs" }
                                p { span { class: "text-gray-400", "OpenVINO — " } "Intel hardware" }
                                p { span { class: "text-gray-400", "CoreML — " } "Apple hardware" }
                                p { span { class: "text-gray-400", "NNAPI — " } "Android" }
                            }
                            p { class: "text-xs text-gray-500 mt-1", "Same model file, different hardware acceleration." }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Quantization" }
                            p { class: "text-xs text-gray-300 mb-0.5", "Quantization shrinks models and speeds them up." }
                            p { class: "text-xs text-gray-300", "FP32 model: ~400 MB, slower. INT8 after quantization: ~100 MB, 2–4× faster on CPU." }
                        }
                    }

                    // Col 3
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "In Your AG Project" }
                        div { class: "text-xs text-gray-300 space-y-1",
                            p { span { class: "text-gray-400", "Embeddings: " } "embedding_model.onnx runs via the ort crate and produces vectors." }
                            p { span { class: "text-gray-400", "LLM: " } "GGUF model runs via Ollama and produces text output." }
                            div { class: "mt-2 space-y-1 text-gray-500",
                                p { "Why ONNX for embeddings? Single forward pass, fast inference, small models (50–400 MB), easy Rust integration via ort." }
                                p { "Why GGUF for LLM? Optimized for token-by-token generation, better quantization for large models, KV cache management." }
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
