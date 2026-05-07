// src/tools/summarizer.rs
// Summarizer Agent - Summarize documents, search results, or text

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::time::Instant;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct SummarizerTool {
    /// Maximum input length before truncation
    max_input_length: usize,
    /// Target summary length
    target_length: SummaryLength,
    success_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum SummaryLength {
    Brief, // ~50 words
    #[default]
    Standard, // ~150 words
    Detailed, // ~300 words
}

impl Default for SummarizerTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SummarizerTool {
    pub fn new() -> Self {
        Self {
            max_input_length: 10000,
            target_length: SummaryLength::Standard,
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn with_length(mut self, length: SummaryLength) -> Self {
        self.target_length = length;
        self
    }

    /// Parse input to extract text and optional parameters
    fn parse_input(&self, input: &str) -> (String, SummaryLength) {
        let input_lower = input.to_lowercase();

        // Check for length hints
        let length = if input_lower.contains("brief") || input_lower.contains("short") {
            SummaryLength::Brief
        } else if input_lower.contains("detailed") || input_lower.contains("comprehensive") {
            SummaryLength::Detailed
        } else {
            self.target_length
        };

        // Remove length keywords from text
        let text = input
            .replace("brief:", "")
            .replace("short:", "")
            .replace("detailed:", "")
            .replace("comprehensive:", "")
            .trim()
            .to_string();

        (text, length)
    }

    /// Generate summary using extractive summarization
    /// (In production, this would call an LLM)
    fn summarize(&self, text: &str, length: SummaryLength) -> String {
        let sentences: Vec<&str> = text
            .split(['.', '!', '?'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && s.len() > 10)
            .collect();

        if sentences.is_empty() {
            return "No content to summarize.".to_string();
        }

        let target_sentences = match length {
            SummaryLength::Brief => 2,
            SummaryLength::Standard => 4,
            SummaryLength::Detailed => 8,
        };

        // Simple extractive: take first N sentences
        // In production, use TF-IDF or LLM for better selection
        let summary_sentences: Vec<&str> = sentences
            .iter()
            .take(target_sentences.min(sentences.len()))
            .copied()
            .collect();

        let summary = summary_sentences.join(". ");

        if !summary.ends_with('.') {
            format!("{}.", summary)
        } else {
            summary
        }
    }

    /// Calculate word count
    fn word_count(text: &str) -> usize {
        text.split_whitespace().count()
    }

    /// Calculate compression ratio
    fn compression_ratio(original: &str, summary: &str) -> f32 {
        let original_words = Self::word_count(original) as f32;
        let summary_words = Self::word_count(summary) as f32;
        if original_words > 0.0 {
            1.0 - (summary_words / original_words)
        } else {
            0.0
        }
    }
}

#[async_trait]
impl Tool for SummarizerTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Summarizer
    }

    fn description(&self) -> String {
        "Summarize text, documents, or search results into concise summaries".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.90
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();
        debug!("SummarizerTool: summarizing {} chars", query.len());

        if query.trim().is_empty() {
            return Ok(ToolResult {
                tool: ToolType::Summarizer,
                success: false,
                result: "No text provided to summarize.".to_string(),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("Summarizer".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        // Truncate if too long
        let text = if query.len() > self.max_input_length {
            &query[..self.max_input_length]
        } else {
            query
        };

        let (text, length) = self.parse_input(text);
        let summary = self.summarize(&text, length);
        let compression = Self::compression_ratio(&text, &summary);
        let original_words = Self::word_count(&text);
        let summary_words = Self::word_count(&summary);

        let result = format!(
            "📝 **Summary** ({:?}):\n\n{}\n\n---\n*Original: {} words → Summary: {} words ({:.0}% compression)*",
            length, summary, original_words, summary_words, compression * 100.0
        );

        Ok(ToolResult {
            tool: ToolType::Summarizer,
            success: true,
            result,
            metadata: ToolMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                confidence: 0.85,
                source: Some("Summarizer/Extractive".to_string()),
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
    async fn test_summarizer_basic() {
        let tool = SummarizerTool::new();
        let text = "This is the first sentence. This is the second sentence. This is the third sentence. This is the fourth sentence. This is the fifth sentence.";
        let result = tool.execute(text).await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.success);
        assert!(res.result.contains("Summary"));
    }

    #[tokio::test]
    async fn test_summarizer_brief() {
        let tool = SummarizerTool::new();
        let text = "brief: This is the first sentence. This is the second sentence. This is the third sentence.";
        let result = tool.execute(text).await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("Brief"));
    }

    #[tokio::test]
    async fn test_summarizer_empty() {
        let tool = SummarizerTool::new();
        let result = tool.execute("").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(!res.success);
    }
}
