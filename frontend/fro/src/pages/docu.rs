//! Documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn Docu() -> Element {
    rsx! {
        div {
            class: "min-h-screen bg-base-200 p-6",
            
            div {
                class: "max-w-4xl mx-auto",
                
                h1 {
                    class: "text-3xl font-bold mb-6",
                    "📚 Documentation"
                }
                
                // Quick links
                div {
                    class: "grid grid-cols-1 md:grid-cols-2 gap-4 mb-8",
                    
                    DocCard {
                        title: "Getting Started",
                        description: "Learn the basics of using the RAG system",
                        icon: "🚀"
                    }
                    
                    DocCard {
                        title: "Uploading Documents",
                        description: "How to add documents for AI to search",
                        icon: "📄"
                    }
                    
                    DocCard {
                        title: "Search & RAG",
                        description: "Understanding retrieval-augmented generation",
                        icon: "🔍"
                    }
                    
                    DocCard {
                        title: "Configuration",
                        description: "Customize settings and parameters",
                        icon: "⚙️"
                    }
                    
                    DocCard {
                        title: "ONNX Speed",
                        description: "Fast embeddings with ONNX Runtime",
                        icon: "⚡"
                    }
                    
                    DocCard {
                        title: "Monitoring",
                        description: "Track performance and usage",
                        icon: "📊"
                    }
                    
                    Link {
                        to: Route::DocuIndex {},
                        DocCard {
                            title: "Index",
                            description: "Index",
                            icon: "📇"
                        }
                    }
                }
                
                // Main content
                div {
                    class: "prose prose-invert max-w-none",
                    
                    h2 { "Overview" }
                    p {
                        "This is a Retrieval-Augmented Generation (RAG) system that allows you to:"
                    }
                    ul {
                        li { "Upload documents (PDF, text, markdown, code files)" }
                        li { "Search through your documents semantically" }
                        li { "Get AI-powered answers grounded in your documents" }
                        li { "Train custom models on your data" }
                    }
                    
                    h2 { "How It Works" }
                    p {
                        "When you ask a question:"
                    }
                    ol {
                        li { "Your question is converted to an embedding (using ONNX for speed)" }
                        li { "Similar document chunks are found using vector search" }
                        li { "The relevant context is sent to the LLM along with your question" }
                        li { "The LLM generates an answer based on your documents" }
                    }
                    
                    h2 { "Supported File Types" }
                    div {
                        class: "grid grid-cols-2 md:grid-cols-4 gap-2 not-prose",
                        FileType { ext: "PDF", icon: "📕" }
                        FileType { ext: "TXT", icon: "📝" }
                        FileType { ext: "MD", icon: "📋" }
                        FileType { ext: "HTML", icon: "🌐" }
                        FileType { ext: "JSON", icon: "📦" }
                        FileType { ext: "RS", icon: "🦀" }
                        FileType { ext: "PY", icon: "🐍" }
                        FileType { ext: "JS/TS", icon: "📜" }
                    }
                }
            }
        }
    }
}

#[component]
fn DocCard(title: String, description: String, icon: String) -> Element {
    rsx! {
        div {
            class: "card bg-base-100 shadow-lg hover:shadow-xl transition-shadow cursor-pointer",
            div {
                class: "card-body",
                div {
                    class: "flex items-center gap-3",
                    span { class: "text-2xl", "{icon}" }
                    div {
                        h3 { class: "card-title text-lg", "{title}" }
                        p { class: "text-sm text-base-content/70", "{description}" }
                    }
                }
            }
        }
    }
}

#[component]
fn FileType(ext: String, icon: String) -> Element {
    rsx! {
        div {
            class: "flex items-center gap-2 bg-base-100 rounded-lg p-2",
            span { "{icon}" }
            span { class: "text-sm font-mono", "{ext}" }
        }
    }
}
