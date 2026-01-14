// src/memory/chunker.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub content: String,
    pub chunk_index: usize,
    pub token_count: usize,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub document_id: String,
    pub source: String,
    pub source_type: SourceType,
    pub created_at: i64,
    pub start_char: usize,
    pub end_char: usize,
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceType {
    Pdf,
    Text,
    Markdown,
    Html,
    Code,
    Json,
    Xml,
    Binary,
}

impl SourceType {
    /// Detect source type from file bytes using MIME type detection.
    /// Falls back to extension-based detection if magic bytes don't match.
    pub fn detect(bytes: &[u8], filename: Option<&str>) -> Self {
        use crate::mime_detect::detect_content_type;

        let content_type = detect_content_type(bytes, filename);
        Self::from_content_type(&content_type)
    }

    /// Detect source type from file extension only (legacy method)
    pub fn from_extension(filename: &str) -> Self {
        use crate::mime_detect::detect_from_extension;

        let content_type = detect_from_extension(filename);
        Self::from_content_type(&content_type)
    }

    /// Convert from mime_detect::ContentType
    fn from_content_type(ct: &crate::mime_detect::ContentType) -> Self {
        use crate::mime_detect::ContentType;

        match ct {
            ContentType::Pdf => SourceType::Pdf,
            ContentType::Text => SourceType::Text,
            ContentType::Markdown => SourceType::Markdown,
            ContentType::Html => SourceType::Html,
            ContentType::Code(_) => SourceType::Code,
            ContentType::Json => SourceType::Json,
            ContentType::Xml => SourceType::Xml,
            ContentType::Binary => SourceType::Binary,
            ContentType::Unknown => SourceType::Text, // Default to text
        }
    }

    /// Check if this source type can be chunked as text
    pub fn is_chunkable(&self) -> bool {
        !matches!(self, SourceType::Binary)
    }
}

// Default values optimized for BGE-small-en-v1.5 (512 token max sequence length)
// These defaults balance retrieval quality with embedding model constraints
pub const DEFAULT_TARGET_SIZE: usize = 256; // Target tokens per chunk (leaves room for query)
pub const DEFAULT_MIN_SIZE: usize = 128; // Minimum tokens (avoid too-small chunks)
pub const DEFAULT_MAX_SIZE: usize = 384; // Maximum tokens (stay well under 512 limit)
pub const DEFAULT_OVERLAP: usize = 32; // Overlap tokens (helps with context continuity)
pub const DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD: f32 = 0.78;

// BGE-small-en-v1.5 specific constants
pub const BGE_SMALL_MAX_TOKENS: usize = 512; // Model's max sequence length
pub const BGE_SMALL_OPTIMAL_CHUNK: usize = 256; // Optimal chunk size for retrieval quality

#[derive(Debug, Clone)]
pub struct ChunkerConfig {
    pub target_size: usize, // Target tokens per chunk
    pub min_size: usize,    // Minimum tokens (allow smaller for semantic boundaries)
    pub max_size: usize,    // Maximum tokens (avoid splitting mid-concept)
    pub overlap: usize,     // Overlap tokens between chunks
    pub semantic_similarity_threshold: f32,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            target_size: DEFAULT_TARGET_SIZE,
            min_size: DEFAULT_MIN_SIZE,
            max_size: DEFAULT_MAX_SIZE,
            overlap: DEFAULT_OVERLAP,
            semantic_similarity_threshold: DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD,
        }
    }
}

impl ChunkerConfig {
    /// Load configuration from environment variables with sensible defaults
    ///
    /// Environment variables:
    /// - CHUNK_TARGET_SIZE: Target tokens per chunk (default: 384)
    /// - CHUNK_MIN_SIZE: Minimum tokens per chunk (default: 192)
    /// - CHUNK_MAX_SIZE: Maximum tokens per chunk (default: 512)
    /// - CHUNK_OVERLAP: Overlap tokens between chunks (default: 50)
    /// - SEMANTIC_SIMILARITY_THRESHOLD: Threshold for semantic chunking (default: 0.78)
    pub fn from_env() -> Self {
        let target_size = env::var("CHUNK_TARGET_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_TARGET_SIZE);

        let min_size = env::var("CHUNK_MIN_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MIN_SIZE);

        let max_size = env::var("CHUNK_MAX_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_SIZE);

        let overlap = env::var("CHUNK_OVERLAP")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_OVERLAP);

        let semantic_similarity_threshold = env::var("SEMANTIC_SIMILARITY_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(|v: f32| v.clamp(0.0, 1.0))
            .unwrap_or(DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD);

        Self {
            target_size,
            min_size,
            max_size,
            overlap,
            semantic_similarity_threshold,
        }
    }

    /// Create config optimized for different LLM context sizes
    pub fn for_model(model_context_size: usize) -> Self {
        match model_context_size {
            // Small models (phi, tinyllama) - 2K context
            0..=2048 => Self {
                target_size: 256,
                min_size: 128,
                max_size: 384,
                overlap: 32,
                semantic_similarity_threshold: DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD,
            },
            // Medium models (llama2-7b) - 4K context
            2049..=4096 => Self {
                target_size: 384,
                min_size: 192,
                max_size: 512,
                overlap: 48,
                semantic_similarity_threshold: DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD,
            },
            // Large models (llama3, mistral) - 8K+ context
            _ => Self {
                target_size: 512,
                min_size: 256,
                max_size: 768,
                overlap: 64,
                semantic_similarity_threshold: DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD,
            },
        }
    }

    /// Create config optimized for specific embedding models
    pub fn for_embedding_model(model_name: &str) -> Self {
        match model_name.to_lowercase().as_str() {
            // BGE-small-en-v1.5: 512 token max, 384-dim embeddings
            "bge-small-en-v1.5" | "bge-small-en-v1.5q" | "bge-small" => Self {
                target_size: 256,                    // ~50% of max to leave room for query context
                min_size: 128,                       // Avoid too-small chunks that lose context
                max_size: 384,                       // Stay well under 512 limit
                overlap: 32,                         // ~12% overlap for continuity
                semantic_similarity_threshold: 0.75, // Slightly lower for better recall
            },
            // BGE-base-en-v1.5: 512 token max, 768-dim embeddings
            "bge-base-en-v1.5" | "bge-base" => Self {
                target_size: 256,
                min_size: 128,
                max_size: 384,
                overlap: 32,
                semantic_similarity_threshold: 0.75,
            },
            // BGE-large-en-v1.5: 512 token max, 1024-dim embeddings
            "bge-large-en-v1.5" | "bge-large" => Self {
                target_size: 256,
                min_size: 128,
                max_size: 384,
                overlap: 32,
                semantic_similarity_threshold: 0.75,
            },
            // all-MiniLM-L6-v2: 256 token max, 384-dim embeddings
            "all-minilm-l6-v2" | "minilm" => Self {
                target_size: 128,
                min_size: 64,
                max_size: 192,
                overlap: 16,
                semantic_similarity_threshold: 0.78,
            },
            // Default: assume BGE-small-like constraints
            _ => Self::default(),
        }
    }
}

pub struct SemanticChunker {
    config: ChunkerConfig,
}

impl SemanticChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self { config }
    }

    pub fn with_default() -> Self {
        Self::new(ChunkerConfig::default())
    }

    /// Main entry point: chunk a document with metadata
    pub fn chunk_document(
        &self,
        content: &str,
        document_id: String,
        source: String,
        source_type: SourceType,
    ) -> Vec<Chunk> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Split into semantic units (paragraphs, sections)
        let units = self.split_into_semantic_units(content, &source_type);

        // Group units into chunks based on token limits
        let grouped_chunks = self.group_into_chunks(&units);

        // Create Chunk objects with metadata
        grouped_chunks
            .into_iter()
            .enumerate()
            .map(|(idx, (text, start_char, end_char))| {
                let token_count = self.estimate_tokens(&text);

                Chunk {
                    id: Uuid::new_v4().to_string(),
                    content: text,
                    chunk_index: idx,
                    token_count,
                    metadata: ChunkMetadata {
                        document_id: document_id.clone(),
                        source: source.clone(),
                        source_type: source_type.clone(),
                        created_at: now,
                        start_char,
                        end_char,
                        extra: HashMap::new(),
                    },
                }
            })
            .collect()
    }

    /// Split text into semantic units based on source type
    fn split_into_semantic_units(
        &self,
        content: &str,
        source_type: &SourceType,
    ) -> Vec<SemanticUnit> {
        match source_type {
            SourceType::Pdf | SourceType::Text => self.split_by_paragraphs(content),
            SourceType::Markdown => self.split_markdown(content),
            SourceType::Html | SourceType::Xml => self.split_html(content),
            SourceType::Code => self.split_code(content),
            SourceType::Json => self.split_json(content),
            SourceType::Binary => vec![], // Binary files cannot be chunked as text
        }
    }

    /// Split JSON by top-level keys or array elements
    fn split_json(&self, content: &str) -> Vec<SemanticUnit> {
        // For JSON, we try to split by logical sections
        // If it's an object, split by top-level keys
        // If it's an array, split by elements
        // Fall back to paragraph splitting if parsing fails

        let trimmed = content.trim();

        // Simple heuristic: if it looks like a JSON object or array, try to split smartly
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            // For now, use paragraph-based splitting which works reasonably for formatted JSON
            // A more sophisticated approach would parse the JSON and split by structure
            self.split_by_paragraphs(content)
        } else {
            self.split_by_paragraphs(content)
        }
    }

    /// Split by paragraphs (double newline or period + newline)
    fn split_by_paragraphs(&self, content: &str) -> Vec<SemanticUnit> {
        let mut units = Vec::new();
        let mut current_pos = 0;

        // Split on paragraph boundaries
        for paragraph in content.split("\n\n") {
            let trimmed = paragraph.trim();
            if trimmed.is_empty() {
                current_pos += paragraph.len() + 2;
                continue;
            }

            // Further split long paragraphs by sentences
            if self.estimate_tokens(trimmed) > self.config.target_size {
                units.extend(self.split_by_sentences(trimmed, current_pos));
            } else {
                units.push(SemanticUnit {
                    text: trimmed.to_string(),
                    start_char: current_pos,
                    end_char: current_pos + trimmed.len(),
                    boundary_strength: BoundaryStrength::Strong,
                });
            }

            current_pos += paragraph.len() + 2;
        }

        units
    }

    /// Split by sentences for long paragraphs
    fn split_by_sentences(&self, text: &str, base_offset: usize) -> Vec<SemanticUnit> {
        let mut units = Vec::new();
        let _current_pos = 0; // or just remove it if unused

        // Simple sentence splitting on .!? followed by space and capital
        let sentence_regex = regex::Regex::new(r"([.!?]+)\s+(?=[A-Z])").unwrap();

        let mut last_end = 0;
        for mat in sentence_regex.find_iter(text) {
            let sentence = &text[last_end..mat.end()].trim();
            if !sentence.is_empty() {
                units.push(SemanticUnit {
                    text: sentence.to_string(),
                    start_char: base_offset + last_end,
                    end_char: base_offset + mat.end(),
                    boundary_strength: BoundaryStrength::Medium,
                });
            }
            last_end = mat.end();
        }

        // Add remaining text
        if last_end < text.len() {
            let sentence = text[last_end..].trim();
            if !sentence.is_empty() {
                units.push(SemanticUnit {
                    text: sentence.to_string(),
                    start_char: base_offset + last_end,
                    end_char: base_offset + text.len(),
                    boundary_strength: BoundaryStrength::Medium,
                });
            }
        }

        units
    }

    /// Split markdown by headers and paragraphs
    fn split_markdown(&self, content: &str) -> Vec<SemanticUnit> {
        let mut units = Vec::new();
        let mut current_pos = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // Headers are strong boundaries
            if trimmed.starts_with('#') {
                if !trimmed.is_empty() {
                    units.push(SemanticUnit {
                        text: trimmed.to_string(),
                        start_char: current_pos,
                        end_char: current_pos + line.len(),
                        boundary_strength: BoundaryStrength::Strong,
                    });
                }
            } else if !trimmed.is_empty() {
                units.push(SemanticUnit {
                    text: trimmed.to_string(),
                    start_char: current_pos,
                    end_char: current_pos + line.len(),
                    boundary_strength: BoundaryStrength::Weak,
                });
            }

            current_pos += line.len() + 1;
        }

        units
    }

    /// Split HTML by tags (simplified - would need proper HTML parser for production)
    fn split_html(&self, content: &str) -> Vec<SemanticUnit> {
        // For now, strip tags and split by paragraphs
        let text = content.replace("<br>", "\n").replace("</p>", "\n\n");
        let cleaned = regex::Regex::new(r"<[^>]+>")
            .unwrap()
            .replace_all(&text, "");
        self.split_by_paragraphs(&cleaned)
    }

    /// Split code by functions/classes
    fn split_code(&self, content: &str) -> Vec<SemanticUnit> {
        let mut units = Vec::new();
        let mut current_pos = 0;
        let mut current_block = String::new();
        let mut block_start = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // Detect function/class boundaries
            let is_boundary = trimmed.starts_with("fn ")
                || trimmed.starts_with("func ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("async fn ");

            if is_boundary && !current_block.is_empty() {
                units.push(SemanticUnit {
                    text: current_block.trim().to_string(),
                    start_char: block_start,
                    end_char: current_pos,
                    boundary_strength: BoundaryStrength::Strong,
                });
                current_block.clear();
                block_start = current_pos;
            }

            current_block.push_str(line);
            current_block.push('\n');
            current_pos += line.len() + 1;
        }

        // Add remaining block
        if !current_block.trim().is_empty() {
            units.push(SemanticUnit {
                text: current_block.trim().to_string(),
                start_char: block_start,
                end_char: current_pos,
                boundary_strength: BoundaryStrength::Strong,
            });
        }

        units
    }

    /// Group semantic units into chunks respecting token limits
    fn group_into_chunks(&self, units: &[SemanticUnit]) -> Vec<(String, usize, usize)> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut current_tokens = 0;
        let mut chunk_start = 0;
        let mut chunk_end = 0;

        for unit in units {
            let unit_tokens = self.estimate_tokens(&unit.text);

            // Check if adding this unit would exceed max_size
            if current_tokens + unit_tokens > self.config.max_size && !current_chunk.is_empty() {
                // Save current chunk
                chunks.push((current_chunk.trim().to_string(), chunk_start, chunk_end));

                // Start new chunk with overlap
                let overlap_text = self.get_overlap_text(&current_chunk);
                current_chunk = overlap_text;
                current_tokens = self.estimate_tokens(&current_chunk);
                chunk_start = unit.start_char;
            }

            // Add unit to current chunk
            if !current_chunk.is_empty() {
                current_chunk.push_str("\n\n");
            }
            current_chunk.push_str(&unit.text);
            current_tokens += unit_tokens;
            chunk_end = unit.end_char;

            // If we've reached target size at a strong boundary, save chunk
            if current_tokens >= self.config.target_size
                && matches!(unit.boundary_strength, BoundaryStrength::Strong)
            {
                chunks.push((current_chunk.trim().to_string(), chunk_start, chunk_end));
                current_chunk.clear();
                current_tokens = 0;
                chunk_start = chunk_end;
            }
        }

        // Add final chunk
        if !current_chunk.trim().is_empty() {
            chunks.push((current_chunk.trim().to_string(), chunk_start, chunk_end));
        }

        chunks
    }

    /// Get last N tokens for overlap
    fn get_overlap_text(&self, text: &str) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        let overlap_words = (self.config.overlap * 3 / 4).min(words.len()); // ~0.75 tokens per word

        words[words.len().saturating_sub(overlap_words)..].join(" ")
    }

    /// Estimate token count (rough approximation: 1 token ≈ 4 chars or 0.75 words)
    fn estimate_tokens(&self, text: &str) -> usize {
        let char_estimate = text.len() / 4;
        let word_estimate = text.split_whitespace().count() * 4 / 3;
        (char_estimate + word_estimate) / 2
    }
}

#[derive(Debug, Clone)]
struct SemanticUnit {
    text: String,
    start_char: usize,
    end_char: usize,
    boundary_strength: BoundaryStrength,
}

#[derive(Debug, Clone)]
enum BoundaryStrength {
    Strong, // Paragraph, header, function
    Medium, // Sentence
    Weak,   // Line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let chunker = SemanticChunker::with_default();
        let content = "This is paragraph one. It has multiple sentences.\n\nThis is paragraph two. It's a bit longer and has more content to test the chunking logic.";

        let chunks = chunker.chunk_document(
            content,
            "doc1".to_string(),
            "test.txt".to_string(),
            SourceType::Text,
        );

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].chunk_index, 0);
        assert!(chunks[0].token_count > 0);
    }

    #[test]
    fn test_markdown_chunking() {
        let chunker = SemanticChunker::with_default();
        let content = "# Header 1\n\nSome content here.\n\n## Header 2\n\nMore content.";

        let chunks = chunker.chunk_document(
            content,
            "doc2".to_string(),
            "test.md".to_string(),
            SourceType::Markdown,
        );

        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_token_estimation() {
        let chunker = SemanticChunker::with_default();
        let text = "The quick brown fox jumps over the lazy dog";
        let tokens = chunker.estimate_tokens(text);

        // Should be roughly 9-12 tokens for this sentence
        assert!(tokens >= 8 && tokens <= 15);
    }

    #[test]
    fn test_bge_small_config() {
        let config = ChunkerConfig::for_embedding_model("bge-small-en-v1.5");

        // BGE-small has 512 token max, so max_size should be well under that
        assert!(config.max_size <= 384);
        assert!(config.target_size <= config.max_size);
        assert!(config.min_size <= config.target_size);
        assert!(config.overlap < config.min_size);
    }

    #[test]
    fn test_minilm_config() {
        let config = ChunkerConfig::for_embedding_model("all-minilm-l6-v2");

        // MiniLM has 256 token max, so max_size should be well under that
        assert!(config.max_size <= 192);
        assert!(config.target_size <= config.max_size);
    }

    #[test]
    fn test_default_config_values() {
        let config = ChunkerConfig::default();

        // Verify new BGE-optimized defaults
        assert_eq!(config.target_size, 256);
        assert_eq!(config.min_size, 128);
        assert_eq!(config.max_size, 384);
        assert_eq!(config.overlap, 32);
    }
}
