// src/tools/spell_checker.rs
// Feature #16: SpellCheckerTool - Check and correct spelling

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};

/// Spelling correction suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellingCorrection {
    pub original: String,
    pub suggestion: String,
    pub position: usize,
    pub confidence: f32,
}

/// Spell check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellCheckResult {
    pub original_text: String,
    pub corrected_text: String,
    pub corrections: Vec<SpellingCorrection>,
    pub error_count: usize,
}

pub struct SpellCheckerTool {
    success_rate: f32,
    dictionary: HashSet<String>,
    common_misspellings: HashMap<String, String>,
}

impl SpellCheckerTool {
    pub fn new() -> Self {
        // Common English words dictionary
        let dictionary: HashSet<String> = include_str!("spell_checker_dict.txt")
            .lines()
            .filter(|l| !l.is_empty())
            .map(|s| s.to_lowercase())
            .collect();

        // Common misspellings and their corrections
        let common_misspellings: HashMap<String, String> = [
            // Common typos
            ("teh", "the"),
            ("hte", "the"),
            ("taht", "that"),
            ("adn", "and"),
            ("nad", "and"),
            ("wiht", "with"),
            ("whit", "with"),
            ("thier", "their"),
            ("recieve", "receive"),
            ("beleive", "believe"),
            ("occured", "occurred"),
            ("occurence", "occurrence"),
            ("seperate", "separate"),
            ("definately", "definitely"),
            ("accomodate", "accommodate"),
            ("occassion", "occasion"),
            ("untill", "until"),
            ("begining", "beginning"),
            ("goverment", "government"),
            ("enviroment", "environment"),
            ("arguement", "argument"),
            ("independant", "independent"),
            ("neccessary", "necessary"),
            ("occurr", "occur"),
            ("refered", "referred"),
            ("succesful", "successful"),
            ("tommorow", "tomorrow"),
            ("tomarrow", "tomorrow"),
            ("tommorrow", "tomorrow"),
            ("wierd", "weird"),
            ("wich", "which"),
            ("wether", "whether"),
            ("wheather", "weather"),
            ("writting", "writing"),
            ("writen", "written"),
            ("youre", "you're"),
            ("your", "you're"), // context-dependent, but common error
            ("its", "it's"),    // context-dependent
            ("alot", "a lot"),
            ("noone", "no one"),
            ("eachother", "each other"),
            ("infact", "in fact"),
            ("aswell", "as well"),
            ("incase", "in case"),
            ("inspite", "in spite"),
            ("eventhough", "even though"),
            // Programming-related
            ("fucntion", "function"),
            ("funciton", "function"),
            ("varaible", "variable"),
            ("varialbe", "variable"),
            ("retrun", "return"),
            ("reutrn", "return"),
            ("pritn", "print"),
            ("pirnt", "print"),
            ("calss", "class"),
            ("clss", "class"),
            ("improt", "import"),
            ("imoprt", "import"),
            ("exprot", "export"),
            ("exoprt", "export"),
            ("consle", "console"),
            ("cosole", "console"),
            ("stirng", "string"),
            ("strign", "string"),
            ("integre", "integer"),
            ("interger", "integer"),
            ("boolen", "boolean"),
            ("bolean", "boolean"),
            ("arrary", "array"),
            ("arrya", "array"),
            ("obejct", "object"),
            ("objcet", "object"),
            ("nulll", "null"),
            ("nul", "null"),
            ("undefiend", "undefined"),
            ("udnefined", "undefined"),
            ("asynch", "async"),
            ("awiat", "await"),
            ("awit", "await"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        Self {
            success_rate: 0.95,
            dictionary,
            common_misspellings,
        }
    }

    /// Check if a word is spelled correctly
    fn is_correct(&self, word: &str) -> bool {
        let lower = word.to_lowercase();

        // Skip if it's a number
        if word.chars().all(|c| c.is_numeric() || c == '.' || c == ',') {
            return true;
        }

        // Skip if it's very short
        if word.len() <= 1 {
            return true;
        }

        // Skip if it looks like an acronym (all caps)
        if word.chars().all(|c| c.is_uppercase()) && word.len() <= 5 {
            return true;
        }

        // Check dictionary
        self.dictionary.contains(&lower)
    }

    /// Get correction suggestion for a misspelled word
    fn get_suggestion(&self, word: &str) -> Option<String> {
        let lower = word.to_lowercase();

        // Check common misspellings first
        if let Some(correction) = self.common_misspellings.get(&lower) {
            return Some(correction.clone());
        }

        // Try edit distance 1 corrections
        let candidates = self.edits1(&lower);
        for candidate in candidates {
            if self.dictionary.contains(&candidate) {
                return Some(candidate);
            }
        }

        // Try edit distance 2 corrections
        let candidates2 = self.edits2(&lower);
        for candidate in candidates2 {
            if self.dictionary.contains(&candidate) {
                return Some(candidate);
            }
        }

        None
    }

    /// Generate all strings that are one edit away
    fn edits1(&self, word: &str) -> Vec<String> {
        let mut results = Vec::new();
        let chars: Vec<char> = word.chars().collect();
        let alphabet = "abcdefghijklmnopqrstuvwxyz";

        // Deletions
        for i in 0..chars.len() {
            let mut new_word: String = chars[..i].iter().collect();
            new_word.extend(chars[i + 1..].iter());
            results.push(new_word);
        }

        // Transpositions
        for i in 0..chars.len().saturating_sub(1) {
            let mut new_chars = chars.clone();
            new_chars.swap(i, i + 1);
            results.push(new_chars.iter().collect());
        }

        // Replacements
        for i in 0..chars.len() {
            for c in alphabet.chars() {
                if c != chars[i] {
                    let mut new_chars = chars.clone();
                    new_chars[i] = c;
                    results.push(new_chars.iter().collect());
                }
            }
        }

        // Insertions
        for i in 0..=chars.len() {
            for c in alphabet.chars() {
                let mut new_word: String = chars[..i].iter().collect();
                new_word.push(c);
                new_word.extend(chars[i..].iter());
                results.push(new_word);
            }
        }

        results
    }

    /// Generate all strings that are two edits away
    fn edits2(&self, word: &str) -> Vec<String> {
        let mut results = Vec::new();
        for e1 in self.edits1(word) {
            // Only check known words from edit1 to reduce computation
            if self.dictionary.contains(&e1) {
                results.push(e1);
            }
        }
        results
    }

    /// Check and correct text
    pub fn check(&self, text: &str) -> SpellCheckResult {
        let mut corrections = Vec::new();
        let mut corrected_words = Vec::new();
        let mut position = 0;

        // Split into words while preserving structure
        let mut current_word = String::new();
        let mut in_word = false;

        for (i, c) in text.char_indices() {
            if c.is_alphabetic() || c == '\'' {
                if !in_word {
                    position = i;
                    in_word = true;
                }
                current_word.push(c);
            } else {
                if in_word && !current_word.is_empty() {
                    // Process the word
                    if !self.is_correct(&current_word) {
                        if let Some(suggestion) = self.get_suggestion(&current_word) {
                            // Preserve original case
                            let corrected = self.preserve_case(&current_word, &suggestion);
                            corrections.push(SpellingCorrection {
                                original: current_word.clone(),
                                suggestion: corrected.clone(),
                                position,
                                confidence: 0.85,
                            });
                            corrected_words.push(corrected);
                        } else {
                            corrected_words.push(current_word.clone());
                        }
                    } else {
                        corrected_words.push(current_word.clone());
                    }
                    current_word.clear();
                    in_word = false;
                }
                // Add non-word character
                if !corrected_words.is_empty() || !corrections.is_empty() {
                    // Append to last word or create placeholder
                }
            }
        }

        // Handle last word
        if !current_word.is_empty() {
            if !self.is_correct(&current_word) {
                if let Some(suggestion) = self.get_suggestion(&current_word) {
                    let corrected = self.preserve_case(&current_word, &suggestion);
                    corrections.push(SpellingCorrection {
                        original: current_word.clone(),
                        suggestion: corrected,
                        position,
                        confidence: 0.85,
                    });
                }
            }
        }

        // Rebuild corrected text
        let mut corrected_text = text.to_string();
        // Apply corrections in reverse order to preserve positions
        for correction in corrections.iter().rev() {
            let start = correction.position;
            let end = start + correction.original.len();
            if end <= corrected_text.len() {
                corrected_text.replace_range(start..end, &correction.suggestion);
            }
        }

        SpellCheckResult {
            original_text: text.to_string(),
            corrected_text,
            error_count: corrections.len(),
            corrections,
        }
    }

    /// Preserve the case pattern of the original word
    fn preserve_case(&self, original: &str, suggestion: &str) -> String {
        if original.chars().all(|c| c.is_uppercase()) {
            suggestion.to_uppercase()
        } else if original
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
        {
            let mut chars: Vec<char> = suggestion.chars().collect();
            if let Some(first) = chars.first_mut() {
                *first = first.to_uppercase().next().unwrap_or(*first);
            }
            chars.iter().collect()
        } else {
            suggestion.to_string()
        }
    }
}

impl Default for SpellCheckerTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SpellCheckerTool {
    fn tool_type(&self) -> ToolType {
        ToolType::SpellChecker
    }

    fn description(&self) -> String {
        "Check and correct spelling errors in text. Supports common English words and programming terms.".to_string()
    }

    fn success_rate(&self) -> f32 {
        self.success_rate
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();

        if query.trim().is_empty() {
            return Err("Empty text provided for spell checking".to_string());
        }

        let check_result = self.check(query);
        let execution_time = start.elapsed().as_millis() as u64;

        let mut result = String::new();

        if check_result.corrections.is_empty() {
            result.push_str("✓ No spelling errors found.\n");
            result.push_str(&format!("Original: {}", query));
        } else {
            result.push_str(&format!(
                "Found {} spelling error(s):\n",
                check_result.error_count
            ));
            for correction in &check_result.corrections {
                result.push_str(&format!(
                    "  • \"{}\" → \"{}\" (confidence: {:.0}%)\n",
                    correction.original,
                    correction.suggestion,
                    correction.confidence * 100.0
                ));
            }
            result.push_str(&format!("\nCorrected: {}", check_result.corrected_text));
        }

        let confidence = if check_result.corrections.is_empty() {
            0.95
        } else {
            check_result
                .corrections
                .iter()
                .map(|c| c.confidence)
                .sum::<f32>()
                / check_result.corrections.len() as f32
        };

        Ok(ToolResult {
            tool: ToolType::SpellChecker,
            success: true,
            result,
            metadata: ToolMetadata {
                execution_time_ms: execution_time,
                confidence,
                source: Some("dictionary-based".to_string()),
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
    async fn test_correct_spelling() {
        let tool = SpellCheckerTool::new();
        let result = tool.execute("This is correct text.").await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("No spelling errors"));
    }

    #[tokio::test]
    async fn test_common_misspelling() {
        let tool = SpellCheckerTool::new();
        let result = tool.execute("I beleive teh answer is correct.").await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.result.contains("believe") || r.result.contains("the"));
    }

    #[test]
    fn test_preserve_case() {
        let tool = SpellCheckerTool::new();
        assert_eq!(tool.preserve_case("HELLO", "world"), "WORLD");
        assert_eq!(tool.preserve_case("Hello", "world"), "World");
        assert_eq!(tool.preserve_case("hello", "world"), "world");
    }
}
