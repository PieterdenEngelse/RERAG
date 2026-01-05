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

    pub fn run(&self, query: &str, top_k: usize) -> AgentResponse {
        let mut steps = Vec::new();

        // Step 1: Recall recent memory
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

        // Step 2: Retrieve relevant chunks
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

        // Step 3: (Optional) Simple planning: if no chunks, fallback
        if used_chunks.is_empty() {
            let answer = "I couldn't find relevant information in the knowledge base.".to_string();
            steps.push(AgentStep {
                kind: "plan".into(),
                message: "No chunks found; returning fallback".into(),
            });
            self.store_memory(query, &answer);
            self.store_episode(query, &answer, 0, false); // Store failed episode
            return AgentResponse {
                answer,
                steps,
                used_chunks,
            };
        }

        // Step 4: Summarize (naive){ join key lines }
        let answer = naive_summarize(query, &used_chunks);
        steps.push(AgentStep {
            kind: "summarize".into(),
            message: format!("Summarized {} chunks", used_chunks.len()),
        });

        // Step 5: Store memory
        self.store_memory(query, &answer);
        self.store_episode(query, &answer, used_chunks.len(), true); // Store successful episode
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
