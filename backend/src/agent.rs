use crate::agent_memory::AgentMemory;
use crate::db::path_resolver;
use crate::retriever::Retriever;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

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

    memories
}

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

        // Handle Agentic mode (Rig integration — stub)
        if matches!(mode, AgentMode::Agentic) {
            steps.push(AgentStep {
                kind: "mode".into(),
                message: "Agentic mode (Rig tool-loop — integration pending)".into(),
            });
            let answer = "Agentic mode received your query. Rig integration is pending — this mode will use an LLM-driven tool-calling loop to dynamically search documents, recall memory, and query the knowledge graph.".to_string();
            self.store_memory(query, &answer);
            self.store_episode(query, &answer, 0, true);
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
                // Auto: check retrieval confidence to decide strict RAG vs Hybrid
                let context = used_chunks.join("\n\n");
                let high_confidence = used_chunks.len() >= 3 && (context.len() / 4) >= 1536;
                if high_confidence {
                    steps.push(AgentStep {
                        kind: "llm".into(),
                        message: format!(
                            "Auto mode: high confidence ({} chunks, ~{} tokens) → strict grounded RAG",
                            used_chunks.len(),
                            context.len() / 4
                        ),
                    });
                    crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                    crate::monitoring::record_tool_dependency_str("SemanticSearch", "LLMGenerate");
                    self.call_llm_strict(query, &context, goal_context_opt)
                } else {
                    steps.push(AgentStep {
                        kind: "llm".into(),
                        message: format!(
                            "Auto mode: low confidence ({} chunks, ~{} tokens) → Hybrid",
                            used_chunks.len(),
                            context.len() / 4
                        ),
                    });
                    crate::monitoring::record_tool_dependency_str("Memory", "SemanticSearch");
                    crate::monitoring::record_tool_dependency_str("SemanticSearch", "LLMGenerate");
                    self.call_llm(query, Some(&context), AgentMode::Hybrid, goal_context_opt)
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
                message: format!("Recorded progress toward goal"),
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
            struct LlamaMsg { role: String, content: String }
            #[derive(serde::Serialize)]
            struct LlamaReq {
                model: String,
                messages: Vec<LlamaMsg>,
                temperature: f32,
                max_tokens: usize,
                stream: bool,
            }
            #[derive(serde::Deserialize)]
            struct LlamaChoice { message: LlamaMsgResp }
            #[derive(serde::Deserialize)]
            struct LlamaMsgResp { content: String }
            #[derive(serde::Deserialize)]
            struct LlamaResp { choices: Vec<LlamaChoice> }

            let mut messages: Vec<LlamaMsg> = Vec::new();
            if let Some(sys) = system_field {
                messages.push(LlamaMsg { role: "system".into(), content: sys });
            }
            messages.push(LlamaMsg { role: "user".into(), content: final_prompt });

            let req_body = LlamaReq {
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
                    Ok(text) => match serde_json::from_str::<LlamaResp>(&text) {
                        Ok(data) => {
                            let content = data.choices.into_iter()
                                .next()
                                .map(|c| c.message.content.trim().to_string())
                                .unwrap_or_default();
                            (content, true)
                        }
                        Err(e) => (
                            format!("llama-server parse error: {} - Raw: {}", e, &text[..text.len().min(200)]),
                            false,
                        ),
                    },
                    Err(e) => (format!("llama-server read error: {}", e), false),
                },
                Err(e) => (
                    format!("llama-server error: {}. Is llama-server running at {}?", e, hw.llama_server_url),
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
            Some(if use_llama { "llama_cpp" } else { "ollama" }),
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
            struct StrictMsg { role: String, content: String }
            #[derive(serde::Serialize)]
            struct StrictReq {
                model: String,
                messages: Vec<StrictMsg>,
                temperature: f32,
                max_tokens: usize,
                stream: bool,
            }
            #[derive(serde::Deserialize)]
            struct StrictChoice { message: StrictMsgResp }
            #[derive(serde::Deserialize)]
            struct StrictMsgResp { content: String }
            #[derive(serde::Deserialize)]
            struct StrictResp { choices: Vec<StrictChoice> }

            let messages = vec![
                StrictMsg { role: "system".into(), content: system.clone() },
                StrictMsg { role: "user".into(), content: prompt },
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
                            let content = data.choices.into_iter()
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
                    format!("llama-server error: {}. Is llama-server running at {}?", e, hw.llama_server_url),
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

fn naive_summarize(_query: &str, chunks: &Vec<String>) -> String {
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
