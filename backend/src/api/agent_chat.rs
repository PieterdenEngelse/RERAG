// ~/ag/backend/src/api/agent_chat.rs  v1.0
// Agent chat system: commands, tool runners, streaming, chat state

use super::*;

// Shared agent session state for chat commands
#[derive(Default, Clone)]
pub(crate) struct AgentChatState {
    pub focus_topic: Option<String>,
    pub persona: Option<String>,
    pub verbosity: Verbosity,
    pub preferred_model: Option<String>,
    pub temperature: Option<f32>,
    pub last_query: Option<String>,
    pub last_response: Option<String>,
    pub last_steps: Vec<AgentStep>,
    pub last_sources: Vec<String>,
    #[allow(dead_code)]
    pub last_tool: Option<String>,
    pub last_token_usage: Option<usize>,
    pub undo_stack: Vec<CommandAction>,
    pub dry_run_plan: Option<String>,
    /// Enable prompt caching (uses /api/chat instead of /api/generate for Ollama)
    pub prompt_caching_enabled: bool,
}

#[derive(Clone)]
pub(crate) enum CommandAction {
    FocusSet(Option<String>),
    PersonaSet(Option<String>),
    VerbosityChanged(Verbosity),
    ModelChanged(Option<String>),
    TemperatureChanged(Option<f32>),
    NoteAdded(#[allow(dead_code)] String),
}

#[derive(Clone, Copy, Default)]
pub(crate) enum Verbosity {
    Brief,
    #[default]
    Normal,
    Verbose,
}

impl Verbosity {
    fn label(&self) -> &'static str {
        match self {
            Verbosity::Brief => "brief",
            Verbosity::Normal => "normal",
            Verbosity::Verbose => "verbose",
        }
    }
}

pub(crate) fn chat_state() -> Arc<Mutex<AgentChatState>> {
    CHAT_STATE
        .get_or_init(|| Arc::new(Mutex::new(AgentChatState::default())))
        .clone()
}

pub(crate) fn update_last_agent_run(query: String, response: &AgentResponse) {
    let state_arc = chat_state();
    let mut state = state_arc.lock().expect("chat state lock");
    state.last_query = Some(query.clone());
    state.last_response = Some(response.answer.clone());
    state.last_steps = response.steps.clone();
    state.last_sources = response.used_chunks.clone();
    let token_estimate = response.answer.split_whitespace().count();
    state.last_token_usage = Some(token_estimate.max(response.used_chunks.len()));
}

pub(crate) fn record_focus_change(new_focus: Option<String>) -> Option<String> {
    let state_arc = chat_state();
    let mut state = state_arc.lock().expect("chat state lock");
    let previous = state.focus_topic.clone();
    state
        .undo_stack
        .push(CommandAction::FocusSet(previous.clone()));
    state.focus_topic = new_focus;
    previous
}

pub(crate) fn record_persona_change(new_persona: Option<String>) -> Option<String> {
    let state_arc = chat_state();
    let mut state = state_arc.lock().expect("chat state lock");
    let previous = state.persona.clone();
    state
        .undo_stack
        .push(CommandAction::PersonaSet(previous.clone()));
    state.persona = new_persona;
    previous
}

pub(crate) fn record_verbosity_change(new_mode: Verbosity) -> Verbosity {
    let state_arc = chat_state();
    let mut state = state_arc.lock().expect("chat state lock");
    let previous = state.verbosity;
    state
        .undo_stack
        .push(CommandAction::VerbosityChanged(previous));
    state.verbosity = new_mode;
    previous
}

pub(crate) fn push_note_action(note: String) {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    guard.undo_stack.push(CommandAction::NoteAdded(note));
}

pub(crate) fn record_model_change(new_model: Option<String>) -> Option<String> {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    let previous = guard.preferred_model.clone();
    guard
        .undo_stack
        .push(CommandAction::ModelChanged(previous.clone()));
    guard.preferred_model = new_model.clone();
    previous
}

pub(crate) fn record_temperature_change(new_temp: Option<f32>) -> Option<f32> {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    let previous = guard.temperature;
    guard
        .undo_stack
        .push(CommandAction::TemperatureChanged(previous));
    guard.temperature = new_temp;
    previous
}

pub(crate) fn pop_undo_action() -> Option<CommandAction> {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    guard.undo_stack.pop()
}

#[allow(dead_code)]
pub(crate) fn snapshots_for_debug() -> (Option<String>, Option<String>, Verbosity, Option<String>) {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    (
        state.focus_topic.clone(),
        state.persona.clone(),
        state.verbosity,
        state.last_query.clone(),
    )
}

/// Get current chat settings for the agent, including RAG memories
pub(crate) fn get_current_chat_settings() -> crate::agent::ChatSettings {
    use crate::agent::{load_categorized_memories, ChatSettings, Verbosity as AgentVerbosity};

    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");

    let verbosity = match state.verbosity {
        Verbosity::Brief => AgentVerbosity::Brief,
        Verbosity::Normal => AgentVerbosity::Normal,
        Verbosity::Verbose => AgentVerbosity::Verbose,
    };

    // Load RAG memories from database (limit to 20 most recent)
    let memories = load_categorized_memories(path_resolver::agent_db_path_str(), "default", 20);

    ChatSettings::new()
        .with_focus(state.focus_topic.clone())
        .with_persona(state.persona.clone())
        .with_verbosity(verbosity)
        .with_temperature(state.temperature)
        .with_model(state.preferred_model.clone())
        .with_memories(memories)
}

pub(crate) fn store_dry_run_plan(plan: String) {
    let state_arc = chat_state();
    let mut guard = state_arc.lock().expect("chat state lock");
    guard.dry_run_plan = Some(plan);
}

#[allow(dead_code)]
pub(crate) fn fetch_dry_run_plan() -> Option<String> {
    let state_arc = chat_state();
    let guard = state_arc.lock().expect("chat state lock");
    guard.dry_run_plan.clone()
}

#[derive(serde::Deserialize)]
pub struct AgentRequest {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub mode: ChatMode,
    pub corpus: Option<String>,
}

// Simple query variant for GET /agent/chat
#[derive(serde::Deserialize)]
pub struct AgentQueryParams {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub mode: ChatMode,
    pub corpus: Option<String>,
}

pub(crate) fn default_top_k() -> usize {
    5
}

pub(crate) fn default_limit() -> usize {
    20
}

/// Chat command types
#[allow(dead_code)]
pub(crate) enum ChatCommand {
    // Existing goal/system helpers
    Goal(String),
    Goals,
    Status,
    Help,
    Models,
    Clear,
    // Knowledge management
    Forget(String),
    History,
    Sources,
    Learn(String),
    Note(String),
    // Goal & task management
    Subgoal(String),
    PauseGoal,
    ResumeGoal,
    AbandonGoal,
    Reflect,
    Why,
    // Context control
    Focus(String),
    Unfocus,
    Persona(String),
    Verbose,
    Brief,
    // Tools & execution
    RunTool(String),
    Chain(String, String),
    Retry,
    Undo,
    DryRun(String),
    // System commands
    Model(String),
    Temperature(String),
    Export,
    Import(Option<String>),
    Debug,
    Tokens,
}

pub(crate) fn extract_argument<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.strip_prefix(marker)
        .map(|rest| rest.trim())
        .filter(|s| !s.is_empty())
}

/// Parse chat commands from user input
pub(crate) fn parse_chat_command(query: &str) -> Option<ChatCommand> {
    let trimmed = query.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    if let Some(arg) = extract_argument(trimmed, "/goal ") {
        return Some(ChatCommand::Goal(arg.to_string()));
    }
    if trimmed == "/goals" {
        return Some(ChatCommand::Goals);
    }
    if trimmed == "/status" {
        return Some(ChatCommand::Status);
    }
    if trimmed == "/help" {
        return Some(ChatCommand::Help);
    }
    if trimmed == "/models" {
        return Some(ChatCommand::Models);
    }
    if trimmed == "/clear" {
        return Some(ChatCommand::Clear);
    }

    if let Some(arg) = extract_argument(trimmed, "/forget ") {
        return Some(ChatCommand::Forget(arg.to_string()));
    }
    if trimmed == "/history" {
        return Some(ChatCommand::History);
    }
    if trimmed == "/sources" {
        return Some(ChatCommand::Sources);
    }
    if let Some(arg) = extract_argument(trimmed, "/learn ") {
        return Some(ChatCommand::Learn(arg.to_string()));
    }
    if let Some(arg) = extract_argument(trimmed, "/note ") {
        return Some(ChatCommand::Note(arg.to_string()));
    }

    if let Some(arg) = extract_argument(trimmed, "/subgoal ") {
        return Some(ChatCommand::Subgoal(arg.to_string()));
    }
    if trimmed == "/pause" {
        return Some(ChatCommand::PauseGoal);
    }
    if trimmed == "/resume" {
        return Some(ChatCommand::ResumeGoal);
    }
    if trimmed == "/abandon" {
        return Some(ChatCommand::AbandonGoal);
    }
    if trimmed == "/reflect" {
        return Some(ChatCommand::Reflect);
    }
    if trimmed == "/why" {
        return Some(ChatCommand::Why);
    }

    if let Some(arg) = extract_argument(trimmed, "/focus ") {
        return Some(ChatCommand::Focus(arg.to_string()));
    }
    if trimmed == "/unfocus" {
        return Some(ChatCommand::Unfocus);
    }
    if let Some(arg) = extract_argument(trimmed, "/persona ") {
        return Some(ChatCommand::Persona(arg.to_string()));
    }
    if trimmed == "/verbose" {
        return Some(ChatCommand::Verbose);
    }
    if trimmed == "/brief" {
        return Some(ChatCommand::Brief);
    }

    if let Some(arg) = extract_argument(trimmed, "/run ") {
        return Some(ChatCommand::RunTool(arg.to_string()));
    }
    if let Some(arg) = extract_argument(trimmed, "/chain ") {
        let parts: Vec<&str> = arg.split("->").map(|p| p.trim()).collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(ChatCommand::Chain(
                parts[0].to_string(),
                parts[1].to_string(),
            ));
        }
    }
    if trimmed == "/retry" {
        return Some(ChatCommand::Retry);
    }
    if trimmed == "/undo" {
        return Some(ChatCommand::Undo);
    }
    if let Some(arg) = extract_argument(trimmed, "/dry-run ") {
        return Some(ChatCommand::DryRun(arg.to_string()));
    }

    if let Some(arg) = extract_argument(trimmed, "/model ") {
        return Some(ChatCommand::Model(arg.to_string()));
    }
    if let Some(arg) = extract_argument(trimmed, "/temperature ") {
        return Some(ChatCommand::Temperature(arg.to_string()));
    }
    if trimmed == "/export" {
        return Some(ChatCommand::Export);
    }
    if trimmed == "/import" {
        return Some(ChatCommand::Import(None));
    }
    if trimmed == "/debug" {
        return Some(ChatCommand::Debug);
    }
    if trimmed == "/tokens" {
        return Some(ChatCommand::Tokens);
    }

    None
}

/// Create a goal via the agentic monitor routes
pub(crate) fn create_goal_from_command(goal_text: &str) -> Result<serde_json::Value, String> {
    use crate::api::agentic_monitor_routes::get_agent_db_connection;

    let conn = get_agent_db_connection().ok_or_else(|| "Database not available".to_string())?;

    let goal_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    let agent_id = "default";

    conn.execute(
        "INSERT INTO goals (id, agent_id, goal, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![&goal_id, agent_id, goal_text, "active", now],
    )
    .map_err(|e| format!("Failed to create goal: {}", e))?;

    Ok(json!({
        "id": goal_id,
        "goal": goal_text,
        "status": "active",
        "agent_id": agent_id,
        "created_at": now
    }))
}

/// Get help text for chat commands
pub(crate) fn get_help_text() -> String {
    r#"Available commands:

/chain a -> b - Execute tool sequences (use $last placeholder)
/clear        - Clear chat history (frontend only)
/debug|/tokens - Inspect internals
/dry-run <query> - Plan without execution
/export|/import <json> - Export or import memories
/focus <topic>|/unfocus - Control attention
/forget <topic> - Forget matching memories
/goal <text>  - Create a new goal
/goals        - List active goals
/help         - Show this help message
/history      - Show recent agent episodes
/learn <url>  - Fetch & ingest a URL (preview)
/model <name> - Switch backend model (use 'default')
/models       - List available models
/note <text>  - Store a quick note
/pause|/resume|/abandon - Control current goal
/persona <name> - Swap agent persona
/reflect      - Generate a reflection summary
/retry|/undo  - Retry last query / undo change
/run <tool>   - Execute calculator/search/fetch
/sources      - Show last response sources
/status       - Show system health status
/subgoal <text> - Add task under current goal
/temperature <n> - Adjust creativity (use 'default')
/verbose|/brief - Change response verbosity
/why          - Explain the last reasoning steps

Examples:
  /goal Find all Rust error handling patterns
  /focus tracing metrics
  /run calculator 5+7"#
        .to_string()
}

/// Get active goals list
pub(crate) fn get_goals_list() -> Result<String, String> {
    use crate::api::agentic_monitor_routes::get_agent_db_connection;

    let conn = get_agent_db_connection().ok_or_else(|| "Database not available".to_string())?;

    let mut stmt = conn.prepare(
        "SELECT goal, status, created_at FROM goals WHERE status = 'active' ORDER BY created_at DESC LIMIT 10"
    ).map_err(|e| e.to_string())?;

    let goals: Vec<String> = stmt
        .query_map([], |row| {
            let goal: String = row.get(0)?;
            Ok(format!("• {}", goal))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    if goals.is_empty() {
        Ok("No active goals. Create one with: /goal <your goal>".to_string())
    } else {
        Ok(format!(
            "Active Goals ({}):\n{}",
            goals.len(),
            goals.join("\n")
        ))
    }
}

/// Get system status
pub(crate) fn get_system_status() -> String {
    let health = if RETRIEVER.get().is_some() {
        "✓ Healthy"
    } else {
        "✗ Retriever not initialized"
    };
    format!(
        "System Status: {}\nBackend: Running\nTimestamp: {}",
        health,
        Utc::now().to_rfc3339()
    )
}

/// Get available models
pub(crate) fn get_models_list() -> String {
    // This would ideally query the actual models, but for now return a placeholder
    "Available models:\n• default (local embedding model)\n\nUse /config to change model settings."
        .to_string()
}

pub(crate) fn forget_topic(topic: &str) -> Result<String, String> {
    let mem = AgentMemory::new(path_resolver::agent_db_path_str()).map_err(|e| e.to_string())?;
    let removed = mem
        .forget_topic("default", topic)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "Removed {} memories mentioning '{}'.",
        removed, topic
    ))
}

pub(crate) fn list_recent_history(limit: usize) -> Result<String, String> {
    use crate::api::agentic_monitor_routes::get_agent_db_connection;
    let conn = get_agent_db_connection().ok_or_else(|| "Database not available".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT query, response, created_at FROM episodes ORDER BY created_at DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit as i64], |row| {
            let ts: i64 = row.get(2)?;
            let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".into());
            Ok(format!(
                "• [{}] {}\n  ↳ {}",
                timestamp,
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?
            ))
        })
        .map_err(|e| e.to_string())?;
    let entries: Vec<String> = rows.filter_map(Result::ok).collect();
    if entries.is_empty() {
        Ok("No recorded history yet. Ask a question to get started.".to_string())
    } else {
        Ok(entries.join("\n"))
    }
}

pub(crate) fn last_sources_summary() -> String {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    if state.last_sources.is_empty() {
        "No sources captured yet. Run a search query first.".to_string()
    } else {
        let lines: Vec<String> = state
            .last_sources
            .iter()
            .enumerate()
            .map(|(idx, s)| format!("{}. {}", idx + 1, s))
            .collect();
        format!("Sources from last response:\n{}", lines.join("\n"))
    }
}

pub(crate) fn record_note(content: &str) -> Result<String, String> {
    let mem = AgentMemory::new(path_resolver::agent_db_path_str()).map_err(|e| e.to_string())?;
    let ts = chrono::Utc::now().to_rfc3339();
    mem.add_note("default", content, &ts)
        .map_err(|e| e.to_string())?;
    push_note_action(content.to_string());
    Ok("Note stored.".to_string())
}

pub(crate) fn add_subgoal(text: &str) -> Result<String, String> {
    let mem = AgentMemory::new(path_resolver::agent_db_path_str()).map_err(|e| e.to_string())?;
    if let Some((goal_id, goal_text)) = mem.latest_goal("default").map_err(|e| e.to_string())? {
        let task_id = mem
            .create_subgoal(&goal_id, text)
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Added subgoal under '{}': {} (task {})",
            goal_text, text, task_id
        ))
    } else {
        Err("No active goal to attach a subgoal. Use /goal first.".to_string())
    }
}

pub(crate) fn update_goal_status_cmd(status: GoalStatus) -> Result<String, String> {
    let mem = AgentMemory::new(path_resolver::agent_db_path_str()).map_err(|e| e.to_string())?;
    if let Some((goal_id, goal_text)) = mem.latest_goal("default").map_err(|e| e.to_string())? {
        mem.update_goal_status(&goal_id, status.as_str())
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Goal '{}' marked as {}.",
            goal_text,
            status.as_str()
        ))
    } else {
        Err("No goal found.".to_string())
    }
}

pub(crate) fn summarize_reflection() -> Result<String, String> {
    use crate::api::agentic_monitor_routes::get_agent_db_connection;
    let conn = get_agent_db_connection().ok_or_else(|| "Database not available".to_string())?;
    let one_day_ago = chrono::Utc::now().timestamp() - 24 * 3600;
    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*), SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) FROM episodes WHERE created_at > ?1",
        )
        .map_err(|e| e.to_string())?;
    let (total, success): (i64, i64) = stmt
        .query_row([one_day_ago], |row| {
            Ok((row.get(0)?, row.get(1).unwrap_or(0)))
        })
        .map_err(|e| e.to_string())?;
    let rate = if total > 0 {
        (success as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    Ok(format!(
        "Last 24h episodes: {} (success {:.1}%)",
        total, rate
    ))
}

pub(crate) fn explain_last_reasoning() -> String {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    if state.last_steps.is_empty() {
        "No reasoning trace available yet.".to_string()
    } else {
        let details: Vec<String> = state
            .last_steps
            .iter()
            .map(|step| format!("- [{}] {}", step.kind, step.message))
            .collect();
        details.join("\n")
    }
}

pub(crate) fn apply_focus(topic: Option<String>) -> String {
    let previous = record_focus_change(topic.clone());
    match (previous, topic) {
        (Some(prev), Some(new_topic)) => {
            format!("Focus switched from '{}' to '{}'.", prev, new_topic)
        }
        (None, Some(new_topic)) => format!("Focus set to '{}'.", new_topic),
        (_, None) => "Focus cleared.".to_string(),
    }
}

pub(crate) fn apply_persona(persona: Option<String>) -> String {
    let previous = record_persona_change(persona.clone());
    match (previous, persona) {
        (Some(prev), Some(new_persona)) => {
            format!("Persona switched from '{}' to '{}'.", prev, new_persona)
        }
        (None, Some(new_persona)) => format!("Persona set to '{}'.", new_persona),
        (_, None) => "Persona reset to default.".to_string(),
    }
}

pub(crate) fn apply_verbosity(mode: Verbosity) -> String {
    let previous = record_verbosity_change(mode);
    format!(
        "Verbosity changed from {} to {}.",
        previous.label(),
        mode.label()
    )
}

pub(crate) fn apply_model(model: Option<String>) -> String {
    let previous = record_model_change(model.clone());
    match (previous, model) {
        (Some(prev), Some(new_model)) => {
            format!("Model switched from '{}' to '{}'.", prev, new_model)
        }
        (None, Some(new_model)) => format!("Model set to '{}'.", new_model),
        (_, None) => "Model reset to default.".to_string(),
    }
}

pub(crate) fn apply_temperature(temp: Option<f32>) -> String {
    let previous = record_temperature_change(temp);
    match (previous, temp) {
        (Some(prev), Some(new_temp)) => {
            format!("Temperature changed from {:.2} to {:.2}.", prev, new_temp)
        }
        (None, Some(new_temp)) => format!("Temperature set to {:.2}.", new_temp),
        (_, None) => "Temperature reset to default.".to_string(),
    }
}

pub(crate) async fn run_calculator_tool(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if let Some(inner) = trimmed
        .strip_prefix("length(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let len = inner.chars().count();
        return Ok(format!("length(...) = {}", len));
    }
    let tool = CalculatorTool::new();
    match tool.execute(trimmed).await {
        Ok(result) if result.success => Ok(result.result),
        Ok(result) => Err(result.result),
        Err(err) => Err(err),
    }
}

pub(crate) async fn run_web_search_tool(input: &str) -> Result<String, String> {
    let tool = WebSearchTool::new();
    let result = tool.execute(input).await.map_err(|e| e.to_string())?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

pub(crate) async fn run_translator_tool(input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Provide text to translate, e.g. 'translate hello to spanish'.".to_string());
    }
    let tool = TranslatorTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

pub(crate) async fn run_sentiment_tool(input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Provide text to analyze, e.g. 'sentiment I love this product'.".to_string());
    }
    let tool = SentimentAnalyzerTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

pub(crate) async fn run_entity_tool(input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err(
            "Provide text to extract entities from, e.g. 'entities Elon Musk founded SpaceX'."
                .to_string(),
        );
    }
    let tool = EntityExtractorTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

pub(crate) async fn run_spell_checker_tool(input: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Provide text to spell check, e.g. 'spellcheck teh quikc fox'.".to_string());
    }
    let tool = SpellCheckerTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

pub(crate) async fn run_scheduler_tool(input: &str) -> Result<String, String> {
    let tool = SchedulerTool::new();
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

pub(crate) async fn run_memory_tool(input: &str) -> Result<String, String> {
    let tool = MemoryTool::new(None);
    let result = tool.execute(input).await?;
    if result.success {
        Ok(result.result)
    } else {
        Err(result.result)
    }
}

pub(crate) fn normalize_pipe_separators(command: &str) -> String {
    command
        .split('|')
        .map(|segment| segment.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) async fn run_tool_command(command: &str) -> Result<String, String> {
    let command = normalize_pipe_separators(command);
    let trimmed = command.trim();
    if trimmed.starts_with("calculator") {
        let expr = trimmed.strip_prefix("calculator").unwrap_or("").trim();
        if expr.is_empty() {
            Err("Provide an expression after 'calculator'.".to_string())
        } else {
            run_calculator_tool(expr).await
        }
    } else if trimmed.starts_with("search") {
        let query = trimmed.strip_prefix("search").unwrap_or("").trim();
        if query.is_empty() {
            Err("Provide a query after 'search'.".to_string())
        } else {
            run_web_search_tool(query).await
        }
    } else if trimmed.starts_with("fetch") {
        let url = trimmed.strip_prefix("fetch").unwrap_or("").trim();
        if url.is_empty() {
            Err("Provide a URL after 'fetch'.".to_string())
        } else {
            preview_url_content(url).await
        }
    } else if trimmed.starts_with("translate") {
        let request = trimmed.strip_prefix("translate").unwrap_or("").trim();
        run_translator_tool(request).await
    } else if trimmed.starts_with("sentiment") {
        let request = trimmed.strip_prefix("sentiment").unwrap_or("").trim();
        run_sentiment_tool(request).await
    } else if trimmed.starts_with("entities") {
        let request = trimmed.strip_prefix("entities").unwrap_or("").trim();
        run_entity_tool(request).await
    } else if trimmed.starts_with("spell") {
        let request = trimmed.strip_prefix("spell").unwrap_or("").trim();
        run_spell_checker_tool(request).await
    } else if trimmed.starts_with("schedule") {
        let request = trimmed.strip_prefix("schedule").unwrap_or("").trim();
        run_scheduler_tool(request).await
    } else if trimmed.starts_with("memory") {
        let request = trimmed.strip_prefix("memory").unwrap_or("").trim();
        run_memory_tool(request).await
    } else {
        Err("Unknown tool. Use 'calculator', 'search', 'fetch', 'translate', 'sentiment', 'entities', 'spell', 'schedule', or 'memory'.".to_string())
    }
}

pub(crate) async fn run_chain_command(chain: (String, String)) -> Result<String, String> {
    let first = run_tool_command(&chain.0).await?;
    let second_input = if chain.1.trim().contains("$last") {
        chain.1.replace("$last", &first)
    } else {
        chain.1.clone()
    };
    let second = run_tool_command(&second_input).await?;
    Ok(format!("Step1:\n{}\n\nStep2:\n{}", first, second))
}

pub(crate) fn retry_last_query(default_top_k: usize) -> Result<AgentResponse, String> {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    if let Some(last_query) = &state.last_query {
        if let Some(retriever) = RETRIEVER.get() {
            let query_clone = last_query.clone();
            drop(state);
            let agent = Agent::new(
                "default",
                path_resolver::agent_db_path_str(),
                Arc::clone(retriever),
            );
            let response = agent.run(&query_clone, default_top_k);
            update_last_agent_run(query_clone, &response);
            Ok(response)
        } else {
            Err("Retriever not initialized".to_string())
        }
    } else {
        Err("No query to retry yet.".to_string())
    }
}

pub(crate) fn apply_undo() -> String {
    if let Some(action) = pop_undo_action() {
        match action {
            CommandAction::FocusSet(previous) => apply_focus(previous),
            CommandAction::PersonaSet(previous) => apply_persona(previous),
            CommandAction::VerbosityChanged(previous) => apply_verbosity(previous),
            CommandAction::ModelChanged(previous) => apply_model(previous),
            CommandAction::TemperatureChanged(previous) => apply_temperature(previous),
            CommandAction::NoteAdded(_) => "Last note removal not supported yet.".to_string(),
        }
    } else {
        "Nothing to undo.".to_string()
    }
}

pub(crate) fn render_dry_run_plan(plan: &str) -> String {
    store_dry_run_plan(plan.to_string());
    format!("Planned actions:\n{}", plan)
}

pub(crate) fn export_state() -> String {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    let payload = json!({
        "focus": state.focus_topic,
        "persona": state.persona,
        "verbosity": state.verbosity.label(),
        "model": state.preferred_model,
        "temperature": state.temperature,
        "last_query": state.last_query,
        "last_response": state.last_response,
        "dry_run_plan": state.dry_run_plan,
    });

    let export_root = env::var("AG_EXPORT_DIR").unwrap_or_else(|_| {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(".local/share/ag/exports").display().to_string()
    });

    if let Err(err) = fs::create_dir_all(&export_root) {
        return format!(
            "Exported in-memory only (failed to write {}): {}",
            export_root, err
        );
    }

    let filename = format!("export-{}.json", chrono::Utc::now().format("%Y%m%dT%H%M%S"));
    let path = Path::new(&export_root).join(filename);
    match fs::write(&path, payload.to_string()) {
        Ok(_) => format!("Exported to {}", path.display()),
        Err(err) => format!(
            "Exported in-memory only (failed to write {}): {}",
            path.display(),
            err
        ),
    }
}

pub(crate) fn import_state(raw: &str) -> String {
    if raw.trim().is_empty() {
        return "Provide JSON payload after /import.".to_string();
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => {
            if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
                record_model_change(if model.eq_ignore_ascii_case("default") {
                    None
                } else {
                    Some(model.to_string())
                });
            }
            if let Some(temp) = value.get("temperature").and_then(|v| v.as_f64()) {
                record_temperature_change(Some(temp as f32));
            }
            if let Some(focus) = value.get("focus").and_then(|v| v.as_str()) {
                record_focus_change(Some(focus.to_string()));
            }
            if let Some(persona) = value.get("persona").and_then(|v| v.as_str()) {
                record_persona_change(Some(persona.to_string()));
            }
            if let Some(verbosity) = value.get("verbosity").and_then(|v| v.as_str()) {
                let mode = match verbosity.to_lowercase().as_str() {
                    "brief" => Verbosity::Brief,
                    "verbose" => Verbosity::Verbose,
                    _ => Verbosity::Normal,
                };
                record_verbosity_change(mode);
            }
            "Import applied.".to_string()
        }
        Err(err) => format!("✗ Invalid import: {}", err),
    }
}

pub(crate) fn debug_state_snapshot() -> String {
    let (focus, persona, verbosity, last_query) = snapshots_for_debug();
    format!(
        "Debug State:\n- Focus: {:?}\n- Persona: {:?}\n- Verbosity: {:?}\n- Last query: {:?}",
        focus,
        persona,
        verbosity.label(),
        last_query
    )
}

pub(crate) fn tokens_usage_snapshot() -> String {
    let state_arc = chat_state();
    let state = state_arc.lock().expect("chat state lock");
    match state.last_token_usage {
        Some(tokens) => format!("Approximate token usage: {}", tokens),
        None => "No token usage recorded yet.".to_string(),
    }
}

pub(crate) async fn preview_url_content(url: &str) -> Result<String, String> {
    let tool = URLFetchTool::new();
    let query = format!("Fetch {}", url);
    let result = tool.execute(&query).await.map_err(|e| e.to_string())?;
    if result.success {
        Ok(format!("Learned from {}:\n{}", url, result.result))
    } else {
        Err(result.result)
    }
}

pub(crate) async fn run_agent(req: web::Json<AgentRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // Check for chat commands
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
    }

    let corpus_slug = req.corpus.as_deref().unwrap_or("default").to_string();
    if let Some(retriever) = get_corpus_retriever(&corpus_slug) {
        // Convert ChatMode to AgentMode
        let agent_mode = match req.mode {
            ChatMode::Rag => crate::agent::AgentMode::Rag,
            ChatMode::Llm => crate::agent::AgentMode::Llm,
            ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
            ChatMode::Auto => crate::agent::AgentMode::Auto,
            ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
            ChatMode::Agentic => crate::agent::AgentMode::Agentic,
            ChatMode::PointerRag => crate::agent::AgentMode::PointerRag,
        };
        let query_clone = req.query.clone();
        let top_k = req.top_k;

        // Get current chat settings
        let chat_settings = get_current_chat_settings();

        // Run agent in blocking thread pool to avoid blocking async runtime
        let resp = web::block(move || {
            let agent = Agent::new("default", path_resolver::agent_db_path_str(), retriever)
                .with_settings(chat_settings);
            agent.run_with_mode(&query_clone, top_k, agent_mode)
        })
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Agent error: {}", e)))?;

        update_last_agent_run(req.query.clone(), &resp);
        Ok(HttpResponse::Ok().json(json!({
            "response": resp,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

// GET-based chat endpoint to avoid CORS preflight
pub(crate) async fn run_agent_get(
    query: web::Query<AgentQueryParams>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // Check for chat commands
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
    }

    let corpus_slug = query.corpus.as_deref().unwrap_or("default").to_string();
    if let Some(retriever) = get_corpus_retriever(&corpus_slug) {
        // Convert ChatMode to AgentMode
        let agent_mode = match query.mode {
            ChatMode::Rag => crate::agent::AgentMode::Rag,
            ChatMode::Llm => crate::agent::AgentMode::Llm,
            ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
            ChatMode::Auto => crate::agent::AgentMode::Auto,
            ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
            ChatMode::Agentic => crate::agent::AgentMode::Agentic,
            ChatMode::PointerRag => crate::agent::AgentMode::PointerRag,
        };
        let query_str = query.query.clone();
        let top_k = query.top_k;

        // Get current chat settings
        let chat_settings = get_current_chat_settings();

        // Run agent in blocking thread pool to avoid blocking async runtime
        let resp = web::block(move || {
            let agent = Agent::new("default", path_resolver::agent_db_path_str(), retriever)
                .with_settings(chat_settings);
            agent.run_with_mode(&query_str, top_k, agent_mode)
        })
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Agent error: {}", e)))?;

        update_last_agent_run(query.query.clone(), &resp);
        Ok(HttpResponse::Ok().json(json!({
            "response": resp,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

// ============================================================================
// OPENAI STREAMING HANDLER
// ============================================================================

/// Stream response from an OpenAI-compatible API (OpenAI, OpenRouter, etc.)
/// Uses Server-Sent Events with format:
/// data: {"choices":[{"delta":{"content":"text"}}]}
/// data: [DONE]
pub(crate) async fn stream_openai_response(
    client: reqwest::Client,
    api_key: &str,
    model: &str,
    body: serde_json::Value,
    chunks_count: usize,
    request_id: String,
) -> Result<HttpResponse, Error> {
    stream_openai_compatible_response(
        client,
        "https://api.openai.com/v1/chat/completions",
        api_key,
        model,
        body,
        chunks_count,
        request_id,
        &[],
    )
    .await
}

/// Stream response from any OpenAI-compatible API endpoint
#[allow(clippy::too_many_arguments)]
pub(crate) async fn stream_openai_compatible_response(
    client: reqwest::Client,
    url: &str,
    api_key: &str,
    model: &str,
    body: serde_json::Value,
    chunks_count: usize,
    request_id: String,
    extra_headers: &[(&str, &str)],
) -> Result<HttpResponse, Error> {
    use actix_web::web::Bytes;
    use futures_util::stream::StreamExt;

    tracing::info!(
        model = %model,
        request_id = %request_id,
        url = %url,
        "Streaming from OpenAI-compatible API"
    );

    let mut req = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json");
    for (key, val) in extra_headers {
        req = req.header(*key, *val);
    }
    match req.json(&body).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("OpenAI-compatible API error: {} - {}", status, error_text);
                let error_response = serde_json::json!({
                    "type": "error",
                    "message": format!("API error: {} - {}", status, error_text),
                    "request_id": request_id
                });
                return Ok(HttpResponse::Ok()
                    .content_type("text/event-stream")
                    .body(format!("data: {}\n\n", error_response)));
            }

            let stream = response.bytes_stream().map(move |chunk_result| {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut output = String::new();

                        for line in text.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }

                            // OpenAI SSE format: "data: {...}" or "data: [DONE]"
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    let event = serde_json::json!({
                                        "type": "done",
                                        "chunks_used": chunks_count
                                    });
                                    output.push_str(&format!("data: {}\n\n", event));
                                } else if let Ok(json) =
                                    serde_json::from_str::<serde_json::Value>(data)
                                {
                                    // Extract content from choices[0].delta.content
                                    if let Some(content) = json
                                        .get("choices")
                                        .and_then(|c| c.get(0))
                                        .and_then(|c| c.get("delta"))
                                        .and_then(|d| d.get("content"))
                                        .and_then(|c| c.as_str())
                                    {
                                        if !content.is_empty() {
                                            let event = serde_json::json!({
                                                "type": "token",
                                                "content": content
                                            });
                                            output.push_str(&format!("data: {}\n\n", event));
                                        }
                                    }
                                }
                            }
                        }

                        Ok::<Bytes, actix_web::error::Error>(Bytes::from(output))
                    }
                    Err(e) => {
                        let error_event = serde_json::json!({
                            "type": "error",
                            "message": format!("Stream error: {}", e)
                        });
                        Ok(Bytes::from(format!("data: {}\n\n", error_event)))
                    }
                }
            });

            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("X-Accel-Buffering", "no"))
                .insert_header(("Access-Control-Allow-Origin", "*"))
                .streaming(stream))
        }
        Err(e) => {
            let error_response = serde_json::json!({
                "type": "error",
                "message": format!("Failed to connect to API: {}", e),
                "request_id": request_id
            });
            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .body(format!("data: {}\n\n", error_response)))
        }
    }
}

// ============================================================================
// ANTHROPIC STREAMING HANDLER
// ============================================================================

/// Stream response from Anthropic API
/// Anthropic uses Server-Sent Events with format:
/// event: content_block_delta
/// data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"content"}}
/// event: message_stop
/// data: {"type":"message_stop"}
#[allow(clippy::too_many_arguments)]
pub(crate) async fn stream_anthropic_response(
    client: reqwest::Client,
    api_key: &str,
    model: &str,
    payload: serde_json::Value,
    temperature: f32,
    max_tokens: usize,
    chunks_count: usize,
    request_id: String,
    use_caching: bool,
) -> Result<HttpResponse, Error> {
    use actix_web::web::Bytes;
    use futures_util::stream::StreamExt;

    let url = "https://api.anthropic.com/v1/messages";

    // Build the request body
    let system = payload
        .get("system")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let messages = payload
        .get("messages")
        .cloned()
        .unwrap_or(serde_json::json!([]));

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": true,
        "messages": messages
    });

    // Add system if present
    if !system.is_null() {
        body["system"] = system;
    }

    tracing::info!(
        model = %model,
        request_id = %request_id,
        caching = use_caching,
        "Streaming from Anthropic API"
    );

    let mut request = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json");

    // Add beta header for prompt caching if enabled
    if use_caching {
        request = request.header("anthropic-beta", "prompt-caching-2024-07-31");
    }

    match request.json(&body).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracing::error!("Anthropic API error: {} - {}", status, error_text);
                let error_response = serde_json::json!({
                    "type": "error",
                    "message": format!("Anthropic API error: {} - {}", status, error_text),
                    "request_id": request_id
                });
                return Ok(HttpResponse::Ok()
                    .content_type("text/event-stream")
                    .body(format!("data: {}\n\n", error_response)));
            }

            let stream = response.bytes_stream().map(move |chunk_result| {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut output = String::new();
                        let mut current_event_type = String::new();

                        for line in text.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }

                            // Anthropic SSE format: "event: type" followed by "data: {...}"
                            if let Some(event_type) = line.strip_prefix("event: ") {
                                current_event_type = event_type.to_string();
                            } else if let Some(data) = line.strip_prefix("data: ") {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                    match current_event_type.as_str() {
                                        "content_block_delta" => {
                                            // Extract text from delta.text
                                            if let Some(text_content) = json
                                                .get("delta")
                                                .and_then(|d| d.get("text"))
                                                .and_then(|t| t.as_str())
                                            {
                                                if !text_content.is_empty() {
                                                    let event = serde_json::json!({
                                                        "type": "token",
                                                        "content": text_content
                                                    });
                                                    output
                                                        .push_str(&format!("data: {}\n\n", event));
                                                }
                                            }
                                        }
                                        "message_stop" | "message_delta" => {
                                            // Check if this is the final message
                                            if json.get("type").and_then(|t| t.as_str())
                                                == Some("message_stop")
                                            {
                                                let event = serde_json::json!({
                                                    "type": "done",
                                                    "chunks_used": chunks_count
                                                });
                                                output.push_str(&format!("data: {}\n\n", event));
                                            }
                                        }
                                        "error" => {
                                            let error_msg = json
                                                .get("error")
                                                .and_then(|e| e.get("message"))
                                                .and_then(|m| m.as_str())
                                                .unwrap_or("Unknown error");
                                            let event = serde_json::json!({
                                                "type": "error",
                                                "message": error_msg
                                            });
                                            output.push_str(&format!("data: {}\n\n", event));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        Ok::<Bytes, actix_web::error::Error>(Bytes::from(output))
                    }
                    Err(e) => {
                        let error_event = serde_json::json!({
                            "type": "error",
                            "message": format!("Stream error: {}", e)
                        });
                        Ok(Bytes::from(format!("data: {}\n\n", error_event)))
                    }
                }
            });

            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("X-Accel-Buffering", "no"))
                .insert_header(("Access-Control-Allow-Origin", "*"))
                .streaming(stream))
        }
        Err(e) => {
            let error_response = serde_json::json!({
                "type": "error",
                "message": format!("Failed to connect to Anthropic: {}", e),
                "request_id": request_id
            });
            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .body(format!("data: {}\n\n", error_response)))
        }
    }
}

// Streaming agent endpoint using Server-Sent Events
pub(crate) async fn run_agent_stream(req: web::Json<AgentRequest>) -> Result<HttpResponse, Error> {
    use crate::memory::prompt_cache::CacheOptimizedPrompt;
    use actix_web::web::Bytes;
    use futures_util::stream::StreamExt;

    let request_id = generate_request_id();
    let stream_corpus_slug = req.corpus.as_deref().unwrap_or("default").to_string();

    // For commands, redirect to non-streaming endpoint (commands don't benefit from streaming)
    if parse_chat_command(&req.query).is_some() {
        // Just call the regular endpoint for commands
        return run_agent(req).await;
    }

    // Get hardware config to determine backend type
    let hardware_config = crate::db::param_hardware::global_config();
    let backend_type = hardware_config.backend_type;
    let prompt_caching = get_prompt_caching_enabled();
    let thread_count = hardware_config.num_thread.max(1);

    // Get Ollama config (used for Ollama backend)
    let ollama_url =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = if !hardware_config.model.is_empty() {
        hardware_config.model.clone()
    } else {
        std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "phi:latest".to_string())
    };

    // Determine mode and build context
    let agent_mode = match req.mode {
        ChatMode::Rag => crate::agent::AgentMode::Rag,
        ChatMode::Llm => crate::agent::AgentMode::Llm,
        ChatMode::Hybrid => crate::agent::AgentMode::Hybrid,
        ChatMode::Auto => crate::agent::AgentMode::Auto,
        ChatMode::RagStrict => crate::agent::AgentMode::RagStrict,
        ChatMode::Agentic => crate::agent::AgentMode::Agentic,
        ChatMode::PointerRag => crate::agent::AgentMode::PointerRag,
    };

    // Agentic mode: Rig-powered tool-calling loop with all tools
    if matches!(agent_mode, crate::agent::AgentMode::Agentic) {
        use crate::rig_tools::*;
        use rig::client::CompletionClient;
        use rig::completion::Prompt;
        use rig::providers::ollama;

        let preamble = "You are a helpful assistant with access to tools. \
                        Use search_documents to find information in the knowledge base. \
                        Use recall_memory to check previous conversations. \
                        Use store_memory to save important facts for later. \
                        Use search_knowledge_graph to find entity relationships. \
                        Call tools when the question might benefit from them. \
                        If tools return no results, answer from your own knowledge. \
                        Be concise and accurate.";

        // Token budget check via shimmytok
        let num_ctx = hardware_config.num_ctx;
        let (tokens_in, counter_exact) = {
            let prompt_text = format!("{}\n\n{}", preamble, req.query);
            match crate::api::get_token_counter() {
                Some(tc) => (tc.count_tokens(&prompt_text), tc.is_exact()),
                None => (prompt_text.split_whitespace().count() * 4 / 3, false),
            }
        };
        crate::monitoring::rig_stats::record_token_usage(tokens_in, num_ctx, counter_exact);

        let ctx_pct = if num_ctx > 0 {
            tokens_in as f64 / num_ctx as f64 * 100.0
        } else {
            0.0
        };
        if ctx_pct > 80.0 {
            tracing::warn!(
                tokens_in = tokens_in,
                num_ctx = num_ctx,
                ctx_pct = ctx_pct,
                "Agentic prompt is using {:.0}% of context budget before tool results",
                ctx_pct
            );
        }

        let rig_t0 = std::time::Instant::now();

        let rig_result: Result<String, String> = async {
            use crate::db::param_hardware::BackendType;
            let retriever_arc = RETRIEVER.get().map(std::sync::Arc::clone);

            if matches!(hardware_config.backend_type, BackendType::OpenAi) {
                use rig::providers::openai;
                let api_keys = crate::db::api_keys::global_config();
                let key = match api_keys.get_openai_key() {
                    Some(k) => k,
                    None => return Err("OpenAI API key not configured. Set OPENAI_API_KEY or add it in Hardware settings.".into()),
                };
                let client = openai::Client::new(&key)
                    .map_err(|e| format!("Failed to build OpenAI client: {}", e))?;
                let base = client.agent(&model).preamble(preamble);
                if let Some(ret) = retriever_arc {
                    base
                        .tool(TantivySearchTool::new(std::sync::Arc::clone(&ret), req.top_k))
                        .tool(MemoryRecallTool::new())
                        .tool(MemoryStoreTool::new())
                        .tool(GraphSearchTool::new(ret))
                        .build()
                        .prompt(&req.query)
                        .await
                        .map_err(|e| format!("Rig agent error: {}", e))
                } else {
                    base
                        .tool(MemoryRecallTool::new())
                        .tool(MemoryStoreTool::new())
                        .build()
                        .prompt(&req.query)
                        .await
                        .map_err(|e| format!("Rig agent error: {}", e))
                }
            } else {
                let client = ollama::Client::builder().api_key(rig::client::Nothing).base_url(&ollama_url).build().unwrap();
                let base = client.agent(&model).preamble(preamble);
                if let Some(ret) = retriever_arc {
                    base
                        .tool(TantivySearchTool::new(std::sync::Arc::clone(&ret), req.top_k))
                        .tool(MemoryRecallTool::new())
                        .tool(MemoryStoreTool::new())
                        .tool(GraphSearchTool::new(ret))
                        .build()
                        .prompt(&req.query)
                        .await
                        .map_err(|e| format!("Rig agent error: {}", e))
                } else {
                    base
                        .tool(MemoryRecallTool::new())
                        .tool(MemoryStoreTool::new())
                        .build()
                        .prompt(&req.query)
                        .await
                        .map_err(|e| format!("Rig agent error: {}", e))
                }
            }
        }.await;

        let rig_elapsed = rig_t0.elapsed().as_millis() as u64;

        match rig_result {
            Ok(text) => {
                crate::monitoring::rig_stats::record_agentic_call(rig_elapsed);
                crate::monitoring::record_tool_execution(
                    "AgenticSession",
                    &req.query,
                    true,
                    &format!("{}ms, ~{} tokens", rig_elapsed, tokens_in),
                    rig_elapsed,
                    1.0,
                    Some("rig_agentic"),
                );
                let json_response = serde_json::json!({
                    "response": text,
                    "model": model,
                    "mode": "agentic",
                    "tokens_in": tokens_in,
                    "ctx_pct": ctx_pct,
                    "done": true
                });
                return Ok(HttpResponse::Ok()
                    .content_type("text/event-stream")
                    .body(format!("data: {}\n\n", json_response)));
            }
            Err(e) => {
                tracing::warn!(error = %e, "Agentic mode failed — falling back to Hybrid");
                crate::monitoring::rig_stats::record_agentic_call(rig_elapsed);
                crate::monitoring::rig_stats::record_agentic_fallback();
                crate::monitoring::record_tool_execution(
                    "AgenticSession",
                    &req.query,
                    false,
                    &e,
                    rig_elapsed,
                    0.0,
                    Some("rig_agentic"),
                );
                // Fall through to Hybrid below — do NOT return here
                // Re-set agent_mode so the rest of the function handles it
                let _ = agent_mode; // silence unused warning
                                    // Run Hybrid inline as fallback
                if let Some(retriever) = get_corpus_retriever(&stream_corpus_slug) {
                    let query_clone = req.query.clone();
                    let top_k = req.top_k;
                    let chat_settings = get_current_chat_settings();
                    let resp = web::block(move || {
                        let agent =
                            Agent::new("default", path_resolver::agent_db_path_str(), retriever)
                                .with_settings(chat_settings);
                        agent.run_with_mode(&query_clone, top_k, crate::agent::AgentMode::Hybrid)
                    })
                    .await
                    .map_err(|err| {
                        actix_web::error::ErrorInternalServerError(format!(
                            "Fallback agent error: {}",
                            err
                        ))
                    })?;
                    update_last_agent_run(req.query.clone(), &resp);
                    let json_response = serde_json::json!({
                        "type": "complete",
                        "answer": resp.answer,
                        "steps": resp.steps,
                        "used_chunks": resp.used_chunks,
                        "mode": "agentic_fallback",
                        "fallback_reason": e,
                        "request_id": request_id
                    });
                    return Ok(HttpResponse::Ok()
                        .content_type("text/event-stream")
                        .body(format!("data: {}\n\n", json_response)));
                }
                // No retriever — return the error clearly
                let json_response = serde_json::json!({
                    "response": format!("Agentic mode unavailable: {}", e),
                    "mode": "agentic_error",
                    "done": true
                });
                return Ok(HttpResponse::Ok()
                    .content_type("text/event-stream")
                    .body(format!("data: {}\n\n", json_response)));
            }
        }
    }

    // For RAG-only mode, use non-streaming (document search doesn't benefit from streaming)
    if matches!(
        agent_mode,
        crate::agent::AgentMode::Rag
            | crate::agent::AgentMode::RagStrict
            | crate::agent::AgentMode::PointerRag
    ) {
        if let Some(retriever) = get_corpus_retriever(&stream_corpus_slug) {
            let query_clone = req.query.clone();
            let top_k = req.top_k;
            let chat_settings = get_current_chat_settings();
            let mode_for_run = agent_mode;

            let resp = web::block(move || {
                let agent = Agent::new("default", path_resolver::agent_db_path_str(), retriever)
                    .with_settings(chat_settings);
                agent.run_with_mode(&query_clone, top_k, mode_for_run)
            })
            .await
            .map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Agent error: {}", e))
            })?;

            update_last_agent_run(req.query.clone(), &resp);
            let json_response = serde_json::json!({
                "type": "complete",
                "answer": resp.answer,
                "steps": resp.steps,
                "used_chunks": resp.used_chunks,
                "request_id": request_id
            });
            return Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .body(format!("data: {}\n\n", json_response)));
        }
    }

    // For LLM and Hybrid modes, stream from Ollama
    let mut context = String::new();
    let mut used_chunks: Vec<String> = Vec::new();

    // For Hybrid/Auto mode, first get RAG context
    if matches!(
        agent_mode,
        crate::agent::AgentMode::Hybrid | crate::agent::AgentMode::Auto
    ) {
        let search_start = std::time::Instant::now();
        if let Some(retriever) = get_corpus_retriever(&stream_corpus_slug) {
            if let Ok(mut r) = retriever.lock() {
                if let Ok(mut results) = r.hybrid_search(&req.query, None) {
                    let search_time = search_start.elapsed().as_millis() as u64;
                    if results.len() > req.top_k {
                        results.truncate(req.top_k);
                    }
                    let result_count = results.len();
                    if !results.is_empty() {
                        context = results.join("\n\n");
                        used_chunks = results;
                    }
                    // Record tool execution
                    crate::monitoring::record_tool_execution(
                        "SemanticSearch",
                        &req.query,
                        true,
                        &format!("{} chunks", result_count),
                        search_time,
                        1.0,
                        Some("api/chat/stream"),
                    );
                }
            }
        }
    }

    // Get chat settings for prompt building
    let chat_settings = get_current_chat_settings();
    let system_prompt = chat_settings.build_system_prompt();

    // Debug: log what's in the system prompt
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

    // Build prompt with settings
    // Note: System prompt/instructions are sent via the 'system' field, not in the prompt
    // This prevents the LLM from echoing instructions back to the user
    let prompt = {
        let mut parts = Vec::new();

        // Add context if present, or fallback instruction for hybrid mode
        if !context.is_empty() {
            parts.push(format!(
                "Context (ignore if not relevant to the question):\n{}\n\nAnswer the question directly. If the context above is not relevant, use your own knowledge.",
                context
            ));
        } else if matches!(
            agent_mode,
            crate::agent::AgentMode::Hybrid | crate::agent::AgentMode::Auto
        ) {
            // Hybrid/Auto mode with no context: tell LLM to answer from its knowledge
            parts.push("Answer the question based on your knowledge.".to_string());
        }

        // Add question
        parts.push(format!("Question: {}", req.query));
        parts.push("Answer:".to_string());

        parts.join("\n\n")
    };

    // Get mode-specific config
    use crate::db::llm_settings::LlmConfig;
    let mut config = match agent_mode {
        crate::agent::AgentMode::Rag
        | crate::agent::AgentMode::RagStrict
        | crate::agent::AgentMode::PointerRag => LlmConfig::documents_only(),
        crate::agent::AgentMode::Llm => LlmConfig::llm_only(),
        crate::agent::AgentMode::Hybrid | crate::agent::AgentMode::Auto => LlmConfig::combined(),
        crate::agent::AgentMode::Agentic => LlmConfig::combined(),
    };

    // Apply temperature override if set
    if let Some(temp) = chat_settings.temperature {
        config.temperature = temp;
    }

    // Use model override if set
    let final_model = chat_settings.model.unwrap_or(model);

    // Build cache-optimized prompt structure
    let cache_prompt = CacheOptimizedPrompt::new()
        .with_system_prompt(&system_prompt)
        .with_context(&context)
        .with_user_query(&req.query);

    // Create streaming request based on backend type and caching preference
    let client = reqwest::Client::new();
    let chunks_count = used_chunks.len();

    // Determine URL and request body based on backend and caching
    let (url, request_body) = match backend_type {
        crate::db::param_hardware::BackendType::Ollama => {
            if prompt_caching {
                // Use /api/chat for KV cache reuse
                let options = serde_json::json!({
                    "temperature": config.temperature,
                    "top_p": config.top_p,
                    "top_k": config.top_k,
                    "num_predict": config.max_tokens,
                    "repeat_penalty": config.repeat_penalty,
                    "num_thread": thread_count,
                    "num_ctx": hardware_config.num_ctx
                });
                let body =
                    cache_prompt.build_ollama_chat_request(&final_model, true, Some(options));
                (format!("{}/api/chat", ollama_url), body)
            } else {
                // Use /api/generate (no caching)
                let body = serde_json::json!({
                    "model": final_model,
                    "prompt": prompt,
                    "stream": true,
                    "options": {
                        "temperature": config.temperature,
                        "top_p": config.top_p,
                        "top_k": config.top_k,
                        "num_predict": config.max_tokens,
                        "repeat_penalty": config.repeat_penalty,
                        "num_thread": thread_count,
                        "num_ctx": hardware_config.num_ctx
                    },
                    "system": if system_prompt.is_empty() { serde_json::Value::Null } else { serde_json::json!(system_prompt) }
                });
                (format!("{}/api/generate", ollama_url), body)
            }
        }
        crate::db::param_hardware::BackendType::OpenAi => {
            // OpenAI API with automatic prefix caching
            let api_key = std::env::var("OPENAI_API_KEY")
                .unwrap_or_else(|_| crate::db::api_keys::global_config().openai_api_key.clone());

            if api_key.is_empty() {
                tracing::warn!("OpenAI API key not configured, falling back to Ollama");
                let fallback_body = serde_json::json!({
                    "model": final_model,
                    "prompt": prompt,
                    "stream": true,
                    "options": {
                        "temperature": config.temperature,
                        "top_p": config.top_p,
                        "top_k": config.top_k,
                        "num_predict": config.max_tokens,
                        "repeat_penalty": config.repeat_penalty,
                        "num_thread": thread_count
                    }
                });
                (format!("{}/api/generate", ollama_url), fallback_body)
            } else {
                let messages = if prompt_caching {
                    cache_prompt.build_openai_messages()
                } else {
                    vec![
                        serde_json::json!({"role": "system", "content": system_prompt}),
                        serde_json::json!({"role": "user", "content": format!("{}\n\nQuestion: {}", context, req.query)}),
                    ]
                };
                let body = serde_json::json!({
                    "model": final_model,
                    "messages": messages,
                    "stream": true,
                    "temperature": config.temperature,
                    "max_tokens": config.max_tokens
                });
                // Return special marker for OpenAI handling
                return stream_openai_response(
                    client,
                    &api_key,
                    &final_model,
                    body,
                    chunks_count,
                    request_id,
                )
                .await;
            }
        }
        crate::db::param_hardware::BackendType::Anthropic => {
            // Anthropic API with explicit cache_control
            let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| {
                crate::db::api_keys::global_config()
                    .anthropic_api_key
                    .clone()
            });

            if api_key.is_empty() {
                tracing::warn!("Anthropic API key not configured, falling back to Ollama");
                let fallback_body = serde_json::json!({
                    "model": final_model,
                    "prompt": prompt,
                    "stream": true,
                    "options": {
                        "temperature": config.temperature,
                        "top_p": config.top_p,
                        "top_k": config.top_k,
                        "num_predict": config.max_tokens,
                        "repeat_penalty": config.repeat_penalty,
                        "num_thread": thread_count
                    }
                });
                (format!("{}/api/generate", ollama_url), fallback_body)
            } else {
                let anthropic_payload = if prompt_caching {
                    cache_prompt.build_anthropic_messages()
                } else {
                    serde_json::json!({
                        "system": system_prompt,
                        "messages": [{
                            "role": "user",
                            "content": format!("{}\n\nQuestion: {}", context, req.query)
                        }]
                    })
                };
                // Return special marker for Anthropic handling
                return stream_anthropic_response(
                    client,
                    &api_key,
                    &final_model,
                    anthropic_payload,
                    config.temperature,
                    config.max_tokens,
                    chunks_count,
                    request_id,
                    prompt_caching,
                )
                .await;
            }
        }
        crate::db::param_hardware::BackendType::OpenRouter => {
            // OpenRouter: OpenAI-compatible API gateway
            let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
                crate::db::api_keys::global_config()
                    .openrouter_api_key
                    .clone()
            });

            if api_key.is_empty() {
                tracing::warn!("OpenRouter API key not configured, falling back to Ollama");
                let fallback_body = serde_json::json!({
                    "model": final_model,
                    "prompt": prompt,
                    "stream": true,
                    "options": {
                        "temperature": config.temperature,
                        "top_p": config.top_p,
                        "top_k": config.top_k,
                        "num_predict": config.max_tokens,
                        "repeat_penalty": config.repeat_penalty,
                        "num_thread": thread_count
                    }
                });
                (format!("{}/api/generate", ollama_url), fallback_body)
            } else {
                let messages = if prompt_caching {
                    cache_prompt.build_openai_messages()
                } else {
                    vec![
                        serde_json::json!({"role": "system", "content": system_prompt}),
                        serde_json::json!({"role": "user", "content": format!("{}\n\nQuestion: {}", context, req.query)}),
                    ]
                };
                let body = serde_json::json!({
                    "model": final_model,
                    "messages": messages,
                    "stream": true,
                    "temperature": config.temperature,
                    "max_tokens": config.max_tokens
                });
                return stream_openai_compatible_response(
                    client,
                    "https://openrouter.ai/api/v1/chat/completions",
                    &api_key,
                    &final_model,
                    body,
                    chunks_count,
                    request_id,
                    &[("HTTP-Referer", "https://github.com/pde/ag")],
                )
                .await;
            }
        }
        crate::db::param_hardware::BackendType::LlamaCpp => {
            // llama-server: OpenAI-compatible streaming + llama.cpp sampler extensions
            let llama_url = hardware_config.llama_server_url.clone();
            let mut messages = Vec::new();
            if !system_prompt.is_empty() {
                messages.push(serde_json::json!({"role": "system", "content": system_prompt}));
            }
            messages.push(serde_json::json!({"role": "user", "content": prompt}));
            let mut body = serde_json::json!({
                "model": final_model,
                "messages": messages,
                "stream": true,
                "temperature": config.temperature,
                "max_tokens": config.max_tokens,
                "top_p": config.top_p,
                "top_k": config.top_k,
                "min_p": config.min_p,
                "frequency_penalty": config.frequency_penalty,
                "presence_penalty": config.presence_penalty,
                "repeat_penalty": config.repeat_penalty,
                "typical_p": config.typical_p,
                "tfs_z": config.tfs_z
            });
            if let Some(seed) = config.seed {
                body["seed"] = serde_json::json!(seed);
            }
            if !config.stop_sequences.is_empty() {
                body["stop"] = serde_json::json!(config.stop_sequences);
            }
            (format!("{}/v1/chat/completions", llama_url), body)
        }
        _ => {
            // Default to Ollama /api/generate for other backends
            let body = serde_json::json!({
                "model": final_model,
                "prompt": prompt,
                "stream": true,
                "options": {
                    "temperature": config.temperature,
                    "top_p": config.top_p,
                    "top_k": config.top_k,
                    "num_predict": config.max_tokens,
                    "repeat_penalty": config.repeat_penalty,
                    "num_thread": thread_count
                },
                "system": if system_prompt.is_empty() { serde_json::Value::Null } else { serde_json::json!(system_prompt) }
            });
            (format!("{}/api/generate", ollama_url), body)
        }
    };

    tracing::warn!(
        backend = ?backend_type,
        caching = prompt_caching,
        url = %url,
        "DEBUG Sending LLM request"
    );

    match client.post(&url).json(&request_body).send().await {
        Ok(response) => {
            let stream = response.bytes_stream().map(move |chunk_result| {
                match chunk_result {
                    Ok(bytes) => {
                        // Parse Ollama's streaming response (newline-delimited JSON)
                        // Handles both /api/generate ("response" field) and /api/chat ("message.content" field)
                        let text = String::from_utf8_lossy(&bytes);
                        let mut output = String::new();

                        for line in text.lines() {
                            if line.is_empty() {
                                continue;
                            }
                            // Handle OpenAI SSE format: "data: {...}" or "data: [DONE]"
                            let json_str = if let Some(payload) = line.strip_prefix("data: ") {
                                if payload == "[DONE]" {
                                    let event = serde_json::json!({
                                        "type": "done",
                                        "chunks_used": chunks_count
                                    });
                                    output.push_str(&format!("data: {}\n\n", event));
                                    continue;
                                }
                                payload
                            } else {
                                line
                            };
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                                // Try OpenAI streaming format (choices[0].delta.content)
                                let response_text = json
                                    .get("choices")
                                    .and_then(|c| c.as_array())
                                    .and_then(|c| c.first())
                                    .and_then(|c| c.get("delta"))
                                    .and_then(|d| d.get("content"))
                                    .and_then(|v| v.as_str())
                                    // Try /api/generate format ("response" field)
                                    .or_else(|| json.get("response").and_then(|v| v.as_str()))
                                    // Try /api/chat format ("message.content" field)
                                    .or_else(|| {
                                        json.get("message")
                                            .and_then(|m| m.get("content"))
                                            .and_then(|c| c.as_str())
                                    });

                                if let Some(text) = response_text {
                                    if !text.is_empty() {
                                        let event = serde_json::json!({
                                            "type": "token",
                                            "content": text
                                        });
                                        output.push_str(&format!("data: {}\n\n", event));
                                    }
                                }
                                if json.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                                    let event = serde_json::json!({
                                        "type": "done",
                                        "chunks_used": chunks_count
                                    });
                                    output.push_str(&format!("data: {}\n\n", event));
                                }
                            }
                        }

                        Ok::<Bytes, actix_web::error::Error>(Bytes::from(output))
                    }
                    Err(e) => {
                        let error_event = serde_json::json!({
                            "type": "error",
                            "message": format!("Stream error: {}", e)
                        });
                        Ok(Bytes::from(format!("data: {}\n\n", error_event)))
                    }
                }
            });

            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .insert_header(("Cache-Control", "no-cache"))
                .insert_header(("X-Accel-Buffering", "no"))
                .insert_header(("Access-Control-Allow-Origin", "*"))
                .streaming(stream))
        }
        Err(e) => {
            let error_response = serde_json::json!({
                "type": "error",
                "message": format!("Failed to connect to LLM backend: {}", e),
                "request_id": request_id
            });
            Ok(HttpResponse::Ok()
                .content_type("text/event-stream")
                .body(format!("data: {}\n\n", error_response)))
        }
    }
}
