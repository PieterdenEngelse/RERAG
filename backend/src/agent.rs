use crate::agent_memory::AgentMemory;
use crate::db::path_resolver;
use crate::retriever::Retriever;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Fragmentation signal for a set of matched chunks.
///
/// Two ratios are tracked because they answer different questions on
/// real corpora:
///
/// - `section_ratio = unique_sections / tracked` — how spread the
///   retrieval is across distinct *section* boundaries. This is the
///   pp4-original "PointerRag wheelhouse" signal: high values mean
///   matches scatter across many sections (Pointer's case).
/// - `doc_ratio = unique_docs / tracked` — how spread the retrieval
///   is across distinct *documents*. On corpora where most files are
///   single-section (e.g. short header-less markdown), `section_ratio`
///   collapses to `doc_ratio` and the within-doc signal vanishes.
///
/// The *gap* `section_ratio − doc_ratio` is the within-document
/// fragmentation: any value above zero means at least one document
/// contributed multiple sections to the retrieval. Invariant:
/// `unique_sections ≥ unique_docs` (sections are nested in docs and
/// `section_id` is a UUID generated per boundary).
///
/// Both ratios are `None` when no chunks were tracked (older index, or
/// every meta lookup returned `None`). `None` is preserved instead of
/// synthesizing `NaN` or `0.0` so callers can distinguish "no signal"
/// from "real low fragmentation".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FragmentationStats {
    pub tracked: usize,
    pub untracked: usize,
    pub unique_sections: usize,
    pub unique_docs: usize,
    pub section_ratio: Option<f32>,
    pub doc_ratio: Option<f32>,
}

/// Compute the fragmentation signal over a set of chunk contents.
///
/// `lookup` returns `Some((section_id, doc_id))` for chunks the caller
/// could associate with a source, and `None` otherwise (missing meta,
/// retriever lock unavailable — the helper doesn't care which). An
/// empty `section_id` is treated as untracked (the chunk lacks a usable
/// section boundary); an empty `doc_id` while `section_id` is present
/// still counts the chunk as tracked but skips the docs-set insert.
///
/// Pure over its inputs — no `Mutex`, no IO. Tests pass a synthetic
/// closure; the Auto arm passes a closure that delegates to
/// `Retriever::meta_for_content` + `Retriever::doc_id_for_content`.
pub fn fragmentation(
    chunks: &[String],
    lookup: impl Fn(&str) -> Option<(String, String)>,
) -> FragmentationStats {
    let mut tracked = 0usize;
    let mut untracked = 0usize;
    let mut sections: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut docs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for chunk in chunks {
        match lookup(chunk) {
            Some((section_id, doc_id)) if !section_id.is_empty() => {
                tracked += 1;
                sections.insert(section_id);
                if !doc_id.is_empty() {
                    docs.insert(doc_id);
                }
            }
            _ => {
                untracked += 1;
            }
        }
    }
    let unique_sections = sections.len();
    let unique_docs = docs.len();
    let (section_ratio, doc_ratio) = if tracked > 0 {
        (
            Some(unique_sections as f32 / tracked as f32),
            Some(unique_docs as f32 / tracked as f32),
        )
    } else {
        (None, None)
    };
    FragmentationStats {
        tracked,
        untracked,
        unique_sections,
        unique_docs,
        section_ratio,
        doc_ratio,
    }
}

/// Format the fragmentation ratio for log output. `None` (no tracked
/// chunks) becomes `"unknown"` so it's never confusable with a real
/// low-fragmentation `0.000`.
fn format_ratio(ratio: Option<f32>) -> String {
    ratio
        .map(|x| format!("{x:.3}"))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Default for the Auto→PointerRag routing threshold. The "gap" is
/// `section_ratio - doc_ratio`: a measure of how much of the
/// retrieval's spread is *within* documents (Pointer's wheelhouse)
/// versus *across* documents (Strict/Hybrid territory). Overridable
/// at runtime via the `POINTERRAG_AUTO_GAP_THRESHOLD` setting key.
///
/// Picked from the n=13 corpus analysis in `docs/pp-conclusion.md`:
/// queries above 0.5 cluster as "PointerRag would help"; queries
/// below cluster as "Strict/Hybrid is fine." Provisional — re-tune
/// when the corpus shape changes.
pub const POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT: f64 = 0.5;

/// The three routing outcomes Auto can pick for a query with chunks.
/// `run_with_mode` consults `auto_route` to map observed retrieval
/// into one of these, then takes the corresponding action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoRoute {
    /// Within-doc fragmentation gap met threshold — hydrate full
    /// sections via `hydrate_pointer_sections` and answer with strict
    /// grounding.
    PointerHydration,
    /// Coherent retrieval with enough material — strict grounded RAG
    /// over the raw chunks.
    Strict,
    /// Coherent retrieval with not-enough material — fall back to
    /// Hybrid (LLM + whatever context exists).
    Hybrid,
}

/// Pure routing decision for `AgentMode::Auto`. Split out from
/// `run_with_mode` so the rules can be unit-tested without standing up
/// an Agent + Retriever + DB.
///
/// Rules (pp4 Phase 2 Step 3):
/// 1. `frag_gap = section_ratio - doc_ratio ≥ gap_threshold`
///    → [`AutoRoute::PointerHydration`] regardless of confidence.
///    **Inclusive** on the boundary: gap == threshold also routes to
///    Pointer.
/// 2. Otherwise, `chunk_count ≥ 3 && est_tokens ≥ 1536`
///    → [`AutoRoute::Strict`].
/// 3. Otherwise → [`AutoRoute::Hybrid`].
///
/// When fragmentation is unmeasurable (all chunks untracked — older
/// index without `section_id`s, retriever lock unavailable),
/// `frag.section_ratio` is `None`; the gap collapses to `0.0` and the
/// Pointer route only fires when the threshold is also `≤ 0.0`.
pub fn auto_route(
    frag: FragmentationStats,
    gap_threshold: f64,
    chunk_count: usize,
    est_tokens: usize,
) -> AutoRoute {
    let frag_gap = frag
        .section_ratio
        .zip(frag.doc_ratio)
        .map(|(s, d)| (s - d) as f64)
        .unwrap_or(0.0);
    if frag_gap >= gap_threshold {
        AutoRoute::PointerHydration
    } else if chunk_count >= 3 && est_tokens >= 1536 {
        AutoRoute::Strict
    } else {
        AutoRoute::Hybrid
    }
}

/// Result of hydrating a chunk set into full sections via the
/// retriever. Produced by `hydrate_pointer_sections`, called from the
/// Auto→Pointer routing decision; the stats are surfaced in the step
/// trace so the user can see how many sections actually came back
/// versus fell back to raw text.
pub(crate) struct PointerHydration {
    pub(crate) context: String,
    pub(crate) hydrated: usize,
    pub(crate) fb_no_section_id: usize,
    pub(crate) fb_fetch_empty: usize,
    pub(crate) fb_lock_failed: usize,
}

impl PointerHydration {
    fn total_fallbacks(&self) -> usize {
        self.fb_no_section_id + self.fb_fetch_empty + self.fb_lock_failed
    }
}

/// Build the fragmentation suffix appended to the Auto-mode step-trace
/// message. Format is locked by tests so step-trace consumers (UI, log
/// scrapers) can parse it. Includes both ratios + the raw counts so a
/// reader can see (a) the section-spread signal pp4 cares about and
/// (b) how much of it is "really" within-doc spread vs cross-doc.
///
/// Returns the suffix with a leading space — callers concatenate it
/// directly onto the existing message.
fn fragmentation_suffix(frag: FragmentationStats) -> String {
    let section = format_ratio(frag.section_ratio);
    let doc = format_ratio(frag.doc_ratio);
    let total = frag.tracked + frag.untracked;
    format!(
        " (fragmentation: section {section}, doc {doc}, {}/{} chunks tracked, {} sections / {} docs)",
        frag.tracked, total, frag.unique_sections, frag.unique_docs
    )
}

/// Emit the Auto-mode fragmentation observability log. Extracted as a
/// free function so the structured fields can be asserted with a
/// `tracing_subscriber::fmt` writer in tests without standing up an
/// Agent + Retriever. Called exactly once per `AgentMode::Auto` query,
/// before the high/low-confidence fork.
fn log_auto_fragmentation(chunks: usize, frag: FragmentationStats, est_tokens: usize) {
    let section_ratio_str = format_ratio(frag.section_ratio);
    let doc_ratio_str = format_ratio(frag.doc_ratio);
    tracing::info!(
        chunks = chunks,
        tracked = frag.tracked,
        untracked = frag.untracked,
        unique_sections = frag.unique_sections,
        unique_docs = frag.unique_docs,
        section_ratio = %section_ratio_str,
        doc_ratio = %doc_ratio_str,
        est_tokens = est_tokens,
        "Auto mode: fragmentation signal"
    );
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentStep {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentResponse {
    pub answer: String,
    pub steps: Vec<AgentStep>,
    pub used_chunks: Vec<String>,
}

/// Chat mode for agent queries
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// Only search documents (RAG)
    Rag,
    /// Only use LLM (no document search)
    Llm,
    /// Combine: search documents + LLM fallback/enhancement
    #[default]
    Hybrid,
    /// Prefer strict grounded RAG when retrieval is strong, else Hybrid
    Auto,
    /// Strict grounded RAG: LLM answers only from retrieved context.
    /// If no chunks found, says "I don't know" (no LLM fallback).
    RagStrict,
    /// Agentic mode: LLM decides which tools to call in a loop (Rig)
    Agentic,
}

/// Verbosity level for responses
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Verbosity {
    Brief,
    #[default]
    Normal,
    Verbose,
}

impl Verbosity {
    pub fn label(&self) -> &'static str {
        match self {
            Verbosity::Brief => "brief",
            Verbosity::Normal => "normal",
            Verbosity::Verbose => "verbose",
        }
    }
}

/// Memory priority for prompt injection
/// Higher priority memories are injected first and given more weight
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPriority {
    /// Highest: Direct instructions to follow
    Instruction = 0,
    /// High: User preferences to respect
    Preference = 1,
    /// Medium-High: Persona definitions
    Persona = 2,
    /// Medium: Contextual information
    Context = 3,
    /// Medium-Low: Factual information
    Fact = 4,
    /// Low: Summaries and notes
    Summary = 5,
    /// Lowest: Other memory types
    Other = 6,
}

impl MemoryPriority {
    pub fn from_memory_type(memory_type: &str) -> Self {
        match memory_type.to_lowercase().as_str() {
            "instruction" => MemoryPriority::Instruction,
            "preference" => MemoryPriority::Preference,
            "persona" => MemoryPriority::Persona,
            "context" => MemoryPriority::Context,
            "fact" => MemoryPriority::Fact,
            "summary" | "note" => MemoryPriority::Summary,
            _ => MemoryPriority::Other,
        }
    }
}

/// Categorized memory for prompt building
#[derive(Debug, Clone)]
pub struct CategorizedMemory {
    pub memory_type: String,
    pub content: String,
    pub priority: MemoryPriority,
}

/// Chat settings that affect prompt generation
#[derive(Debug, Clone, Default)]
pub struct ChatSettings {
    /// Focus topic - narrows responses to this topic
    pub focus_topic: Option<String>,
    /// Persona - changes the assistant's personality/style
    pub persona: Option<String>,
    /// Verbosity - controls response length
    pub verbosity: Verbosity,
    /// Custom temperature override
    pub temperature: Option<f32>,
    /// Custom model override
    pub model: Option<String>,
    /// RAG memories to inject into prompt
    pub memories: Vec<CategorizedMemory>,
}

impl ChatSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_focus(mut self, topic: Option<String>) -> Self {
        self.focus_topic = topic;
        self
    }

    pub fn with_persona(mut self, persona: Option<String>) -> Self {
        self.persona = persona;
        self
    }

    pub fn with_verbosity(mut self, verbosity: Verbosity) -> Self {
        self.verbosity = verbosity;
        self
    }

    pub fn with_temperature(mut self, temp: Option<f32>) -> Self {
        self.temperature = temp;
        self
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    pub fn with_memories(mut self, memories: Vec<CategorizedMemory>) -> Self {
        self.memories = memories;
        self
    }

    /// Build system prompt prefix based on settings and memories
    /// Priority order:
    /// 1. Instructions (from memories)
    /// 2. Persona (from settings or memories)
    /// 3. Preferences (from memories)
    /// 4. Focus (from settings)
    /// 5. Verbosity (from settings)
    /// 6. Context/Facts (from memories)
    pub fn build_system_prompt(&self) -> String {
        let mut parts = Vec::new();

        // Sort memories by priority
        let mut sorted_memories = self.memories.clone();
        sorted_memories.sort_by_key(|m| m.priority);

        // 1. Add instruction memories (highest priority - these are directives)
        let instructions: Vec<&CategorizedMemory> = sorted_memories
            .iter()
            .filter(|m| m.priority == MemoryPriority::Instruction)
            .collect();
        if !instructions.is_empty() {
            let instruction_text: Vec<String> = instructions
                .iter()
                .map(|m| format!("• {}", m.content))
                .collect();
            parts.push(format!(
                "INSTRUCTIONS (follow these directives):\n{}",
                instruction_text.join("\n")
            ));
        }

        // 2. Add persona (from settings first, then from memories)
        if let Some(persona) = &self.persona {
            parts.push(format!(
                "PERSONA: You are acting as '{}'. Adopt this persona's communication style, expertise, and perspective.",
                persona
            ));
        } else {
            // Check for persona in memories
            let persona_memories: Vec<&CategorizedMemory> = sorted_memories
                .iter()
                .filter(|m| m.priority == MemoryPriority::Persona)
                .collect();
            if !persona_memories.is_empty() {
                let persona_text: Vec<String> =
                    persona_memories.iter().map(|m| m.content.clone()).collect();
                parts.push(format!("PERSONA: {}", persona_text.join(" ")));
            }
        }

        // 3. Add preference memories
        let preferences: Vec<&CategorizedMemory> = sorted_memories
            .iter()
            .filter(|m| m.priority == MemoryPriority::Preference)
            .collect();
        if !preferences.is_empty() {
            let pref_text: Vec<String> = preferences
                .iter()
                .map(|m| format!("• {}", m.content))
                .collect();
            parts.push(format!(
                "USER PREFERENCES (respect these when responding):\n{}",
                pref_text.join("\n")
            ));
        }

        // 4. Add focus instruction (from settings)
        if let Some(focus) = &self.focus_topic {
            parts.push(format!(
                "FOCUS: Prioritize information related to '{}'. Filter out unrelated details.",
                focus
            ));
        }

        // 5. Add verbosity instruction (from settings)
        match self.verbosity {
            Verbosity::Brief => {
                parts.push("RESPONSE STYLE: Be concise and brief. Give short, direct answers. Aim for 1-3 sentences.".to_string());
            }
            Verbosity::Normal => {
                // Default - no special instruction needed
            }
            Verbosity::Verbose => {
                parts.push("RESPONSE STYLE: Provide detailed, comprehensive responses. Include explanations, examples, and relevant context.".to_string());
            }
        }

        // 6. Add context memories (informational, not directive)
        let context_memories: Vec<&CategorizedMemory> = sorted_memories
            .iter()
            .filter(|m| m.priority == MemoryPriority::Context)
            .collect();
        if !context_memories.is_empty() {
            let ctx_text: Vec<String> = context_memories
                .iter()
                .map(|m| format!("• {}", m.content))
                .collect();
            parts.push(format!(
                "CONTEXT (background information):\n{}",
                ctx_text.join("\n")
            ));
        }

        // 7. Add fact memories (informational)
        let facts: Vec<&CategorizedMemory> = sorted_memories
            .iter()
            .filter(|m| m.priority == MemoryPriority::Fact)
            .collect();
        if !facts.is_empty() {
            let fact_text: Vec<String> = facts.iter().map(|m| format!("• {}", m.content)).collect();
            parts.push(format!("KNOWN FACTS:\n{}", fact_text.join("\n")));
        }

        // 8. Add summary/note memories (lowest priority informational)
        let summaries: Vec<&CategorizedMemory> = sorted_memories
            .iter()
            .filter(|m| {
                m.priority == MemoryPriority::Summary || m.priority == MemoryPriority::Other
            })
            .take(3) // Limit to avoid prompt bloat
            .collect();
        if !summaries.is_empty() {
            let sum_text: Vec<String> = summaries
                .iter()
                .map(|m| format!("• {}", m.content))
                .collect();
            parts.push(format!("NOTES:\n{}", sum_text.join("\n")));
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join("\n\n")
        }
    }

    /// Check if any settings are active
    pub fn has_active_settings(&self) -> bool {
        self.focus_topic.is_some()
            || self.persona.is_some()
            || self.verbosity != Verbosity::Normal
            || !self.memories.is_empty()
    }
}

/// Safety filter for memory content
/// Returns true if the memory content is safe to inject
pub fn is_safe_memory_content(content: &str) -> bool {
    let content_lower = content.to_lowercase();

    // Reject memories that look like injection attempts
    let dangerous_patterns = [
        "ignore previous",
        "ignore all",
        "disregard",
        "forget everything",
        "new instructions",
        "override",
        "system prompt",
        "jailbreak",
        "bypass",
        "pretend you",
        "act as if",
        "roleplay as",
        "you are now",
        "from now on",
        "ignore safety",
        "ignore rules",
        "ignore guidelines",
        "do not follow",
        "don't follow",
    ];

    for pattern in dangerous_patterns {
        if content_lower.contains(pattern) {
            return false;
        }
    }

    // Reject very long memories (potential prompt injection)
    if content.len() > 1000 {
        return false;
    }

    // Reject memories with suspicious characters
    if content.contains("```") && content.contains("system") {
        return false;
    }

    true
}

/// Load and categorize memories from the database
pub fn load_categorized_memories(
    db_path: &str,
    agent_id: &str,
    limit: usize,
) -> Vec<CategorizedMemory> {
    if let Some(hit) = MEMORY_CACHE.lookup(agent_id, limit) {
        return hit;
    }

    let resolved_path = path_resolver::resolve_db_path(db_path);
    let resolved_string = resolved_path.to_string_lossy().into_owned();
    let mut memories = Vec::new();

    if let Ok(mem) = AgentMemory::new(&resolved_string) {
        if let Ok(items) = mem.recall_rag(agent_id, limit) {
            for item in items {
                // Apply safety filter
                if !is_safe_memory_content(&item.content) {
                    continue;
                }

                let priority = MemoryPriority::from_memory_type(&item.memory_type);
                memories.push(CategorizedMemory {
                    memory_type: item.memory_type,
                    content: item.content,
                    priority,
                });
            }
        }
    }

    MEMORY_CACHE.insert(agent_id, limit, memories.clone());
    memories
}

/// Bump the cache so the next `load_categorized_memories` call re-reads
/// SQLite. Cheap to call from any write path; cache misses are O(1).
pub fn invalidate_memory_cache(agent_id: &str) {
    MEMORY_CACHE.invalidate(agent_id);
}

// In-process cache for categorized memories, keyed by agent_id. Loading
// these used to open a fresh SQLite connection and run ~6 CREATE TABLE
// IF NOT EXISTS statements on every chat turn (see AgentMemory::new) —
// this collapses that work to a single Mutex<HashMap> read for the
// repeated case. Invalidation is comprehensive (every rag_memory writer
// calls `invalidate_memory_cache`), so the TTL only exists as a backstop
// in case a future writer is added and forgets the invalidate call.
const MEMORY_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

struct MemoryCacheEntry {
    inserted_at: std::time::Instant,
    limit: usize,
    memories: Vec<CategorizedMemory>,
}

struct MemoryCacheInner {
    map: std::sync::Mutex<std::collections::HashMap<String, MemoryCacheEntry>>,
}

impl MemoryCacheInner {
    // Note: cache key is agent_id only, not (agent_id, db_path). Safe
    // because every caller resolves the path through
    // `path_resolver::agent_db_path_str()` and gets the same value.
    // If a future caller passes a different db_path for the same
    // agent_id, this cache will return rows from the original db.
    fn lookup(&self, agent_id: &str, limit: usize) -> Option<Vec<CategorizedMemory>> {
        let map = self.map.lock().ok()?;
        let entry = map.get(agent_id)?;
        // A request with a larger limit may legitimately want more rows
        // than the cached entry contains; force a refresh in that case.
        if entry.limit < limit {
            return None;
        }
        if entry.inserted_at.elapsed() > MEMORY_CACHE_TTL {
            return None;
        }
        Some(entry.memories.clone())
    }

    fn insert(&self, agent_id: &str, limit: usize, memories: Vec<CategorizedMemory>) {
        if let Ok(mut map) = self.map.lock() {
            map.insert(
                agent_id.to_string(),
                MemoryCacheEntry {
                    inserted_at: std::time::Instant::now(),
                    limit,
                    memories,
                },
            );
        }
    }

    fn invalidate(&self, agent_id: &str) {
        if let Ok(mut map) = self.map.lock() {
            map.remove(agent_id);
        }
    }
}

static MEMORY_CACHE: once_cell::sync::Lazy<MemoryCacheInner> =
    once_cell::sync::Lazy::new(|| MemoryCacheInner {
        map: std::sync::Mutex::new(std::collections::HashMap::new()),
    });

pub struct Agent<'a> {
    pub agent_id: &'a str,
    pub memory_db_path: String,
    pub retriever: Arc<Mutex<Retriever>>,
    pub settings: ChatSettings,
}

impl<'a> Agent<'a> {
    pub fn new(
        agent_id: &'a str,
        memory_db_path: &'a str,
        retriever: Arc<Mutex<Retriever>>,
    ) -> Self {
        let resolved = path_resolver::resolve_db_path(memory_db_path);
        Self {
            agent_id,
            memory_db_path: resolved.to_string_lossy().into_owned(),
            retriever,
            settings: ChatSettings::default(),
        }
    }

    pub fn with_settings(mut self, settings: ChatSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Run with default hybrid mode (backward compatible)
    pub fn run(&self, query: &str, top_k: usize) -> AgentResponse {
        self.run_with_mode(query, top_k, AgentMode::Hybrid)
    }

    /// Run with specified mode
    pub fn run_with_mode(&self, query: &str, top_k: usize, mode: AgentMode) -> AgentResponse {
        let mut steps = Vec::new();

        // Log active settings
        if self.settings.has_active_settings() {
            let mut settings_info = Vec::new();
            if let Some(focus) = &self.settings.focus_topic {
                settings_info.push(format!("focus: {}", focus));
            }
            if let Some(persona) = &self.settings.persona {
                settings_info.push(format!("persona: {}", persona));
            }
            if self.settings.verbosity != Verbosity::Normal {
                settings_info.push(format!("verbosity: {}", self.settings.verbosity.label()));
            }
            if !self.settings.memories.is_empty() {
                settings_info.push(format!("memories: {}", self.settings.memories.len()));
            }
            steps.push(AgentStep {
                kind: "settings".into(),
                message: format!("Active settings: {}", settings_info.join(", ")),
            });
        }

        // Step 1: Recall recent memory (always do this)
        let recall_start = std::time::Instant::now();
        let recalled: Vec<String> = if let Ok(mem) = AgentMemory::new(&self.memory_db_path) {
            mem.recall(self.agent_id)
                .map(|items| items.into_iter().take(5).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let recall_time = recall_start.elapsed().as_millis() as u64;

        // Record memory recall tool execution
        crate::monitoring::record_tool_execution(
            "Memory",
            &format!("recall: {}", &query[..query.len().min(50)]),
            true,
            &format!("{} items recalled", recalled.len()),
            recall_time,
            1.0,
            Some("agent_memory"),
        );

        if !recalled.is_empty() {
            steps.push(AgentStep {
                kind: "memory".into(),
                message: format!("Recalled {} memory items", recalled.len()),
            });
        }

        // Step 1b: Check active goals and find related goal
        let active_goals = self.get_active_goals();
        let related_goal = self.find_related_goal(query, &active_goals);

        if !active_goals.is_empty() {
            let goal_msg = if let Some((_, ref goal_text)) = related_goal {
                format!(
                    "Found {} active goals, query relates to: {}",
                    active_goals.len(),
                    goal_text
                )
            } else {
                format!(
                    "Found {} active goals (none directly related to query)",
                    active_goals.len()
                )
            };
            steps.push(AgentStep {
                kind: "goals".into(),
                message: goal_msg,
            });
        }

        // Build goal context string for prompt injection
        let goal_context_str = self.build_goal_context(&active_goals, related_goal.as_ref());
        let goal_context_opt = if goal_context_str.is_empty() {
            None
        } else {
            Some(goal_context_str.as_str())
        };

        // Agentic mode is handled by the streaming endpoint (run_agent_stream).
        // The synchronous /agent and GET /agent paths do not support Agentic mode
        // because Rig's tool-calling loop is async and cannot block safely.
        // Callers should use POST /agent/stream with mode="agentic".
        if matches!(mode, AgentMode::Agentic) {
            steps.push(AgentStep {
                kind: "mode".into(),
                message: "Agentic mode: use POST /agent/stream with mode=agentic".into(),
            });
            let answer = "Agentic mode requires the streaming endpoint (/agent/stream). \
                          Please send your request to POST /agent/stream with {\"mode\":\"agentic\"}."
                .to_string();
            self.store_memory(query, &answer);
            self.store_episode(query, &answer, 0, false);
            return AgentResponse {
                answer,
                steps,
                used_chunks: Vec::new(),
            };
        }

        // Handle LLM-only mode
        if matches!(mode, AgentMode::Llm) {
            steps.push(AgentStep {
                kind: "mode".into(),
                message: "LLM-only mode (no document search)".into(),
            });
            let answer = self.call_llm(query, None, mode, goal_context_opt);
            steps.push(AgentStep {
                kind: "llm".into(),
                message: "Generated response with LLM".into(),
            });
            self.store_memory(query, &answer);
            self.store_episode(query, &answer, 0, true);
            // Record goal progress if query was related to a goal
            if let Some((ref goal_id, _)) = related_goal {
                self.record_goal_progress(goal_id, query, true);
            }
            return AgentResponse {
                answer,
                steps,
                used_chunks: Vec::new(),
            };
        }

        // Step 2: Retrieve relevant chunks (for RAG and Hybrid modes)
        let mut used_chunks: Vec<String> = Vec::new();
        let retrieval_msg: String;
        {
            let search_start = std::time::Instant::now();
            if let Ok(mut r) = self.retriever.lock() {
                match r.hybrid_search(query, None) {
                    Ok(mut results) => {
                        let search_time = search_start.elapsed().as_millis() as u64;
                        if results.len() > top_k {
                            results.truncate(top_k);
                        }
                        let result_count = results.len();
                        used_chunks = results;
                        retrieval_msg = format!("Retrieved {} chunks", result_count);
                        // Record tool execution for monitoring
                        crate::monitoring::record_tool_execution(
                            "SemanticSearch",
                            query,
                            true,
                            &format!("{} chunks found", result_count),
                            search_time,
                            1.0, // confidence
                            Some("retriever"),
                        );
                    }
                    Err(e) => {
                        let search_time = search_start.elapsed().as_millis() as u64;
                        retrieval_msg = format!("Retrieval failed: {}", e);
                        // Record failed tool execution
                        crate::monitoring::record_tool_execution(
                            "SemanticSearch",
                            query,
                            false,
                            &e.to_string(),
                            search_time,
                            0.0,
                            Some("retriever"),
                        );
                    }
                }
            } else {
                retrieval_msg = "Failed to acquire retriever lock".into();
            }
        }
        steps.push(AgentStep {
            kind: "retrieve".into(),
            message: retrieval_msg,
        });

        // Step 3: Handle based on mode and results
        if used_chunks.is_empty() {
            match mode {
                AgentMode::Rag => {
                    // RAG-only mode: return fallback if no chunks
                    let answer =
                        "I couldn't find relevant information in the knowledge base.".to_string();
                    steps.push(AgentStep {
                        kind: "plan".into(),
                        message: "No chunks found; returning fallback (RAG-only mode)".into(),
                    });
                    self.store_memory(query, &answer);
                    self.store_episode(query, &answer, 0, false);
                    return AgentResponse {
                        answer,
                        steps,
                        used_chunks,
                    };
                }
                AgentMode::Hybrid => {
                    // Hybrid mode: fall back to LLM when no chunks found
                    steps.push(AgentStep {
                        kind: "plan".into(),
                        message: "No chunks found; falling back to LLM".into(),
                    });
                    // Record tool chain: Memory -> Search (failed) -> LLM fallback
                    crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                    crate::monitoring::record_tool_dependency_str("SemanticSearch", "LLMGenerate");
                    let answer = self.call_llm(query, None, mode, goal_context_opt);
                    steps.push(AgentStep {
                        kind: "llm".into(),
                        message: "Generated response with LLM (fallback)".into(),
                    });
                    self.store_memory(query, &answer);
                    self.store_episode(query, &answer, 0, true);
                    // Record goal progress if query was related to a goal
                    if let Some((ref goal_id, _)) = related_goal {
                        self.record_goal_progress(goal_id, query, true);
                    }
                    return AgentResponse {
                        answer,
                        steps,
                        used_chunks,
                    };
                }
                AgentMode::RagStrict => {
                    // Strict RAG: no chunks means "I don't know"
                    let answer =
                        "I don't have enough information in the knowledge base to answer this question.".to_string();
                    steps.push(AgentStep {
                        kind: "plan".into(),
                        message: "No chunks found; strict RAG returns 'I don't know'".into(),
                    });
                    self.store_memory(query, &answer);
                    self.store_episode(query, &answer, 0, false);
                    return AgentResponse {
                        answer,
                        steps,
                        used_chunks,
                    };
                }
                AgentMode::Auto => {
                    // Auto with no chunks: fall back to Hybrid (LLM-only)
                    steps.push(AgentStep {
                        kind: "plan".into(),
                        message: "Auto mode: no chunks found; falling back to LLM".into(),
                    });
                    crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                    crate::monitoring::record_tool_dependency_str("SemanticSearch", "LLMGenerate");
                    let answer = self.call_llm(query, None, AgentMode::Hybrid, goal_context_opt);
                    steps.push(AgentStep {
                        kind: "llm".into(),
                        message: "Generated response with LLM (Auto fallback)".into(),
                    });
                    self.store_memory(query, &answer);
                    self.store_episode(query, &answer, 0, true);
                    if let Some((ref goal_id, _)) = related_goal {
                        self.record_goal_progress(goal_id, query, true);
                    }
                    return AgentResponse {
                        answer,
                        steps,
                        used_chunks,
                    };
                }
                AgentMode::Llm => unreachable!(), // Already handled above
                AgentMode::Agentic => unreachable!(), // Already handled above
            }
        }

        // Step 4: We have chunks - generate answer
        let answer = match mode {
            AgentMode::Rag => {
                // RAG-only: just summarize chunks
                steps.push(AgentStep {
                    kind: "summarize".into(),
                    message: format!("Summarized {} chunks", used_chunks.len()),
                });
                naive_summarize(query, &used_chunks)
            }
            AgentMode::Hybrid => {
                // Hybrid: use LLM with context from chunks
                let context = used_chunks.join("\n\n");
                steps.push(AgentStep {
                    kind: "llm".into(),
                    message: format!(
                        "Generating answer with LLM using {} chunks as context",
                        used_chunks.len()
                    ),
                });
                // Record tool chain: Memory -> Search -> LLM
                crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                crate::monitoring::record_tool_dependency_str("SemanticSearch", "LLMGenerate");
                self.call_llm(query, Some(&context), mode, goal_context_opt)
            }
            AgentMode::RagStrict => {
                // Strict grounded RAG: call LLM but force it to answer only from context
                let context = used_chunks.join("\n\n");
                steps.push(AgentStep {
                    kind: "llm".into(),
                    message: format!(
                        "Strict RAG: generating grounded answer from {} chunks",
                        used_chunks.len()
                    ),
                });
                crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                crate::monitoring::record_tool_dependency_str("SemanticSearch", "LLMGenerate");
                self.call_llm_strict(query, &context, goal_context_opt)
            }
            AgentMode::Auto => {
                // Auto: fragmentation gap routes to PointerHydration; otherwise
                // chunk count + est_tokens picks Strict vs Hybrid via `auto_route`.
                let context = used_chunks.join("\n\n");
                let est_tokens = context.len() / 4;
                // Fragmentation signal (pp4 Phase 1). The helper handles
                // empty section_id and lock-failure cases by treating both
                // as untracked.
                let frag = if let Ok(r) = self.retriever.lock() {
                    fragmentation(&used_chunks, |c| {
                        let section_id = r
                            .meta_for_content(c)
                            .map(|m| m.section_id.clone())
                            .unwrap_or_default();
                        let doc_id = r.doc_id_for_content(c).unwrap_or_default();
                        Some((section_id, doc_id))
                    })
                } else {
                    fragmentation(&used_chunks, |_| None)
                };
                log_auto_fragmentation(used_chunks.len(), frag, est_tokens);
                let frag_suffix = fragmentation_suffix(frag);
                // pp4 Phase 2: delegate routing to the pure `auto_route`
                // helper so the decision logic stays unit-testable. The
                // gap and threshold values are recomputed here only for
                // the log/step-message payloads (the decision itself is
                // already made).
                let frag_gap = frag
                    .section_ratio
                    .zip(frag.doc_ratio)
                    .map(|(s, d)| (s - d) as f64)
                    .unwrap_or(0.0);
                let gap_threshold = crate::settings::effective_f64(
                    "POINTERRAG_AUTO_GAP_THRESHOLD",
                    POINTERRAG_AUTO_GAP_THRESHOLD_DEFAULT,
                );
                let route = auto_route(frag, gap_threshold, used_chunks.len(), est_tokens);
                crate::monitoring::pointer_stats::record_auto_route(route);
                match route {
                    AutoRoute::PointerHydration => {
                        let h = self.hydrate_pointer_sections(&used_chunks);
                        crate::monitoring::pointer_stats::record_pointer_hydration(
                            used_chunks.len(),
                            &h,
                            frag_gap,
                            gap_threshold,
                        );
                        tracing::info!(
                            chunks = used_chunks.len(),
                            hydrated = h.hydrated,
                            fallback_no_section_id = h.fb_no_section_id,
                            fallback_fetch_empty = h.fb_fetch_empty,
                            fallback_lock_failed = h.fb_lock_failed,
                            gap = frag_gap,
                            threshold = gap_threshold,
                            "Auto mode: routing to PointerRag (within-doc fragmentation)"
                        );
                        steps.push(AgentStep {
                            kind: "llm".into(),
                            message: format!(
                                "Auto mode: within-doc fragmentation (gap {:.3} ≥ {:.3}) → PointerRag (hydrated {} sections from {} chunks, {} fallbacks){}",
                                frag_gap,
                                gap_threshold,
                                h.hydrated,
                                used_chunks.len(),
                                h.total_fallbacks(),
                                frag_suffix
                            ),
                        });
                        crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                        crate::monitoring::record_tool_dependency_str(
                            "SemanticSearch",
                            "LLMGenerate",
                        );
                        self.call_llm_strict(query, &h.context, goal_context_opt)
                    }
                    AutoRoute::Strict => {
                        steps.push(AgentStep {
                            kind: "llm".into(),
                            message: format!(
                                "Auto mode: high confidence ({} chunks, ~{} tokens) → strict grounded RAG{}",
                                used_chunks.len(),
                                est_tokens,
                                frag_suffix
                            ),
                        });
                        crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                        crate::monitoring::record_tool_dependency_str(
                            "SemanticSearch",
                            "LLMGenerate",
                        );
                        self.call_llm_strict(query, &context, goal_context_opt)
                    }
                    AutoRoute::Hybrid => {
                        steps.push(AgentStep {
                            kind: "llm".into(),
                            message: format!(
                                "Auto mode: low confidence ({} chunks, ~{} tokens) → Hybrid{}",
                                used_chunks.len(),
                                est_tokens,
                                frag_suffix
                            ),
                        });
                        crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                        crate::monitoring::record_tool_dependency_str(
                            "SemanticSearch",
                            "LLMGenerate",
                        );
                        self.call_llm(query, Some(&context), AgentMode::Hybrid, goal_context_opt)
                    }
                }
            }
            AgentMode::Llm => unreachable!(), // Already handled above
            AgentMode::Agentic => unreachable!(), // Already handled above
        };

        // Step 5: Store memory
        self.store_memory(query, &answer);
        self.store_episode(query, &answer, used_chunks.len(), true);

        // Record goal progress if query was related to a goal
        if let Some((ref goal_id, _)) = related_goal {
            self.record_goal_progress(goal_id, query, true);
            steps.push(AgentStep {
                kind: "goal_progress".into(),
                message: "Recorded progress toward goal".to_string(),
            });
        }

        steps.push(AgentStep {
            kind: "memory".into(),
            message: "Stored interaction in memory".into(),
        });

        AgentResponse {
            answer,
            steps,
            used_chunks,
        }
    }

    /// Map matched chunks → unique section_ids → reassembled section
    /// text via `Retriever::fetch_section`. Falls back to the raw chunk
    /// text when the section_id is missing (older index), the fetch
    /// returns empty, or the retriever lock is unavailable. The four
    /// fallback counters let the step-trace surface silent degradation.
    ///
    /// Called by the `AgentMode::Auto` routing decision when
    /// `gap ≥ threshold` — Auto's only path into section hydration.
    fn hydrate_pointer_sections(&self, used_chunks: &[String]) -> PointerHydration {
        let mut seen_sections: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut sections: Vec<String> = Vec::with_capacity(used_chunks.len());
        let mut hydrated = 0usize;
        let mut fb_no_section_id = 0usize;
        let mut fb_fetch_empty = 0usize;
        let mut fb_lock_failed = 0usize;
        if let Ok(r) = self.retriever.lock() {
            for chunk in used_chunks {
                let section_id = r
                    .meta_for_content(chunk)
                    .map(|m| m.section_id.clone())
                    .unwrap_or_default();
                if section_id.is_empty() {
                    fb_no_section_id += 1;
                    sections.push(chunk.clone());
                    continue;
                }
                if !seen_sections.insert(section_id.clone()) {
                    continue;
                }
                match r.fetch_section(&section_id) {
                    Ok(text) if !text.is_empty() => {
                        hydrated += 1;
                        sections.push(text);
                    }
                    _ => {
                        fb_fetch_empty += 1;
                        sections.push(chunk.clone());
                    }
                }
            }
        } else {
            fb_lock_failed = used_chunks.len();
            sections.extend_from_slice(used_chunks);
        }
        PointerHydration {
            context: sections.join("\n\n---\n\n"),
            hydrated,
            fb_no_section_id,
            fb_fetch_empty,
            fb_lock_failed,
        }
    }

    /// Call LLM to generate a response (blocking)
    /// Uses mode-specific LlmConfig for optimal parameters
    fn call_llm(
        &self,
        query: &str,
        context: Option<&str>,
        mode: AgentMode,
        goal_context: Option<&str>,
    ) -> String {
        use crate::db::llm_settings::LlmConfig;

        // Get mode-specific config
        let mut config = match mode {
            AgentMode::Rag => LlmConfig::documents_only(),
            AgentMode::Llm => LlmConfig::llm_only(),
            AgentMode::Hybrid | AgentMode::Auto => LlmConfig::combined(),
            AgentMode::RagStrict => LlmConfig::documents_only(),
            AgentMode::Agentic => LlmConfig::combined(),
        };

        // Apply temperature override if set
        if let Some(temp) = self.settings.temperature {
            config.temperature = temp;
        }

        // Get Ollama URL from environment or use default
        let ollama_url =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());

        // Use model override if set, otherwise use environment or default
        let model = self.settings.model.clone().unwrap_or_else(|| {
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "phi:latest".to_string())
        });

        // Build system prompt from settings (includes memories)
        let system_prompt = self.settings.build_system_prompt();

        // Build the full prompt with goal context
        let prompt = self.build_prompt(query, context, &system_prompt, goal_context);

        // Extract prompt/system before backend dispatch
        let (final_prompt, system_field) = if !system_prompt.is_empty() {
            (prompt, Some(system_prompt))
        } else {
            (prompt, None)
        };

        // Dispatch to correct backend
        let hw = crate::db::param_hardware::global_config();

        crate::monitoring::mark_llm_started();
        let llm_start = std::time::Instant::now();

        // Shared OpenAI-compatible request/response types (used by llama.cpp and OpenAI)
        #[derive(serde::Serialize)]
        struct OaiMsg {
            role: String,
            content: String,
        }
        #[derive(serde::Serialize)]
        struct OaiReq {
            model: String,
            messages: Vec<OaiMsg>,
            temperature: f32,
            max_tokens: usize,
            stream: bool,
        }
        #[derive(serde::Deserialize)]
        struct OaiChoice {
            message: OaiMsgResp,
        }
        #[derive(serde::Deserialize)]
        struct OaiMsgResp {
            content: String,
        }
        #[derive(serde::Deserialize)]
        struct OaiResp {
            choices: Vec<OaiChoice>,
        }

        fn call_oai_compat(
            url: &str,
            bearer: Option<&str>,
            req_body: &OaiReq,
            backend_label: &str,
        ) -> (String, bool) {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());
            let mut req = client.post(url).json(req_body);
            if let Some(key) = bearer {
                req = req.bearer_auth(key);
            }
            match req.send() {
                Ok(resp) => match resp.text() {
                    Ok(text) => match serde_json::from_str::<OaiResp>(&text) {
                        Ok(data) => {
                            let content = data
                                .choices
                                .into_iter()
                                .next()
                                .map(|c| c.message.content.trim().to_string())
                                .unwrap_or_default();
                            (content, true)
                        }
                        Err(e) => (
                            format!(
                                "{} parse error: {} - Raw: {}",
                                backend_label,
                                e,
                                &text[..text.len().min(200)]
                            ),
                            false,
                        ),
                    },
                    Err(e) => (format!("{} read error: {}", backend_label, e), false),
                },
                Err(e) => (format!("{} error: {}", backend_label, e), false),
            }
        }

        let backend_label = hw.backend_type.label().to_lowercase();
        let backend_label = backend_label.replace([' ', '.'], "_");

        let (result, success) = match hw.backend_type {
            crate::db::param_hardware::BackendType::LlamaCpp => {
                let url = format!("{}/v1/chat/completions", hw.llama_server_url);
                let mut messages: Vec<OaiMsg> = Vec::new();
                if let Some(sys) = system_field {
                    messages.push(OaiMsg {
                        role: "system".into(),
                        content: sys,
                    });
                }
                messages.push(OaiMsg {
                    role: "user".into(),
                    content: final_prompt,
                });
                let req_body = OaiReq {
                    model,
                    messages,
                    temperature: config.temperature,
                    max_tokens: config.max_tokens,
                    stream: false,
                };
                let (mut res, ok) = call_oai_compat(&url, None, &req_body, "llama-server");
                if !ok {
                    res = format!(
                        "{}. Is llama-server running at {}?",
                        res, hw.llama_server_url
                    );
                }
                (res, ok)
            }

            crate::db::param_hardware::BackendType::OpenAi => {
                let api_keys = crate::db::api_keys::global_config();
                match api_keys.get_openai_key() {
                    None => (
                        "OpenAI API key not configured. Set OPENAI_API_KEY or add it in Hardware settings.".into(),
                        false,
                    ),
                    Some(key) => {
                        let mut messages: Vec<OaiMsg> = Vec::new();
                        if let Some(sys) = system_field {
                            messages.push(OaiMsg { role: "system".into(), content: sys });
                        }
                        messages.push(OaiMsg { role: "user".into(), content: final_prompt });
                        let req_body = OaiReq {
                            model,
                            messages,
                            temperature: config.temperature,
                            max_tokens: config.max_tokens,
                            stream: false,
                        };
                        call_oai_compat(
                            "https://api.openai.com/v1/chat/completions",
                            Some(&key),
                            &req_body,
                            "OpenAI",
                        )
                    }
                }
            }

            _ => {
                // Ollama: /api/generate
                let url = format!("{}/api/generate", ollama_url);

                #[derive(serde::Serialize)]
                struct OllamaOptions {
                    temperature: f32,
                    top_p: f32,
                    top_k: usize,
                    num_predict: usize,
                    repeat_penalty: f32,
                }
                #[derive(serde::Serialize)]
                struct OllamaRequest {
                    model: String,
                    prompt: String,
                    stream: bool,
                    options: OllamaOptions,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    system: Option<String>,
                }
                #[derive(serde::Deserialize, Debug)]
                struct OllamaResponse {
                    response: String,
                    #[serde(default)]
                    #[allow(dead_code)]
                    done: bool,
                }

                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(120))
                    .build()
                    .unwrap_or_else(|_| reqwest::blocking::Client::new());

                let request_body = OllamaRequest {
                    model,
                    prompt: final_prompt,
                    stream: false,
                    options: OllamaOptions {
                        temperature: config.temperature,
                        top_p: config.top_p,
                        top_k: config.top_k,
                        num_predict: config.max_tokens,
                        repeat_penalty: config.repeat_penalty,
                    },
                    system: system_field,
                };

                match client.post(&url).json(&request_body).send() {
                    Ok(response) => match response.text() {
                        Ok(text) => match serde_json::from_str::<OllamaResponse>(&text) {
                            Ok(data) => (data.response.trim().to_string(), true),
                            Err(e) => (
                                format!(
                                    "Failed to parse LLM response: {} - Raw: {}",
                                    e,
                                    &text[..text.len().min(200)]
                                ),
                                false,
                            ),
                        },
                        Err(e) => (format!("Failed to read LLM response: {}", e), false),
                    },
                    Err(e) => (
                        format!("LLM error: {}. Make sure Ollama is running.", e),
                        false,
                    ),
                }
            }
        };

        let llm_time = llm_start.elapsed().as_millis() as u64;

        // Mark LLM call as finished for health status
        crate::monitoring::mark_llm_finished();

        // Record tool execution for monitoring
        crate::monitoring::record_tool_execution(
            "LLMGenerate",
            query,
            success,
            &if success {
                format!("{} chars generated", result.len())
            } else {
                result.clone()
            },
            llm_time,
            if success { 0.9 } else { 0.0 },
            Some(&backend_label),
        );

        result
    }

    /// Build the prompt with context and settings
    /// Note: System prompt/instructions are sent via the 'system' field to the LLM,
    /// NOT included in the prompt. This prevents the LLM from echoing instructions to users.
    fn build_prompt(
        &self,
        query: &str,
        context: Option<&str>,
        _system_prompt: &str,
        goal_context: Option<&str>,
    ) -> String {
        let mut prompt_parts = Vec::new();

        // Add goal context if present
        if let Some(goals) = goal_context {
            if !goals.is_empty() {
                prompt_parts.push(goals.to_string());
            }
        }

        // Add context if present, or fallback instruction when no context
        if let Some(ctx) = context {
            prompt_parts.push(format!(
                "Context (ignore if not relevant to the question):\n{}\n\nAnswer the question directly. If the context above is not relevant, use your own knowledge.",
                ctx
            ));
        } else {
            // No context available - tell LLM to answer from its knowledge
            prompt_parts.push("Answer the question based on your knowledge.".to_string());
        }

        // Add the question
        prompt_parts.push(format!("Question: {}", query));

        // Add answer prompt
        prompt_parts.push("Answer:".to_string());

        prompt_parts.join("\n\n")
    }

    /// Call LLM with strict grounding: answer ONLY from the provided context.
    /// If the context doesn't contain the answer, the LLM is instructed to say so.
    fn call_llm_strict(&self, query: &str, context: &str, goal_context: Option<&str>) -> String {
        use crate::db::llm_settings::LlmConfig;

        let mut config = LlmConfig::documents_only();
        // Use lower temperature for strict grounding (more deterministic)
        config.temperature = config.temperature.min(0.3);

        if let Some(temp) = self.settings.temperature {
            config.temperature = temp;
        }

        let ollama_url =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = self.settings.model.clone().unwrap_or_else(|| {
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "phi:latest".to_string())
        });

        // Build strict grounding prompt
        let mut prompt_parts = Vec::new();
        if let Some(goals) = goal_context {
            if !goals.is_empty() {
                prompt_parts.push(goals.to_string());
            }
        }
        prompt_parts.push(format!("Context:\n{}", context));
        prompt_parts.push(format!("Question: {}", query));
        prompt_parts.push("Answer:".to_string());
        let prompt = prompt_parts.join("\n\n");

        // Strict grounding system instruction
        let system =
            "You are a precise assistant. Answer the question using ONLY the provided context. \
            Do not use any outside knowledge. If the context does not contain enough information \
            to answer the question, respond with: \"I don't have enough information in the \
            knowledge base to answer this question.\" Be concise and accurate."
                .to_string();

        // Dispatch to correct backend
        let hw = crate::db::param_hardware::global_config();
        let use_llama = matches!(
            hw.backend_type,
            crate::db::param_hardware::BackendType::LlamaCpp
        );

        crate::monitoring::mark_llm_started();
        let llm_start = std::time::Instant::now();

        let (result, success) = if use_llama {
            // llama-server: OpenAI-compatible /v1/chat/completions
            let url = format!("{}/v1/chat/completions", hw.llama_server_url);

            #[derive(serde::Serialize)]
            struct StrictMsg {
                role: String,
                content: String,
            }
            #[derive(serde::Serialize)]
            struct StrictReq {
                model: String,
                messages: Vec<StrictMsg>,
                temperature: f32,
                max_tokens: usize,
                stream: bool,
            }
            #[derive(serde::Deserialize)]
            struct StrictChoice {
                message: StrictMsgResp,
            }
            #[derive(serde::Deserialize)]
            struct StrictMsgResp {
                content: String,
            }
            #[derive(serde::Deserialize)]
            struct StrictResp {
                choices: Vec<StrictChoice>,
            }

            let messages = vec![
                StrictMsg {
                    role: "system".into(),
                    content: system.clone(),
                },
                StrictMsg {
                    role: "user".into(),
                    content: prompt,
                },
            ];

            let req_body = StrictReq {
                model,
                messages,
                temperature: config.temperature,
                max_tokens: config.max_tokens,
                stream: false,
            };

            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());

            match client.post(&url).json(&req_body).send() {
                Ok(resp) => match resp.text() {
                    Ok(text) => match serde_json::from_str::<StrictResp>(&text) {
                        Ok(data) => {
                            let content = data
                                .choices
                                .into_iter()
                                .next()
                                .map(|c| c.message.content.trim().to_string())
                                .unwrap_or_default();
                            (content, true)
                        }
                        Err(e) => (format!("llama-server parse error: {}", e), false),
                    },
                    Err(e) => (format!("llama-server read error: {}", e), false),
                },
                Err(e) => (
                    format!(
                        "llama-server error: {}. Is llama-server running at {}?",
                        e, hw.llama_server_url
                    ),
                    false,
                ),
            }
        } else {
            // Ollama: /api/generate
            let url = format!("{}/api/generate", ollama_url);

            #[derive(serde::Serialize)]
            struct OllamaOptions {
                temperature: f32,
                top_p: f32,
                top_k: usize,
                num_predict: usize,
                repeat_penalty: f32,
            }
            #[derive(serde::Serialize)]
            struct OllamaRequest {
                model: String,
                prompt: String,
                stream: bool,
                options: OllamaOptions,
                system: String,
            }
            #[derive(serde::Deserialize, Debug)]
            struct OllamaResponse {
                response: String,
                #[serde(default)]
                #[allow(dead_code)]
                done: bool,
            }

            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());

            let request_body = OllamaRequest {
                model,
                prompt,
                stream: false,
                options: OllamaOptions {
                    temperature: config.temperature,
                    top_p: config.top_p,
                    top_k: config.top_k,
                    num_predict: config.max_tokens,
                    repeat_penalty: config.repeat_penalty,
                },
                system,
            };

            match client.post(&url).json(&request_body).send() {
                Ok(response) => match response.text() {
                    Ok(text) => match serde_json::from_str::<OllamaResponse>(&text) {
                        Ok(data) => (data.response.trim().to_string(), true),
                        Err(e) => (format!("Failed to parse LLM response: {}", e), false),
                    },
                    Err(e) => (format!("Failed to read LLM response: {}", e), false),
                },
                Err(e) => (
                    format!("LLM error: {}. Make sure Ollama is running.", e),
                    false,
                ),
            }
        };

        let llm_time = llm_start.elapsed().as_millis() as u64;
        crate::monitoring::mark_llm_finished();

        crate::monitoring::record_tool_execution(
            "LLMGenerate",
            query,
            success,
            &if success {
                format!("{} chars generated (strict)", result.len())
            } else {
                result.clone()
            },
            llm_time,
            if success { 1.0 } else { 0.0 },
            Some(if use_llama { "llama_cpp" } else { "ollama" }),
        );

        result
    }

    /// Fetch active goals for this agent
    fn get_active_goals(&self) -> Vec<(String, String)> {
        let mut goals = Vec::new();
        if let Ok(conn) = Connection::open(&self.memory_db_path) {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT id, goal FROM goals WHERE agent_id = ?1 AND status = 'active' ORDER BY created_at DESC LIMIT 5"
            ) {
                if let Ok(rows) = stmt.query_map(rusqlite::params![self.agent_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }) {
                    for row in rows.flatten() {
                        goals.push(row);
                    }
                }
            }
        }
        goals
    }

    /// Check if query relates to any active goal
    fn find_related_goal(
        &self,
        query: &str,
        goals: &[(String, String)],
    ) -> Option<(String, String)> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        for (goal_id, goal_text) in goals {
            let goal_lower = goal_text.to_lowercase();
            // Check if any significant words from the query appear in the goal
            let matches = query_words
                .iter()
                .filter(|w| w.len() > 3) // Skip short words
                .filter(|w| goal_lower.contains(*w))
                .count();
            if matches >= 2 || (matches >= 1 && query_words.len() <= 5) {
                return Some((goal_id.clone(), goal_text.clone()));
            }
        }
        None
    }

    /// Build goal context for prompt
    fn build_goal_context(
        &self,
        goals: &[(String, String)],
        related_goal: Option<&(String, String)>,
    ) -> String {
        if goals.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();
        parts.push("ACTIVE GOALS:".to_string());

        for (i, (_, goal_text)) in goals.iter().enumerate() {
            let marker = if related_goal.map(|(_, g)| g == goal_text).unwrap_or(false) {
                "→" // Mark the related goal
            } else {
                "•"
            };
            parts.push(format!("{} {}", marker, goal_text));
            if i >= 2 {
                break;
            } // Limit to 3 goals in prompt
        }

        if let Some((_, related_text)) = related_goal {
            parts.push(format!("\nThis query appears related to goal: \"{}\". Consider how your response advances this goal.", related_text));
        }

        parts.join("\n")
    }

    /// Update goal progress after a successful query
    fn record_goal_progress(&self, goal_id: &str, query: &str, success: bool) {
        if let Ok(conn) = Connection::open(&self.memory_db_path) {
            // Create goal_progress table if not exists
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS goal_progress (
                    id TEXT PRIMARY KEY,
                    goal_id TEXT NOT NULL,
                    agent_id TEXT NOT NULL,
                    query TEXT NOT NULL,
                    success INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    FOREIGN KEY(goal_id) REFERENCES goals(id)
                )",
                [],
            );

            let progress_id = Uuid::new_v4().to_string();
            let created_at = Utc::now().timestamp();
            let success_int = if success { 1 } else { 0 };

            let _ = conn.execute(
                "INSERT INTO goal_progress (id, goal_id, agent_id, query, success, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![progress_id, goal_id, self.agent_id, query, success_int, created_at],
            );
        }
    }

    fn store_memory(&self, query: &str, answer: &str) {
        let start = std::time::Instant::now();
        let success = if let Ok(mem) = AgentMemory::new(&self.memory_db_path) {
            let ts = Utc::now().to_rfc3339();
            let r1 = mem.store(self.agent_id, &format!("Q: {}", query), &ts);
            let r2 = mem.store(self.agent_id, &format!("A: {}", answer), &ts);
            r1.is_ok() && r2.is_ok()
        } else {
            false
        };
        let elapsed = start.elapsed().as_millis() as u64;
        crate::monitoring::record_tool_execution(
            "Memory",
            &format!("store: {}", &query[..query.len().min(50)]),
            success,
            if success {
                "stored Q&A"
            } else {
                "failed to store"
            },
            elapsed,
            if success { 1.0 } else { 0.0 },
            Some("agent_memory"),
        );
    }

    /// Store episode for monitoring dashboard
    fn store_episode(&self, query: &str, response: &str, chunks_used: usize, success: bool) {
        if let Ok(conn) = Connection::open(&self.memory_db_path) {
            // Ensure episodes table exists
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS episodes (
                    id TEXT PRIMARY KEY,
                    agent_id TEXT NOT NULL,
                    query TEXT NOT NULL,
                    response TEXT NOT NULL,
                    context_chunks_used INTEGER NOT NULL,
                    success INTEGER NOT NULL,
                    created_at INTEGER NOT NULL
                )",
                [],
            );

            let episode_id = Uuid::new_v4().to_string();
            let created_at = Utc::now().timestamp();
            let success_int = if success { 1 } else { 0 };

            let _ = conn.execute(
                "INSERT INTO episodes (id, agent_id, query, response, context_chunks_used, success, created_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![episode_id, self.agent_id, query, response, chunks_used, success_int, created_at],
            );
        }
    }
}

fn naive_summarize(_query: &str, chunks: &[String]) -> String {
    // Very basic: take up to first 3 non-empty lines
    let mut out = String::new();
    for (i, c) in chunks.iter().enumerate() {
        if i >= 3 {
            break;
        }
        out.push_str("- ");
        let line = c.lines().next().unwrap_or("");
        out.push_str(line);
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str("No relevant content found.");
    }
    out
}

#[cfg(test)]
mod fragmentation_tests {
    use super::{fragmentation, FragmentationStats};

    fn chunks(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("chunk-{i}")).collect()
    }

    #[test]
    fn all_chunks_share_one_section_and_one_doc() {
        // Every chunk maps to ("section-A", "doc-X"). 1 section + 1 doc
        // over N tracked → both ratios = 1/N. Low fragmentation on
        // both axes: matches cluster in one section of one doc.
        let xs = chunks(5);
        let stats = fragmentation(&xs, |_| Some(("section-A".into(), "doc-X".into())));
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 5,
                untracked: 0,
                unique_sections: 1,
                unique_docs: 1,
                section_ratio: Some(0.2),
                doc_ratio: Some(0.2),
            }
        );
    }

    #[test]
    fn every_chunk_distinct_section_and_distinct_doc() {
        // Each chunk has its own section and its own doc. Both
        // ratios = 1.0. Canonical maximum-spread case across both
        // axes: every match is from a different document.
        let xs = chunks(4);
        let stats = fragmentation(&xs, |c| Some((format!("section-{c}"), format!("doc-{c}"))));
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 4,
                untracked: 0,
                unique_sections: 4,
                unique_docs: 4,
                section_ratio: Some(1.0),
                doc_ratio: Some(1.0),
            }
        );
    }

    #[test]
    fn within_doc_fragmentation_section_ratio_gt_doc_ratio() {
        // The new insight pp4 needed to surface: chunks spread across
        // multiple sections of the SAME doc. 4 chunks, 4 distinct
        // sections, but only 2 docs. section_ratio = 1.0, doc_ratio
        // = 0.5. The gap (0.5) is within-doc fragmentation — the case
        // PointerRag was originally designed for.
        let xs = chunks(4);
        let stats = fragmentation(&xs, |c| {
            let doc = match c {
                "chunk-0" | "chunk-1" => "doc-A",
                _ => "doc-B",
            };
            Some((format!("section-{c}"), doc.to_string()))
        });
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 4,
                untracked: 0,
                unique_sections: 4,
                unique_docs: 2,
                section_ratio: Some(1.0),
                doc_ratio: Some(0.5),
            }
        );
    }

    #[test]
    fn cross_doc_no_within_doc_fragmentation_ratios_match() {
        // Mirror case: 4 chunks across 4 distinct docs, but each doc
        // is single-section (every chunk in a given doc shares its
        // section_id). section_ratio == doc_ratio == 1.0. This is
        // what we observed on the actual corpus: header-less .md
        // files collapse section_ratio onto doc_ratio.
        let xs = chunks(4);
        let stats = fragmentation(&xs, |c| {
            let doc = match c {
                "chunk-0" => "doc-A",
                "chunk-1" => "doc-B",
                "chunk-2" => "doc-C",
                _ => "doc-D",
            };
            Some((format!("section-{doc}"), doc.to_string()))
        });
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 4,
                untracked: 0,
                unique_sections: 4,
                unique_docs: 4,
                section_ratio: Some(1.0),
                doc_ratio: Some(1.0),
            }
        );
    }

    #[test]
    fn mixed_some_chunks_missing_meta() {
        // chunk-0, chunk-2 → ("section-A", "doc-X"); chunk-1 → None;
        // chunk-3 → ("section-B", "doc-Y"). untracked counts the None
        // case; both sections and docs ignore it; ratios are over
        // tracked, not over the full chunk count.
        let xs = chunks(4);
        let stats = fragmentation(&xs, |c| match c {
            "chunk-0" | "chunk-2" => Some(("section-A".into(), "doc-X".into())),
            "chunk-3" => Some(("section-B".into(), "doc-Y".into())),
            _ => None,
        });
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 3,
                untracked: 1,
                unique_sections: 2,
                unique_docs: 2,
                section_ratio: Some(2.0 / 3.0),
                doc_ratio: Some(2.0 / 3.0),
            }
        );
    }

    #[test]
    fn empty_input_no_panic_returns_none_ratios() {
        // Empty chunk slice. No division by zero, no NaN, no panic.
        // Both ratios are None — distinguishable from a real low-
        // fragmentation 0.0 by callers that care.
        let stats = fragmentation(&[], |_| {
            Some(("never-called-s".into(), "never-called-d".into()))
        });
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 0,
                untracked: 0,
                unique_sections: 0,
                unique_docs: 0,
                section_ratio: None,
                doc_ratio: None,
            }
        );
    }

    #[test]
    fn lock_failure_all_chunks_untracked() {
        // Simulates the Auto arm's lock-failure closure: every lookup
        // returns None. Chunks exist but none are tracked.
        // Distinguished from the empty-input case by untracked > 0;
        // both ratios None so callers don't synthesize a value.
        let xs = chunks(7);
        let stats = fragmentation(&xs, |_| None);
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 0,
                untracked: 7,
                unique_sections: 0,
                unique_docs: 0,
                section_ratio: None,
                doc_ratio: None,
            }
        );
    }

    #[test]
    fn empty_section_id_treated_as_untracked() {
        // Empty section_id at the lookup boundary → chunk is untracked
        // even if doc_id is present. The section-spread signal is the
        // primary one; without it the chunk doesn't contribute.
        let xs = chunks(3);
        let stats = fragmentation(&xs, |_| Some((String::new(), "doc-X".into())));
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 0,
                untracked: 3,
                unique_sections: 0,
                unique_docs: 0,
                section_ratio: None,
                doc_ratio: None,
            }
        );
    }

    #[test]
    fn empty_doc_id_with_section_id_still_tracks_section() {
        // Inverse of the above: section_id present, doc_id empty. The
        // chunk still contributes to the section signal (and untracked
        // stays 0), but the docs-set insert is skipped — unique_docs
        // stays 0 and doc_ratio = 0/N. This is a defensive shape:
        // in practice every indexed chunk has both, but the helper
        // shouldn't assume.
        let xs = chunks(3);
        let stats = fragmentation(&xs, |_| Some(("section-A".into(), String::new())));
        assert_eq!(
            stats,
            FragmentationStats {
                tracked: 3,
                untracked: 0,
                unique_sections: 1,
                unique_docs: 0,
                section_ratio: Some(1.0 / 3.0),
                doc_ratio: Some(0.0),
            }
        );
    }
}

#[cfg(test)]
mod auto_log_tests {
    //! Tests for `log_auto_fragmentation` — the seam that the Auto arm
    //! calls so observability is verifiable without standing up a full
    //! Agent + Retriever harness.
    //!
    //! Captures `tracing` output by installing a `fmt::Subscriber` whose
    //! writer is a shared `Vec<u8>`. `with_default` makes the
    //! subscriber active only for the scope of the test closure.

    use super::{log_auto_fragmentation, FragmentationStats};
    use std::io;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;

    /// Thread-safe `MakeWriter` that appends every byte written into a
    /// shared `Vec<u8>`. Tests then decode that buffer to UTF-8 and
    /// inspect the formatted log lines.
    #[derive(Clone, Default)]
    struct BufWriter(Arc<Mutex<Vec<u8>>>);

    impl io::Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for BufWriter {
        type Writer = BufWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    fn capture<F: FnOnce()>(f: F) -> String {
        let buf = BufWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(buf.clone())
            .with_max_level(tracing::Level::INFO)
            .with_ansi(false)
            .without_time()
            .finish();
        tracing::subscriber::with_default(subscriber, f);
        let bytes = buf.0.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn high_confidence_shape_emits_all_fields() {
        // High-confidence-like inputs: 5 chunks, 1 section, 1 doc,
        // both ratios 0.2 (low fragmentation, single section in one
        // doc — Strict's wheelhouse). All structured fields must
        // appear so a reader can reconstruct the signal from logs.
        let frag = FragmentationStats {
            tracked: 5,
            untracked: 0,
            unique_sections: 1,
            unique_docs: 1,
            section_ratio: Some(0.2),
            doc_ratio: Some(0.2),
        };
        let out = capture(|| log_auto_fragmentation(5, frag, 1700));
        assert!(
            out.contains("Auto mode: fragmentation signal"),
            "missing message: {out}"
        );
        for field in [
            "chunks=5",
            "tracked=5",
            "untracked=0",
            "unique_sections=1",
            "unique_docs=1",
            "section_ratio=0.200",
            "doc_ratio=0.200",
            "est_tokens=1700",
        ] {
            assert!(out.contains(field), "missing {field}: {out}");
        }
    }

    #[test]
    fn within_doc_fragmentation_shape_emits_split_ratios() {
        // The new key signal: 4 chunks across 2 docs but with 4
        // distinct sections. section_ratio = 1.0, doc_ratio = 0.5.
        // The gap (0.5) is within-doc fragmentation. Asserts both
        // ratios appear independently so the gap is visible.
        let frag = FragmentationStats {
            tracked: 4,
            untracked: 0,
            unique_sections: 4,
            unique_docs: 2,
            section_ratio: Some(1.0),
            doc_ratio: Some(0.5),
        };
        let out = capture(|| log_auto_fragmentation(4, frag, 1200));
        assert!(out.contains("Auto mode: fragmentation signal"));
        for field in [
            "chunks=4",
            "tracked=4",
            "untracked=0",
            "unique_sections=4",
            "unique_docs=2",
            "section_ratio=1.000",
            "doc_ratio=0.500",
            "est_tokens=1200",
        ] {
            assert!(out.contains(field), "missing {field}: {out}");
        }
    }

    #[test]
    fn ratios_none_render_as_unknown() {
        // tracked=0 → both ratios are None → log renders "unknown"
        // for each, not "NaN" or "0.000". Older indexes / lock-
        // failure cases both land here.
        let frag = FragmentationStats {
            tracked: 0,
            untracked: 6,
            unique_sections: 0,
            unique_docs: 0,
            section_ratio: None,
            doc_ratio: None,
        };
        let out = capture(|| log_auto_fragmentation(6, frag, 400));
        assert!(
            out.contains("section_ratio=unknown"),
            "missing section unknown: {out}"
        );
        assert!(
            out.contains("doc_ratio=unknown"),
            "missing doc unknown: {out}"
        );
        assert!(!out.contains("NaN"));
        assert!(!out.contains("ratio=0.000"));
    }

    #[test]
    fn each_call_emits_exactly_one_line() {
        // Anti-test from pp4: calling the function twice produces
        // exactly two log lines, not one (hoisted) or three (duplicate
        // emit). Counts lines that contain the message string —
        // robust to fmt-subscriber line-format changes.
        let frag = FragmentationStats {
            tracked: 1,
            untracked: 0,
            unique_sections: 1,
            unique_docs: 1,
            section_ratio: Some(1.0),
            doc_ratio: Some(1.0),
        };
        let out = capture(|| {
            log_auto_fragmentation(1, frag, 100);
            log_auto_fragmentation(1, frag, 100);
        });
        let count = out
            .lines()
            .filter(|l| l.contains("Auto mode: fragmentation signal"))
            .count();
        assert_eq!(count, 2, "expected 2 log lines, got {count}: {out}");
    }
}

#[cfg(test)]
mod auto_suffix_tests {
    //! Tests for `fragmentation_suffix` — the step-trace suffix the
    //! Auto arm appends to both its high- and low-confidence
    //! `AgentStep` messages. Format is locked by `assert_eq!` so any
    //! drift surfaces in CI before reaching log scrapers or the UI.

    use super::{fragmentation_suffix, FragmentationStats};

    #[test]
    fn cross_doc_no_within_doc_suffix_is_exact() {
        // 4 sections over 5 tracked chunks (1 untracked), 4 distinct
        // docs. section_ratio = 0.8, doc_ratio = 0.8 — the gap is
        // zero, no within-doc fragmentation. This is the case the
        // actual corpus produces on its header-less .md files.
        let frag = FragmentationStats {
            tracked: 5,
            untracked: 1,
            unique_sections: 4,
            unique_docs: 4,
            section_ratio: Some(0.8),
            doc_ratio: Some(0.8),
        };
        assert_eq!(
            fragmentation_suffix(frag),
            " (fragmentation: section 0.800, doc 0.800, 5/6 chunks tracked, 4 sections / 4 docs)"
        );
    }

    #[test]
    fn within_doc_fragmentation_suffix_shows_ratio_gap() {
        // 4 sections, 2 docs — the canonical pp4 "PointerRag
        // wheelhouse" shape. section_ratio (1.0) > doc_ratio (0.5);
        // the visible gap in the suffix is the within-doc signal a
        // future router would key on.
        let frag = FragmentationStats {
            tracked: 4,
            untracked: 0,
            unique_sections: 4,
            unique_docs: 2,
            section_ratio: Some(1.0),
            doc_ratio: Some(0.5),
        };
        assert_eq!(
            fragmentation_suffix(frag),
            " (fragmentation: section 1.000, doc 0.500, 4/4 chunks tracked, 4 sections / 2 docs)"
        );
    }

    #[test]
    fn ratios_none_render_as_unknown_suffix() {
        // tracked=0 → both ratios None → both render "unknown"
        // (not "0.000", not "NaN"). Older index / lock-failure case.
        let frag = FragmentationStats {
            tracked: 0,
            untracked: 6,
            unique_sections: 0,
            unique_docs: 0,
            section_ratio: None,
            doc_ratio: None,
        };
        assert_eq!(
            fragmentation_suffix(frag),
            " (fragmentation: section unknown, doc unknown, 0/6 chunks tracked, 0 sections / 0 docs)"
        );
    }

    #[test]
    fn suffix_starts_with_leading_space() {
        // Callers concatenate the suffix directly onto the existing
        // message string; the leading space is what separates it from
        // the previous word. Regression-prevention: never drop it.
        let frag = FragmentationStats {
            tracked: 1,
            untracked: 0,
            unique_sections: 1,
            unique_docs: 1,
            section_ratio: Some(1.0),
            doc_ratio: Some(1.0),
        };
        let s = fragmentation_suffix(frag);
        assert!(s.starts_with(' '), "suffix should start with space: {s:?}");
    }
}

#[cfg(test)]
mod auto_route_tests {
    //! Tests for the `auto_route` routing decision (pp4 Phase 2 Step 3).
    //!
    //! Confirms the five rules the plan called out:
    //! 1. Fragmented (gap ≥ threshold) → PointerHydration, regardless of confidence.
    //! 2. Non-fragmented + high-confidence (≥3 chunks, ≥1536 tokens) → Strict.
    //! 3. Non-fragmented + low-confidence → Hybrid.
    //! 4. Boundary `gap == threshold` → inclusive (also PointerHydration).
    //! 5. All chunks untracked → does NOT trigger Pointer (gap collapses to 0,
    //!    so unless the threshold is also ≤ 0 the route falls through).
    use super::{auto_route, AutoRoute, FragmentationStats};

    /// Build a `FragmentationStats` with the requested gap.
    /// Picks section_ratio = gap + 0.2, doc_ratio = 0.2 (so the gap
    /// is exactly the difference and both ratios stay in `[0, 1]`).
    fn frag_with_gap(gap: f32) -> FragmentationStats {
        FragmentationStats {
            tracked: 5,
            untracked: 0,
            unique_sections: 5,
            unique_docs: 1,
            section_ratio: Some(gap + 0.2),
            doc_ratio: Some(0.2),
        }
    }

    /// Stats representing "we couldn't measure fragmentation" — older
    /// index without `section_id`s, or retriever lock unavailable. Both
    /// ratios are `None`, so the helper collapses gap to `0.0`.
    fn frag_untracked() -> FragmentationStats {
        FragmentationStats {
            tracked: 0,
            untracked: 5,
            unique_sections: 0,
            unique_docs: 0,
            section_ratio: None,
            doc_ratio: None,
        }
    }

    // Rule 1 — fragmented routes to Pointer regardless of confidence.

    #[test]
    fn fragmented_high_confidence_routes_to_pointer() {
        let frag = frag_with_gap(0.6);
        // chunks=5, tokens=2000 → would-be high_confidence shape.
        let route = auto_route(frag, 0.5, 5, 2000);
        assert_eq!(route, AutoRoute::PointerHydration);
    }

    #[test]
    fn fragmented_low_confidence_routes_to_pointer() {
        let frag = frag_with_gap(0.6);
        // chunks=2, tokens=400 → low-confidence shape, but gap wins.
        let route = auto_route(frag, 0.5, 2, 400);
        assert_eq!(route, AutoRoute::PointerHydration);
    }

    // Rule 2 — non-fragmented + high-confidence → Strict (existing behavior preserved).

    #[test]
    fn non_fragmented_high_confidence_routes_to_strict() {
        let frag = frag_with_gap(0.1);
        let route = auto_route(frag, 0.5, 5, 2000);
        assert_eq!(route, AutoRoute::Strict);
    }

    #[test]
    fn high_confidence_lower_bound_inclusive() {
        // Exactly at the confidence cutoff (3 chunks, 1536 tokens) → Strict.
        let frag = frag_with_gap(0.1);
        let route = auto_route(frag, 0.5, 3, 1536);
        assert_eq!(route, AutoRoute::Strict);
    }

    // Rule 3 — non-fragmented + low-confidence → Hybrid.

    #[test]
    fn non_fragmented_low_confidence_routes_to_hybrid() {
        let frag = frag_with_gap(0.1);
        let route = auto_route(frag, 0.5, 2, 400);
        assert_eq!(route, AutoRoute::Hybrid);
    }

    #[test]
    fn just_under_high_confidence_chunks_routes_to_hybrid() {
        // 2 chunks (below the 3-chunk cutoff) but plenty of tokens.
        let frag = frag_with_gap(0.1);
        let route = auto_route(frag, 0.5, 2, 10_000);
        assert_eq!(route, AutoRoute::Hybrid);
    }

    #[test]
    fn just_under_high_confidence_tokens_routes_to_hybrid() {
        // 5 chunks but est_tokens below the 1536 floor.
        let frag = frag_with_gap(0.1);
        let route = auto_route(frag, 0.5, 5, 1535);
        assert_eq!(route, AutoRoute::Hybrid);
    }

    // Rule 4 — boundary at `gap == threshold` is inclusive.

    #[test]
    fn gap_equal_to_threshold_routes_to_pointer() {
        // 0.5 == 0.5 → PointerHydration (the `>=` convention locked in).
        // Use exact f32 values so the comparison doesn't drift.
        let frag = FragmentationStats {
            tracked: 5,
            untracked: 0,
            unique_sections: 5,
            unique_docs: 1,
            section_ratio: Some(0.7),
            doc_ratio: Some(0.2),
        };
        let route = auto_route(frag, 0.5, 5, 2000);
        assert_eq!(
            route,
            AutoRoute::PointerHydration,
            "boundary should be inclusive (`gap >= threshold`)"
        );
    }

    #[test]
    fn gap_just_below_threshold_falls_through() {
        let frag = FragmentationStats {
            tracked: 5,
            untracked: 0,
            unique_sections: 5,
            unique_docs: 1,
            // section_ratio - doc_ratio = 0.499...
            section_ratio: Some(0.7),
            doc_ratio: Some(0.201),
        };
        let route = auto_route(frag, 0.5, 5, 2000);
        assert_ne!(route, AutoRoute::PointerHydration);
        assert_eq!(route, AutoRoute::Strict);
    }

    // Rule 5 — untracked retrieval (older index / lock failure) doesn't trigger Pointer
    // unless threshold is also ≤ 0.

    #[test]
    fn all_untracked_falls_through_to_high_confidence() {
        let frag = frag_untracked();
        // gap collapses to 0.0; threshold 0.5 → falls through.
        let route = auto_route(frag, 0.5, 5, 2000);
        assert_eq!(route, AutoRoute::Strict);
    }

    #[test]
    fn all_untracked_falls_through_to_hybrid_on_low_confidence() {
        let frag = frag_untracked();
        let route = auto_route(frag, 0.5, 2, 400);
        assert_eq!(route, AutoRoute::Hybrid);
    }

    #[test]
    fn always_pointer_setting_fires_even_on_untracked() {
        // Threshold == 0.0 is the documented "Always Pointer" slider
        // setting. The `gap >= 0.0` clause is satisfied even with
        // untracked retrieval (collapsed gap = 0.0). Inclusive boundary
        // is the load-bearing detail that makes the extreme work.
        let frag = frag_untracked();
        let route = auto_route(frag, 0.0, 5, 2000);
        assert_eq!(route, AutoRoute::PointerHydration);
    }

    #[test]
    fn never_pointer_setting_never_routes_to_pointer() {
        // Threshold > 1.0 — the documented "Never Pointer" extreme.
        // gap can never exceed 1.0 (it's section_ratio - doc_ratio,
        // both in [0, 1]), so Pointer never fires.
        let frag = frag_with_gap(1.0);
        let route = auto_route(frag, 1.0001, 5, 2000);
        assert_ne!(route, AutoRoute::PointerHydration);
    }
}
