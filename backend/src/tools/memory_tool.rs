// src/tools/memory_tool.rs
// Feature #18: MemoryTool - store/search RAG memory from the agent DB

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agent_memory::AgentMemory;
use crate::db::path_resolver;
use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryToolConfig {
    pub default_agent_id: String,
    pub default_memory_type: String,
}

impl Default for MemoryToolConfig {
    fn default() -> Self {
        Self {
            default_agent_id: "default".to_string(),
            default_memory_type: "context".to_string(),
        }
    }
}

pub struct MemoryTool {
    success_rate: f32,
    config: MemoryToolConfig,
}

impl MemoryTool {
    pub fn new(config: Option<MemoryToolConfig>) -> Self {
        Self {
            success_rate: 0.9,
            config: config.unwrap_or_default(),
        }
    }

    fn open_memory(&self) -> Result<AgentMemory, String> {
        let path = path_resolver::agent_db_path();
        AgentMemory::new(
            path.to_str()
                .ok_or_else(|| "Invalid agent DB path".to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    fn parse_command<'a>(&self, query: &'a str) -> (&'a str, &'a str) {
        if let Some(rest) = query.strip_prefix("store ") {
            ("store", rest)
        } else if let Some(rest) = query.strip_prefix("search ") {
            ("search", rest)
        } else if let Some(rest) = query.strip_prefix("forget ") {
            ("forget", rest)
        } else {
            ("search", query)
        }
    }

    fn store_memory(&self, content: &str) -> Result<String, String> {
        let memory = self.open_memory()?;
        let timestamp = chrono::Utc::now().to_rfc3339();
        memory
            .store_rag(
                &self.config.default_agent_id,
                &self.config.default_memory_type,
                content,
                &timestamp,
            )
            .map_err(|e| e.to_string())?;
        Ok(format!("Stored memory entry at {}", timestamp))
    }

    fn search_memory(&self, query: &str) -> Result<String, String> {
        let memory = self.open_memory()?;
        let mut results = memory
            .recall_rag(&self.config.default_agent_id, 50)
            .map_err(|e| e.to_string())?;

        if results.is_empty() {
            return Ok("No memories stored yet.".to_string());
        }

        if !query.is_empty() {
            results.retain(|item| item.content.to_lowercase().contains(&query.to_lowercase()));
        }

        if results.is_empty() {
            return Ok(format!("No memories matching '{}'.", query));
        }

        let preview: Vec<String> = results
            .into_iter()
            .take(10)
            .map(|item| format!("- [{}] {}", item.timestamp, item.content))
            .collect();

        if query.is_empty() {
            Ok(format!("Recent memories:\n{}", preview.join("\n")))
        } else {
            Ok(format!(
                "Memories matching '{}':\n{}",
                query,
                preview.join("\n")
            ))
        }
    }

    fn forget_topic(&self, topic: &str) -> Result<String, String> {
        if topic.is_empty() {
            return Err("Provide a topic to forget, e.g. 'memory forget API key'.".to_string());
        }
        let memory = self.open_memory()?;
        let removed = memory
            .forget_topic(&self.config.default_agent_id, topic)
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Removed {} entries containing '{}'.",
            removed, topic
        ))
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Memory
    }

    fn description(&self) -> String {
        "Store, search, or forget agent memories. Example commands: 'memory store user likes rust', 'memory search rust', 'memory forget password'.".to_string()
    }

    fn success_rate(&self) -> f32 {
        self.success_rate
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = std::time::Instant::now();
        if query.trim().is_empty() {
            return Err(
                "Provide a memory command, e.g. 'memory store user prefers tabs'.".to_string(),
            );
        }

        let (cmd, rest) = self.parse_command(query.trim());
        let response = match cmd {
            "store" => self.store_memory(rest.trim())?,
            "forget" => self.forget_topic(rest.trim())?,
            _ => self.search_memory(rest.trim())?,
        };

        Ok(ToolResult {
            tool: ToolType::Memory,
            success: true,
            result: response,
            metadata: ToolMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                confidence: 0.9,
                source: Some("agent-memory".to_string()),
                cost: Some(0.0),
            },
        })
    }

    fn update_success(&mut self, success: bool) {
        if success {
            self.success_rate = (self.success_rate * 0.95) + 0.05;
        } else {
            self.success_rate *= 0.95;
        }
    }
}
