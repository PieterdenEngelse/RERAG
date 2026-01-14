// src/tools/tool_executor.rs - FIXED
// Phase 10: Execute individual tools in a chain
// Handles actual tool execution with result passing

use crate::tools::calculator::CalculatorTool;
use crate::tools::classifier::ClassifierTool;
use crate::tools::code_execution::CodeExecutionTool;
use crate::tools::database_query::DatabaseQueryTool;
use crate::tools::entity_extractor::EntityExtractorTool;
use crate::tools::file_analyzer::FileAnalyzerTool;
use crate::tools::image_generation::ImageGenerationTool;
use crate::tools::memory_tool::MemoryTool;
use crate::tools::notification::NotificationTool;
use crate::tools::query_rewriter::QueryRewriterTool;
use crate::tools::scheduler::SchedulerTool;
use crate::tools::semantic_search::SemanticSearchTool;
use crate::tools::sentiment::SentimentAnalyzerTool;
use crate::tools::spell_checker::SpellCheckerTool;
use crate::tools::summarizer::SummarizerTool;
use crate::tools::translator::TranslatorTool;
use crate::tools::url_fetch::URLFetchTool;
use crate::tools::web_search::WebSearchTool;
use crate::tools::{Tool, ToolResult, ToolType};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub step_number: usize,
    pub tool: ToolType,
    pub query: String,
    pub previous_result: Option<String>,
}

pub struct ToolExecutor;

impl ToolExecutor {
    /// Execute a single tool with context
    pub async fn execute_tool(
        tool_type: &ToolType,
        query: &str,
        _previous_result: Option<&str>,
    ) -> Result<ToolResult, String> {
        let start = Instant::now();

        // Use query as-is, don't append context
        let input_query = query.to_string();

        // Execute appropriate tool
        let result = match tool_type {
            ToolType::Calculator => {
                let calculator = CalculatorTool::new();
                calculator.execute(&input_query).await?
            }
            ToolType::WebSearch => {
                let web_search = WebSearchTool::new();
                web_search.execute(&input_query).await?
            }
            ToolType::URLFetch => {
                let url_fetch = URLFetchTool::new();
                url_fetch.execute(&input_query).await?
            }
            ToolType::SemanticSearch => {
                let semantic_search = SemanticSearchTool::new();
                semantic_search.execute(&input_query).await?
            }
            ToolType::DatabaseQuery => {
                let db_query = DatabaseQueryTool::new();
                db_query.execute(&input_query).await?
            }
            ToolType::CodeExecution => {
                let code_exec = CodeExecutionTool::new();
                code_exec.execute(&input_query).await?
            }
            ToolType::ImageGeneration => {
                let image_gen = ImageGenerationTool::new();
                image_gen.execute(&input_query).await?
            }
            ToolType::Summarizer => {
                let summarizer = SummarizerTool::new();
                summarizer.execute(&input_query).await?
            }
            ToolType::QueryRewriter => {
                let rewriter = QueryRewriterTool::new();
                rewriter.execute(&input_query).await?
            }
            ToolType::Classifier => {
                let classifier = ClassifierTool::new();
                classifier.execute(&input_query).await?
            }
            ToolType::FileAnalyzer => {
                let analyzer = FileAnalyzerTool::new();
                analyzer.execute(&input_query).await?
            }
            ToolType::Notification => {
                let notifier = NotificationTool::new();
                notifier.execute(&input_query).await?
            }
            ToolType::Translator => {
                let translator = TranslatorTool::new();
                translator.execute(&input_query).await?
            }
            ToolType::SentimentAnalyzer => {
                let sentiment = SentimentAnalyzerTool::new();
                sentiment.execute(&input_query).await?
            }
            ToolType::EntityExtractor => {
                let extractor = EntityExtractorTool::new();
                extractor.execute(&input_query).await?
            }
            ToolType::SpellChecker => {
                let spell_checker = SpellCheckerTool::new();
                spell_checker.execute(&input_query).await?
            }
            ToolType::Scheduler => {
                let scheduler = SchedulerTool::new();
                scheduler.execute(&input_query).await?
            }
            ToolType::Memory => {
                let memory = MemoryTool::new(None);
                memory.execute(&input_query).await?
            }
        };

        // Record execution for monitoring
        let execution_time = start.elapsed().as_millis() as u64;
        crate::monitoring::record_tool_execution(
            &tool_type.to_string(),
            query,
            result.success,
            &result.result,
            execution_time,
            result.metadata.confidence,
            result.metadata.source.as_deref(),
        );
        crate::monitoring::record_execution(
            &tool_type.to_string(),
            result.success,
            execution_time,
            result.metadata.confidence,
            result.metadata.cost.unwrap_or(0.0) as f64,
        );
        if let Some(cost) = result.metadata.cost {
            if cost > 0.0 {
                crate::monitoring::record_tool_cost(&tool_type.to_string(), cost);
            }
        }

        Ok(result)
    }

    /// Extract relevant data from tool result
    pub fn extract_data(result: &str) -> String {
        // Try to extract numbers if it's a calculation result
        if let Some(pos) = result.rfind('=') {
            let number_part = &result[pos + 1..].trim();
            if number_part.chars().all(|c| c.is_numeric() || c == '.') {
                return number_part.to_string();
            }
        }

        // Otherwise return the whole result
        result.to_string()
    }

    /// Validate tool result
    pub fn validate_result(result: &ToolResult) -> bool {
        result.success && !result.result.is_empty()
    }

    /// Retry tool execution with fallback
    pub async fn execute_with_fallback(
        primary_tool: &ToolType,
        fallback_tools: &[ToolType],
        query: &str,
        previous_result: Option<&str>,
    ) -> Result<ToolResult, String> {
        // Try primary tool
        match Self::execute_tool(primary_tool, query, previous_result).await {
            Ok(result) if Self::validate_result(&result) => {
                return Ok(result);
            }
            _ => {
                // Try fallback tools
                for fallback_tool in fallback_tools {
                    match Self::execute_tool(fallback_tool, query, previous_result).await {
                        Ok(result) if Self::validate_result(&result) => {
                            return Ok(result);
                        }
                        _ => continue,
                    }
                }
            }
        }

        Err(format!(
            "All tools failed. Primary: {:?}, Fallbacks: {:?}",
            primary_tool, fallback_tools
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_calculator() {
        let result = ToolExecutor::execute_tool(&ToolType::Calculator, "5 + 3", None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[tokio::test]
    async fn test_execute_web_search() {
        let result = ToolExecutor::execute_tool(&ToolType::WebSearch, "AI papers", None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[test]
    fn test_extract_data() {
        let result = "5 + 3 = 8";
        let extracted = ToolExecutor::extract_data(result);
        assert_eq!(extracted, "8");
    }

    #[test]
    fn test_validate_result() {
        let valid_result = ToolResult {
            tool: ToolType::Calculator,
            success: true,
            result: "8".to_string(),
            metadata: crate::tools::ToolMetadata {
                execution_time_ms: 100,
                confidence: 0.99,
                source: None,
                cost: None,
            },
        };

        assert!(ToolExecutor::validate_result(&valid_result));
    }
}
