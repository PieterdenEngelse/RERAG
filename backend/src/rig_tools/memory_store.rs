//! Memory store tool — wraps AgentMemory::store_rag()

use crate::agent_memory::AgentMemory;
use crate::db::path_resolver;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct StoreArgs {
    /// The content to remember
    pub content: String,
    /// Category for the memory (e.g. "fact", "preference", "conversation")
    pub category: Option<String>,
}

#[derive(Serialize)]
pub struct StoreResult {
    pub stored: bool,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
#[error("Memory store error: {0}")]
pub struct StoreError(pub String);

pub struct MemoryStoreTool;

impl Default for MemoryStoreTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStoreTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for MemoryStoreTool {
    const NAME: &'static str = "store_memory";
    type Error = StoreError;
    type Args = StoreArgs;
    type Output = StoreResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "store_memory".to_string(),
            description: "Store important information for future reference. Use this to remember facts, user preferences, or key findings from the current conversation.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The information to remember"
                    },
                    "category": {
                        "type": "string",
                        "description": "Category: fact, preference, conversation, or note"
                    }
                },
                "required": ["content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let content = args.content.clone();
        let category = args
            .category
            .clone()
            .unwrap_or_else(|| "conversation".to_string());
        let db_path = path_resolver::agent_db_path_str();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let t0 = std::time::Instant::now();

        let cat_clone = category.clone();
        let content_clone = content.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mem = AgentMemory::new(db_path).map_err(|e| StoreError(e.to_string()))?;
            mem.store_rag("default", &content_clone, &cat_clone, &timestamp)
                .map_err(|e| StoreError(e.to_string()))?;
            Ok::<_, StoreError>(())
        })
        .await
        .map_err(|e| StoreError(format!("Task join error: {}", e)))?;

        let elapsed = t0.elapsed().as_millis() as u64;
        crate::monitoring::rig_stats::record_rig_tool_call();

        match result {
            Ok(()) => {
                crate::monitoring::record_tool_execution(
                    "RigMemoryStore",
                    &content,
                    true,
                    &format!("stored in '{}'", category),
                    elapsed,
                    1.0,
                    Some("rig_memory"),
                );
                Ok(StoreResult {
                    stored: true,
                    message: format!("Stored memory in category '{}'", category),
                })
            }
            Err(e) => {
                crate::monitoring::record_tool_execution(
                    "RigMemoryStore",
                    &content,
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
