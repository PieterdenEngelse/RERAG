//! ONNX Parameters documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuOnnxParams() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4", "ONNX Parameters" }
                    p { class: "text-lg text-gray-200 mb-6",
                        "Environment variables and configuration for ONNX in your AG project."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "Required Environment Variables"
                    }
                    div { class: "bg-gray-700 rounded p-4 font-mono text-sm text-gray-200 space-y-2",
                        p { "export ORT_DYLIB_PATH=/usr/local/lib/libonnxruntime.so" }
                        p { "export ONNX_MODEL_PATH=models/embedding_model.onnx" }
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ORT_DYLIB_PATH" }
                    p { class: "text-sm text-gray-300",
                        "Path to the ONNX Runtime shared library. Required for the ort crate to load the runtime."
                    }
                    p { class: "text-sm text-gray-300 mt-2", "Common locations:" }
                    p { class: "text-sm text-gray-400 font-mono",
                        "Linux: /usr/local/lib/libonnxruntime.so"
                    }
                    p { class: "text-sm text-gray-400 font-mono",
                        "macOS: /usr/local/lib/libonnxruntime.dylib"
                    }
                    p { class: "text-sm text-gray-400 font-mono",
                        "Windows: C:\\onnxruntime\\lib\\onnxruntime.dll"
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX_MODEL_PATH" }
                    p { class: "text-sm text-gray-300",
                        "Path to your ONNX embedding model file. Defaults to models/embedding_model.onnx if not set."
                    }
                    p { class: "text-sm text-gray-300 mt-2",
                        "Your current model: models/embedding_model.onnx"
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Runtime Library" }
                    p { class: "text-sm text-gray-300", "Location: /usr/local/lib/" }
                    p { class: "text-sm text-gray-300", "Version: 1.20.1" }
                    p { class: "text-sm text-gray-300", "Files:" }
                    p { class: "text-sm text-gray-400 font-mono ml-4", "libonnxruntime.so.1.20.1" }
                    p { class: "text-sm text-gray-400 font-mono ml-4",
                        "libonnxruntime.so.1 \u{2192} libonnxruntime.so.1.20.1"
                    }
                    p { class: "text-sm text-gray-400 font-mono ml-4",
                        "libonnxruntime.so \u{2192} libonnxruntime.so.1"
                    }
                    p { class: "text-sm text-gray-400 font-mono ml-4",
                        "libonnxruntime_providers_shared.so"
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Rust Integration" }
                    p { class: "text-sm text-gray-300", "Crate: ort" }
                    p { class: "text-sm text-gray-300", "Feature flag: --features onnx" }
                    p { class: "text-sm text-gray-300 mt-2", "Build command:" }
                    p { class: "text-sm text-gray-400 font-mono", "cargo run --features onnx" }

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
