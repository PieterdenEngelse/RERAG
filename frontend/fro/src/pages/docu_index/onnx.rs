//! ONNX documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuOnnx() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4",
                        "ONNX - Open Neural Network Exchange"
                    }
                    p { class: "text-lg text-gray-200 mb-6",
                        "ONNX is a universal file format for ML models. Train anywhere, run anywhere."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "The Problem It Solves" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Before ONNX, a PyTorch model only ran in PyTorch. A TensorFlow model only ran in TensorFlow. Want to run a PyTorch model on mobile? Rewrite it. Want to run a TensorFlow model in Rust? Good luck. Want to optimize for Intel, NVIDIA, or AMD? Different format for each."
                    }
                    p { class: "text-sm text-gray-300",
                        "After ONNX, you export from PyTorch, TensorFlow, Keras, or JAX into one .onnx file. That file runs anywhere: ONNX Runtime, TensorRT, OpenVINO, CoreML, DirectML, WebNN."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "What's Inside an ONNX File" }
                    p { class: "text-sm text-gray-300 mb-1",
                        "First, the graph. This defines the computation structure: what operations to perform (MatMul, Add, ReLU, Softmax) and how data flows between them."
                    }
                    p { class: "text-sm text-gray-300 mb-1",
                        "Second, the weights. These are the learned parameters: layer weights and biases stored as tensors."
                    }
                    p { class: "text-sm text-gray-300 mb-1",
                        "Third, metadata. This includes the opset version, the producer, and the model version."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Runtime Execution Providers" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "ONNX Runtime can run the same model file on different hardware through execution providers."
                    }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "CPU \u{2014} default, works everywhere" }
                        li { "CUDA \u{2014} NVIDIA GPUs" }
                        li { "TensorRT \u{2014} optimized NVIDIA" }
                        li { "ROCm \u{2014} AMD GPUs" }
                        li { "OpenVINO \u{2014} Intel hardware" }
                        li { "CoreML \u{2014} Apple hardware" }
                        li { "NNAPI \u{2014} Android" }
                    }
                    p { class: "text-sm text-gray-400 mt-2", "Same model file, different hardware acceleration." }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Quantization" }
                    p { class: "text-sm text-gray-300 mb-2",
                        "Quantization shrinks models and speeds them up."
                    }
                    p { class: "text-sm text-gray-300",
                        "An original FP32 model might be 400 MB and slower. After quantization to INT8, it becomes 100 MB and runs 2-4x faster."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "For Your AG Project" }
                    div { class: "bg-gray-700 rounded p-4 text-sm text-gray-200",
                        p { class: "mb-2",
                            "The model is embedding_model.onnx. It runs through the ort crate and produces vectors."
                        }
                        p { class: "mb-2",
                            "You use GGUF for your LLM. The model runs through Ollama and produces text output."
                        }
                        p { class: "text-xs text-gray-400 mt-2",
                            "Why ONNX for embeddings? Single forward pass, fast inference, small models (50-400 MB), easy Rust integration via ort."
                        }
                        p { class: "text-xs text-gray-400",
                            "Why GGUF for the LLM? Optimized for token-by-token generation, better quantization for large models, KV cache management."
                        }
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Summary" }
                    ul { class: "space-y-1 text-sm text-gray-300 ml-4 list-disc",
                        li { "Stands for Open Neural Network Exchange" }
                        li { "Created by Microsoft and Meta" }
                        li { "File extension: .onnx" }
                        li { "Main benefit: train anywhere, run anywhere" }
                        li { "Rust crate: ort" }
                        li { "Best for: inference, embeddings, cross-platform deployment" }
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
