//! ONNX documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuOnnx() -> Element {
    let mut show_ort = use_signal(|| false);
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                if show_ort() {
                    {ort_vs_candle_modal(show_ort)}
                }

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "ONNX — Open Neural Network Exchange" }
                    span { class: "text-xs text-gray-400", "Train anywhere, run anywhere." }
                }

                // First tile — conceptual primer for the rest of the page.
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 mb-2",
                    h3 { class: "text-sm font-bold text-white mb-1",
                        "ONNX vs ort settings — three layers, only one is configurable"
                    }
                    p { class: "text-xs text-gray-300 mb-2",
                        "The names blur because ag's "
                        span { class: "font-mono text-gray-100", "OnnxConfig" }
                        " struct is misleadingly named — almost every field is actually an "
                        em { "ort runtime" }
                        " setting, not an ONNX-file setting. Three distinct layers:"
                    }
                    div { class: "grid grid-cols-3 gap-2 text-xs text-gray-300",
                        div { class: "border border-gray-700 rounded p-2",
                            div { class: "text-gray-100 font-semibold mb-1",
                                "1. ONNX — the file format"
                            }
                            p { class: "mb-1",
                                "A serialized graph: operators (MatMul, Add, Softmax), connections, weights. Like PNG or PDF — describes a thing, doesn't execute it."
                            }
                            p { class: "text-gray-400",
                                "What it has: opset version, IR version, producer metadata, weights, overridable initializers (rarely used)."
                            }
                            p { class: "mt-1 text-gray-300",
                                "No threading, no optimization, no hardware selection. The file says "
                                em { "what" }
                                " to compute; it says nothing about "
                                em { "how" }
                                "."
                            }
                        }
                        div { class: "border border-gray-700 rounded p-2",
                            div { class: "text-gray-100 font-semibold mb-1",
                                "2. ONNX Runtime — Microsoft's engine"
                            }
                            p { class: "mb-1",
                                "C++ library that loads the .onnx file and runs it. "
                                strong { "All the knobs live here." }
                                " Three scopes:"
                            }
                            ul { class: "list-disc ml-3 space-y-0.5",
                                li {
                                    span { class: "text-gray-400", "Env options — " }
                                    "process-wide, set once at startup."
                                }
                                li {
                                    span { class: "text-gray-400", "SessionOptions — " }
                                    "per-model. Threading, optimization level, memory, execution providers, profiling. ~25 knobs."
                                }
                                li {
                                    span { class: "text-gray-400", "RunOptions — " }
                                    "per call. Tag, log severity, cancel signal."
                                }
                            }
                        }
                        div { class: "border border-gray-700 rounded p-2",
                            div { class: "text-gray-100 font-semibold mb-1",
                                "3. ort — Rust crate"
                            }
                            p { class: "mb-1",
                                "Thin binding over ONNX Runtime. No extra settings — every ort setter maps 1:1 to an ONNX Runtime C API call. Adds:"
                            }
                            ul { class: "list-disc ml-3 space-y-0.5",
                                li { "Type safety (typed enums vs int constants)" }
                                li { "Lifetime safety (Session borrow-checks tensor inputs)" }
                                li { "Cargo features for execution providers" }
                            }
                            p { class: "mt-1 text-gray-300",
                                "\"ort settings\" = ONNX Runtime settings, with Rust wrappers."
                            }
                        }
                    }
                    div { class: "mt-2 text-xs text-gray-300",
                        div { class: "text-gray-100 font-semibold mb-1", "Which layer answers which question" }
                        div { class: "grid grid-cols-2 gap-x-4 gap-y-0.5",
                            div { "What does this model compute?" }
                            div { class: "text-gray-400", "ONNX file" }
                            div { "How fast / on what hardware does it run?" }
                            div { class: "text-gray-400", "ONNX Runtime (= ort settings)" }
                            div { "What do I call from Rust to configure that?" }
                            div { class: "text-gray-400", "ort crate (type-safe wrappers)" }
                            div { "How does ag's OnnxConfig pick defaults?" }
                            div { class: "text-gray-400", "Application code in onnx_embedder.rs" }
                        }
                    }
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
                                li {
                                    "Rust crate: "
                                    span {
                                        class: "text-blue-400 hover:text-blue-300 underline cursor-pointer",
                                        onclick: move |_| show_ort.set(true),
                                        title: "Why ort over Candle",
                                        "ort"
                                    }
                                }
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
                            p { class: "text-xs text-gray-300 mt-1", "Same model file, different hardware acceleration." }
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
                            div { class: "mt-2 space-y-1 text-gray-300",
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

/// Why ag uses the `ort` crate (ONNX Runtime) instead of Candle for inference.
fn ort_vs_candle_modal(mut show: Signal<bool>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| show.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-3xl max-h-[85vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-lg font-semibold text-gray-100",
                        "ort vs Candle — why ag picks ort"
                    }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| show.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-3 leading-relaxed",
                    p {
                        span { class: "font-mono text-gray-100", "ort" }
                        " is the Rust binding for "
                        strong { "ONNX Runtime" }
                        ", Microsoft's production inference engine. "
                        strong { "Candle" }
                        " is Hugging Face's pure-Rust ML framework. For inference — which is all ag does — ort wins on basically every axis that matters here."
                    }

                    h3 { class: "text-gray-100 font-semibold pt-1", "Model availability" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "ort runs "
                            em { "any" }
                            " ONNX file. Export from PyTorch / TF / JAX → load → done."
                        }
                        li {
                            "Candle requires the architecture to be implemented in Rust (in "
                            span { class: "font-mono text-gray-100", "candle-transformers" }
                            " or hand-written). DETR, LayoutXLM, and many less-common models simply aren't there, or are partial."
                        }
                    }

                    h3 { class: "text-gray-100 font-semibold pt-1", "Performance" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "ort is Microsoft's production engine — graph-level optimizations (constant folding, op fusion, layout transforms), well-tuned CPU kernels (MLAS, oneDNN, optional MKL), mature memory planning."
                        }
                        li {
                            "Candle's CPU path is younger and generally slower for the same model — sometimes by a lot, especially on non-LLM workloads like vision transformers."
                        }
                    }

                    h3 { class: "text-gray-100 font-semibold pt-1", "Hardware backends (Execution Providers)" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "ort: CPU, CUDA, TensorRT, DirectML, ROCm, CoreML, OpenVINO, QNN, WebGPU — swap with a config flag, no model code changes."
                        }
                        li { "Candle: CPU, CUDA, Metal. Smaller surface, less mature on each." }
                    }

                    h3 { class: "text-gray-100 font-semibold pt-1", "Quantization" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li { "ort has first-class INT8/INT4 support and a mature quantization toolkit. A quantized ONNX just loads." }
                        li { "Candle's quantization is mostly GGUF/GGML-flavoured for LLMs; not a general path." }
                    }

                    h3 { class: "text-gray-100 font-semibold pt-1", "\"Load and run\" ergonomics" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li {
                            "ort: download a model file, point a "
                            span { class: "font-mono text-gray-100", "Session" }
                            " at it. No code needed to describe the architecture."
                        }
                        li {
                            "Candle: you need the architecture in Rust "
                            em { "and" }
                            " the right weights file format. Adding a new model often means writing the network."
                        }
                    }

                    h3 { class: "text-gray-100 font-semibold pt-1", "Maturity" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li { "ort: 2018, battle-tested in Office, Bing, Azure; ships inside Windows." }
                        li { "Candle: late 2023. Moving fast, but breaking changes and gaps still happen." }
                    }

                    h3 { class: "text-gray-100 font-semibold pt-1", "Where Candle is the better pick (not ag's case)" }
                    ul { class: "list-disc pl-5 space-y-1",
                        li { "Pure-Rust toolchain — no C++ DLL/.so to ship. Useful for WASM, embedded, single-binary distribution." }
                        li { "Training, not just inference." }
                        li {
                            "LLM inference with quantized GGUF weights — "
                            span { class: "font-mono text-gray-100", "candle-transformers" }
                            " is competitive there."
                        }
                        li { "Writing novel architectures in Rust rather than loading pretrained ones." }
                    }

                    p { class: "pt-1",
                        "For ag's PDF layout-detection use case — pretrained DETR ONNX from Hugging Face, CPU-only box, just need fast inference — ort is unambiguously the right choice."
                    }
                }
                button {
                    class: "btn btn-sm w-full mt-4",
                    style: "background-color:#7C2A02;",
                    onclick: move |_| show.set(false),
                    "Got it"
                }
            }
        }
    }
}
