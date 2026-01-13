// src/tools/semantic_search.rs
// Semantic Search Tool - Uses the retriever for vector similarity search

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::debug;

use crate::retriever::Retriever;

#[derive(Clone)]
pub struct SemanticSearchTool {
    retriever: Option<Arc<Mutex<Retriever>>>,
    top_k: usize,
    success_count: usize,
    total_count: usize,
}

impl SemanticSearchTool {
    pub fn new() -> Self {
        Self {
            retriever: None,
            top_k: 5,
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn with_retriever(retriever: Arc<Mutex<Retriever>>) -> Self {
        Self {
            retriever: Some(retriever),
            top_k: 5,
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn set_top_k(&mut self, k: usize) {
        self.top_k = k;
    }

    /// Try to get the global retriever handle if not set
    fn get_retriever(&self) -> Option<Arc<Mutex<Retriever>>> {
        if self.retriever.is_some() {
            return self.retriever.clone();
        }
        // Try to get from global handle
        crate::api::get_retriever_handle()
    }
}

impl std::fmt::Debug for SemanticSearchTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemanticSearchTool")
            .field("top_k", &self.top_k)
            .field("has_retriever", &self.retriever.is_some())
            .field("success_count", &self.success_count)
            .field("total_count", &self.total_count)
            .finish()
    }
}

#[async_trait]
impl Tool for SemanticSearchTool {
    fn tool_type(&self) -> ToolType {
        ToolType::SemanticSearch
    }

    fn description(&self) -> String {
        "Search indexed documents using semantic similarity to find relevant content".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.85
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();
        debug!("SemanticSearchTool: executing query '{}'", query);

        let retriever_handle = match self.get_retriever() {
            Some(r) => r,
            None => {
                return Ok(ToolResult {
                    tool: ToolType::SemanticSearch,
                    success: false,
                    result: "Retriever not available. Index documents first.".to_string(),
                    metadata: ToolMetadata {
                        execution_time_ms: start.elapsed().as_millis() as u64,
                        confidence: 0.0,
                        source: Some("SemanticSearch".to_string()),
                        cost: Some(0.0),
                    },
                });
            }
        };

        // Perform the search
        let search_results = match retriever_handle.lock() {
            Ok(mut retriever) => {
                match retriever.search(query) {
                    Ok(results) => results,
                    Err(e) => {
                        return Ok(ToolResult {
                            tool: ToolType::SemanticSearch,
                            success: false,
                            result: format!("Search failed: {}", e),
                            metadata: ToolMetadata {
                                execution_time_ms: start.elapsed().as_millis() as u64,
                                confidence: 0.0,
                                source: Some("SemanticSearch".to_string()),
                                cost: Some(0.0),
                            },
                        });
                    }
                }
            }
            Err(e) => {
                return Ok(ToolResult {
                    tool: ToolType::SemanticSearch,
                    success: false,
                    result: format!("Failed to acquire retriever lock: {}", e),
                    metadata: ToolMetadata {
                        execution_time_ms: start.elapsed().as_millis() as u64,
                        confidence: 0.0,
                        source: Some("SemanticSearch".to_string()),
                        cost: Some(0.0),
                    },
                });
            }
        };

        if search_results.is_empty() {
            return Ok(ToolResult {
                tool: ToolType::SemanticSearch,
                success: true,
                result: format!("No results found for query: '{}'", query),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.5,
                    source: Some("SemanticSearch".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        // Format results - search returns Vec<String> (content chunks)
        let mut result_text = format!("Found {} results for '{}':\n\n", search_results.len(), query);

        for (i, content) in search_results.iter().take(self.top_k).enumerate() {
            // Truncate content for display
            let content_preview = if content.len() > 300 {
                format!("{}...", &content[..300])
            } else {
                content.clone()
            };

            result_text.push_str(&format!(
                "{}. {}\n\n",
                i + 1,
                content_preview,
            ));
        }

        Ok(ToolResult {
            tool: ToolType::SemanticSearch,
            success: true,
            result: result_text,
            metadata: ToolMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                confidence: 0.8,
                source: Some("SemanticSearch".to_string()),
                cost: Some(0.0),
            },
        })
    }

    fn update_success(&mut self, success: bool) {
        self.total_count += 1;
        if success {
            self.success_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_semantic_search_no_retriever() {
        let tool = SemanticSearchTool::new();
        let result = tool.execute("test query").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        // Should fail gracefully without retriever
        assert!(!res.success || res.result.contains("not available"));
    }
}
