// src/tools/query_rewriter.rs
// Query Rewriter Agent - Improve queries for better search results

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct QueryRewriterTool {
    /// Common abbreviation expansions
    abbreviations: HashMap<String, String>,
    /// Synonym mappings for query expansion
    synonyms: HashMap<String, Vec<String>>,
    success_count: usize,
    total_count: usize,
}

impl QueryRewriterTool {
    pub fn new() -> Self {
        let mut abbreviations = HashMap::new();
        // Common tech abbreviations
        abbreviations.insert("ai".to_string(), "artificial intelligence".to_string());
        abbreviations.insert("ml".to_string(), "machine learning".to_string());
        abbreviations.insert("nlp".to_string(), "natural language processing".to_string());
        abbreviations.insert("api".to_string(), "application programming interface".to_string());
        abbreviations.insert("db".to_string(), "database".to_string());
        abbreviations.insert("ui".to_string(), "user interface".to_string());
        abbreviations.insert("ux".to_string(), "user experience".to_string());
        abbreviations.insert("js".to_string(), "javascript".to_string());
        abbreviations.insert("ts".to_string(), "typescript".to_string());
        abbreviations.insert("py".to_string(), "python".to_string());
        abbreviations.insert("llm".to_string(), "large language model".to_string());
        abbreviations.insert("rag".to_string(), "retrieval augmented generation".to_string());
        abbreviations.insert("gpu".to_string(), "graphics processing unit".to_string());
        abbreviations.insert("cpu".to_string(), "central processing unit".to_string());
        abbreviations.insert("os".to_string(), "operating system".to_string());
        abbreviations.insert("sql".to_string(), "structured query language".to_string());

        let mut synonyms = HashMap::new();
        // Common search synonyms
        synonyms.insert("find".to_string(), vec!["search".to_string(), "locate".to_string(), "discover".to_string()]);
        synonyms.insert("create".to_string(), vec!["make".to_string(), "build".to_string(), "generate".to_string()]);
        synonyms.insert("delete".to_string(), vec!["remove".to_string(), "erase".to_string()]);
        synonyms.insert("update".to_string(), vec!["modify".to_string(), "change".to_string(), "edit".to_string()]);
        synonyms.insert("error".to_string(), vec!["bug".to_string(), "issue".to_string(), "problem".to_string()]);
        synonyms.insert("fast".to_string(), vec!["quick".to_string(), "rapid".to_string(), "speedy".to_string()]);
        synonyms.insert("slow".to_string(), vec!["sluggish".to_string(), "delayed".to_string()]);

        Self {
            abbreviations,
            synonyms,
            success_count: 0,
            total_count: 0,
        }
    }

    /// Expand abbreviations in the query
    fn expand_abbreviations(&self, query: &str) -> String {
        let words: Vec<&str> = query.split_whitespace().collect();
        let expanded: Vec<String> = words
            .iter()
            .map(|word| {
                let lower = word.to_lowercase();
                if let Some(expansion) = self.abbreviations.get(&lower) {
                    expansion.clone()
                } else {
                    word.to_string()
                }
            })
            .collect();
        expanded.join(" ")
    }

    /// Fix common typos
    fn fix_typos(&self, query: &str) -> String {
        let typo_fixes: HashMap<&str, &str> = [
            ("teh", "the"),
            ("adn", "and"),
            ("taht", "that"),
            ("wiht", "with"),
            ("hte", "the"),
            ("fo", "of"),
            ("ot", "to"),
            ("recieve", "receive"),
            ("seperate", "separate"),
            ("occured", "occurred"),
            ("definately", "definitely"),
            ("accomodate", "accommodate"),
            ("occurence", "occurrence"),
            ("untill", "until"),
            ("begining", "beginning"),
        ]
        .iter()
        .cloned()
        .collect();

        let words: Vec<&str> = query.split_whitespace().collect();
        let fixed: Vec<String> = words
            .iter()
            .map(|word| {
                let lower = word.to_lowercase();
                if let Some(fix) = typo_fixes.get(lower.as_str()) {
                    fix.to_string()
                } else {
                    word.to_string()
                }
            })
            .collect();
        fixed.join(" ")
    }

    /// Add synonyms for query expansion
    fn add_synonyms(&self, query: &str) -> Vec<String> {
        let mut expansions = vec![query.to_string()];
        
        for (word, syns) in &self.synonyms {
            if query.to_lowercase().contains(word) {
                for syn in syns.iter().take(2) {
                    let expanded = query.to_lowercase().replace(word, syn);
                    if !expansions.contains(&expanded) {
                        expansions.push(expanded);
                    }
                }
            }
        }
        
        expansions
    }

    /// Remove stop words for cleaner queries
    fn remove_stop_words(&self, query: &str) -> String {
        let stop_words = [
            "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
            "have", "has", "had", "do", "does", "did", "will", "would", "could",
            "should", "may", "might", "must", "shall", "can", "need", "dare",
            "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
            "about", "as", "into", "through", "during", "before", "after",
            "above", "below", "from", "up", "down", "out", "off", "over", "under",
            "again", "further", "then", "once", "here", "there", "when", "where",
            "why", "how", "all", "each", "few", "more", "most", "other", "some",
            "such", "no", "nor", "not", "only", "own", "same", "so", "than",
            "too", "very", "just", "also",
        ];

        let words: Vec<&str> = query.split_whitespace().collect();
        let filtered: Vec<&str> = words
            .iter()
            .filter(|w| !stop_words.contains(&w.to_lowercase().as_str()))
            .copied()
            .collect();
        
        if filtered.is_empty() {
            query.to_string()
        } else {
            filtered.join(" ")
        }
    }

    /// Rewrite the query with all improvements
    fn rewrite(&self, query: &str) -> RewriteResult {
        let original = query.to_string();
        
        // Step 1: Fix typos
        let typo_fixed = self.fix_typos(query);
        
        // Step 2: Expand abbreviations
        let expanded = self.expand_abbreviations(&typo_fixed);
        
        // Step 3: Generate synonym expansions
        let synonyms = self.add_synonyms(&expanded);
        
        // Step 4: Create a clean version without stop words
        let clean = self.remove_stop_words(&expanded);

        RewriteResult {
            original,
            rewritten: expanded.clone(),
            clean,
            alternatives: synonyms,
            changes: vec![
                if typo_fixed != query { "Fixed typos".to_string() } else { String::new() },
                if expanded != typo_fixed { "Expanded abbreviations".to_string() } else { String::new() },
            ].into_iter().filter(|s| !s.is_empty()).collect(),
        }
    }
}

#[derive(Debug)]
struct RewriteResult {
    original: String,
    rewritten: String,
    clean: String,
    alternatives: Vec<String>,
    changes: Vec<String>,
}

#[async_trait]
impl Tool for QueryRewriterTool {
    fn tool_type(&self) -> ToolType {
        ToolType::QueryRewriter
    }

    fn description(&self) -> String {
        "Improve search queries by fixing typos, expanding abbreviations, and adding synonyms".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.95
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();
        debug!("QueryRewriterTool: rewriting '{}'", query);

        if query.trim().is_empty() {
            return Ok(ToolResult {
                tool: ToolType::QueryRewriter,
                success: false,
                result: "No query provided to rewrite.".to_string(),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("QueryRewriter".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        let result = self.rewrite(query);
        
        let mut output = format!("🔄 **Query Rewrite Results**\n\n");
        output.push_str(&format!("**Original:** {}\n", result.original));
        output.push_str(&format!("**Rewritten:** {}\n", result.rewritten));
        output.push_str(&format!("**Clean (no stop words):** {}\n", result.clean));
        
        if !result.changes.is_empty() {
            output.push_str(&format!("\n**Changes made:** {}\n", result.changes.join(", ")));
        }
        
        if result.alternatives.len() > 1 {
            output.push_str("\n**Alternative queries:**\n");
            for (i, alt) in result.alternatives.iter().skip(1).take(3).enumerate() {
                output.push_str(&format!("  {}. {}\n", i + 1, alt));
            }
        }

        let confidence = if result.rewritten != result.original { 0.9 } else { 0.7 };

        Ok(ToolResult {
            tool: ToolType::QueryRewriter,
            success: true,
            result: output,
            metadata: ToolMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                confidence,
                source: Some("QueryRewriter".to_string()),
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
    async fn test_abbreviation_expansion() {
        let tool = QueryRewriterTool::new();
        let result = tool.execute("how to use ml for nlp").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("machine learning"));
        assert!(res.result.contains("natural language processing"));
    }

    #[tokio::test]
    async fn test_typo_fix() {
        let tool = QueryRewriterTool::new();
        let result = tool.execute("teh best way to seperate data").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("the"));
        assert!(res.result.contains("separate"));
    }

    #[tokio::test]
    async fn test_empty_query() {
        let tool = QueryRewriterTool::new();
        let result = tool.execute("").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(!res.success);
    }
}
