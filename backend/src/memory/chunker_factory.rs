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
    Sentence,
    Pipeline,
}

impl From<crate::config::ChunkerMode> for ChunkerMode {
    fn from(mode: crate::config::ChunkerMode) -> Self {
        match mode {
            crate::config::ChunkerMode::Fixed => ChunkerMode::Fixed,
            crate::config::ChunkerMode::Lightweight => ChunkerMode::Lightweight,
            crate::config::ChunkerMode::Semantic => ChunkerMode::Semantic,
            crate::config::ChunkerMode::Sentence => ChunkerMode::Sentence,
            crate::config::ChunkerMode::Pipeline => ChunkerMode::Pipeline,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChunkingStats {
    pub semantic_similarity_threshold: f32,
    pub semantic_flushes: usize,
    pub heading_flushes: usize,
    pub size_flushes: usize,
    pub sentence_flushes: usize,
    pub total_segments: usize,
    pub total_chunks: usize,
    pub similarity_observations: usize,
    pub similarity_sum: f32,
    // Universal stats (all modes)
    pub avg_chunk_tokens: usize,
    pub min_chunk_tokens: usize,
    pub max_chunk_tokens: usize,
    // Preprocessing stats
    pub html_tags_stripped: usize,
    pub unicode_chars_normalized: usize,
    pub context_prefixes_added: usize,
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

/// IR-aware chunking: atomic blocks (Table, Code, Formula) are emitted as single
/// chunks; header blocks flush pending text and lead the next accumulation;
/// everything else is passed through the underlying chunker unchanged.
///
/// Returns `(chunk_text, ChunkMeta)` pairs so block provenance survives into
/// the index and search results.  When a chunker splits one accumulation into
/// several chunks, all share the same meta (page + type of the first block).
pub fn chunk_ir(
    ir: &crate::doc_ir::DocIR,
    chunker: &dyn Chunker,
) -> Vec<(String, crate::doc_ir::ChunkMeta)> {
    use crate::doc_ir::ChunkMeta;

    let mut result: Vec<(String, ChunkMeta)> = Vec::new();
    let mut pending = String::new();
    let mut pending_meta = ChunkMeta::default();

    let block_extractor = |block: &crate::doc_ir::DocBlock| -> String {
        block
            .metadata
            .get("extractor")
            .cloned()
            .unwrap_or_else(|| "builtin".into())
    };

    for block in &ir.blocks {
        if block.block_type.is_atomic() {
            // Flush accumulated text first
            if !pending.trim().is_empty() {
                for text in chunker.chunk_text(pending.trim()) {
                    result.push((text, pending_meta.clone()));
                }
                pending.clear();
                pending_meta = ChunkMeta::default();
            }
            let text = block.embed_text().trim().to_string();
            if !text.is_empty() {
                let meta = ChunkMeta {
                    block_type: block.block_type.name().to_string(),
                    page: block.page,
                    extractor: block_extractor(block),
                };
                result.push((text, meta));
            }
        } else if block.block_type.is_strong_boundary() {
            // Flush, then start fresh accumulation with the header leading
            if !pending.trim().is_empty() {
                for text in chunker.chunk_text(pending.trim()) {
                    result.push((text, pending_meta.clone()));
                }
                pending.clear();
            }
            let header = block.embed_text().trim().to_string();
            pending_meta = ChunkMeta {
                block_type: block.block_type.name().to_string(),
                page: block.page,
                extractor: block_extractor(block),
            };
            if !header.is_empty() {
                pending.push_str(&header);
            }
        } else {
            if pending.is_empty() {
                // First text block in a new accumulation — claim its meta.
                pending_meta = ChunkMeta {
                    block_type: block.block_type.name().to_string(),
                    page: block.page,
                    extractor: block_extractor(block),
                };
            }
            if !pending.is_empty() {
                pending.push_str("\n\n");
            }
            pending.push_str(block.text.trim());
        }
    }

    if !pending.trim().is_empty() {
        for text in chunker.chunk_text(pending.trim()) {
            result.push((text, pending_meta.clone()));
        }
    }

    result
}

pub trait Chunker {
    fn chunk_text(&self, text: &str) -> Vec<String>;

    /// Chunk from pre-split segments rather than raw text.
    /// The default joins segments with double-newlines and re-chunks; chunkers
    /// that can make better use of pre-split units (e.g. Semantic) override this.
    fn chunk_from_segments(&self, segments: Vec<String>) -> Vec<String> {
        self.chunk_text(&segments.join("\n\n"))
    }

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

    fn stats(&self) -> Option<ChunkingStats> {
        None // Fixed chunker doesn't track per-run stats; filled in by index layer
    }
}

pub struct LightweightAdaptiveChunker {
    config: ChunkerConfig,
    last_stats: RefCell<Option<ChunkingStats>>,
}

impl LightweightAdaptiveChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self {
            config,
            last_stats: RefCell::new(None),
        }
    }
}

impl Chunker for LightweightAdaptiveChunker {
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let mut stats = ChunkingStats::default();
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_tokens = 0usize;

        for segment in split_into_segments(text) {
            if segment.is_empty() {
                continue;
            }
            stats.total_segments += 1;
            let seg_tokens = estimate_token_count(&segment);
            let heading = is_heading_segment(&segment);

            let size_overflow =
                !current.is_empty() && current_tokens + seg_tokens > self.config.max_size;
            let heading_boundary = heading && current_tokens >= self.config.min_size;

            if size_overflow || heading_boundary {
                stats.size_flushes += if size_overflow { 1 } else { 0 };
                stats.heading_flushes += if heading_boundary && !size_overflow {
                    1
                } else {
                    0
                };
                let chunk = current.trim().to_string();
                let ct = estimate_token_count(&chunk);
                update_token_range(&mut stats, ct);
                chunks.push(chunk);
                current.clear();
                current_tokens = 0;
            }

            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(segment.trim());
            current_tokens += seg_tokens;

            if current_tokens >= self.config.target_size {
                stats.size_flushes += 1;
                let chunk = current.trim().to_string();
                let ct = estimate_token_count(&chunk);
                update_token_range(&mut stats, ct);
                chunks.push(chunk);
                current.clear();
                current_tokens = 0;
            }
        }

        if !current.trim().is_empty() {
            let chunk = current.trim().to_string();
            let ct = estimate_token_count(&chunk);
            update_token_range(&mut stats, ct);
            chunks.push(chunk);
        }

        let filtered: Vec<String> = chunks.into_iter().filter(|c| !c.is_empty()).collect();
        stats.total_chunks = filtered.len();
        if stats.total_chunks > 0 {
            stats.avg_chunk_tokens = filtered
                .iter()
                .map(|c| estimate_token_count(c))
                .sum::<usize>()
                / stats.total_chunks;
        }
        self.last_stats.replace(Some(stats));
        filtered
    }

    fn stats(&self) -> Option<ChunkingStats> {
        self.last_stats.borrow().clone()
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

impl SemanticAdaptiveChunker {
    /// Core segment-accumulation loop shared by both `chunk_text` and
    /// `chunk_from_segments`.  Accepts an iterator of non-empty segment strings.
    fn process_segments<I>(&self, segments: I) -> Vec<String>
    where
        I: Iterator<Item = String>,
    {
        let mut stats = ChunkingStats {
            semantic_similarity_threshold: self.config.semantic_similarity_threshold,
            ..ChunkingStats::default()
        };

        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_tokens = 0usize;
        let mut chunk_embedding_sum: Option<Vec<f32>> = None;

        for segment in segments {
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
        if stats.total_chunks > 0 {
            let token_counts: Vec<usize> =
                filtered.iter().map(|c| estimate_token_count(c)).collect();
            stats.min_chunk_tokens = *token_counts.iter().min().unwrap_or(&0);
            stats.max_chunk_tokens = *token_counts.iter().max().unwrap_or(&0);
            stats.avg_chunk_tokens = token_counts.iter().sum::<usize>() / stats.total_chunks;
        }
        self.last_stats.replace(Some(stats));

        filtered
    }
}

impl Chunker for SemanticAdaptiveChunker {
    fn chunk_text(&self, text: &str) -> Vec<String> {
        self.process_segments(split_into_segments(text).into_iter())
    }

    /// Pipeline-aware variant: receives pre-split sentence units from upstream
    /// stages and feeds them directly into the centroid loop, skipping the
    /// paragraph-level `split_into_segments` call.
    fn chunk_from_segments(&self, segments: Vec<String>) -> Vec<String> {
        self.process_segments(segments.into_iter())
    }

    fn stats(&self) -> Option<ChunkingStats> {
        self.last_stats.borrow().clone()
    }
}

/// Sentence-boundary chunker: splits text at `.!?` boundaries, accumulates
/// sentences to target_size, hard-flushes at max_size, carries overlap forward.
pub struct SentenceChunker {
    config: ChunkerConfig,
    last_stats: RefCell<Option<ChunkingStats>>,
}

impl SentenceChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self {
            config,
            last_stats: RefCell::new(None),
        }
    }
}

impl Chunker for SentenceChunker {
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let mut stats = ChunkingStats::default();
        let sentences = split_into_sentences(text);
        stats.total_segments = sentences.len();

        let mut chunks: Vec<String> = Vec::new();
        let mut current_sentences: Vec<String> = Vec::new();
        let mut current_tokens = 0usize;

        for sentence in &sentences {
            let st = estimate_token_count(sentence);

            // Hard flush if adding this sentence would exceed max_size
            if !current_sentences.is_empty() && current_tokens + st > self.config.max_size {
                let chunk = current_sentences.join(" ");
                let ct = estimate_token_count(&chunk);
                update_token_range(&mut stats, ct);
                chunks.push(chunk);
                stats.size_flushes += 1;
                // Carry overlap: remove sentences from front until under overlap budget
                while current_tokens > self.config.overlap && !current_sentences.is_empty() {
                    let removed = current_sentences.remove(0);
                    current_tokens -= estimate_token_count(&removed);
                }
            }

            current_sentences.push(sentence.clone());
            current_tokens += st;

            // Soft flush at target_size on sentence boundary
            if current_tokens >= self.config.target_size {
                let chunk = current_sentences.join(" ");
                let ct = estimate_token_count(&chunk);
                update_token_range(&mut stats, ct);
                chunks.push(chunk);
                stats.sentence_flushes += 1;
                // Carry overlap
                while current_tokens > self.config.overlap && !current_sentences.is_empty() {
                    let removed = current_sentences.remove(0);
                    current_tokens -= estimate_token_count(&removed);
                }
            }
        }

        if !current_sentences.is_empty() {
            let chunk = current_sentences.join(" ");
            if !chunk.trim().is_empty() {
                let ct = estimate_token_count(&chunk);
                update_token_range(&mut stats, ct);
                chunks.push(chunk);
            }
        }

        let filtered: Vec<String> = chunks
            .into_iter()
            .filter(|c| !c.trim().is_empty())
            .collect();
        stats.total_chunks = filtered.len();
        if stats.total_chunks > 0 {
            stats.avg_chunk_tokens = filtered
                .iter()
                .map(|c| estimate_token_count(c))
                .sum::<usize>()
                / stats.total_chunks;
        }
        self.last_stats.replace(Some(stats));
        filtered
    }

    fn stats(&self) -> Option<ChunkingStats> {
        self.last_stats.borrow().clone()
    }
}

/// Which sub-chunkers are active in a pipeline run.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PipelineStage {
    Lightweight,
    Sentence,
    Semantic,
}

fn parse_pipeline_stages(stages_str: &str) -> Vec<PipelineStage> {
    // Parse comma-separated stage tokens in canonical order: lw → sent → sem.
    // Unknown tokens are silently ignored; duplicates are deduplicated.
    let mut seen_lw = false;
    let mut seen_sent = false;
    let mut seen_sem = false;
    for token in stages_str.split(',') {
        match token.trim() {
            "lw" => seen_lw = true,
            "sent" => seen_sent = true,
            "sem" => seen_sem = true,
            _ => {}
        }
    }
    let mut stages = Vec::new();
    if seen_lw {
        stages.push(PipelineStage::Lightweight);
    }
    if seen_sent {
        stages.push(PipelineStage::Sentence);
    }
    if seen_sem {
        stages.push(PipelineStage::Semantic);
    }
    stages
}

/// Configurable multi-stage chunking pipeline.
///
/// The active stages are read from `ChunkerConfig::pipeline_stages` (a
/// comma-separated list of `"lw"`, `"sent"`, `"sem"`).  Order is always
/// Lightweight → Sentence → Semantic regardless of the order in the string.
///
/// Splitting stages (Lightweight, Sentence) map-flatten over their inputs so
/// each upstream chunk is independently refined.  The Semantic stage receives
/// all upstream chunks at once via `chunk_from_segments` so it can make
/// cross-chunk merge/split decisions using the centroid loop.
pub struct PipelineChunker {
    stages: Vec<PipelineStage>,
    lightweight: LightweightAdaptiveChunker,
    sentence: SentenceChunker,
    semantic: SemanticAdaptiveChunker,
}

impl PipelineChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        let stages = parse_pipeline_stages(&config.pipeline_stages);
        Self {
            stages,
            lightweight: LightweightAdaptiveChunker::new(config.clone()),
            sentence: SentenceChunker::new(config.clone()),
            semantic: SemanticAdaptiveChunker::new(config),
        }
    }
}

impl Chunker for PipelineChunker {
    fn chunk_text(&self, text: &str) -> Vec<String> {
        // `current` is None until the first stage runs (receives raw `text`).
        // Subsequent stages receive the previous stage's output.
        let mut current: Option<Vec<String>> = None;

        for stage in &self.stages {
            current = Some(match (stage, current) {
                // First stage — operate on raw text
                (PipelineStage::Lightweight, None) => self.lightweight.chunk_text(text),
                (PipelineStage::Sentence, None) => self.sentence.chunk_text(text),
                (PipelineStage::Semantic, None) => self.semantic.chunk_text(text),
                // Splitting stages — map-flatten over upstream chunks
                (PipelineStage::Lightweight, Some(segs)) => segs
                    .into_iter()
                    .flat_map(|s| self.lightweight.chunk_text(&s))
                    .collect(),
                (PipelineStage::Sentence, Some(segs)) => segs
                    .into_iter()
                    .flat_map(|s| self.sentence.chunk_text(&s))
                    .collect(),
                // Semantic — receives all upstream chunks at once for centroid loop
                (PipelineStage::Semantic, Some(segs)) => self.semantic.chunk_from_segments(segs),
            });
        }

        current.unwrap_or_default()
    }

    fn stats(&self) -> Option<ChunkingStats> {
        // Surface semantic stage stats when it is active; fall back to None.
        if self.stages.contains(&PipelineStage::Semantic) {
            self.semantic.stats()
        } else {
            None
        }
    }
}

pub fn create_chunker(mode: ChunkerMode, config: &ChunkerConfig) -> Box<dyn Chunker> {
    match mode {
        ChunkerMode::Fixed => Box::new(FixedChunker),
        ChunkerMode::Lightweight => Box::new(LightweightAdaptiveChunker::new(config.clone())),
        ChunkerMode::Semantic => Box::new(SemanticAdaptiveChunker::new(config.clone())),
        ChunkerMode::Sentence => Box::new(SentenceChunker::new(config.clone())),
        ChunkerMode::Pipeline => Box::new(PipelineChunker::new(config.clone())),
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

/// Update min/max token range in stats for a single chunk.
fn update_token_range(stats: &mut ChunkingStats, token_count: usize) {
    if stats.total_chunks == 0 {
        stats.min_chunk_tokens = token_count;
        stats.max_chunk_tokens = token_count;
    } else {
        if token_count < stats.min_chunk_tokens {
            stats.min_chunk_tokens = token_count;
        }
        if token_count > stats.max_chunk_tokens {
            stats.max_chunk_tokens = token_count;
        }
    }
}

/// Split text into individual sentences on `.!?` followed by whitespace.
fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let ch = chars[i];
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            // Check if followed by whitespace or end of text
            let next_is_boundary = i + 1 >= len || chars[i + 1].is_whitespace();
            if next_is_boundary && !current.trim().is_empty() {
                sentences.push(current.trim().to_string());
                current.clear();
            }
        }
        i += 1;
    }
    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }
    sentences
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
