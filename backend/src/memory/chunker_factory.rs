use super::chunker::ChunkerConfig;
use crate::embedder;
use crate::embedder::similarity;
use serde::Serialize;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug)]
pub enum ChunkerMode {
    Fixed,
    Lightweight,
    Semantic,
}

impl From<crate::config::ChunkerMode> for ChunkerMode {
    fn from(mode: crate::config::ChunkerMode) -> Self {
        match mode {
            crate::config::ChunkerMode::Fixed => ChunkerMode::Fixed,
            crate::config::ChunkerMode::Lightweight => ChunkerMode::Lightweight,
            crate::config::ChunkerMode::Semantic => ChunkerMode::Semantic,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChunkingStats {
    pub semantic_similarity_threshold: f32,
    pub semantic_flushes: usize,
    pub heading_flushes: usize,
    pub size_flushes: usize,
    pub total_segments: usize,
    pub total_chunks: usize,
    pub similarity_observations: usize,
    pub similarity_sum: f32,
}

impl ChunkingStats {
    pub fn average_similarity(&self) -> Option<f32> {
        if self.similarity_observations == 0 {
            None
        } else {
            Some(self.similarity_sum / self.similarity_observations as f32)
        }
    }
}

pub trait Chunker {
    fn chunk_text(&self, text: &str) -> Vec<String>;

    fn stats(&self) -> Option<ChunkingStats> {
        None
    }
}

pub struct FixedChunker;

impl Chunker for FixedChunker {
    fn chunk_text(&self, text: &str) -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect()
    }
}

pub struct LightweightAdaptiveChunker {
    config: ChunkerConfig,
}

impl LightweightAdaptiveChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self { config }
    }
}

impl Chunker for LightweightAdaptiveChunker {
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_tokens = 0usize;

        for segment in split_into_segments(text) {
            if segment.is_empty() {
                continue;
            }
            let seg_tokens = estimate_token_count(&segment);
            let heading = is_heading_segment(&segment);

            let should_flush = !current.is_empty()
                && (current_tokens + seg_tokens > self.config.max_size
                    || (heading && current_tokens >= self.config.min_size));

            if should_flush {
                chunks.push(current.trim().to_string());
                current.clear();
                current_tokens = 0;
            }

            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(segment.trim());
            current_tokens += seg_tokens;

            if current_tokens >= self.config.target_size {
                chunks.push(current.trim().to_string());
                current.clear();
                current_tokens = 0;
            }
        }

        if !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
        }

        chunks.into_iter().filter(|c| !c.is_empty()).collect()
    }
}

pub struct SemanticAdaptiveChunker {
    config: ChunkerConfig,
    last_stats: RefCell<Option<ChunkingStats>>,
}

impl SemanticAdaptiveChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self {
            config,
            last_stats: RefCell::new(None),
        }
    }
}

impl Chunker for SemanticAdaptiveChunker {
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let mut stats = ChunkingStats {
            semantic_similarity_threshold: self.config.semantic_similarity_threshold,
            ..ChunkingStats::default()
        };

        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_tokens = 0usize;
        let mut chunk_embedding_sum: Option<Vec<f32>> = None;

        for segment in split_into_segments(text) {
            if segment.is_empty() {
                continue;
            }

            stats.total_segments += 1;

            let seg_tokens = estimate_token_count(&segment);
            let seg_embedding = embedder::embed(&segment);
            let heading = is_heading_segment(&segment);

            let similarity_score = chunk_embedding_sum
                .as_ref()
                .map(|sum| similarity::cosine(sum, &seg_embedding));

            if let Some(score) = similarity_score {
                stats.similarity_observations += 1;
                stats.similarity_sum += score;
            }

            let semantic_boundary = similarity_score
                .map(|score| {
                    current_tokens >= self.config.min_size
                        && score < self.config.semantic_similarity_threshold
                })
                .unwrap_or(false);

            let size_overflow =
                !current.is_empty() && current_tokens + seg_tokens > self.config.max_size;
            let heading_boundary = heading && current_tokens >= self.config.min_size;

            if !current.is_empty() && (size_overflow || semantic_boundary || heading_boundary) {
                if semantic_boundary {
                    stats.semantic_flushes += 1;
                } else if heading_boundary {
                    stats.heading_flushes += 1;
                } else if size_overflow {
                    stats.size_flushes += 1;
                }

                chunks.push(current.trim().to_string());
                current.clear();
                current_tokens = 0;
                chunk_embedding_sum = None;
            }

            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(segment.trim());
            current_tokens += seg_tokens;

            if let Some(sum) = chunk_embedding_sum.as_mut() {
                for (sum_val, seg_val) in sum.iter_mut().zip(seg_embedding.iter()) {
                    *sum_val += *seg_val;
                }
            } else {
                chunk_embedding_sum = Some(seg_embedding);
            }

            if current_tokens >= self.config.target_size {
                stats.size_flushes += 1;
                chunks.push(current.trim().to_string());
                current.clear();
                current_tokens = 0;
                chunk_embedding_sum = None;
            }
        }

        if !current.trim().is_empty() {
            stats.size_flushes += 1;
            chunks.push(current.trim().to_string());
        }

        let filtered: Vec<String> = chunks.into_iter().filter(|c| !c.is_empty()).collect();
        stats.total_chunks = filtered.len();
        self.last_stats.replace(Some(stats));

        filtered
    }

    fn stats(&self) -> Option<ChunkingStats> {
        self.last_stats.borrow().clone()
    }
}

pub fn create_chunker(mode: ChunkerMode, config: &ChunkerConfig) -> Box<dyn Chunker> {
    match mode {
        ChunkerMode::Fixed => Box::new(FixedChunker),
        ChunkerMode::Lightweight => Box::new(LightweightAdaptiveChunker::new(config.clone())),
        ChunkerMode::Semantic => Box::new(SemanticAdaptiveChunker::new(config.clone())),
    }
}

fn is_heading_segment(segment: &str) -> bool {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('#') {
        return true;
    }
    if trimmed.ends_with(':') {
        return true;
    }
    let uppercase = trimmed.chars().filter(|c| c.is_ascii_uppercase()).count();
    let letters = trimmed.chars().filter(|c| c.is_ascii_alphabetic()).count();
    letters > 0 && uppercase * 2 >= letters * 3
}

fn estimate_token_count(text: &str) -> usize {
    if let Some(handle) = crate::api::get_token_counter() {
        handle.count_tokens(text)
    } else {
        let char_estimate = text.len() / 4;
        let word_estimate = text.split_whitespace().count() * 4 / 3;
        (char_estimate + word_estimate) / 2
    }
}

fn split_into_segments(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                segments.push(current.trim().to_string());
                current.clear();
            }
            continue;
        }

        if trimmed.starts_with('-') || trimmed.starts_with('*') {
            if !current.is_empty() {
                segments.push(current.trim().to_string());
                current.clear();
            }
            segments.push(trimmed.to_string());
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(trimmed);
        }
    }

    if !current.is_empty() {
        segments.push(current.trim().to_string());
    }

    segments
}
