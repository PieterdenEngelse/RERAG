//! Memory recall tool — wraps AgentMemory::search_rag (semantic) and recall_rag (recency).

use crate::agent_memory::AgentMemory;
use crate::db::path_resolver;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct RecallArgs {
    /// Free-text query; when present, recall is ranked by embedding similarity.
    /// When absent, the most recent memories are returned.
    pub query: Option<String>,
    /// Number of memories to recall (max 20).
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct RecallResult {
    pub count: usize,
    pub mode: &'static str,
    pub memories: Vec<MemoryEntry>,
}

#[derive(Serialize)]
pub struct MemoryEntry {
    pub content: String,
    pub category: String,
    pub timestamp: String,
    /// Cosine similarity vs. the recall query; `None` for recency-mode recalls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

#[derive(Debug, thiserror::Error)]
#[error("Memory recall error: {0}")]
pub struct RecallError(pub String);

pub struct MemoryRecallTool;

impl Default for MemoryRecallTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryRecallTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for MemoryRecallTool {
    const NAME: &'static str = "recall_memory";
    type Error = RecallError;
    type Args = RecallArgs;
    type Output = RecallResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "recall_memory".to_string(),
            description: "Recall memories from past conversations. Pass a `query` to get the most semantically similar memories (embedding cosine match); omit it to get the most recent memories. Use this to check whether a topic has come up before or to retrieve relevant prior context.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Free-text query to match against past memories by semantic similarity. Omit for recency-only recall."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of memories to recall (default 10, max 20)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let limit = args.limit.unwrap_or(10).min(20);
        let query = args.query.clone();
        let db_path = path_resolver::agent_db_path_str();
        let t0 = std::time::Instant::now();

        let result = tokio::task::spawn_blocking(move || {
            let mem = AgentMemory::new(db_path).map_err(|e| RecallError(e.to_string()))?;
            match query {
                Some(q) if !q.trim().is_empty() => {
                    let hits = mem
                        .search_rag("default", &q, limit)
                        .map_err(|e| RecallError(e.to_string()))?;
                    let entries: Vec<MemoryEntry> = hits
                        .into_iter()
                        .map(|h| MemoryEntry {
                            content: h.item.content,
                            category: h.item.memory_type,
                            timestamp: h.item.timestamp,
                            score: Some(h.score),
                        })
                        .collect();
                    Ok::<_, RecallError>(("semantic", entries))
                }
                _ => {
                    let items = mem
                        .recall_rag("default", limit)
                        .map_err(|e| RecallError(e.to_string()))?;
                    let entries: Vec<MemoryEntry> = items
                        .into_iter()
                        .map(|item| MemoryEntry {
                            content: item.content,
                            category: item.memory_type,
                            timestamp: item.timestamp,
                            score: None,
                        })
                        .collect();
                    Ok::<_, RecallError>(("recency", entries))
                }
            }
        })
        .await
        .map_err(|e| RecallError(format!("Task join error: {}", e)))?;

        let elapsed = t0.elapsed().as_millis() as u64;
        crate::monitoring::rig_stats::record_rig_tool_call();

        match result {
            Ok((mode, memories)) => {
                let count = memories.len();
                crate::monitoring::record_tool_execution(
                    "RigMemoryRecall",
                    "recall_memory",
                    true,
                    &format!("{} memories ({})", count, mode),
                    elapsed,
                    if count > 0 { 1.0 } else { 0.3 },
                    Some("rig_memory"),
                );
                Ok(RecallResult {
                    count,
                    mode,
                    memories,
                })
            }
            Err(e) => {
                crate::monitoring::record_tool_execution(
                    "RigMemoryRecall",
                    "recall_memory",
                    false,
                    &e.to_string(),
                    elapsed,
                    0.0,
                    Some("rig_memory"),
                );
                Err(e)
            }
        }
    }
}
