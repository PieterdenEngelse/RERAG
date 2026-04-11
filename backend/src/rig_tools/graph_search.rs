//! Graph search tool — wraps Retriever::graph_search() (petgraph)

use crate::retriever::Retriever;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
pub struct GraphArgs {
    /// Entity names to look up in the knowledge graph
    pub entities: Vec<String>,
}

#[derive(Serialize)]
pub struct GraphResult {
    pub count: usize,
    pub relations: String,
}

#[derive(Debug, thiserror::Error)]
#[error("Graph search error: {0}")]
pub struct GraphError(pub String);

pub struct GraphSearchTool {
    retriever: Arc<Mutex<Retriever>>,
}

impl GraphSearchTool {
    pub fn new(retriever: Arc<Mutex<Retriever>>) -> Self {
        Self { retriever }
    }
}

impl Tool for GraphSearchTool {
    const NAME: &'static str = "search_knowledge_graph";
    type Error = GraphError;
    type Args = GraphArgs;
    type Output = GraphResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "search_knowledge_graph".to_string(),
            description: "Search the knowledge graph for relationships between entities. Use this when you need to find how concepts, people, or topics are related to each other.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "entities": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Entity names to look up (e.g. ['Rust', 'Tantivy'])"
                    }
                },
                "required": ["entities"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let retriever = self.retriever.clone();
        let entities = args.entities.clone();
        let query_label = args.entities.join(", ");
        let t0 = std::time::Instant::now();

        let result = tokio::task::spawn_blocking(move || {
            let r = retriever.lock().map_err(|e| GraphError(e.to_string()))?;
            let results = r.graph_search(
                &entities
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            );
            Ok::<_, GraphError>(results)
        })
        .await
        .map_err(|e| GraphError(format!("Task join error: {}", e)))?;

        let elapsed = t0.elapsed().as_millis() as u64;
        crate::monitoring::rig_stats::record_rig_tool_call();

        match result {
            Ok(relations) => {
                let count = relations.len();
                crate::monitoring::record_tool_execution(
                    "RigGraphSearch",
                    &query_label,
                    true,
                    &format!("{} relations", count),
                    elapsed,
                    if count > 0 { 1.0 } else { 0.3 },
                    Some("rig_graph"),
                );
                let text = if relations.is_empty() {
                    "No relationships found for the given entities.".to_string()
                } else {
                    relations.join("\n")
                };
                Ok(GraphResult {
                    count,
                    relations: text,
                })
            }
            Err(e) => {
                crate::monitoring::record_tool_execution(
                    "RigGraphSearch",
                    &query_label,
                    false,
                    &e.to_string(),
                    elapsed,
                    0.0,
                    Some("rig_graph"),
                );
                Err(e)
            }
        }
    }
}
