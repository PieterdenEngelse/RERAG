#!/usr/bin/env python3
"""
Batch 1: mod.rs-only quick wins
  - Strip phase comments (~18 lines)
  - Remove debug logging in stream handler (~20 lines)
  - ChatMode → AgentMode From impl (~15 lines)
  - Chat command dispatch dedup (~110 lines)

Expected savings: ~163 lines
"""
import sys, os, re, shutil
from datetime import datetime

MOD_RS = os.path.expanduser("~/ag/backend/src/api/mod.rs")
ts = datetime.now().strftime("%Y%m%d_%H%M%S")
errors = []
changes = []

if not os.path.exists(MOD_RS):
    print(f"FATAL: {MOD_RS} not found")
    sys.exit(1)

shutil.copy2(MOD_RS, f"{MOD_RS}.bak.{ts}")
print(f"[OK] Backed up to mod.rs.bak.{ts}")

with open(MOD_RS, 'r') as f:
    content = f.read()

original_lines = content.count('\n')

# ═══════════════════════════════════════════════════════════════
# CHANGE 1: Strip phase comments
# ═══════════════════════════════════════════════════════════════

# Remove comments like "// Phase 15: ..." or "// Phase 27" or "(Phase 20)" etc.
# But preserve section headers that happen to mention a phase
phase_pattern = re.compile(
    r'^(\s*)//\s*Phase \d+.*\n',
    re.MULTILINE
)
new_content, count = phase_pattern.subn('', content)
if count > 0:
    content = new_content
    changes.append(f"Stripped {count} phase comments")
else:
    errors.append("WARNING: No phase comments found")

# Also clean inline phase refs like "(Phase 20)" in section headers
# e.g. "// TRAINING DATA COLLECTION ENDPOINTS (Phase 20)" → "// TRAINING DATA COLLECTION ENDPOINTS"
inline_phase = re.compile(r'\s*\(Phase \d+[^)]*\)')
new_content, count2 = inline_phase.subn('', content)
if count2 > 0:
    content = new_content
    changes.append(f"Stripped {count2} inline phase references")

# Also clean /// doc-comment phase prefixes: "/// Phase 16: Export..." → "/// Export..."
doc_phase = re.compile(r'(///\s*)Phase \d+[^:]*:\s*', re.MULTILINE)
new_content, count3 = doc_phase.subn(r'\1', content)
if count3 > 0:
    content = new_content
    changes.append(f"Stripped {count3} doc-comment phase prefixes")

# ═══════════════════════════════════════════════════════════════
# CHANGE 2: Remove debug logging in stream handler
# ═══════════════════════════════════════════════════════════════

debug_block = '''    // Debug: log what's in the system prompt
    tracing::warn!(
        request_id = %request_id,
        system_prompt_len = system_prompt.len(),
        system_prompt_full = %system_prompt,
        memories_count = chat_settings.memories.len(),
        "DEBUG: Full system prompt being sent"
    );
    for (i, mem) in chat_settings.memories.iter().enumerate() {
        tracing::warn!(
            request_id = %request_id,
            memory_index = i,
            memory_type = %mem.memory_type,
            memory_content = %mem.content,
            "DEBUG: Memory item"
        );
    }

'''

if debug_block in content:
    content = content.replace(debug_block, '')
    changes.append("Removed debug logging block from run_agent_stream")
else:
    errors.append("WARNING: Debug logging block not found (may already be removed)")

# ═══════════════════════════════════════════════════════════════
# CHANGE 3: ChatMode → AgentMode From impl
# ═══════════════════════════════════════════════════════════════

# Add From impl right after the ChatMode enum and default_top_k/default_limit fns
from_impl_anchor = '''fn default_limit() -> usize {
    20
}'''

from_impl_replacement = '''fn default_limit() -> usize {
    20
}

impl From<ChatMode> for crate::agent::AgentMode {
    fn from(mode: ChatMode) -> Self {
        match mode {
            ChatMode::Rag => Self::Rag,
            ChatMode::Llm => Self::Llm,
            ChatMode::Hybrid => Self::Hybrid,
            ChatMode::Auto => Self::Auto,
            ChatMode::RagStrict => Self::RagStrict,
        }
    }
}'''

if from_impl_anchor in content:
    content = content.replace(from_impl_anchor, from_impl_replacement, 1)
    changes.append("Added From<ChatMode> for AgentMode impl")
else:
    errors.append("WARNING: Could not find anchor for From impl insertion")

# Now replace the 3 match blocks with .into()
# Occurrence 1: in run_agent (uses req.mode)
match_block_req = '''        // Convert ChatMode to AgentMode
        let agent_mode = match req.mode {
            ChatMode::Rag => crate::agent::AgentMode::Rag,
            ChatMode::Llm => crate::agent::AgentMode::Llm,
            ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
            ChatMode::Auto => crate::agent::AgentMode::Auto,
            ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
        };'''

match_replacement_req = '''        let agent_mode: crate::agent::AgentMode = req.mode.into();'''

# This appears in run_agent and run_agent_stream (both use req.mode)
req_count = content.count(match_block_req)
if req_count >= 1:
    content = content.replace(match_block_req, match_replacement_req)
    changes.append(f"Replaced {req_count} ChatMode->AgentMode match block(s) using req.mode")
else:
    errors.append("WARNING: ChatMode match block (req.mode) not found")

# Occurrence 2: in run_agent_get (uses query.mode)
match_block_query = '''        // Convert ChatMode to AgentMode
        let agent_mode = match query.mode {
            ChatMode::Rag => crate::agent::AgentMode::Rag,
            ChatMode::Llm => crate::agent::AgentMode::Llm,
            ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
            ChatMode::Auto => crate::agent::AgentMode::Auto,
            ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
        };'''

match_replacement_query = '''        let agent_mode: crate::agent::AgentMode = query.mode.into();'''

if match_block_query in content:
    content = content.replace(match_block_query, match_replacement_query, 1)
    changes.append("Replaced ChatMode->AgentMode match block using query.mode")
else:
    errors.append("WARNING: ChatMode match block (query.mode) not found")

# Also handle the one in run_agent_stream that uses a slightly different format
# (no "Convert ChatMode to AgentMode" comment)
match_block_stream = '''    // Determine mode and build context
    let agent_mode = match req.mode {
        ChatMode::Rag => crate::agent::AgentMode::Rag,
        ChatMode::Llm => crate::agent::AgentMode::Llm,
        ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
        ChatMode::Auto => crate::agent::AgentMode::Auto,
        ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
    };'''

match_replacement_stream = '''    // Determine mode and build context
    let agent_mode: crate::agent::AgentMode = req.mode.into();'''

if match_block_stream in content:
    content = content.replace(match_block_stream, match_replacement_stream, 1)
    changes.append("Replaced ChatMode->AgentMode match block in run_agent_stream")

# ═══════════════════════════════════════════════════════════════
# CHANGE 4: Chat command dispatch dedup
# ═══════════════════════════════════════════════════════════════

# Add the shared helper functions right before run_agent
helper_anchor = 'async fn run_agent(req: web::Json<AgentRequest>) -> Result<HttpResponse, Error> {'

helper_functions = '''/// Execute a parsed chat command and return (answer, optional_extra_data)
async fn execute_chat_command(cmd: ChatCommand, top_k: usize) -> (String, Option<Value>) {
    match cmd {
        ChatCommand::Goal(goal_text) => match create_goal_from_command(&goal_text) {
            Ok(goal) => (
                format!("✓ Goal created: {}", goal_text),
                Some(json!({ "goal": goal })),
            ),
            Err(e) => (format!("✗ Failed to create goal: {}", e), None),
        },
        ChatCommand::Goals => match get_goals_list() {
            Ok(list) => (list, None),
            Err(e) => (format!("✗ Failed to get goals: {}", e), None),
        },
        ChatCommand::Status => (get_system_status(), None),
        ChatCommand::Help => (get_help_text(), None),
        ChatCommand::Models => (get_models_list(), None),
        ChatCommand::Clear => (
            "Chat cleared. (This is handled by the frontend)".to_string(),
            None,
        ),
        ChatCommand::Forget(topic) => match forget_topic(&topic) {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::History => match list_recent_history(5) {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::Sources => (last_sources_summary(), None),
        ChatCommand::Learn(url) => match preview_url_content(&url).await {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::Note(text) => match record_note(&text) {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::Subgoal(text) => match add_subgoal(&text) {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::PauseGoal => match update_goal_status_cmd(GoalStatus::Paused) {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::ResumeGoal => match update_goal_status_cmd(GoalStatus::Active) {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::AbandonGoal => match update_goal_status_cmd(GoalStatus::Abandoned) {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::Reflect => match summarize_reflection() {
            Ok(msg) => (msg, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::Why => (explain_last_reasoning(), None),
        ChatCommand::Focus(topic) => (apply_focus(Some(topic)), None),
        ChatCommand::Unfocus => (apply_focus(None), None),
        ChatCommand::Persona(name) => {
            let persona_value =
                if name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("reset") {
                    None
                } else {
                    Some(name)
                };
            (apply_persona(persona_value), None)
        }
        ChatCommand::Verbose => (apply_verbosity(Verbosity::Verbose), None),
        ChatCommand::Brief => (apply_verbosity(Verbosity::Brief), None),
        ChatCommand::RunTool(spec) => match run_tool_command(&spec).await {
            Ok(result) => (result, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::Chain(first, second) => match run_chain_command((first, second)).await {
            Ok(result) => (result, None),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::Retry => match retry_last_query(top_k) {
            Ok(agent_response) => (
                agent_response.answer.clone(),
                Some(json!({ "retry": agent_response })),
            ),
            Err(err) => (format!("✗ {}", err), None),
        },
        ChatCommand::Undo => (apply_undo(), None),
        ChatCommand::DryRun(plan) => (render_dry_run_plan(&plan), None),
        ChatCommand::Model(name) => {
            let model_value = if name.eq_ignore_ascii_case("default") {
                None
            } else {
                Some(name)
            };
            (apply_model(model_value), None)
        }
        ChatCommand::Temperature(value) => {
            let parsed = value.parse::<f32>().ok();
            (apply_temperature(parsed), None)
        }
        ChatCommand::Export => (export_state(), None),
        ChatCommand::Import(payload) => {
            let body = payload.unwrap_or_else(|| "{}".to_string());
            (import_state(&body), None)
        }
        ChatCommand::Debug => (debug_state_snapshot(), None),
        ChatCommand::Tokens => (tokens_usage_snapshot(), None),
    }
}

/// Build a standard JSON response for a chat command result
fn build_command_response(answer: String, extra: Option<Value>, request_id: &str) -> HttpResponse {
    let mut response = json!({
        "response": {
            "answer": answer,
            "chunks_used": 0,
            "sources": []
        },
        "request_id": request_id
    });
    if let Some(extra_data) = extra {
        if let Some(obj) = response.as_object_mut() {
            for (k, v) in extra_data.as_object().unwrap() {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    HttpResponse::Ok().json(response)
}

async fn run_agent(req: web::Json<AgentRequest>) -> Result<HttpResponse, Error> {'''

if helper_anchor in content:
    content = content.replace(helper_anchor, helper_functions, 1)
    changes.append("Added execute_chat_command() and build_command_response() helpers")
else:
    errors.append("FATAL: Could not find run_agent function anchor")

# Now replace the big match+response block in run_agent with the short version
old_run_agent_dispatch = '''    // Check for chat commands
    if let Some(cmd) = parse_chat_command(&req.query) {
        let (answer, extra) = match cmd {
            ChatCommand::Goal(goal_text) => match create_goal_from_command(&goal_text) {
                Ok(goal) => (
                    format!("✓ Goal created: {}", goal_text),
                    Some(json!({ "goal": goal })),
                ),
                Err(e) => (format!("✗ Failed to create goal: {}", e), None),
            },
            ChatCommand::Goals => match get_goals_list() {
                Ok(list) => (list, None),
                Err(e) => (format!("✗ Failed to get goals: {}", e), None),
            },
            ChatCommand::Status => (get_system_status(), None),
            ChatCommand::Help => (get_help_text(), None),
            ChatCommand::Models => (get_models_list(), None),
            ChatCommand::Clear => (
                "Chat cleared. (This is handled by the frontend)".to_string(),
                None,
            ),
            ChatCommand::Forget(topic) => match forget_topic(&topic) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::History => match list_recent_history(5) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Sources => (last_sources_summary(), None),
            ChatCommand::Learn(url) => match preview_url_content(&url).await {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Note(text) => match record_note(&text) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Subgoal(text) => match add_subgoal(&text) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::PauseGoal => match update_goal_status_cmd(GoalStatus::Paused) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::ResumeGoal => match update_goal_status_cmd(GoalStatus::Active) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::AbandonGoal => match update_goal_status_cmd(GoalStatus::Abandoned) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Reflect => match summarize_reflection() {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Why => (explain_last_reasoning(), None),
            ChatCommand::Focus(topic) => (apply_focus(Some(topic)), None),
            ChatCommand::Unfocus => (apply_focus(None), None),
            ChatCommand::Persona(name) => {
                let persona_value =
                    if name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("reset") {
                        None
                    } else {
                        Some(name)
                    };
                (apply_persona(persona_value), None)
            }
            ChatCommand::Verbose => (apply_verbosity(Verbosity::Verbose), None),
            ChatCommand::Brief => (apply_verbosity(Verbosity::Brief), None),
            ChatCommand::RunTool(spec) => match run_tool_command(&spec).await {
                Ok(result) => (result, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Chain(first, second) => match run_chain_command((first, second)).await {
                Ok(result) => (result, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Retry => match retry_last_query(req.top_k) {
                Ok(agent_response) => (
                    agent_response.answer.clone(),
                    Some(json!({ "retry": agent_response })),
                ),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Undo => (apply_undo(), None),
            ChatCommand::DryRun(plan) => (render_dry_run_plan(&plan), None),
            ChatCommand::Model(name) => {
                let model_value = if name.eq_ignore_ascii_case("default") {
                    None
                } else {
                    Some(name)
                };
                (apply_model(model_value), None)
            }
            ChatCommand::Temperature(value) => {
                let parsed = value.parse::<f32>().ok();
                (apply_temperature(parsed), None)
            }
            ChatCommand::Export => (export_state(), None),
            ChatCommand::Import(payload) => {
                let body = payload.unwrap_or_else(|| "{}".to_string());
                (import_state(&body), None)
            }
            ChatCommand::Debug => (debug_state_snapshot(), None),
            ChatCommand::Tokens => (tokens_usage_snapshot(), None),
        };

        let mut response = json!({
            "response": {
                "answer": answer,
                "chunks_used": 0,
                "sources": []
            },
            "request_id": request_id
        });

        if let Some(extra_data) = extra {
            if let Some(obj) = response.as_object_mut() {
                for (k, v) in extra_data.as_object().unwrap() {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        return Ok(HttpResponse::Ok().json(response));
    }'''

new_run_agent_dispatch = '''    // Check for chat commands
    if let Some(cmd) = parse_chat_command(&req.query) {
        let (answer, extra) = execute_chat_command(cmd, req.top_k).await;
        return Ok(build_command_response(answer, extra, &request_id));
    }'''

if old_run_agent_dispatch in content:
    content = content.replace(old_run_agent_dispatch, new_run_agent_dispatch, 1)
    changes.append("Replaced run_agent chat command dispatch with helper call")
else:
    errors.append("FATAL: Could not find run_agent dispatch block")

# Now replace the identical block in run_agent_get
# The only difference: query.top_k instead of req.top_k, and query.query
old_run_agent_get_dispatch = '''    // Check for chat commands
    if let Some(cmd) = parse_chat_command(&query.query) {
        let (answer, extra) = match cmd {
            ChatCommand::Goal(goal_text) => match create_goal_from_command(&goal_text) {
                Ok(goal) => (
                    format!("✓ Goal created: {}", goal_text),
                    Some(json!({ "goal": goal })),
                ),
                Err(e) => (format!("✗ Failed to create goal: {}", e), None),
            },
            ChatCommand::Goals => match get_goals_list() {
                Ok(list) => (list, None),
                Err(e) => (format!("✗ Failed to get goals: {}", e), None),
            },
            ChatCommand::Status => (get_system_status(), None),
            ChatCommand::Help => (get_help_text(), None),
            ChatCommand::Models => (get_models_list(), None),
            ChatCommand::Clear => (
                "Chat cleared. (This is handled by the frontend)".to_string(),
                None,
            ),
            ChatCommand::Forget(topic) => match forget_topic(&topic) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::History => match list_recent_history(5) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Sources => (last_sources_summary(), None),
            ChatCommand::Learn(url) => match preview_url_content(&url).await {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Note(text) => match record_note(&text) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Subgoal(text) => match add_subgoal(&text) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::PauseGoal => match update_goal_status_cmd(GoalStatus::Paused) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::ResumeGoal => match update_goal_status_cmd(GoalStatus::Active) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::AbandonGoal => match update_goal_status_cmd(GoalStatus::Abandoned) {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Reflect => match summarize_reflection() {
                Ok(msg) => (msg, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Why => (explain_last_reasoning(), None),
            ChatCommand::Focus(topic) => (apply_focus(Some(topic)), None),
            ChatCommand::Unfocus => (apply_focus(None), None),
            ChatCommand::Persona(name) => {
                let persona_value =
                    if name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("reset") {
                        None
                    } else {
                        Some(name)
                    };
                (apply_persona(persona_value), None)
            }
            ChatCommand::Verbose => (apply_verbosity(Verbosity::Verbose), None),
            ChatCommand::Brief => (apply_verbosity(Verbosity::Brief), None),
            ChatCommand::RunTool(spec) => match run_tool_command(&spec).await {
                Ok(result) => (result, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Chain(first, second) => match run_chain_command((first, second)).await {
                Ok(result) => (result, None),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Retry => match retry_last_query(query.top_k) {
                Ok(agent_response) => (
                    agent_response.answer.clone(),
                    Some(json!({ "retry": agent_response })),
                ),
                Err(err) => (format!("✗ {}", err), None),
            },
            ChatCommand::Undo => (apply_undo(), None),
            ChatCommand::DryRun(plan) => (render_dry_run_plan(&plan), None),
            ChatCommand::Model(name) => {
                let model_value = if name.eq_ignore_ascii_case("default") {
                    None
                } else {
                    Some(name)
                };
                (apply_model(model_value), None)
            }
            ChatCommand::Temperature(value) => {
                let parsed = value.parse::<f32>().ok();
                (apply_temperature(parsed), None)
            }
            ChatCommand::Export => (export_state(), None),
            ChatCommand::Import(payload) => {
                let body = payload.unwrap_or_else(|| "{}".to_string());
                (import_state(&body), None)
            }
            ChatCommand::Debug => (debug_state_snapshot(), None),
            ChatCommand::Tokens => (tokens_usage_snapshot(), None),
        };

        let mut response = json!({
            "response": {
                "answer": answer,
                "chunks_used": 0,
                "sources": []
            },
            "request_id": request_id
        });

        if let Some(extra_data) = extra {
            if let Some(obj) = response.as_object_mut() {
                for (k, v) in extra_data.as_object().unwrap() {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        return Ok(HttpResponse::Ok().json(response));
    }'''

new_run_agent_get_dispatch = '''    // Check for chat commands
    if let Some(cmd) = parse_chat_command(&query.query) {
        let (answer, extra) = execute_chat_command(cmd, query.top_k).await;
        return Ok(build_command_response(answer, extra, &request_id));
    }'''

if old_run_agent_get_dispatch in content:
    content = content.replace(old_run_agent_get_dispatch, new_run_agent_get_dispatch, 1)
    changes.append("Replaced run_agent_get chat command dispatch with helper call")
else:
    errors.append("FATAL: Could not find run_agent_get dispatch block")

# ═══════════════════════════════════════════════════════════════
# WRITE RESULT
# ═══════════════════════════════════════════════════════════════

new_lines = content.count('\n')
saved = original_lines - new_lines

with open(MOD_RS, 'w') as f:
    f.write(content)

print(f"\n{'='*60}")
print(f"CHANGES APPLIED:")
for c in changes:
    print(f"  ✓ {c}")

if errors:
    print(f"\nWARNINGS/ERRORS:")
    for e in errors:
        print(f"  ⚠ {e}")

print(f"\nLines: {original_lines} → {new_lines} (saved {saved})")
print(f"\nNext: cd ~/ag && cargo check 2>&1 | head -30")
