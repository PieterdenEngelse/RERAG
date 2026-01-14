// src/tools/classifier.rs
// Classifier Agent - Categorize and tag content

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct ClassifierTool {
    /// Category definitions with keywords
    categories: HashMap<String, Vec<&'static str>>,
    /// Intent patterns
    intents: HashMap<String, Vec<&'static str>>,
    success_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone)]
pub struct Classification {
    pub category: String,
    pub confidence: f32,
    pub tags: Vec<String>,
    pub intent: Option<String>,
    pub sentiment: Sentiment,
}

#[derive(Debug, Clone, Copy)]
pub enum Sentiment {
    Positive,
    Negative,
    Neutral,
    Question,
}

impl ClassifierTool {
    pub fn new() -> Self {
        let mut categories = HashMap::new();

        // Document categories
        categories.insert(
            "technical".to_string(),
            vec![
                "code",
                "programming",
                "api",
                "function",
                "class",
                "method",
                "algorithm",
                "database",
                "server",
                "client",
                "framework",
                "library",
                "module",
                "package",
                "debug",
                "error",
                "exception",
                "compile",
                "runtime",
                "syntax",
            ],
        );
        categories.insert(
            "business".to_string(),
            vec![
                "revenue",
                "profit",
                "sales",
                "marketing",
                "customer",
                "client",
                "contract",
                "budget",
                "forecast",
                "strategy",
                "growth",
                "market",
                "competition",
                "stakeholder",
                "roi",
                "kpi",
                "metrics",
                "performance",
            ],
        );
        categories.insert(
            "scientific".to_string(),
            vec![
                "research",
                "study",
                "experiment",
                "hypothesis",
                "data",
                "analysis",
                "methodology",
                "results",
                "conclusion",
                "peer-review",
                "journal",
                "citation",
                "abstract",
                "findings",
                "statistical",
            ],
        );
        categories.insert(
            "legal".to_string(),
            vec![
                "contract",
                "agreement",
                "clause",
                "liability",
                "compliance",
                "regulation",
                "law",
                "statute",
                "court",
                "plaintiff",
                "defendant",
                "jurisdiction",
                "intellectual property",
                "patent",
                "trademark",
                "copyright",
            ],
        );
        categories.insert(
            "educational".to_string(),
            vec![
                "learn",
                "teach",
                "course",
                "lesson",
                "tutorial",
                "guide",
                "example",
                "exercise",
                "practice",
                "student",
                "instructor",
                "curriculum",
                "assessment",
                "quiz",
                "exam",
                "certification",
            ],
        );
        categories.insert(
            "support".to_string(),
            vec![
                "help",
                "issue",
                "problem",
                "error",
                "fix",
                "solution",
                "troubleshoot",
                "ticket",
                "request",
                "urgent",
                "priority",
                "escalate",
                "resolve",
            ],
        );

        let mut intents = HashMap::new();

        // User intents
        intents.insert(
            "question".to_string(),
            vec![
                "what", "how", "why", "when", "where", "who", "which", "can", "could", "would",
                "should", "is", "are", "does", "do", "?",
            ],
        );
        intents.insert(
            "command".to_string(),
            vec![
                "create", "delete", "update", "add", "remove", "change", "modify", "set", "get",
                "list", "show", "display", "run", "execute", "start", "stop",
            ],
        );
        intents.insert(
            "search".to_string(),
            vec![
                "find", "search", "look", "locate", "discover", "retrieve", "fetch", "query",
                "filter", "browse",
            ],
        );
        intents.insert(
            "comparison".to_string(),
            vec![
                "compare",
                "versus",
                "vs",
                "difference",
                "between",
                "better",
                "worse",
                "pros",
                "cons",
                "advantages",
                "disadvantages",
            ],
        );
        intents.insert(
            "explanation".to_string(),
            vec![
                "explain",
                "describe",
                "elaborate",
                "clarify",
                "define",
                "meaning",
                "understand",
                "tell me about",
            ],
        );

        Self {
            categories,
            intents,
            success_count: 0,
            total_count: 0,
        }
    }

    /// Classify text into categories
    fn classify_category(&self, text: &str) -> Vec<(String, f32)> {
        let text_lower = text.to_lowercase();
        let words: Vec<&str> = text_lower.split_whitespace().collect();
        let word_count = words.len() as f32;

        let mut scores: Vec<(String, f32)> = self
            .categories
            .iter()
            .map(|(category, keywords)| {
                let matches = keywords
                    .iter()
                    .filter(|kw| text_lower.contains(*kw))
                    .count() as f32;
                let score = if word_count > 0.0 {
                    (matches / word_count).min(1.0) * 0.5 + (matches / keywords.len() as f32) * 0.5
                } else {
                    0.0
                };
                (category.clone(), score)
            })
            .filter(|(_, score)| *score > 0.05)
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(3);
        scores
    }

    /// Detect user intent
    fn detect_intent(&self, text: &str) -> Option<(String, f32)> {
        let text_lower = text.to_lowercase();

        let mut best_intent: Option<(String, f32)> = None;

        for (intent, patterns) in &self.intents {
            let matches = patterns.iter().filter(|p| text_lower.contains(*p)).count() as f32;

            let score = matches / patterns.len() as f32;

            if score > 0.1 {
                if let Some((_, best_score)) = &best_intent {
                    if score > *best_score {
                        best_intent = Some((intent.clone(), score));
                    }
                } else {
                    best_intent = Some((intent.clone(), score));
                }
            }
        }

        best_intent
    }

    /// Analyze sentiment
    fn analyze_sentiment(&self, text: &str) -> Sentiment {
        let text_lower = text.to_lowercase();

        // Check if it's a question
        if text.contains('?')
            || text_lower.starts_with("what")
            || text_lower.starts_with("how")
            || text_lower.starts_with("why")
            || text_lower.starts_with("when")
            || text_lower.starts_with("where")
            || text_lower.starts_with("who")
            || text_lower.starts_with("can")
            || text_lower.starts_with("could")
            || text_lower.starts_with("would")
        {
            return Sentiment::Question;
        }

        let positive_words = [
            "good",
            "great",
            "excellent",
            "amazing",
            "wonderful",
            "fantastic",
            "love",
            "like",
            "best",
            "perfect",
            "awesome",
            "helpful",
            "thanks",
            "thank",
            "appreciate",
            "success",
            "successful",
        ];

        let negative_words = [
            "bad",
            "terrible",
            "awful",
            "horrible",
            "hate",
            "worst",
            "poor",
            "fail",
            "failed",
            "error",
            "bug",
            "broken",
            "issue",
            "problem",
            "wrong",
            "incorrect",
            "disappointed",
            "frustrating",
        ];

        let pos_count = positive_words
            .iter()
            .filter(|w| text_lower.contains(*w))
            .count();

        let neg_count = negative_words
            .iter()
            .filter(|w| text_lower.contains(*w))
            .count();

        if pos_count > neg_count {
            Sentiment::Positive
        } else if neg_count > pos_count {
            Sentiment::Negative
        } else {
            Sentiment::Neutral
        }
    }

    /// Extract tags from text
    fn extract_tags(&self, text: &str) -> Vec<String> {
        let text_lower = text.to_lowercase();
        let mut tags = Vec::new();

        // Extract hashtags
        for word in text.split_whitespace() {
            if word.starts_with('#') && word.len() > 1 {
                tags.push(word[1..].to_string());
            }
        }

        // Extract key terms from all categories
        for keywords in self.categories.values() {
            for kw in keywords {
                if text_lower.contains(kw) && !tags.contains(&kw.to_string()) {
                    tags.push(kw.to_string());
                }
            }
        }

        tags.truncate(10);
        tags
    }

    /// Full classification
    fn classify(&self, text: &str) -> Classification {
        let categories = self.classify_category(text);
        let intent = self.detect_intent(text);
        let sentiment = self.analyze_sentiment(text);
        let tags = self.extract_tags(text);

        let (category, confidence) = categories
            .first()
            .map(|(c, s)| (c.clone(), *s))
            .unwrap_or(("general".to_string(), 0.5));

        Classification {
            category,
            confidence,
            tags,
            intent: intent.map(|(i, _)| i),
            sentiment,
        }
    }
}

#[async_trait]
impl Tool for ClassifierTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Classifier
    }

    fn description(&self) -> String {
        "Classify and categorize text content, detect intent, and extract tags".to_string()
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
        debug!("ClassifierTool: classifying {} chars", query.len());

        if query.trim().is_empty() {
            return Ok(ToolResult {
                tool: ToolType::Classifier,
                success: false,
                result: "No text provided to classify.".to_string(),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("Classifier".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        let classification = self.classify(query);

        let sentiment_str = match classification.sentiment {
            Sentiment::Positive => "😊 Positive",
            Sentiment::Negative => "😞 Negative",
            Sentiment::Neutral => "😐 Neutral",
            Sentiment::Question => "❓ Question",
        };

        let mut output = format!("🏷️ **Classification Results**\n\n");
        output.push_str(&format!(
            "**Category:** {} ({:.0}% confidence)\n",
            classification.category,
            classification.confidence * 100.0
        ));
        output.push_str(&format!("**Sentiment:** {}\n", sentiment_str));

        if let Some(intent) = &classification.intent {
            output.push_str(&format!("**Intent:** {}\n", intent));
        }

        if !classification.tags.is_empty() {
            output.push_str(&format!("\n**Tags:** {}\n", classification.tags.join(", ")));
        }

        Ok(ToolResult {
            tool: ToolType::Classifier,
            success: true,
            result: output,
            metadata: ToolMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                confidence: classification.confidence,
                source: Some("Classifier".to_string()),
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
    async fn test_technical_classification() {
        let tool = ClassifierTool::new();
        let result = tool
            .execute("How do I fix this runtime error in my Python code?")
            .await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("technical") || res.result.contains("support"));
    }

    #[tokio::test]
    async fn test_question_intent() {
        let tool = ClassifierTool::new();
        let result = tool
            .execute("What is the best way to learn machine learning?")
            .await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("Question"));
    }

    #[tokio::test]
    async fn test_sentiment() {
        let tool = ClassifierTool::new();
        let result = tool.execute("This is a great product, I love it!").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("Positive"));
    }
}
