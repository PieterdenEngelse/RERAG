// src/tools/file_analyzer.rs
// File Analyzer Agent - Analyze file contents and extract metadata

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct FileAnalyzerTool {
    success_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone)]
pub struct FileAnalysis {
    pub file_type: String,
    pub mime_type: String,
    pub language: Option<String>,
    pub encoding: String,
    pub line_count: usize,
    pub word_count: usize,
    pub char_count: usize,
    pub entities: Vec<Entity>,
    pub structure: Option<DocumentStructure>,
    pub quality_score: f32,
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub entity_type: String,
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct DocumentStructure {
    pub has_headers: bool,
    pub has_lists: bool,
    pub has_code_blocks: bool,
    pub has_links: bool,
    pub has_images: bool,
    pub sections: usize,
}

impl FileAnalyzerTool {
    pub fn new() -> Self {
        Self {
            success_count: 0,
            total_count: 0,
        }
    }

    /// Detect file type from content and optional filename
    fn detect_file_type(&self, content: &str, filename: Option<&str>) -> (String, String) {
        // Check by extension first
        if let Some(name) = filename {
            let ext = Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let (file_type, mime) = match ext.as_str() {
                "rs" => ("Rust Source", "text/x-rust"),
                "py" => ("Python Source", "text/x-python"),
                "js" => ("JavaScript Source", "text/javascript"),
                "ts" => ("TypeScript Source", "text/typescript"),
                "json" => ("JSON", "application/json"),
                "yaml" | "yml" => ("YAML", "text/yaml"),
                "toml" => ("TOML", "text/toml"),
                "md" => ("Markdown", "text/markdown"),
                "txt" => ("Plain Text", "text/plain"),
                "html" | "htm" => ("HTML", "text/html"),
                "xml" => ("XML", "text/xml"),
                "css" => ("CSS", "text/css"),
                "sql" => ("SQL", "text/x-sql"),
                "sh" | "bash" => ("Shell Script", "text/x-shellscript"),
                "c" => ("C Source", "text/x-c"),
                "cpp" | "cc" | "cxx" => ("C++ Source", "text/x-c++"),
                "h" | "hpp" => ("C/C++ Header", "text/x-c"),
                "java" => ("Java Source", "text/x-java"),
                "go" => ("Go Source", "text/x-go"),
                "rb" => ("Ruby Source", "text/x-ruby"),
                "php" => ("PHP Source", "text/x-php"),
                "swift" => ("Swift Source", "text/x-swift"),
                "kt" | "kts" => ("Kotlin Source", "text/x-kotlin"),
                "scala" => ("Scala Source", "text/x-scala"),
                "r" => ("R Source", "text/x-r"),
                "csv" => ("CSV", "text/csv"),
                "log" => ("Log File", "text/plain"),
                _ => ("Unknown", "application/octet-stream"),
            };

            if file_type != "Unknown" {
                return (file_type.to_string(), mime.to_string());
            }
        }

        // Detect from content
        let content_start = &content[..content.len().min(500)];

        if content_start.contains("fn ") && content_start.contains("let ") {
            ("Rust Source".to_string(), "text/x-rust".to_string())
        } else if content_start.contains("def ") && content_start.contains("import ") {
            ("Python Source".to_string(), "text/x-python".to_string())
        } else if content_start.contains("function") || content_start.contains("const ") {
            (
                "JavaScript Source".to_string(),
                "text/javascript".to_string(),
            )
        } else if content_start.starts_with('{') || content_start.starts_with('[') {
            ("JSON".to_string(), "application/json".to_string())
        } else if content_start.starts_with("<!DOCTYPE") || content_start.starts_with("<html") {
            ("HTML".to_string(), "text/html".to_string())
        } else if content_start.starts_with("<?xml") {
            ("XML".to_string(), "text/xml".to_string())
        } else if content_start.contains("# ") && content_start.contains("## ") {
            ("Markdown".to_string(), "text/markdown".to_string())
        } else {
            ("Plain Text".to_string(), "text/plain".to_string())
        }
    }

    /// Detect language of text content
    fn detect_language(&self, content: &str) -> Option<String> {
        let content_lower = content.to_lowercase();

        // Simple language detection based on common words
        let english_words = [
            "the", "is", "are", "and", "or", "but", "in", "on", "at", "to", "for",
        ];
        let spanish_words = [
            "el", "la", "los", "las", "es", "son", "y", "o", "pero", "en", "de",
        ];
        let french_words = [
            "le", "la", "les", "est", "sont", "et", "ou", "mais", "dans", "de",
        ];
        let german_words = [
            "der", "die", "das", "ist", "sind", "und", "oder", "aber", "in", "von",
        ];

        let words: Vec<&str> = content_lower.split_whitespace().collect();

        let english_count = words.iter().filter(|w| english_words.contains(w)).count();
        let spanish_count = words.iter().filter(|w| spanish_words.contains(w)).count();
        let french_count = words.iter().filter(|w| french_words.contains(w)).count();
        let german_count = words.iter().filter(|w| german_words.contains(w)).count();

        let max_count = english_count
            .max(spanish_count)
            .max(french_count)
            .max(german_count);

        if max_count < 3 {
            return None;
        }

        if english_count == max_count {
            Some("English".to_string())
        } else if spanish_count == max_count {
            Some("Spanish".to_string())
        } else if french_count == max_count {
            Some("French".to_string())
        } else if german_count == max_count {
            Some("German".to_string())
        } else {
            None
        }
    }

    /// Extract entities from content
    fn extract_entities(&self, content: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        let mut entity_counts: HashMap<(String, String), usize> = HashMap::new();

        // Simple email detection
        for word in content.split_whitespace() {
            if word.contains('@') && word.contains('.') {
                let key = ("email".to_string(), word.to_string());
                *entity_counts.entry(key).or_insert(0) += 1;
            }
        }

        // Simple URL detection
        for word in content.split_whitespace() {
            if word.starts_with("http://") || word.starts_with("https://") {
                let key = ("url".to_string(), word.to_string());
                *entity_counts.entry(key).or_insert(0) += 1;
            }
        }

        for ((entity_type, value), count) in entity_counts {
            entities.push(Entity {
                entity_type,
                value,
                count,
            });
        }

        entities.sort_by(|a, b| b.count.cmp(&a.count));
        entities.truncate(20);
        entities
    }

    /// Analyze document structure
    fn analyze_structure(&self, content: &str) -> DocumentStructure {
        DocumentStructure {
            has_headers: content.contains("# ")
                || content.contains("## ")
                || content.contains("<h1")
                || content.contains("<h2"),
            has_lists: content.contains("- ")
                || content.contains("* ")
                || content.contains("1. ")
                || content.contains("<li"),
            has_code_blocks: content.contains("```")
                || content.contains("<code")
                || content.contains("<pre"),
            has_links: content.contains("](")
                || content.contains("href=")
                || content.contains("http"),
            has_images: content.contains("![") || content.contains("<img"),
            sections: content.matches("# ").count() + content.matches("## ").count(),
        }
    }

    /// Calculate quality score
    fn calculate_quality(&self, content: &str, structure: &DocumentStructure) -> f32 {
        let mut score: f32 = 0.5; // Base score

        let word_count = content.split_whitespace().count();

        // Length bonus
        if word_count > 100 {
            score += 0.1;
        }
        if word_count > 500 {
            score += 0.1;
        }

        // Structure bonus
        if structure.has_headers {
            score += 0.1;
        }
        if structure.has_lists {
            score += 0.05;
        }
        if structure.sections > 2 {
            score += 0.1;
        }

        // Penalize very short content
        if word_count < 20 {
            score -= 0.2;
        }

        score.max(0.0).min(1.0)
    }

    /// Full analysis
    fn analyze(&self, content: &str, filename: Option<&str>) -> FileAnalysis {
        let (file_type, mime_type) = self.detect_file_type(content, filename);
        let language = self.detect_language(content);
        let entities = self.extract_entities(content);
        let structure = self.analyze_structure(content);
        let quality_score = self.calculate_quality(content, &structure);

        FileAnalysis {
            file_type,
            mime_type,
            language,
            encoding: "UTF-8".to_string(),
            line_count: content.lines().count(),
            word_count: content.split_whitespace().count(),
            char_count: content.chars().count(),
            entities,
            structure: Some(structure),
            quality_score,
        }
    }
}

#[async_trait]
impl Tool for FileAnalyzerTool {
    fn tool_type(&self) -> ToolType {
        ToolType::FileAnalyzer
    }

    fn description(&self) -> String {
        "Analyze file contents, extract metadata, detect language, and identify entities"
            .to_string()
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
        debug!("FileAnalyzerTool: analyzing {} chars", query.len());

        if query.trim().is_empty() {
            return Ok(ToolResult {
                tool: ToolType::FileAnalyzer,
                success: false,
                result: "No content provided to analyze.".to_string(),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("FileAnalyzer".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        // Check if input includes filename (format: "filename: content" or just content)
        let (filename, content): (Option<&str>, &str) = if query.contains('\n') {
            let first_line = query.lines().next().unwrap_or("");
            if first_line.ends_with(':') || (first_line.contains('.') && first_line.len() < 100) {
                let rest = query.get(first_line.len()..).unwrap_or("").trim_start();
                (Some(first_line.trim_end_matches(':')), rest)
            } else {
                (None, query)
            }
        } else {
            (None, query)
        };

        let analysis = self.analyze(content, filename);

        let mut output = String::from("File Analysis Results\n\n");
        output.push_str(&format!(
            "Type: {} ({})\n",
            analysis.file_type, analysis.mime_type
        ));

        if let Some(lang) = &analysis.language {
            output.push_str(&format!("Language: {}\n", lang));
        }

        output.push_str(&format!("Encoding: {}\n", analysis.encoding));
        output.push_str(&format!(
            "Statistics: {} lines, {} words, {} characters\n",
            analysis.line_count, analysis.word_count, analysis.char_count
        ));
        output.push_str(&format!(
            "Quality Score: {:.0}%\n",
            analysis.quality_score * 100.0
        ));

        if let Some(structure) = &analysis.structure {
            output.push_str("\nStructure:\n");
            if structure.has_headers {
                output.push_str("  - Has headers\n");
            }
            if structure.has_lists {
                output.push_str("  - Has lists\n");
            }
            if structure.has_code_blocks {
                output.push_str("  - Has code blocks\n");
            }
            if structure.has_links {
                output.push_str("  - Has links\n");
            }
            if structure.has_images {
                output.push_str("  - Has images\n");
            }
            if structure.sections > 0 {
                output.push_str(&format!("  - {} sections\n", structure.sections));
            }
        }

        if !analysis.entities.is_empty() {
            output.push_str("\nEntities Found:\n");
            for entity in analysis.entities.iter().take(10) {
                let display_value = if entity.value.len() > 30 {
                    format!("{}...", &entity.value[..30])
                } else {
                    entity.value.clone()
                };
                output.push_str(&format!(
                    "  - {} ({}): {} occurrence(s)\n",
                    entity.entity_type, display_value, entity.count
                ));
            }
        }

        Ok(ToolResult {
            tool: ToolType::FileAnalyzer,
            success: true,
            result: output,
            metadata: ToolMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                confidence: analysis.quality_score,
                source: Some("FileAnalyzer".to_string()),
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
    async fn test_rust_detection() {
        let tool = FileAnalyzerTool::new();
        let content = "fn main() {\n    let x = 5;\n    println!(\"Hello\");\n}";
        let result = tool.execute(content).await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("Rust"));
    }

    #[tokio::test]
    async fn test_markdown_detection() {
        let tool = FileAnalyzerTool::new();
        let content =
            "# Title\n\n## Section 1\n\nSome text here.\n\n## Section 2\n\n- Item 1\n- Item 2";
        let result = tool.execute(content).await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("Markdown") || res.result.contains("headers"));
    }

    #[tokio::test]
    async fn test_entity_extraction() {
        let tool = FileAnalyzerTool::new();
        let content = "Contact us at test@example.com or visit https://example.com";
        let result = tool.execute(content).await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("email") || res.result.contains("url"));
    }
}
