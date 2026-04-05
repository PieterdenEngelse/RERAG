#!/usr/bin/env python3
"""Phase A: Add Agentic mode stub — button + enum + placeholder response."""
import sys, os

HOME = os.path.expanduser("~")

def patch(filepath, old, new, description, count=1):
    with open(filepath, "r") as f:
        content = f.read()
    occurrences = content.count(old)
    if occurrences == 0:
        print(f"FATAL: pattern not found in {filepath}: {description}")
        print(f"  looked for: {repr(old[:120])}")
        sys.exit(1)
    if count > 0 and occurrences != count:
        print(f"WARNING: expected {count} occurrences, found {occurrences} in {filepath}: {description}")
    content = content.replace(old, new)
    with open(filepath, "w") as f:
        f.write(content)
    print(f"  OK: {description} ({filepath})")

# ============================================================================
# 1. api/mod.rs — Add Agentic to ChatMode enum
# ============================================================================
print("\n[1/7] api/mod.rs: ChatMode enum")
patch(
    f"{HOME}/ag/backend/src/api/mod.rs",
    '    #[serde(alias = "rag_strict")]\n    RagStrict,\n}',
    '    #[serde(alias = "rag_strict")]\n    RagStrict,\n    /// Agentic mode: LLM-driven tool-use loop (Rig integration)\n    Agentic,\n}',
    "Add Agentic variant to ChatMode",
)

# ============================================================================
# 2. agent.rs — Add Agentic to AgentMode enum
# ============================================================================
print("\n[2/7] agent.rs: AgentMode enum")
patch(
    f"{HOME}/ag/backend/src/agent.rs",
    '    /// Strict grounded RAG: LLM answers only from retrieved context.\n    /// If no chunks found, says "I don\'t know" (no LLM fallback).\n    RagStrict,\n}',
    '    /// Strict grounded RAG: LLM answers only from retrieved context.\n    /// If no chunks found, says "I don\'t know" (no LLM fallback).\n    RagStrict,\n    /// Agentic mode: LLM decides which tools to call in a loop (Rig)\n    Agentic,\n}',
    "Add Agentic variant to AgentMode",
)

# ============================================================================
# 3. agent.rs — Handle Agentic in run_with_mode (early return stub)
# ============================================================================
print("\n[3/7] agent.rs: Agentic stub in run_with_mode")
patch(
    f"{HOME}/ag/backend/src/agent.rs",
    '        // Handle LLM-only mode\n        if matches!(mode, AgentMode::Llm) {',
    '        // Handle Agentic mode (Rig integration \u2014 stub)\n        if matches!(mode, AgentMode::Agentic) {\n            steps.push(AgentStep {\n                kind: "mode".into(),\n                message: "Agentic mode (Rig tool-loop \u2014 integration pending)".into(),\n            });\n            let answer = "Agentic mode received your query. Rig integration is pending \u2014 this mode will use an LLM-driven tool-calling loop to dynamically search documents, recall memory, and query the knowledge graph.".to_string();\n            self.store_memory(query, &answer);\n            self.store_episode(query, &answer, 0, true);\n            return AgentResponse {\n                answer,\n                steps,\n                used_chunks: Vec::new(),\n            };\n        }\n\n        // Handle LLM-only mode\n        if matches!(mode, AgentMode::Llm) {',
    "Add Agentic early return stub",
)

# ============================================================================
# 4. agent_chat.rs — Add ChatMode::Agentic => AgentMode::Agentic (all 3 match arms)
# ============================================================================
print("\n[4/7] agent_chat.rs: ChatMode->AgentMode mapping (x3)")
patch(
    f"{HOME}/ag/backend/src/api/agent_chat.rs",
    "ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,",
    "ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,\n        ChatMode::Agentic => crate::agent::AgentMode::Agentic,",
    "Add Agentic to ChatMode->AgentMode mapping",
    count=3,
)

# ============================================================================
# 5. agent_chat.rs — Add Agentic early return in streaming handler
# ============================================================================
print("\n[5/7] agent_chat.rs: Agentic stub in stream handler")
patch(
    f"{HOME}/ag/backend/src/api/agent_chat.rs",
    '    // For RAG-only mode, use non-streaming (document search doesn\'t benefit from streaming)\n    if matches!(\n        agent_mode,\n        crate::agent::AgentMode::Rag | crate::agent::AgentMode::RagStrict\n    ) {',
    '    // Agentic mode stub: return placeholder (non-streaming for now)\n    if matches!(agent_mode, crate::agent::AgentMode::Agentic) {\n        let placeholder = "Agentic mode received your query. Rig integration is pending.";\n        let json_response = serde_json::json!({\n            "response": placeholder,\n            "model": model_name,\n            "mode": "agentic",\n            "done": true\n        });\n        return Ok(HttpResponse::Ok()\n            .content_type("text/event-stream")\n            .body(format!("data: {}\\n\\n", json_response)));\n    }\n    // For RAG-only mode, use non-streaming (document search doesn\'t benefit from streaming)\n    if matches!(\n        agent_mode,\n        crate::agent::AgentMode::Rag | crate::agent::AgentMode::RagStrict\n    ) {',
    "Add Agentic early return in stream handler",
)

# ============================================================================
# 6. agent_chat.rs — Add Agentic to LlmConfig match arm
# ============================================================================
print("\n[6/7] agent_chat.rs: LlmConfig match arm")
patch(
    f"{HOME}/ag/backend/src/api/agent_chat.rs",
    "        crate::agent::AgentMode::Hybrid | crate::agent::AgentMode::Auto => LlmConfig::combined(),\n    };",
    "        crate::agent::AgentMode::Hybrid | crate::agent::AgentMode::Auto => LlmConfig::combined(),\n        crate::agent::AgentMode::Agentic => LlmConfig::combined(),\n    };",
    "Add Agentic to LlmConfig match",
)

# ============================================================================
# 7. Frontend: home_settings_boards.rs — Add Agentic button + info modal
# ============================================================================
print("\n[7/7] home_settings_boards.rs: Agentic button + info modal")

HSB = f"{HOME}/ag/frontend/fro/src/components/home_settings_boards.rs"

# 7a. Add show_agentic_info signal
patch(
    HSB,
    'let mut show_llm_info = use_signal(|| false);',
    'let mut show_llm_info = use_signal(|| false);\n    let mut show_agentic_info = use_signal(|| false);',
    "Add show_agentic_info signal",
)

# 7b. Add Agentic button before Tune button
patch(
    HSB,
    '                                    // Tune button',
    '                                    // Agentic mode button with info\n                                    div {\n                                        class: "flex items-center gap-1",\n                                        button {\n                                            class: "btn btn-sm rounded-lg px-3",\n                                            style: if chat_mode() == "agentic" {\n                                                "background-color:#7C2A02; border-color:#7C2A02; color:white; box-shadow:none;"\n                                            } else {\n                                                "background-color:transparent; border: 1px solid rgba(255,255,255,0.3); color:white; box-shadow:none;"\n                                            },\n                                            onclick: move |_| chat_mode.set("agentic".to_string()),\n                                            title: "Agentic: LLM decides which tools to call in a loop",\n                                            span { style: "font-size: 0.75em;", "\\u{1F9E0}" }\n                                            " Agent"\n                                        }\n                                        button {\n                                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",\n                                            style: "width: 1.75rem; height: 1.75rem; min-width: 1.75rem; min-height: 1.75rem; background-color: transparent; border: 1.5px solid #026B7C;",\n                                            onclick: move |_| show_agentic_info.set(true),\n                                            title: "Info about Agentic mode",\n                                            svg {\n                                                class: INFO_ICON_SVG_CLASS,\n                                                view_box: "0 0 20 20",\n                                                fill: "none",\n                                                stroke: "#026B7C",\n                                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1.5" }\n                                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }\n                                                circle { cx: "10", cy: "6.3", r: "1", fill: "#026B7C", stroke: "none" }\n                                            }\n                                        }\n                                    }\n\n                                    // Tune button',
    "Add Agentic button before Tune",
)

# 7c. Add agentic to mode description match
patch(
    HSB,
    '                                            match chat_mode().as_str() {\n                                                "auto" =>',
    '                                            match chat_mode().as_str() {\n                                                "agentic" => "LLM-driven tool loop. The model decides when to search documents, recall memory, or answer directly.",\n                                                "auto" =>',
    "Add agentic to mode description match",
    count=0,
)

# 7d. Add Agentic info modal before Auto Mode Info Modal
patch(
    HSB,
    '            // Auto Mode Info Modal',
    '            // Agentic Mode Info Modal\n            if show_agentic_info() {\n                div {\n                    class: "fixed inset-0 bg-black/60 flex items-center justify-center p-4",\n                    style: "z-index: 100;",\n                    onclick: move |_| show_agentic_info.set(false),\n                    div {\n                        class: "bg-gray-800 rounded-xl p-6 max-w-md w-full border border-gray-600 shadow-2xl",\n                        onclick: move |e| e.stop_propagation(),\n                        div { class: "flex justify-between items-center mb-4",\n                            h3 { class: "text-base font-bold", "\\u{1F9E0} Agentic Mode" }\n                            button {\n                                class: "text-gray-400 hover:text-white text-lg",\n                                onclick: move |_| show_agentic_info.set(false),\n                                "\\u{2715}"\n                            }\n                        }\n                        div { class: "text-sm text-gray-300 space-y-3",\n                            p { "Instead of a fixed pipeline, the LLM itself decides what to do at each step." }\n                            p { "It can call tools in a loop:" }\n                            p { class: "text-xs text-gray-400 pl-3",\n                                "\\u{1F50D} Search your documents via Tantivy"\n                                br {}\n                                "\\u{1F9E0} Recall from agent memory"\n                                br {}\n                                "\\u{1F578} Query the knowledge graph"\n                                br {}\n                                "\\u{1F4AD} Reason and combine results"\n                                br {}\n                                "\\u{2705} Decide when it has enough to answer"\n                            }\n                            p { "This is powered by the Rig framework talking to your local Ollama or llama-server." }\n                            p { class: "text-xs text-yellow-400 mt-2", "\\u{26A0} Status: stub \\u{2014} Rig integration pending. Currently returns a placeholder." }\n                        }\n                        button {\n                            class: "btn btn-primary btn-sm mt-4 w-full",\n                            onclick: move |_| show_agentic_info.set(false),\n                            "Got it!"\n                        }\n                    }\n                }\n            }\n\n            // Auto Mode Info Modal',
    "Add Agentic info modal",
)

print("\n\u2705 Phase A complete.")
print("  Backend:  cd ~/ag/backend && cargo check 2>&1 | tail -30")
print("  Frontend: cd ~/ag/frontend/fro && dx serve  (then press r)")
