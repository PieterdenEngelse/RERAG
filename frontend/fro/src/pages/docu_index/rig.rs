//! Documentation - Rig framework

use dioxus::prelude::*;

#[component]
pub fn DocuRig() -> Element {
    let mut show_tool_loop = use_signal(|| false);
    let mut show_preamble = use_signal(|| false);

    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "max-w-7xl mx-auto",
                a {
                    href: "/docu/index",
                    class: "text-primary hover:underline mb-4 inline-block",
                    "← Back to Index"
                }
                h1 { class: "text-xl font-bold mb-3 text-white", "Rig — Agentic Tool-Calling Framework" }

                // All boards in one grid — 4 per row
                div { class: "gap-2 mb-2", style: "display:grid;grid-template-columns:repeat(5,minmax(0,1fr));",

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "What is Rig?" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Rig ("
                            code { class: "text-primary font-mono", "rig-core" }
                            ") is a Rust library for building LLM-powered agents that can call tools. "
                            "It wraps any LLM backend and manages the "
                            span {
                                class: "text-primary underline cursor-pointer",
                                onclick: move |_| show_tool_loop.set(true),
                                "tool-calling loop"
                            }
                            " automatically."
                        }
                        ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                            li { "Typed tool trait: define inputs, outputs, errors in Rust" }
                            li { "Automatic JSON schema generation for the LLM" }
                            li { "Multi-turn loop until the LLM stops calling tools" }
                            li { "Provider-agnostic: Ollama, OpenAI, Anthropic, etc." }
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Usage in AG" }
                        div { class: "text-xs text-gray-300 space-y-0.5",
                            p {
                                span { class: "text-gray-400", "Mode: " }
                                "Activated when chat mode = "
                                code { class: "text-primary font-mono", "Agentic" }
                            }
                            p {
                                span { class: "text-gray-400", "Backend: " }
                                "Ollama via "
                                code { class: "text-primary font-mono", "rig::providers::ollama" }
                            }
                            p {
                                span { class: "text-gray-400", "Tools registered: " }
                                "4 (search, recall, store, graph)"
                            }
                            p {
                                span { class: "text-gray-400", "Entry point: " }
                                code { class: "text-primary font-mono", "api/agent_chat.rs" }
                            }
                            p {
                                span { class: "text-gray-400", "Tool impls: " }
                                code { class: "text-primary font-mono", "backend/src/rig_tools/" }
                            }
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "The Four Tools" }
                        div { class: "text-xs text-gray-300 space-y-0.5",
                            p {
                                code { class: "text-primary font-mono", "search_documents" }
                                " — hybrid Tantivy + vector search over uploaded docs"
                            }
                            p {
                                code { class: "text-primary font-mono", "recall_memory" }
                                " — reads past episodes from SQLite agent memory"
                            }
                            p {
                                code { class: "text-primary font-mono", "store_memory" }
                                " — persists a fact/preference to agent memory"
                            }
                            p {
                                code { class: "text-primary font-mono", "search_knowledge_graph" }
                                " — petgraph entity-relationship lookups"
                            }
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 min-w-0",
                        h3 { class: "text-sm font-bold text-white mb-1", "The Tool Trait" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Every tool implements "
                            code { class: "text-primary font-mono", "rig::tool::Tool" }
                            ". Rig calls "
                            code { class: "text-primary font-mono", "definition()" }
                            " to generate the JSON schema it sends to the LLM, "
                            "and "
                            code { class: "text-primary font-mono", "call()" }
                            " when the LLM decides to use it."
                        }
                        pre { class: "text-[10px] text-gray-300 font-mono leading-tight whitespace-pre-wrap break-all",
                            "impl Tool for TantivySearchTool {{\n  const NAME: &str = \"search_documents\";\n  type Args   = SearchArgs;   // deserialised from LLM JSON\n  type Output = SearchResult; // serialised back to LLM\n  type Error  = SearchError;\n\n  async fn definition(&self, _: String)\n    -> ToolDefinition {{ ... }}\n\n  async fn call(&self, args: SearchArgs)\n    -> Result<SearchResult, SearchError> {{ ... }}\n}}"
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 min-w-0",
                        h3 { class: "text-sm font-bold text-white mb-1", "Building the Agent" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "In "
                            code { class: "text-primary font-mono", "agent_chat.rs" }
                            " the Rig agent is assembled with a builder pattern. Each "
                            code { class: "text-primary font-mono", ".tool()" }
                            " call registers one tool. "
                            code { class: "text-primary font-mono", ".preamble()" }
                            " sets the system prompt. "
                            code { class: "text-primary font-mono", ".build()" }
                            " seals it."
                        }
                        pre { class: "text-[10px] text-gray-300 font-mono leading-tight whitespace-pre-wrap break-all",
                            "let agent = client\n  .agent(&model)\n  .preamble(SYSTEM_PROMPT)\n  .tool(TantivySearchTool::new(ret, top_k))\n  .tool(MemoryRecallTool::new())\n  .tool(MemoryStoreTool::new())\n  .tool(GraphSearchTool::new(ret))\n  .build();\n\nlet answer = agent.prompt(&query).await?;"
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Preamble (System Prompt)" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "The "
                            span {
                                class: "text-primary underline cursor-pointer",
                                onclick: move |_| show_preamble.set(true),
                                "preamble"
                            }
                            " is a system-level instruction given to the LLM before the user query. "
                            "It tells the model which tools exist, when to use them, and what style to adopt."
                        }
                        p { class: "text-xs text-gray-300",
                            "In AG the preamble instructs the LLM to use "
                            code { class: "text-primary font-mono", "search_documents" }
                            " when the question might be in uploaded docs, "
                            code { class: "text-primary font-mono", "recall_memory" }
                            " when it needs prior-conversation context, and to be concise. "
                            "Without a good preamble the LLM either calls tools when it shouldn't "
                            "or ignores them when it should."
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Observability" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Every Rig tool call is instrumented. "
                            code { class: "text-primary font-mono", "rig_stats::record_rig_tool_call()" }
                            " increments a Prometheus counter, and "
                            code { class: "text-primary font-mono", "record_tool_execution()" }
                            " logs tool name, query, latency, success, and confidence to the monitoring layer."
                        }
                        p { class: "text-xs text-gray-300",
                            "You can watch tool call counts, latencies, and token-budget warnings live on the "
                            span { class: "text-primary", "Monitor → Agentic" }
                            " page. Context budget is pre-checked: if the prompt already exceeds 80 % of "
                            code { class: "text-primary font-mono", "num_ctx" }
                            ", a warning is emitted before the agent is even called."
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Fallback Without Retriever" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "If the global "
                            code { class: "text-primary font-mono", "RETRIEVER" }
                            " singleton is unavailable (e.g. indexing hasn't run yet), "
                            "the agent is built without the document-search and graph tools. "
                            "Only "
                            code { class: "text-primary font-mono", "recall_memory" }
                            " and "
                            code { class: "text-primary font-mono", "store_memory" }
                            " are registered. The LLM still works — it just can't search uploaded documents."
                        }
                        p { class: "text-xs text-yellow-400",
                            "⚠ If search_documents always returns 0 results, check SKIP_INITIAL_INDEXING and the Tantivy index health on Monitor → RAG."
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Rig vs Other Modes" }
                        div { class: "text-xs text-gray-300 space-y-0.5",
                            p {
                                span { class: "text-gray-400", "Rag / RagStrict: " }
                                "ag retrieves chunks then hands them to the LLM. No loop — one-shot."
                            }
                            p {
                                span { class: "text-gray-400", "Hybrid: " }
                                "ag decides what to retrieve; LLM doesn't choose."
                            }
                            p {
                                span { class: "text-gray-400 font-semibold", "Agentic (Rig): " }
                                span { class: "text-green-400", "LLM decides which tools to call, when, and how many times." }
                            }
                        }
                        p { class: "text-xs text-gray-300 mt-1",
                            "The key difference: in non-agentic modes the retrieval strategy is hardcoded by ag. "
                            "In Agentic mode the LLM reasons about what it needs, calls the right tool, "
                            "reads the result, and may call another tool before producing a final answer."
                        }
                        p { class: "text-xs text-green-400 mt-1",
                            "✅ Use Agentic when queries require multi-step reasoning or when you don't know which tool the query needs."
                        }
                    }
                }

                a { href: "/docu/index", class: "btn btn-primary btn-sm mt-4 inline-block", "← Back to Index" }
            }
        }

        // Tool loop modal
        if show_tool_loop() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_tool_loop.set(false),
                div {
                    class: "relative bg-gray-800 border border-gray-700 rounded-lg shadow-xl w-full max-w-6xl mx-4 overflow-hidden",
                    onclick: move |e| e.stop_propagation(),
                    button {
                        class: "absolute top-2 right-2 text-gray-400 hover:text-white text-xl leading-none",
                        onclick: move |_| show_tool_loop.set(false),
                        "×"
                    }
                    div { class: "p-4 pb-2",
                        h2 { class: "text-base font-semibold text-gray-100 mb-3 pr-6", "The Tool-Calling Loop" }
                        div { class: "gap-2", style: "display:grid;grid-template-columns:repeat(5,minmax(0,1fr));",
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "1 — Send" }
                                p { class: "text-xs text-gray-300",
                                    "You send the user query together with a list of available tools described as JSON schemas. The LLM sees the tool names, their parameters, and descriptions."
                                }
                            }
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "2 — LLM Calls a Tool" }
                                p { class: "text-xs text-gray-300",
                                    "The LLM replies not with a final answer but with a tool call: the tool name and the arguments it chose, serialised as JSON."
                                }
                            }
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "3 — Execute & Return" }
                                p { class: "text-xs text-gray-300",
                                    "Your code (Rig) executes the tool and sends the result back to the LLM as a new message. The LLM now sees what the tool found."
                                }
                            }
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "4 — Repeat or Done" }
                                p { class: "text-xs text-gray-300",
                                    "The LLM may call another tool, or produce its final answer. Steps 2–4 repeat until it stops calling tools. Rig manages the entire loop — you only implement "
                                    code { class: "text-primary font-mono", "call()" }
                                    "."
                                }
                            }
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "Why it matters" }
                                p { class: "text-xs text-gray-300",
                                    "Each tool call adds context the LLM didn't have. A single question can trigger search, then memory recall, then a final synthesis — all decided by the model, not hardcoded by the app."
                                }
                            }
                        }
                    }
                    div { class: "border-t border-gray-700 p-3",
                        button {
                            class: "btn btn-primary btn-sm w-full",
                            onclick: move |_| show_tool_loop.set(false),
                            "Got it!"
                        }
                    }
                }
            }
        }

        // Preamble modal
        if show_preamble() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_preamble.set(false),
                div {
                    class: "relative bg-gray-800 border border-gray-700 rounded-lg shadow-xl w-full max-w-6xl mx-4 overflow-hidden",
                    onclick: move |e| e.stop_propagation(),
                    button {
                        class: "absolute top-2 right-2 text-gray-400 hover:text-white text-xl leading-none",
                        onclick: move |_| show_preamble.set(false),
                        "×"
                    }
                    div { class: "p-4 pb-2",
                        h2 { class: "text-base font-semibold text-gray-100 mb-3 pr-6", "Preamble (System Prompt)" }
                        div { class: "gap-2", style: "display:grid;grid-template-columns:repeat(5,minmax(0,1fr));",
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "What it is" }
                                p { class: "text-xs text-gray-300",
                                    "A message sent to the LLM before the user's query. It establishes the model's role, rules, and capabilities for the entire session."
                                }
                            }
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "Invisible but powerful" }
                                p { class: "text-xs text-gray-300",
                                    "The preamble is never shown to the end user, but shapes every response the model gives. It's the difference between a model that knows it has tools and one that doesn't."
                                }
                            }
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "In AG" }
                                p { class: "text-xs text-gray-300",
                                    "AG's preamble explicitly lists all four tools, tells the model when to use each one, and asks for concise accurate answers. Without it the LLM guesses tool purpose from JSON schema alone."
                                }
                            }
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "Good vs bad preamble" }
                                p { class: "text-xs text-gray-300",
                                    "Too vague: LLM ignores tools. Too prescriptive: LLM calls tools on every query whether useful or not. The sweet spot names each tool with a one-line hint on when to use it."
                                }
                            }
                            div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2",
                                h3 { class: "text-xs font-bold text-white mb-1", "Updating it" }
                                p { class: "text-xs text-gray-300",
                                    "Change the preamble string in "
                                    code { class: "text-primary font-mono", "agent_chat.rs" }
                                    " and restart. No schema changes needed — the LLM adapts its behaviour purely from the new instructions."
                                }
                            }
                        }
                    }
                    div { class: "border-t border-gray-700 p-3",
                        button {
                            class: "btn btn-primary btn-sm w-full",
                            onclick: move |_| show_preamble.set(false),
                            "Got it!"
                        }
                    }
                }
            }
        }
    }
}
