//! Memory recall tool — wraps AgentMemory::recall_rag()

use crate::agent_memory::AgentMemory;
use crate::db::path_resolver;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct RecallArgs {
    /// Number of recent memories to recall (max 20)
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct RecallResult {
    pub count: usize,
    pub memories: Vec<MemoryEntry>,
}

#[derive(Serialize)]
pub struct MemoryEntry {
    pub content: String,
    pub category: String,
    pub timestamp: String,
}

#[derive(Debug, thiserror::Error)]
#[error("Memory recall error: {0}")]
pub struct RecallError(pub String);

pub struct MemoryRecallTool;

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
            description: "Recall recent memories from past conversations. Use this to check if you have previously discussed a topic with the user or to retrieve context from earlier interactions.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of recent memories to recall (default 10, max 20)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let limit = args.limit.unwrap_or(10).min(20);
        let db_path = path_resolver::agent_db_path_str();

        let result = tokio::task::spawn_blocking(move || {
            let mem = AgentMemory::new(&db_path).map_err(|e| RecallError(e.to_string()))?;
            let items = mem
                .recall_rag("default", limit)
                .map_err(|e| RecallError(e.to_string()))?;
            Ok::<_, RecallError>(items)
        })
        .await
        .map_err(|e| RecallError(format!("Task join error: {}", e)))?;

        let items = result?;
        let count = items.len();
        let memories = items
            .into_iter()
            .map(|item| MemoryEntry {
                content: item.content,
                category: item.memory_type.clone(),
                timestamp: item.timestamp,
            })
            .collect();

        Ok(RecallResult { count, memories })
    }
}
