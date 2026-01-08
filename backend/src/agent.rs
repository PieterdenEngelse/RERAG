use crate::agent_memory::AgentMemory;
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
#[derive(Debug, Clone, Copy, Default)]
pub enum AgentMode {
    /// Only search documents (RAG)
    Rag,
    /// Only use LLM (no document search)
    Llm,
    /// Combine: search documents + LLM fallback/enhancement
    #[default]
    Hybrid,
}

pub struct Agent<'a> {
    pub agent_id: &'a str,
    pub memory_db_path: &'a str,
    pub retriever: Arc<Mutex<Retriever>>,
}

impl<'a> Agent<'a> {
    pub fn new(
        agent_id: &'a str,
        memory_db_path: &'a str,
        retriever: Arc<Mutex<Retriever>>,
    ) -> Self {
        Self {
            agent_id,
            memory_db_path,
            retriever,
        }
    }

    /// Run with default hybrid mode (backward compatible)
    pub fn run(&self, query: &str, top_k: usize) -> AgentResponse {
        self.run_with_mode(query, top_k, AgentMode::Hybrid)
    }

    /// Run with specified mode
    pub fn run_with_mode(&self, query: &str, top_k: usize, mode: AgentMode) -> AgentResponse {
        let mut steps = Vec::new();

        // Step 1: Recall recent memory (always do this)
        let recalled: Vec<String> = if let Ok(mem) = AgentMemory::new(self.memory_db_path) {
            mem.recall(self.agent_id)
                .map(|items| items.into_iter().take(5).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if !recalled.is_empty() {
            steps.push(AgentStep {
                kind: "memory".into(),
                message: format!("Recalled {} memory items", recalled.len()),
            });
        }

        // Handle LLM-only mode
        if matches!(mode, AgentMode::Llm) {
            steps.push(AgentStep {
                kind: "mode".into(),
                message: "LLM-only mode (no document search)".into(),
            });
            let answer = self.call_llm(query, None, mode);
            steps.push(AgentStep {
                kind: "llm".into(),
                message: "Generated response with LLM".into(),
            });
            self.store_memory(query, &answer);
            self.store_episode(query, &answer, 0, true);
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
            if let Ok(mut r) = self.retriever.lock() {
                match r.hybrid_search(query, None) {
                    Ok(mut results) => {
                        if results.len() > top_k {
                            results.truncate(top_k);
                        }
                        used_chunks = results;
                        retrieval_msg = format!("Retrieved {} chunks", used_chunks.len());
                    }
                    Err(e) => {
                        retrieval_msg = format!("Retrieval failed: {}", e);
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
                    let answer = "I couldn't find relevant information in the knowledge base.".to_string();
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
                    let answer = self.call_llm(query, None, mode);
                    steps.push(AgentStep {
                        kind: "llm".into(),
                        message: "Generated response with LLM (fallback)".into(),
                    });
                    self.store_memory(query, &answer);
                    self.store_episode(query, &answer, 0, true);
                    return AgentResponse {
                        answer,
                        steps,
                        used_chunks,
                    };
                }
                AgentMode::Llm => unreachable!(), // Already handled above
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
                    message: format!("Generating answer with LLM using {} chunks as context", used_chunks.len()),
                });
                self.call_llm(query, Some(&context), mode)
            }
            AgentMode::Llm => unreachable!(), // Already handled above
        };

        // Step 5: Store memory
        self.store_memory(query, &answer);
        self.store_episode(query, &answer, used_chunks.len(), true);
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
    fn call_llm(&self, query: &str, context: Option<&str>, mode: AgentMode) -> String {
        use crate::db::llm_settings::LlmConfig;
        
        // Get mode-specific config
        let config = match mode {
            AgentMode::Rag => LlmConfig::documents_only(),
            AgentMode::Llm => LlmConfig::llm_only(),
            AgentMode::Hybrid => LlmConfig::combined(),
        };
        
        // Get Ollama URL from environment or use default
        let ollama_url = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "phi:latest".to_string());
        
        // Build prompt with optional context
        let prompt = match context {
            Some(ctx) => format!(
                "Use the following context to answer the question. If the context doesn't contain relevant information, answer based on your knowledge.\n\nContext:\n{}\n\nQuestion: {}\n\nAnswer:",
                ctx, query
            ),
            None => query.to_string(),
        };
        
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
        }
        
        #[derive(serde::Deserialize, Debug)]
        struct OllamaResponse {
            response: String,
            #[serde(default)]
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
        };
        
        match client.post(&url).json(&request_body).send() {
            Ok(response) => {
                // Get the response text first
                match response.text() {
                    Ok(text) => {
                        // Try to parse as JSON
                        match serde_json::from_str::<OllamaResponse>(&text) {
                            Ok(data) => data.response.trim().to_string(),
                            Err(e) => format!("Failed to parse LLM response: {} - Raw: {}", e, &text[..text.len().min(200)]),
                        }
                    }
                    Err(e) => format!("Failed to read LLM response: {}", e),
                }
            }
            Err(e) => format!("LLM error: {}. Make sure Ollama is running.", e),
        }
    }

    fn store_memory(&self, query: &str, answer: &str) {
        if let Ok(mem) = AgentMemory::new(self.memory_db_path) {
            let ts = Utc::now().to_rfc3339();
            let _ = mem.store(self.agent_id, &format!("Q: {}", query), &ts);
            let _ = mem.store(self.agent_id, &format!("A: {}", answer), &ts);
        }
    }

    /// Store episode for monitoring dashboard
    fn store_episode(&self, query: &str, response: &str, chunks_used: usize, success: bool) {
        if let Ok(conn) = Connection::open(self.memory_db_path) {
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
