//! ONNX Parameters documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuOnnxParams() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "ONNX Parameters" }
                    span { class: "text-xs text-gray-400", "Environment variables and configuration for ONNX in AG." }
                }

                div { class: "grid grid-cols-2 gap-2",

                    // Left column
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Required Environment Variables" }
                            div { class: "bg-gray-700 rounded p-2 font-mono text-xs text-gray-200 space-y-0.5",
                                p { "export ORT_DYLIB_PATH=/usr/local/lib/libonnxruntime.so" }
                                p { "export ONNX_MODEL_PATH=models/embedding_model.onnx" }
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "ORT_DYLIB_PATH" }
                            p { class: "text-xs text-gray-300 mb-1", "Path to the ONNX Runtime shared library. Required for the ort crate to load the runtime." }
                            div { class: "text-xs text-gray-400 font-mono space-y-0.5",
                                p { "Linux:   /usr/local/lib/libonnxruntime.so" }
                                p { "macOS:   /usr/local/lib/libonnxruntime.dylib" }
                                p { "Windows: C:\\onnxruntime\\lib\\onnxruntime.dll" }
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "ONNX_MODEL_PATH" }
                            p { class: "text-xs text-gray-300 mb-0.5", "Path to your ONNX embedding model file. Defaults to models/embedding_model.onnx if not set." }
                            p { class: "text-xs text-gray-400", "Current model: models/embedding_model.onnx" }
                        }
                    }

                    // Right column
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "ONNX Runtime Library" }
                            div { class: "text-xs text-gray-300 space-y-0.5",
                                p { span { class: "text-gray-400", "Location: " } "/usr/local/lib/" }
                                p { span { class: "text-gray-400", "Version: " } "1.20.1" }
                            }
                            div { class: "text-xs text-gray-400 font-mono mt-1 space-y-0.5",
                                p { "libonnxruntime.so.1.20.1" }
                                p { "libonnxruntime.so.1 → libonnxruntime.so.1.20.1" }
                                p { "libonnxruntime.so → libonnxruntime.so.1" }
                                p { "libonnxruntime_providers_shared.so" }
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Rust Integration" }
                            div { class: "text-xs text-gray-300 space-y-0.5",
                                p { span { class: "text-gray-400", "Crate: " } "ort" }
                                p { span { class: "text-gray-400", "Feature flag: " } "--features onnx" }
                                p { span { class: "text-gray-400", "Build command: " } code { class: "text-gray-400 font-mono", "cargo run --features onnx" } }
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
