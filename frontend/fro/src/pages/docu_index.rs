//! Documentation - Index page

use dioxus::prelude::*;

#[component]
pub fn DocuIndex() -> Element {
    let mut show_embeddings_modal = use_signal(|| false);
    
    rsx! {
        div {
            class: "min-h-screen bg-base-200 p-6",
            
            div {
                class: "max-w-4xl mx-auto",
                
                // Back link
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
                    class: "prose prose-invert max-w-none",
                    
                    // Embeddings link
                    p {
                        a {
                            href: "#",
                            class: "text-primary hover:underline text-lg font-semibold",
                            onclick: move |evt| {
                                evt.prevent_default();
                                show_embeddings_modal.set(true);
                            },
                            "embeddings"
                        }
                    }
                }
            }
        }
        
        // Embeddings Modal
        if show_embeddings_modal() {
            div {
                class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
                onclick: move |_| show_embeddings_modal.set(false),
                
                div {
                    class: "bg-base-100 rounded-lg p-6 max-w-lg mx-4 shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    
                    div {
                        class: "flex justify-between items-center mb-4",
                        h2 { class: "text-xl font-bold", "When are embeddings used?" }
                        button {
                            class: "btn btn-ghost btn-sm",
                            onclick: move |_| show_embeddings_modal.set(false),
                            "✕"
                        }
                    }
                    
                    ol {
                        class: "list-decimal list-inside space-y-3 text-sm",
                        
                        li {
                            strong { "Document indexing" }
                            " - When you upload/add documents to RAG"
                        }
                        
                        li {
                            strong { "Search queries" }
                            " - Every time you search, your query is embedded to find similar documents"
                        }
                        
                        li {
                            strong { "RAG retrieval" }
                            " - When the AI answers questions, it embeds the question to find relevant context"
                        }
                        
                        li {
                            strong { "Similarity matching" }
                            " - Comparing documents/chunks to find related content"
                        }
                        
                        li {
                            strong { "Agent memory storage" }
                            " - When agents store memories/context for later retrieval"
                        }
                        
                        li {
                            strong { "Agent memory retrieval" }
                            " - When agents recall relevant past interactions"
                        }
                        
                        li {
                            strong { "Agent tool selection" }
                            " - Finding the right tool based on task description"
                        }
                        
                        li {
                            strong { "Tool input matching" }
                            " - Matching user requests to tool parameters"
                        }
                        
                        li {
                            strong { "Tool output processing" }
                            " - Indexing and searching tool results"
                        }
                        
                        li {
                            strong { "Agent context building" }
                            " - Gathering relevant information before responding"
                        }
                    }
                    
                    button {
                        class: "btn btn-primary btn-sm mt-6 w-full",
                        onclick: move |_| show_embeddings_modal.set(false),
                        "Got it!"
                    }
                }
            }
        }
    }
}
