//! Tantivy search tool — wraps Retriever::hybrid_search()

use crate::retriever::Retriever;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
pub struct SearchArgs {
    /// The search query to find relevant documents
    pub query: String,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub count: usize,
    pub context: String,
}

#[derive(Debug, thiserror::Error)]
#[error("Search error: {0}")]
pub struct SearchError(pub String);

pub struct TantivySearchTool {
    retriever: Arc<Mutex<Retriever>>,
    top_k: usize,
}

impl TantivySearchTool {
    pub fn new(retriever: Arc<Mutex<Retriever>>, top_k: usize) -> Self {
        Self { retriever, top_k }
    }
}

impl Tool for TantivySearchTool {
    const NAME: &'static str = "search_documents";
    type Error = SearchError;
    type Args = SearchArgs;
    type Output = SearchResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "search_documents".to_string(),
            description: "Search the local document knowledge base using full-text and semantic search. Use this when the user asks a question that might be answered by uploaded documents.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to find relevant documents"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let retriever = self.retriever.clone();
        let top_k = self.top_k;
        let query = args.query;

        let result = tokio::task::spawn_blocking(move || {
            let mut r = retriever.lock().map_err(|e| SearchError(e.to_string()))?;
            let mut results = r
                .hybrid_search(&query, None)
                .map_err(|e| SearchError(e.to_string()))?;
            if results.len() > top_k {
                results.truncate(top_k);
            }
            Ok::<_, SearchError>(results)
        })
        .await
        .map_err(|e| SearchError(format!("Task join error: {}", e)))?;

        let chunks = result?;
        let count = chunks.len();
        let context = if chunks.is_empty() {
            "No relevant documents found.".to_string()
        } else {
            chunks.join("\n\n---\n\n")
        };

        Ok(SearchResult { count, context })
    }
}
