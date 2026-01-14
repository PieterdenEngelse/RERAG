// src/tools/tool_composer.rs - UPDATED
// Phase 10: Tool Composition Engine (v2 - Better Query Cleaning)

use crate::tools::tool_selector::{QueryIntent, ToolSelector};
use crate::tools::ToolType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step: usize,
    pub tool: ToolType,
    pub query: String,
    pub formatted_query: Option<String>,
    pub result: Option<String>,
    pub confidence: f32,
    pub execution_time_ms: u64,
    pub metadata_extra: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChain {
    pub steps: Vec<ExecutionStep>,
    pub final_answer: String,
    pub total_confidence: f32,
    pub total_execution_time_ms: u64,
    pub is_multi_step: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainPlan {
    pub query: String,
    pub is_multi_step: bool,
    pub planned_steps: Vec<PlannedStep>,
    pub total_planned_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedStep {
    pub step: usize,
    pub tool: ToolType,
    pub purpose: String,
    pub expected_confidence: f32,
}

pub struct ToolComposer;

impl ToolComposer {
    /// Detect if query needs multiple tools
    pub fn is_multi_step_query(query: &str) -> bool {
        let q = query.to_lowercase();
        let multi_step_keywords = vec![
            "and",
            "then",
            "after",
            "also",
            "plus",
            "additionally",
            "furthermore",
            "moreover",
            "followed by",
            "next",
            "first",
            "second",
            "third",
        ];

        multi_step_keywords.iter().any(|kw| q.contains(kw))
    }

    /// Clean query by removing instruction words
    fn clean_query(query: &str) -> String {
        let cleaned = query
            .replace("calculate ", "")
            .replace("Calculate ", "")
            .replace("compute ", "")
            .replace("Compute ", "")
            .replace("find ", "")
            .replace("Find ", "")
            .replace("search ", "")
            .replace("Search ", "")
            .replace("fetch ", "")
            .replace("Fetch ", "")
            .trim()
            .to_string();

        cleaned
    }

    /// Split query into sub-queries
    pub fn split_query(query: &str) -> Vec<String> {
        let q = query.to_lowercase();
        let mut sub_queries = Vec::new();

        // Split by common conjunctions
        if q.contains(" and ") {
            let parts: Vec<&str> = query.split(" and ").collect();
            sub_queries.extend(parts.iter().map(|s| Self::clean_query(s)));
        } else if q.contains(" then ") {
            let parts: Vec<&str> = query.split(" then ").collect();
            sub_queries.extend(parts.iter().map(|s| Self::clean_query(s)));
        } else {
            sub_queries.push(Self::clean_query(query));
        }

        sub_queries.into_iter().filter(|s| !s.is_empty()).collect()
    }

    /// Plan tool chain for query
    pub fn plan_chain(query: &str) -> ChainPlan {
        let is_multi_step = Self::is_multi_step_query(query);
        let sub_queries = Self::split_query(query);

        let mut planned_steps = Vec::new();

        for (idx, sub_query) in sub_queries.iter().enumerate() {
            let intent = ToolSelector::detect_intent(sub_query);
            let tool = Self::intent_to_tool(&intent);
            let confidence = Self::get_expected_confidence(&intent);

            planned_steps.push(PlannedStep {
                step: idx + 1,
                tool,
                purpose: sub_query.clone(),
                expected_confidence: confidence,
            });
        }

        ChainPlan {
            query: query.to_string(),
            is_multi_step,
            total_planned_steps: planned_steps.len(),
            planned_steps,
        }
    }

    /// Convert intent to tool type
    fn intent_to_tool(intent: &QueryIntent) -> ToolType {
        match intent {
            QueryIntent::Math => ToolType::Calculator,
            QueryIntent::WebSearch => ToolType::WebSearch,
            QueryIntent::UrlFetch => ToolType::URLFetch,
            QueryIntent::Database => ToolType::DatabaseQuery,
            QueryIntent::CodeExecution => ToolType::CodeExecution,
            QueryIntent::ImageGeneration => ToolType::ImageGeneration,
            QueryIntent::Scheduler => ToolType::Scheduler,
            QueryIntent::Memory => ToolType::Memory,
            _ => ToolType::SemanticSearch,
        }
    }

    /// Get expected confidence for intent
    fn get_expected_confidence(intent: &QueryIntent) -> f32 {
        match intent {
            QueryIntent::Math => 0.95,
            QueryIntent::WebSearch => 0.85,
            QueryIntent::UrlFetch => 0.80,
            QueryIntent::Database => 0.75,
            QueryIntent::CodeExecution => 0.70,
            QueryIntent::ImageGeneration => 0.65,
            QueryIntent::Scheduler => 0.78,
            QueryIntent::Memory => 0.70,
            _ => 0.60,
        }
    }

    /// Create initial chain from plan
    pub fn create_chain_from_plan(plan: &ChainPlan) -> ToolChain {
        let mut steps = Vec::new();

        for planned_step in &plan.planned_steps {
            steps.push(ExecutionStep {
                step: planned_step.step,
                tool: planned_step.tool.clone(),
                query: planned_step.purpose.clone(),
                formatted_query: None,
                result: None,
                confidence: planned_step.expected_confidence,
                execution_time_ms: 0,
                metadata_extra: None,
            });
        }

        ToolChain {
            steps,
            final_answer: String::new(),
            total_confidence: 0.0,
            total_execution_time_ms: 0,
            is_multi_step: plan.is_multi_step,
        }
    }

    /// Calculate aggregate confidence
    pub fn calculate_aggregate_confidence(confidences: &[f32]) -> f32 {
        if confidences.is_empty() {
            return 0.0;
        }

        // Geometric mean (product of all, nth root)
        let product: f32 = confidences.iter().product();
        product.powf(1.0 / confidences.len() as f32)
    }

    /// Compose final answer from chain
    pub fn compose_answer(chain: &ToolChain) -> String {
        if chain.steps.is_empty() {
            return String::new();
        }

        let mut answer = String::new();

        for step in &chain.steps {
            if let Some(result) = &step.result {
                if !answer.is_empty() {
                    answer.push_str(". ");
                }
                answer.push_str(&format!("Step {}: {}", step.step, result));
            }
        }

        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_multi_step_query() {
        assert!(ToolComposer::is_multi_step_query("Find papers and count"));
        assert!(ToolComposer::is_multi_step_query("Search then calculate"));
        assert!(!ToolComposer::is_multi_step_query("Calculate 5 + 3"));
    }

    #[test]
    fn test_split_query() {
        let query = "Calculate 10 + 5 and multiply by 2";
        let sub_queries = ToolComposer::split_query(query);
        assert_eq!(sub_queries.len(), 2);
    }

    #[test]
    fn test_clean_query() {
        let cleaned = ToolComposer::clean_query("Calculate 10 + 5");
        assert_eq!(cleaned, "10 + 5");
    }

    #[test]
    fn test_plan_chain() {
        let query = "Find latest papers and count";
        let plan = ToolComposer::plan_chain(query);
        assert!(plan.is_multi_step);
        assert_eq!(plan.total_planned_steps, 2);
    }

    #[test]
    fn test_aggregate_confidence() {
        let confidences = vec![0.95, 0.85, 0.90];
        let agg = ToolComposer::calculate_aggregate_confidence(&confidences);
        assert!(agg > 0.0 && agg < 1.0);
    }
}
