// src/training/feedback.rs
// Version: 1.0.0
//
// User feedback collection for training data quality scoring

use serde::{Deserialize, Serialize};

/// Quality score for training examples (1-5 scale)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
#[derive(Default)]
pub enum QualityScore {
    /// Very poor - incorrect, harmful, or completely off-topic
    VeryPoor = 1,
    /// Poor - mostly incorrect or unhelpful
    Poor = 2,
    /// Acceptable - partially correct, could be better
    #[default]
    Acceptable = 3,
    /// Good - correct and helpful
    Good = 4,
    /// Excellent - perfect response, ideal for training
    Excellent = 5,
}

impl QualityScore {
    /// Convert from numeric value (clamped to 1-5)
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 | 1 => Self::VeryPoor,
            2 => Self::Poor,
            3 => Self::Acceptable,
            4 => Self::Good,
            _ => Self::Excellent,
        }
    }

    /// Convert to numeric value
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Check if this score meets the minimum threshold for training
    pub fn is_usable(&self, min_score: u8) -> bool {
        self.as_u8() >= min_score
    }

    /// Human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            Self::VeryPoor => "Very Poor",
            Self::Poor => "Poor",
            Self::Acceptable => "Acceptable",
            Self::Good => "Good",
            Self::Excellent => "Excellent",
        }
    }

    /// Emoji representation for UI
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::VeryPoor => "😞",
            Self::Poor => "😕",
            Self::Acceptable => "😐",
            Self::Good => "🙂",
            Self::Excellent => "😊",
        }
    }
}

impl From<u8> for QualityScore {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

impl From<QualityScore> for u8 {
    fn from(score: QualityScore) -> Self {
        score.as_u8()
    }
}

/// Feedback request from user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRequest {
    /// The original query
    pub query: String,
    /// The model's response
    pub response: String,
    /// Retrieved context (if any)
    pub context: Option<String>,
    /// User's quality rating (1-5)
    pub quality_score: u8,
    /// Optional conversation ID for grouping
    pub conversation_id: Option<String>,
    /// Optional text feedback from user
    pub feedback_text: Option<String>,
}

/// Feedback response to user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResponse {
    /// Status of the feedback submission
    pub status: String,
    /// ID of the created training example
    pub example_id: String,
    /// Message to display
    pub message: String,
}

impl FeedbackResponse {
    pub fn success(example_id: String) -> Self {
        Self {
            status: "collected".to_string(),
            example_id,
            message: "Thank you for your feedback!".to_string(),
        }
    }

    pub fn skipped(reason: &str) -> Self {
        Self {
            status: "skipped".to_string(),
            example_id: String::new(),
            message: reason.to_string(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            status: "error".to_string(),
            example_id: String::new(),
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_score_conversion() {
        assert_eq!(QualityScore::from_u8(0).as_u8(), 1);
        assert_eq!(QualityScore::from_u8(1).as_u8(), 1);
        assert_eq!(QualityScore::from_u8(3).as_u8(), 3);
        assert_eq!(QualityScore::from_u8(5).as_u8(), 5);
        assert_eq!(QualityScore::from_u8(10).as_u8(), 5); // Clamped
    }

    #[test]
    fn test_quality_score_usable() {
        assert!(!QualityScore::VeryPoor.is_usable(3));
        assert!(!QualityScore::Poor.is_usable(3));
        assert!(QualityScore::Acceptable.is_usable(3));
        assert!(QualityScore::Good.is_usable(3));
        assert!(QualityScore::Excellent.is_usable(3));
    }

    #[test]
    fn test_quality_score_labels() {
        assert_eq!(QualityScore::Excellent.label(), "Excellent");
        assert_eq!(QualityScore::Good.emoji(), "🙂");
    }
}
