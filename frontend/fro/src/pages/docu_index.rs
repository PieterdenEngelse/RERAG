//! Documentation - Index page

use dioxus::prelude::*;

#[component]
pub fn DocuIndex() -> Element {
    let mut show_embeddings_modal = use_signal(|| false);
    let mut show_onnx_modal = use_signal(|| false);
    let mut show_onnx_params_modal = use_signal(|| false);
    
    rsx! {
        div {
            class: "min-h-screen bg-base-200 p-6",
            
            div {
                class: "max-w-4xl mx-auto",
                
                a {
                    href: "/docu",
                    class: "text-primary hover:underline mb-4 inline-block",
                    "← Back to Documentation"
                }
                
                h1 {
                    class: "text-3xl font-bold mb-6",
                    "📇 Index"
                }
                
                div {
                    class: "space-y-2",
                    
                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_embeddings_modal.set(true),
                        "Embeddings"
                    }
                    
                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_onnx_modal.set(true),
                        "ONNX"
                    }
                    
                    button {
                        class: "text-primary hover:underline text-lg font-semibold text-left block",
                        onclick: move |_| show_onnx_params_modal.set(true),
                        "ONNX parameters"
                    }
                }
            }
        }
        
        // Embeddings Modal
        if show_embeddings_modal() {
            div {
                class: "fixed top-0 left-0 right-0 bottom-0 bg-black bg-opacity-50 flex items-center justify-center",
                style: "z-index: 9999;",
                onclick: move |_| show_embeddings_modal.set(false),
                
                div {
                    class: "bg-gray-800 rounded-lg p-6 max-w-lg mx-4 shadow-xl",
                    onclick: move |e| e.stop_propagation(),
                    
                    h2 { class: "text-xl font-bold mb-4 text-white", "When are embeddings used?" }
                    
                    ul {
                        class: "space-y-2 text-sm text-gray-200",
                        li { "1. Document indexing - When you upload/add documents to RAG" }
                        li { "2. Search queries - Every time you search" }
                        li { "3. RAG retrieval - When the AI answers questions" }
                        li { "4. Similarity matching - Comparing documents/chunks" }
                        li { "5. Agent memory storage - When agents store memories" }
                        li { "6. Agent memory retrieval - When agents recall past interactions" }
                    }
                    
                    button {
                        class: "btn btn-primary btn-sm mt-4 w-full",
                        onclick: move |_| show_embeddings_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }
        
        // ONNX Modal
        if show_onnx_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",
                
                div {
                    class: "p-6 max-w-6xl mx-auto pb-20",
                    
                    // Close button
                    div {
                        class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white", "ONNX - Open Neural Network Exchange" }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_onnx_modal.set(false),
                            "×"
                        }
                    }
                    
                    p { class: "text-lg text-gray-200 mb-6", "ONNX is a universal file format for ML models. Train anywhere, run anywhere." }
                    
                    // The Problem It Solves
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "The Problem It Solves" }
                    p { class: "text-sm text-gray-300 mb-2", "Before ONNX, a PyTorch model only ran in PyTorch. A TensorFlow model only ran in TensorFlow. Want to run a PyTorch model on mobile? Rewrite it. Want to run a TensorFlow model in Rust? Good luck. Want to optimize for Intel, NVIDIA, or AMD? Different format for each." }
                    p { class: "text-sm text-gray-300", "After ONNX, you export from PyTorch, TensorFlow, Keras, or JAX into one .onnx file. That file runs anywhere: ONNX Runtime, TensorRT, OpenVINO, CoreML, DirectML, WebNN." }
                    
                    // What's Inside an ONNX File
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "What's Inside an ONNX File" }
                    p { class: "text-sm text-gray-300 mb-2", "An ONNX file contains three main parts." }
                    p { class: "text-sm text-gray-300 mb-1", "First, the graph. This defines the computation structure: what operations to perform (MatMul, Add, ReLU, Softmax) and how data flows between them. It also specifies input and output tensor shapes and types." }
                    p { class: "text-sm text-gray-300 mb-1", "Second, the weights. These are the learned parameters: layer weights and biases stored as tensors with their shapes and data types." }
                    p { class: "text-sm text-gray-300 mb-1", "Third, metadata. This includes the opset version (which operators are available), the producer (like \"pytorch 2.0\"), and the model version." }
                    p { class: "text-sm text-gray-300", "Optionally, it may include quantization information." }
                    
                    // ONNX Operators
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Operators" }
                    p { class: "text-sm text-gray-300 mb-2", "ONNX defines standard building blocks that all runtimes understand." }
                    p { class: "text-sm text-gray-300", "Math operators include Add, Sub, Mul, Div, MatMul, Gemm, Sqrt, Exp, and Log." }
                    p { class: "text-sm text-gray-300", "Activation operators include Relu, Sigmoid, Tanh, Softmax, Gelu, and Silu." }
                    p { class: "text-sm text-gray-300", "Tensor operators include Reshape, Transpose, Concat, Split, Slice, and Squeeze." }
                    p { class: "text-sm text-gray-300", "Reduction operators include ReduceSum, ReduceMean, and ReduceMax." }
                    p { class: "text-sm text-gray-300", "Normalization operators include BatchNorm, LayerNorm, and InstanceNorm." }
                    p { class: "text-sm text-gray-300", "Convolution operators include Conv, ConvTranspose, MaxPool, and AveragePool." }
                    p { class: "text-sm text-gray-300", "Quantization operators include QuantizeLinear and DequantizeLinear." }
                    p { class: "text-sm text-gray-400 mt-2", "The opset version determines which operators are available. Higher versions have more features." }
                    
                    // ONNX vs Other Formats
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX vs Other Formats" }
                    p { class: "text-sm text-gray-300", "ONNX was created by Microsoft and Meta. It's best for cross-platform inference and has the highest portability across platforms." }
                    p { class: "text-sm text-gray-300", "GGUF was created by llama.cpp. It's best for LLM text generation but only works with llama.cpp and its derivatives." }
                    p { class: "text-sm text-gray-300", "SafeTensors was created by HuggingFace. It's best for weight storage and has good portability." }
                    p { class: "text-sm text-gray-300", "TorchScript was created by PyTorch. It's best for PyTorch deployment but has limited portability outside PyTorch." }
                    p { class: "text-sm text-gray-300", "SavedModel was created by TensorFlow. It's best for TensorFlow deployment but has limited portability outside TensorFlow." }
                    p { class: "text-sm text-gray-300", "TensorRT was created by NVIDIA. It's best for NVIDIA GPU inference but only works on NVIDIA hardware." }
                    p { class: "text-sm text-gray-300", "CoreML was created by Apple. It's best for Apple devices but only works on Apple hardware." }
                    
                    // Creating ONNX Models
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Creating ONNX Models" }
                    p { class: "text-sm text-gray-300", "From PyTorch, you use torch.onnx.export. Pass your model, a dummy input tensor, the output filename, input names, output names, and optionally dynamic axes for variable batch sizes." }
                    p { class: "text-sm text-gray-300", "From TensorFlow, you use tf2onnx.convert.from_keras. Pass your loaded Keras model and the output path." }
                    p { class: "text-sm text-gray-300", "From HuggingFace, the easiest way is the optimum CLI. Install optimum with onnxruntime, then run optimum-cli export onnx with your model name and output directory." }
                    
                    // Running ONNX Models
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Running ONNX Models" }
                    p { class: "text-sm text-gray-300", "In Python, you create an InferenceSession with onnxruntime, passing the model path. Then call session.run with None for output names and a dictionary of inputs." }
                    p { class: "text-sm text-gray-300", "In Rust, you use the ort crate. Build a Session from the model file, then call session.run with your input tensors." }
                    p { class: "text-sm text-gray-300", "In C++, you create an Ort::Session with the environment, model path, and session options. Then call session.Run with run options, input names, input tensors, and output names." }
                    
                    // ONNX Runtime Execution Providers
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Runtime Execution Providers" }
                    p { class: "text-sm text-gray-300 mb-2", "ONNX Runtime can run the same model file on different hardware through execution providers." }
                    p { class: "text-sm text-gray-300", "CPU is the default and works everywhere." }
                    p { class: "text-sm text-gray-300", "CUDA runs on NVIDIA GPUs." }
                    p { class: "text-sm text-gray-300", "TensorRT runs optimized on NVIDIA GPUs." }
                    p { class: "text-sm text-gray-300", "ROCm runs on AMD GPUs." }
                    p { class: "text-sm text-gray-300", "OpenVINO runs optimized on Intel hardware." }
                    p { class: "text-sm text-gray-300", "DirectML runs on Windows GPUs." }
                    p { class: "text-sm text-gray-300", "CoreML runs on Apple hardware." }
                    p { class: "text-sm text-gray-300", "NNAPI runs on Android." }
                    p { class: "text-sm text-gray-300", "WebNN runs in browsers." }
                    p { class: "text-sm text-gray-400 mt-2", "Same model file, different hardware acceleration." }
                    
                    // ONNX Optimizations
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Optimizations" }
                    p { class: "text-sm text-gray-300 mb-2", "ONNX Runtime optimizes the graph before running." }
                    p { class: "text-sm text-gray-300", "It fuses operations. A sequence of MatMul, Add, and Relu becomes a single FusedMatMulAddRelu operation." }
                    p { class: "text-sm text-gray-300", "It folds constants. Conv followed by BatchNorm gets folded into modified weights." }
                    p { class: "text-sm text-gray-300", "It removes redundant operations like unnecessary Cast ops." }
                    p { class: "text-sm text-gray-300", "It combines multiple Transpose operations into one." }
                    p { class: "text-sm text-gray-400 mt-2", "You control optimization level when creating the session. Level 3 or ORT_ENABLE_ALL gives maximum optimization." }
                    
                    // Quantization
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Quantization" }
                    p { class: "text-sm text-gray-300 mb-2", "Quantization shrinks models and speeds them up." }
                    p { class: "text-sm text-gray-300", "An original FP32 model might be 400 MB and slower. After quantization to INT8, it becomes 100 MB and runs 2-4x faster." }
                    p { class: "text-sm text-gray-300", "You use onnxruntime.quantization.quantize_dynamic in Python. Pass the input model path, output model path, and weight type like QInt8." }
                    
                    // For Your AG Project
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "For Your AG Project" }
                    div { class: "bg-gray-700 rounded p-4 text-sm text-gray-200",
                        p { class: "mb-2", "The model is embedding_model.onnx. It runs through the ort crate and produces vectors." }
                        p { class: "mb-3", "You use GGUF for your LLM. The model is something like phi-3.gguf. It runs through Ollama or llama.cpp and produces text output." }
                        p { class: "text-xs text-gray-400 mt-2", "Why ONNX for embeddings? It's a single forward pass with no autoregressive loop. It has fast inference. The models are small at 50-400 MB. The ort crate makes Rust integration easy." }
                        p { class: "text-xs text-gray-400", "Why GGUF for the LLM? It's optimized for token-by-token generation. It has better quantization for large models. It handles KV cache management. It has the whole llama.cpp ecosystem." }
                    }
                    
                    // Summary
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Summary" }
                    p { class: "text-sm text-gray-300", "ONNX stands for Open Neural Network Exchange." }
                    p { class: "text-sm text-gray-300", "Microsoft and Meta created it." }
                    p { class: "text-sm text-gray-300", "The file extension is .onnx." }
                    p { class: "text-sm text-gray-300", "The main benefit is train anywhere, run anywhere." }
                    p { class: "text-sm text-gray-300", "The Rust crate is ort." }
                    p { class: "text-sm text-gray-300", "It's best for inference, embeddings, and cross-platform deployment." }
                    
                    button {
                        class: "btn btn-primary btn-sm mt-6 w-full",
                        onclick: move |_| show_onnx_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }
        
        // ONNX Parameters Modal
        if show_onnx_params_modal() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",
                
                div {
                    class: "p-6 max-w-6xl mx-auto pb-20",
                    
                    div {
                        class: "flex justify-between items-start mb-4",
                        h2 { class: "text-2xl font-bold text-white", "ONNX Parameters" }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_onnx_params_modal.set(false),
                            "×"
                        }
                    }
                    
                    p { class: "text-lg text-gray-200 mb-6", "Environment variables and configuration for ONNX in your AG project." }
                    
                    // Required Environment Variables
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Required Environment Variables" }
                    div { class: "bg-gray-700 rounded p-4 font-mono text-sm text-gray-200 space-y-2",
                        p { "export ORT_DYLIB_PATH=/usr/local/lib/libonnxruntime.so" }
                        p { "export ONNX_MODEL_PATH=models/embedding_model.onnx" }
                    }
                    
                    // ORT_DYLIB_PATH
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ORT_DYLIB_PATH" }
                    p { class: "text-sm text-gray-300", "Path to the ONNX Runtime shared library. Required for the ort crate to load the runtime." }
                    p { class: "text-sm text-gray-300 mt-2", "Common locations:" }
                    p { class: "text-sm text-gray-400 font-mono", "Linux: /usr/local/lib/libonnxruntime.so" }
                    p { class: "text-sm text-gray-400 font-mono", "macOS: /usr/local/lib/libonnxruntime.dylib" }
                    p { class: "text-sm text-gray-400 font-mono", "Windows: C:\\onnxruntime\\lib\\onnxruntime.dll" }
                    
                    // ONNX_MODEL_PATH
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX_MODEL_PATH" }
                    p { class: "text-sm text-gray-300", "Path to your ONNX embedding model file. Defaults to models/embedding_model.onnx if not set." }
                    p { class: "text-sm text-gray-300 mt-2", "Your current model: models/embedding_model.onnx" }
                    
                    // Runtime Library
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "ONNX Runtime Library" }
                    p { class: "text-sm text-gray-300", "Location: /usr/local/lib/" }
                    p { class: "text-sm text-gray-300", "Version: 1.20.1" }
                    p { class: "text-sm text-gray-300", "Files:" }
                    p { class: "text-sm text-gray-400 font-mono ml-4", "libonnxruntime.so.1.20.1" }
                    p { class: "text-sm text-gray-400 font-mono ml-4", "libonnxruntime.so.1 → libonnxruntime.so.1.20.1" }
                    p { class: "text-sm text-gray-400 font-mono ml-4", "libonnxruntime.so → libonnxruntime.so.1" }
                    p { class: "text-sm text-gray-400 font-mono ml-4", "libonnxruntime_providers_shared.so" }
                    
                    // Rust Crate
                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Rust Integration" }
                    p { class: "text-sm text-gray-300", "Crate: ort" }
                    p { class: "text-sm text-gray-300", "Feature flag: --features onnx" }
                    p { class: "text-sm text-gray-300 mt-2", "Build command:" }
                    p { class: "text-sm text-gray-400 font-mono", "cargo run --features onnx" }
                    
                    button {
                        class: "btn btn-primary btn-sm mt-6 w-full",
                        onclick: move |_| show_onnx_params_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }
    }
}
