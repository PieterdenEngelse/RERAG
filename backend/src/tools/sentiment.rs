// src/tools/sentiment.rs
// Feature #14: SentimentAnalyzerTool - Analyze sentiment of text

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Instant;

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};

/// Sentiment classification
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Sentiment {
    VeryPositive,
    Positive,
    Neutral,
    Negative,
    VeryNegative,
}

impl Sentiment {
    pub fn label(&self) -> &'static str {
        match self {
            Sentiment::VeryPositive => "Very Positive",
            Sentiment::Positive => "Positive",
            Sentiment::Neutral => "Neutral",
            Sentiment::Negative => "Negative",
            Sentiment::VeryNegative => "Very Negative",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Sentiment::VeryPositive => "😄",
            Sentiment::Positive => "🙂",
            Sentiment::Neutral => "😐",
            Sentiment::Negative => "🙁",
            Sentiment::VeryNegative => "😢",
        }
    }

    pub fn from_score(score: f32) -> Self {
        if score >= 0.6 {
            Sentiment::VeryPositive
        } else if score >= 0.2 {
            Sentiment::Positive
        } else if score >= -0.2 {
            Sentiment::Neutral
        } else if score >= -0.6 {
            Sentiment::Negative
        } else {
            Sentiment::VeryNegative
        }
    }
}

/// Detailed sentiment analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAnalysis {
    pub sentiment: Sentiment,
    pub score: f32,      // -1.0 to 1.0
    pub confidence: f32, // 0.0 to 1.0
    pub positive_words: Vec<String>,
    pub negative_words: Vec<String>,
    pub intensity_modifiers: Vec<String>,
}

pub struct SentimentAnalyzerTool {
    success_rate: f32,
    positive_words: HashSet<String>,
    negative_words: HashSet<String>,
    intensifiers: HashSet<String>,
    negators: HashSet<String>,
}

impl SentimentAnalyzerTool {
    pub fn new() -> Self {
        let positive_words: HashSet<String> = [
            "good",
            "great",
            "excellent",
            "amazing",
            "wonderful",
            "fantastic",
            "awesome",
            "love",
            "happy",
            "joy",
            "beautiful",
            "perfect",
            "best",
            "brilliant",
            "outstanding",
            "superb",
            "delightful",
            "pleasant",
            "positive",
            "success",
            "successful",
            "win",
            "winning",
            "winner",
            "like",
            "enjoy",
            "enjoyed",
            "enjoying",
            "glad",
            "pleased",
            "exciting",
            "excited",
            "thrilled",
            "satisfied",
            "impressive",
            "remarkable",
            "incredible",
            "fabulous",
            "terrific",
            "marvelous",
            "splendid",
            "nice",
            "fine",
            "cool",
            "fun",
            "helpful",
            "useful",
            "valuable",
            "recommend",
            "recommended",
            "praise",
            "praised",
            "thank",
            "thanks",
            "grateful",
            "appreciate",
            "appreciated",
            "blessed",
            "fortunate",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let negative_words: HashSet<String> = [
            "bad",
            "terrible",
            "awful",
            "horrible",
            "poor",
            "worst",
            "hate",
            "sad",
            "angry",
            "upset",
            "disappointed",
            "disappointing",
            "fail",
            "failed",
            "failure",
            "wrong",
            "mistake",
            "error",
            "problem",
            "issue",
            "bug",
            "broken",
            "crash",
            "crashed",
            "slow",
            "annoying",
            "annoyed",
            "frustrating",
            "frustrated",
            "confusing",
            "confused",
            "difficult",
            "hard",
            "impossible",
            "useless",
            "worthless",
            "waste",
            "boring",
            "dull",
            "ugly",
            "nasty",
            "disgusting",
            "gross",
            "sick",
            "pain",
            "painful",
            "hurt",
            "hurts",
            "damage",
            "damaged",
            "destroy",
            "destroyed",
            "ruin",
            "ruined",
            "regret",
            "sorry",
            "unfortunately",
            "never",
            "nothing",
            "nobody",
            "nowhere",
            "neither",
            "nor",
            "dislike",
            "unhappy",
            "miserable",
            "depressed",
            "anxious",
            "worried",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let intensifiers: HashSet<String> = [
            "very",
            "really",
            "extremely",
            "incredibly",
            "absolutely",
            "totally",
            "completely",
            "utterly",
            "highly",
            "deeply",
            "strongly",
            "particularly",
            "especially",
            "exceptionally",
            "remarkably",
            "so",
            "such",
            "too",
            "most",
            "more",
            "much",
            "quite",
            "rather",
            "fairly",
            "pretty",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let negators: HashSet<String> = [
            "not",
            "no",
            "never",
            "neither",
            "nobody",
            "nothing",
            "nowhere",
            "none",
            "nor",
            "cannot",
            "can't",
            "won't",
            "wouldn't",
            "shouldn't",
            "couldn't",
            "didn't",
            "doesn't",
            "don't",
            "isn't",
            "aren't",
            "wasn't",
            "weren't",
            "hasn't",
            "haven't",
            "hadn't",
            "without",
            "barely",
            "hardly",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            success_rate: 0.95,
            positive_words,
            negative_words,
            intensifiers,
            negators,
        }
    }

    /// Analyze sentiment of text
    pub fn analyze(&self, text: &str) -> SentimentAnalysis {
        let words: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphabetic())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let mut score = 0.0f32;
        let mut found_positive = Vec::new();
        let mut found_negative = Vec::new();
        let mut found_intensifiers = Vec::new();
        let mut negation_active = false;
        let mut intensity_multiplier = 1.0f32;

        for (i, word) in words.iter().enumerate() {
            // Check for negators
            if self.negators.contains(word) {
                negation_active = true;
                continue;
            }

            // Check for intensifiers
            if self.intensifiers.contains(word) {
                intensity_multiplier = 1.5;
                found_intensifiers.push(word.clone());
                continue;
            }

            // Check for sentiment words
            let mut word_score = 0.0f32;

            if self.positive_words.contains(word) {
                word_score = 1.0;
                found_positive.push(word.clone());
            } else if self.negative_words.contains(word) {
                word_score = -1.0;
                found_negative.push(word.clone());
            }

            // Apply negation
            if negation_active && word_score != 0.0 {
                word_score = -word_score * 0.8; // Negation reduces intensity slightly
                negation_active = false;
            }

            // Apply intensity
            word_score *= intensity_multiplier;
            intensity_multiplier = 1.0; // Reset after use

            score += word_score;

            // Reset negation after a few words
            if i > 0 && negation_active {
                let words_since_negation = words[..i]
                    .iter()
                    .rev()
                    .take_while(|w| !self.negators.contains(*w))
                    .count();
                if words_since_negation > 3 {
                    negation_active = false;
                }
            }
        }

        // Normalize score
        let word_count = words.len().max(1) as f32;
        let sentiment_word_count = (found_positive.len() + found_negative.len()).max(1) as f32;

        // Normalize to -1 to 1 range
        let normalized_score = (score / sentiment_word_count).clamp(-1.0, 1.0);

        // Calculate confidence based on sentiment word density
        let sentiment_density = sentiment_word_count / word_count;
        let confidence = (sentiment_density * 2.0).clamp(0.3, 0.95);

        SentimentAnalysis {
            sentiment: Sentiment::from_score(normalized_score),
            score: normalized_score,
            confidence,
            positive_words: found_positive,
            negative_words: found_negative,
            intensity_modifiers: found_intensifiers,
        }
    }
}

impl Default for SentimentAnalyzerTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SentimentAnalyzerTool {
    fn tool_type(&self) -> ToolType {
        ToolType::SentimentAnalyzer
    }

    fn description(&self) -> String {
        "Analyze the sentiment of text. Returns positive, negative, or neutral classification with confidence score.".to_string()
    }

    fn success_rate(&self) -> f32 {
        self.success_rate
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();

        if query.trim().is_empty() {
            return Err("Empty text provided for sentiment analysis".to_string());
        }

        let analysis = self.analyze(query);
        let execution_time = start.elapsed().as_millis() as u64;

        let result = format!(
            "Sentiment: {} {} (score: {:.2}, confidence: {:.0}%)\n\
             Positive words: {}\n\
             Negative words: {}",
            analysis.sentiment.emoji(),
            analysis.sentiment.label(),
            analysis.score,
            analysis.confidence * 100.0,
            if analysis.positive_words.is_empty() {
                "none".to_string()
            } else {
                analysis.positive_words.join(", ")
            },
            if analysis.negative_words.is_empty() {
                "none".to_string()
            } else {
                analysis.negative_words.join(", ")
            },
        );

        Ok(ToolResult {
            tool: ToolType::SentimentAnalyzer,
            success: true,
            result,
            metadata: ToolMetadata {
                execution_time_ms: execution_time,
                confidence: analysis.confidence,
                source: Some("lexicon-based".to_string()),
                cost: Some(0.0),
            },
        })
    }

    fn update_success(&mut self, success: bool) {
        if success {
            self.success_rate = (self.success_rate * 0.95) + 0.05;
        } else {
            self.success_rate = self.success_rate * 0.95;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_positive_sentiment() {
        let tool = SentimentAnalyzerTool::new();
        let result = tool
            .execute("I love this amazing product! It's wonderful and fantastic.")
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
        assert!(r.result.contains("Positive"));
    }

    #[tokio::test]
    async fn test_negative_sentiment() {
        let tool = SentimentAnalyzerTool::new();
        let result = tool.execute("This is terrible and awful. I hate it.").await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("Negative"));
    }

    #[tokio::test]
    async fn test_neutral_sentiment() {
        let tool = SentimentAnalyzerTool::new();
        let result = tool
            .execute("The meeting is scheduled for tomorrow at 3pm.")
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("Neutral"));
    }

    #[tokio::test]
    async fn test_negation() {
        let tool = SentimentAnalyzerTool::new();
        let analysis = tool.analyze("This is not good");
        // "not good" should be negative
        assert!(analysis.score < 0.0);
    }

    #[test]
    fn test_sentiment_from_score() {
        assert_eq!(Sentiment::from_score(0.8), Sentiment::VeryPositive);
        assert_eq!(Sentiment::from_score(0.3), Sentiment::Positive);
        assert_eq!(Sentiment::from_score(0.0), Sentiment::Neutral);
        assert_eq!(Sentiment::from_score(-0.3), Sentiment::Negative);
        assert_eq!(Sentiment::from_score(-0.8), Sentiment::VeryNegative);
    }
}
