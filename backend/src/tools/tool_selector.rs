// src/tools/tool_selector.rs - UPDATED v2
// Phase 9: Tool Selection Logic (Improved Intent Detection)

use crate::tools::ToolType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelection {
    pub primary_tool: ToolType,
    pub secondary_tools: Vec<ToolType>,
    pub reasoning: String,
    pub confidence: f32,
    pub intent: QueryIntent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueryIntent {
    Math,
    WebSearch,
    UrlFetch,
    Database,
    CodeExecution,
    ImageGeneration,
    Scheduler,
    Memory,
    SemanticSearch,
    Unknown,
}

impl ToString for QueryIntent {
    fn to_string(&self) -> String {
        match self {
            QueryIntent::Math => "Math".to_string(),
            QueryIntent::WebSearch => "WebSearch".to_string(),
            QueryIntent::UrlFetch => "UrlFetch".to_string(),
            QueryIntent::Database => "Database".to_string(),
            QueryIntent::CodeExecution => "CodeExecution".to_string(),
            QueryIntent::ImageGeneration => "ImageGeneration".to_string(),
            QueryIntent::Scheduler => "Scheduler".to_string(),
            QueryIntent::Memory => "Memory".to_string(),
            QueryIntent::SemanticSearch => "SemanticSearch".to_string(),
            QueryIntent::Unknown => "Unknown".to_string(),
        }
    }
}

pub struct ToolSelector;

impl ToolSelector {
    /// Analyze query and detect intent
    pub fn detect_intent(query: &str) -> QueryIntent {
        let q = query.to_lowercase();

        // URL fetch detection
        if Self::is_url_fetch_query(&q) {
            return QueryIntent::UrlFetch;
        }

        // Math detection
        if Self::is_math_query(&q) {
            return QueryIntent::Math;
        }

        // Web search detection
        if Self::is_web_search_query(&q) {
            return QueryIntent::WebSearch;
        }

        // Database query detection
        if Self::is_database_query(&q) {
            return QueryIntent::Database;
        }

        // Code execution detection
        if Self::is_code_execution_query(&q) {
            return QueryIntent::CodeExecution;
        }

        // Image generation detection
        if Self::is_image_generation_query(&q) {
            return QueryIntent::ImageGeneration;
        }

        if Self::is_scheduler_query(&q) {
            return QueryIntent::Scheduler;
        }

        if Self::is_memory_query(&q) {
            return QueryIntent::Memory;
        }

        QueryIntent::SemanticSearch
    }

    /// Select best tool(s) for query
    pub fn select_tools(query: &str) -> ToolSelection {
        let intent = Self::detect_intent(query);

        let (primary, secondary, confidence) = match intent {
            QueryIntent::Math => (ToolType::Calculator, vec![ToolType::SemanticSearch], 0.95),
            QueryIntent::WebSearch => (
                ToolType::WebSearch,
                vec![ToolType::SemanticSearch, ToolType::URLFetch],
                0.85,
            ),
            QueryIntent::UrlFetch => (
                ToolType::URLFetch,
                vec![ToolType::WebSearch, ToolType::SemanticSearch],
                0.80,
            ),
            QueryIntent::Database => (
                ToolType::DatabaseQuery,
                vec![ToolType::SemanticSearch],
                0.75,
            ),
            QueryIntent::CodeExecution => (
                ToolType::CodeExecution,
                vec![ToolType::SemanticSearch],
                0.70,
            ),
            QueryIntent::ImageGeneration => (
                ToolType::ImageGeneration,
                vec![ToolType::SemanticSearch],
                0.65,
            ),
            QueryIntent::Scheduler => (ToolType::Scheduler, vec![ToolType::SemanticSearch], 0.78),
            QueryIntent::Memory => (ToolType::Memory, vec![ToolType::SemanticSearch], 0.70),
            QueryIntent::SemanticSearch => {
                (ToolType::SemanticSearch, vec![ToolType::WebSearch], 0.60)
            }
            QueryIntent::Unknown => (ToolType::SemanticSearch, vec![ToolType::WebSearch], 0.50),
        };

        let reasoning = format!(
            "Query intent: {}. Selected {} (confidence: {:.2}). \
             Fallback tools: {:?}",
            intent.to_string(),
            primary.to_string(),
            confidence,
            secondary.iter().map(|t| t.to_string()).collect::<Vec<_>>()
        );

        ToolSelection {
            primary_tool: primary,
            secondary_tools: secondary,
            reasoning,
            confidence,
            intent,
        }
    }

    // ============ Intent Detection Methods ============

    fn is_math_query(query: &str) -> bool {
        let math_keywords = vec![
            "calculate",
            "compute",
            "math",
            "add",
            "subtract",
            "multiply",
            "divide",
            "plus",
            "minus",
            "times",
            "equals",
            "=",
            "+",
            "-",
            "*",
            "/",
            "sum",
            "product",
            "quotient",
            "percentage",
            "%",
            "count",
        ];

        // Check if it's just a number or contains math operators
        if query
            .trim()
            .chars()
            .all(|c| c.is_numeric() || c == '.' || c == '-')
        {
            return true;
        }

        math_keywords.iter().any(|kw| query.contains(kw))
    }

    fn is_web_search_query(query: &str) -> bool {
        let search_keywords = vec![
            "search",
            "find",
            "look for",
            "what is",
            "who is",
            "where is",
            "when is",
            "how",
            "latest",
            "recent",
            "current",
            "news",
            "trending",
            "popular",
            "top",
            "best",
            "papers",
            "articles",
            "information",
            "research",
            "studies",
            "data",
        ];

        search_keywords.iter().any(|kw| query.contains(kw))
    }

    fn is_url_fetch_query(query: &str) -> bool {
        let fetch_keywords = vec![
            "fetch",
            "get",
            "retrieve",
            "download",
            "read",
            "http://",
            "https://",
            "url",
            "website",
            "page",
            "content from",
            "visit",
            "extract",
        ];

        fetch_keywords.iter().any(|kw| query.contains(kw))
            || query.contains("http://")
            || query.contains("https://")
    }

    fn is_database_query(query: &str) -> bool {
        let db_keywords = vec![
            "query",
            "select",
            "from",
            "where",
            "database",
            "sql",
            "table",
            "record",
            "data",
            "store",
            "retrieve from db",
        ];

        db_keywords.iter().any(|kw| query.contains(kw))
    }

    fn is_code_execution_query(query: &str) -> bool {
        let code_keywords = vec![
            "execute",
            "run",
            "code",
            "script",
            "function",
            "compile",
            "debug",
            "test code",
            "write code",
        ];

        code_keywords.iter().any(|kw| query.contains(kw))
    }

    fn is_image_generation_query(query: &str) -> bool {
        let image_keywords = vec![
            "generate",
            "create image",
            "draw",
            "paint",
            "visualize",
            "image of",
            "picture of",
            "design",
            "artwork",
        ];

        image_keywords.iter().any(|kw| query.contains(kw))
    }

    fn is_scheduler_query(query: &str) -> bool {
        let scheduler_keywords = vec![
            "schedule",
            "remind",
            "reminder",
            "follow up",
            "in 30 minutes",
            "tomorrow",
            "next week",
            "list tasks",
            "show reminders",
            "calendar",
        ];
        scheduler_keywords.iter().any(|kw| query.contains(kw))
    }

    fn is_memory_query(query: &str) -> bool {
        let memory_keywords = vec![
            "memory",
            "remember",
            "store",
            "save note",
            "note that",
            "remember that",
            "recall",
            "forget",
            "what did we store",
        ];
        memory_keywords.iter().any(|kw| query.contains(kw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_math_intent() {
        assert_eq!(
            ToolSelector::detect_intent("What is 5 + 3?"),
            QueryIntent::Math
        );
        assert_eq!(
            ToolSelector::detect_intent("Calculate 100 * 2"),
            QueryIntent::Math
        );
        assert_eq!(
            ToolSelector::detect_intent("Count the papers"),
            QueryIntent::Math
        );
    }

    #[test]
    fn test_detect_web_search_intent() {
        assert_eq!(
            ToolSelector::detect_intent("Find latest AI news"),
            QueryIntent::WebSearch
        );
        assert_eq!(
            ToolSelector::detect_intent("What is Rust?"),
            QueryIntent::WebSearch
        );
        assert_eq!(
            ToolSelector::detect_intent("Find papers about AI"),
            QueryIntent::WebSearch
        );
    }

    #[test]
    fn test_detect_url_fetch_intent() {
        assert_eq!(
            ToolSelector::detect_intent("Fetch https://example.com"),
            QueryIntent::UrlFetch
        );
        assert_eq!(
            ToolSelector::detect_intent("Get content from http://test.com"),
            QueryIntent::UrlFetch
        );
    }

    #[test]
    fn test_select_tools_math() {
        let selection = ToolSelector::select_tools("Calculate 15 * 3");
        assert_eq!(selection.primary_tool, ToolType::Calculator);
        assert!(selection.confidence > 0.9);
    }

    #[test]
    fn test_select_tools_web_search() {
        let selection = ToolSelector::select_tools("Find the latest papers");
        assert_eq!(selection.primary_tool, ToolType::WebSearch);
    }

    #[test]
    fn test_confidence_scores() {
        let math_selection = ToolSelector::select_tools("5 + 3");
        let search_selection = ToolSelector::select_tools("Find papers");
        let semantic_selection = ToolSelector::select_tools("Tell me about quantum computing");

        assert!(math_selection.confidence > semantic_selection.confidence);
        assert!(search_selection.confidence > semantic_selection.confidence);
    }
}
