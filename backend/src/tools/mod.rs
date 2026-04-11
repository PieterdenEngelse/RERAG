// src/tools/mod.rs
// Phase 9: Tool Registry and Interfaces
// Provides tool abstraction for agent decision engine

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod calculator;
pub mod classifier;
pub mod code_execution;
pub mod database_query;
pub mod entity_extractor;
pub mod file_analyzer;
pub mod image_generation;
pub mod memory_tool;
pub mod ner_extractor;
pub mod notification;
pub mod query_optimizer;
pub mod query_rewriter;
pub mod result_cache;
pub mod result_compressor;
pub mod result_formatter;
pub mod scheduler;
pub mod semantic_search;
pub mod sentiment;
pub mod spell_checker;
pub mod summarizer;
pub mod tool_cache;
pub mod tool_chain_executor;
pub mod tool_composer;
pub mod tool_executor;
pub mod tool_permissions;
pub mod tool_rate_limiter;
pub mod tool_selector;
pub mod translator;
pub mod url_fetch;
pub mod web_search;
pub use query_optimizer::QueryOptimizer;
pub use result_compressor::ResultCompressor;
pub mod connection_pool;
pub mod rate_limiter;
pub use connection_pool::ConnectionPool;
pub use rate_limiter::RateLimiter;

// Re-export agent tools
pub use classifier::ClassifierTool;
pub use entity_extractor::EntityExtractorTool;
pub use file_analyzer::FileAnalyzerTool;
pub use memory_tool::MemoryTool;
pub use notification::NotificationTool;
pub use query_rewriter::QueryRewriterTool;
pub use scheduler::SchedulerTool;
pub use sentiment::SentimentAnalyzerTool;
pub use spell_checker::SpellCheckerTool;
pub use summarizer::SummarizerTool;
pub use translator::TranslatorTool;

// ============ Tool Types ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolType {
    SemanticSearch,
    WebSearch,
    DatabaseQuery,
    Calculator,
    URLFetch,
    CodeExecution,
    ImageGeneration,
    // Agent tools
    Summarizer,
    QueryRewriter,
    Classifier,
    FileAnalyzer,
    Notification,
    Translator,
    SentimentAnalyzer,
    EntityExtractor,
    SpellChecker,
    Scheduler,
    Memory,
}

impl ToString for ToolType {
    fn to_string(&self) -> String {
        match self {
            ToolType::SemanticSearch => "SemanticSearch".to_string(),
            ToolType::WebSearch => "WebSearch".to_string(),
            ToolType::DatabaseQuery => "DatabaseQuery".to_string(),
            ToolType::Calculator => "Calculator".to_string(),
            ToolType::URLFetch => "URLFetch".to_string(),
            ToolType::CodeExecution => "CodeExecution".to_string(),
            ToolType::ImageGeneration => "ImageGeneration".to_string(),
            ToolType::Summarizer => "Summarizer".to_string(),
            ToolType::QueryRewriter => "QueryRewriter".to_string(),
            ToolType::Classifier => "Classifier".to_string(),
            ToolType::FileAnalyzer => "FileAnalyzer".to_string(),
            ToolType::Notification => "Notification".to_string(),
            ToolType::Translator => "Translator".to_string(),
            ToolType::SentimentAnalyzer => "SentimentAnalyzer".to_string(),
            ToolType::EntityExtractor => "EntityExtractor".to_string(),
            ToolType::SpellChecker => "SpellChecker".to_string(),
            ToolType::Scheduler => "Scheduler".to_string(),
            ToolType::Memory => "Memory".to_string(),
        }
    }
}

// ============ Tool Result ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: ToolType,
    pub success: bool,
    pub result: String,
    pub metadata: ToolMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    pub execution_time_ms: u64,
    pub confidence: f32,
    pub source: Option<String>,
    pub cost: Option<f32>,
}

// ============ Tool Trait ============

#[async_trait]
pub trait Tool: Send + Sync {
    fn tool_type(&self) -> ToolType;
    fn description(&self) -> String;
    fn success_rate(&self) -> f32;

    async fn execute(&self, query: &str) -> Result<ToolResult, String>;

    fn update_success(&mut self, success: bool);
}

// ============ Tool Registry ============

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
    tool_stats: std::collections::HashMap<String, ToolStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStats {
    pub name: String,
    pub executions: usize,
    pub successes: usize,
    pub avg_time_ms: f32,
    pub avg_confidence: f32,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            tool_stats: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.tool_type().to_string();
        self.tools.push(tool);
        self.tool_stats.insert(
            name.clone(),
            ToolStats {
                name,
                executions: 0,
                successes: 0,
                avg_time_ms: 0.0,
                avg_confidence: 0.0,
            },
        );
    }

    pub fn get_tool(&self, tool_type: &ToolType) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| &t.tool_type() == tool_type)
            .map(|t| t.as_ref())
    }

    pub fn list_available(&self) -> Vec<(ToolType, String, f32)> {
        self.tools
            .iter()
            .map(|t| (t.tool_type(), t.description(), t.success_rate()))
            .collect()
    }

    pub fn get_stats(&self, tool_type: &ToolType) -> Option<ToolStats> {
        let name = tool_type.to_string();
        self.tool_stats.get(&name).cloned()
    }

    pub fn update_stats(
        &mut self,
        tool_type: &ToolType,
        time_ms: u64,
        success: bool,
        confidence: f32,
    ) {
        let name = tool_type.to_string();
        if let Some(stats) = self.tool_stats.get_mut(&name) {
            stats.executions += 1;
            if success {
                stats.successes += 1;
            }
            stats.avg_time_ms = (stats.avg_time_ms * (stats.executions - 1) as f32
                + time_ms as f32)
                / stats.executions as f32;
            stats.avg_confidence = (stats.avg_confidence * (stats.executions - 1) as f32
                + confidence)
                / stats.executions as f32;
        }
    }
}

// ============ Reasoning Step ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub step_number: usize,
    pub description: String,
    pub tool: ToolType,
    pub query: String,
    pub result: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainOfThought {
    pub steps: Vec<ReasoningStep>,
    pub final_answer: String,
    pub total_confidence: f32,
}
