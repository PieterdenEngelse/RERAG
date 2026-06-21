use super::chunker::ChunkerConfig;
use crate::embedder;
use crate::embedder::similarity;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
///
/// Two retrieval-quality features ride on the same heading walk:
///   * `heading_path` — the H1>H2>H3 ancestor chain at the chunk's position,
///     also prepended to the chunk text so the embedding model sees it.
///   * `section_id` — a UUID shared by every chunk between two heading
///     boundaries, so PointerRag can hydrate the full section at query time.
pub fn chunk_ir(
    ir: &crate::doc_ir::DocIR,
    chunker: &dyn Chunker,
) -> Vec<(String, crate::doc_ir::ChunkMeta)> {
    use crate::doc_ir::{ChunkMeta, ColumnPosition};
    use std::collections::BTreeSet;

    let mut result: Vec<(String, ChunkMeta)> = Vec::new();
    let mut pending = String::new();
    let mut pending_meta = ChunkMeta::default();
    let mut pending_columns: BTreeSet<ColumnPosition> = BTreeSet::new();
    // (level, header_text) stack — newest level at the back.
    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut current_section_id = uuid::Uuid::new_v4().to_string();
    // Track the column position the current accumulation lives in so a
    // same-page transition to a different column forces a flush (column-pure
    // chunks). None means "we haven't committed to a column yet" — Single
    // and Multi don't trigger flushes; only Left↔Right cross-column moves do.
    let mut pending_column: Option<ColumnPosition> = None;
    let mut pending_page: Option<u32> = None;

    let block_extractor = |block: &crate::doc_ir::DocBlock| -> String {
        block
            .metadata
            .get("extractor")
            .cloned()
            .unwrap_or_else(|| "builtin".into())
    };

    let block_column = |block: &crate::doc_ir::DocBlock| -> Option<ColumnPosition> {
        block
            .metadata
            .get("column_position")
            .and_then(|s| ColumnPosition::from_str_opt(s))
    };

    let heading_path =
        |stack: &[(u8, String)]| -> Vec<String> { stack.iter().map(|(_, t)| t.clone()).collect() };
    let format_breadcrumb = |stack: &[(u8, String)]| -> String {
        stack
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    };

    for block in &ir.blocks {
        // Same-page transition between different non-Multi columns is a
        // strong boundary. Multi and Single never trigger this — they're
        // either ambiguous (don't pretend to know) or single-column (no
        // disambiguation needed). With adaptive-k the source columns are
        // arbitrary Col(a) / Col(b) labels — any a != b crossing fires.
        if let (Some(prev_page), Some(prev_col), Some(cur_col)) =
            (pending_page, pending_column, block_column(block))
        {
            let same_page = block.page == Some(prev_page);
            let crossing = matches!(
                (prev_col, cur_col),
                (ColumnPosition::Col(a), ColumnPosition::Col(b)) if a != b
            );
            if same_page && crossing && !pending.trim().is_empty() {
                let mut flush_meta = pending_meta.clone();
                flush_meta.column_position_set = std::mem::take(&mut pending_columns);
                for text in chunker.chunk_text(pending.trim()) {
                    result.push((text, flush_meta.clone()));
                }
                pending.clear();
                pending_meta = ChunkMeta {
                    heading_path: heading_path(&heading_stack),
                    section_id: current_section_id.clone(),
                    ..Default::default()
                };
                pending_column = None;
                pending_page = None;
            }
        }

        if block.block_type.is_atomic() {
            // Flush accumulated text first
            if !pending.trim().is_empty() {
                let mut flush_meta = pending_meta.clone();
                flush_meta.column_position_set = std::mem::take(&mut pending_columns);
                for text in chunker.chunk_text(pending.trim()) {
                    result.push((text, flush_meta.clone()));
                }
                pending.clear();
                pending_meta = ChunkMeta {
                    heading_path: heading_path(&heading_stack),
                    section_id: current_section_id.clone(),
                    ..Default::default()
                };
                pending_column = None;
                pending_page = None;
            }
            let text = block.embed_text().trim().to_string();
            if !text.is_empty() {
                let crumb = format_breadcrumb(&heading_stack);
                let embed_text = if crumb.is_empty() {
                    text
                } else {
                    format!("{}\n\n{}", crumb, text)
                };
                let mut col_set = BTreeSet::new();
                if let Some(c) = block_column(block) {
                    col_set.insert(c);
                }
                let meta = ChunkMeta {
                    block_type: block.block_type.name().to_string(),
                    page: block.page,
                    extractor: block_extractor(block),
                    heading_path: heading_path(&heading_stack),
                    section_id: current_section_id.clone(),
                    column_position_set: col_set,
                };
                result.push((embed_text, meta));
            }
        } else if block.block_type.is_strong_boundary() {
            // Flush, then start fresh accumulation.
            if !pending.trim().is_empty() {
                let mut flush_meta = pending_meta.clone();
                flush_meta.column_position_set = std::mem::take(&mut pending_columns);
                for text in chunker.chunk_text(pending.trim()) {
                    result.push((text, flush_meta.clone()));
                }
                pending.clear();
            }
            pending_column = None;
            pending_page = None;
            // Only Header blocks contribute to the heading stack; PageBreak is a
            // strong boundary too but carries no heading text.
            if let crate::doc_ir::BlockType::Header { level } = block.block_type {
                while let Some(&(lvl, _)) = heading_stack.last() {
                    if lvl >= level {
                        heading_stack.pop();
                    } else {
                        break;
                    }
                }
                let header_text = block.text.trim().to_string();
                if !header_text.is_empty() {
                    heading_stack.push((level, header_text));
                }
            }
            // Every strong boundary starts a new section.
            current_section_id = uuid::Uuid::new_v4().to_string();
            pending_meta = ChunkMeta {
                block_type: block.block_type.name().to_string(),
                page: block.page,
                extractor: block_extractor(block),
                heading_path: heading_path(&heading_stack),
                section_id: current_section_id.clone(),
                column_position_set: BTreeSet::new(),
            };
            let crumb = format_breadcrumb(&heading_stack);
            if !crumb.is_empty() {
                pending.push_str(&crumb);
            }
        } else {
            if pending.is_empty() {
                // First text block in a new accumulation — claim its meta.
                pending_meta = ChunkMeta {
                    block_type: block.block_type.name().to_string(),
                    page: block.page,
                    extractor: block_extractor(block),
                    heading_path: heading_path(&heading_stack),
                    section_id: current_section_id.clone(),
                    column_position_set: BTreeSet::new(),
                };
                let crumb = format_breadcrumb(&heading_stack);
                if !crumb.is_empty() {
                    pending.push_str(&crumb);
                }
            }
            if !pending.is_empty() {
                pending.push_str("\n\n");
            }
            pending.push_str(block.text.trim());

            // Track the current accumulation's column + page so the next
            // block's cross-column transition check fires when it should.
            if let Some(col) = block_column(block) {
                pending_columns.insert(col);
                if pending_column.is_none() && matches!(col, ColumnPosition::Col(_)) {
                    pending_column = Some(col);
                }
            }
            if pending_page.is_none() {
                pending_page = block.page;
            }
        }
    }

    if !pending.trim().is_empty() {
        let mut flush_meta = pending_meta.clone();
        flush_meta.column_position_set = std::mem::take(&mut pending_columns);
        for text in chunker.chunk_text(pending.trim()) {
            result.push((text, flush_meta.clone()));
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

pub struct FixedChunker {
    config: ChunkerConfig,
    last_stats: RefCell<Option<ChunkingStats>>,
}

impl FixedChunker {
    pub fn new(config: ChunkerConfig) -> Self {
        Self {
            config,
            last_stats: RefCell::new(None),
        }
    }
}

impl Chunker for FixedChunker {
    /// Chunk text using three complementary techniques:
    ///
    /// 1. **Recursive splitting** — split on `\n\n` → `\n` → `.!? ` → ` ` → chars
    ///    so no unit ever exceeds `max_size`.
    /// 2. **Sentence-boundary snapping** — on every flush, walk backward within
    ///    the last `snap_tolerance` fraction of the text to find the nearest `.!?`
    ///    boundary; the tail after the snap seeds the next chunk.
    /// 3. **Overlap** — the last `overlap` tokens of every flushed chunk are
    ///    prepended to the next chunk for retrieval continuity.
    fn chunk_text(&self, text: &str) -> Vec<String> {
        let mut stats = ChunkingStats::default();

        let units = recursive_split(text, self.config.max_size);
        stats.total_segments = units.len();

        let mut chunks: Vec<String> = Vec::new();
        let mut acc: Vec<String> = Vec::new();
        let mut acc_tokens = 0usize;

        // Inline flush: snap to sentence boundary, push chunk, seed next with overlap + tail.
        macro_rules! flush_acc {
            () => {
                if !acc.is_empty() {
                    let joined = acc.join("\n\n");
                    let (head, tail) = sentence_snap_split(&joined, self.config.snap_tolerance);
                    let head = head.trim().to_string();
                    let tail = tail.trim().to_string();

                    if !head.is_empty() {
                        let ct = estimate_token_count(&head);
                        update_token_range(&mut stats, ct);
                        stats.size_flushes += 1;
                        if !tail.is_empty() {
                            stats.sentence_flushes += 1;
                        }
                        chunks.push(head.clone());
                    }

                    acc.clear();
                    acc_tokens = 0;

                    // Overlap: seed next chunk with the tail end of what was just flushed.
                    if self.config.overlap > 0 && !head.is_empty() {
                        let ov = overlap_tail(&head, self.config.overlap);
                        if !ov.is_empty() {
                            acc_tokens += estimate_token_count(&ov);
                            acc.push(ov);
                        }
                    }
                    // Sentence tail: text after the snap boundary goes into the next chunk.
                    if !tail.is_empty() {
                        acc_tokens += estimate_token_count(&tail);
                        acc.push(tail);
                    }
                }
            };
        }

        for unit in units {
            let unit_tokens = estimate_token_count(&unit);

            // Hard flush: this unit would push us past max_size.
            if !acc.is_empty() && acc_tokens + unit_tokens > self.config.max_size {
                flush_acc!();
            }

            acc.push(unit);
            acc_tokens += unit_tokens;

            // Soft flush: at or past target on a clean unit boundary.
            if acc_tokens >= self.config.target_size {
                flush_acc!();
            }
        }

        // Flush any remaining text.
        if !acc.is_empty() {
            let text = acc.join("\n\n");
            let text = text.trim().to_string();
            if !text.is_empty() {
                update_token_range(&mut stats, estimate_token_count(&text));
                chunks.push(text);
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
        ChunkerMode::Fixed => Box::new(FixedChunker::new(config.clone())),
        ChunkerMode::Lightweight => Box::new(LightweightAdaptiveChunker::new(config.clone())),
        ChunkerMode::Semantic => Box::new(SemanticAdaptiveChunker::new(config.clone())),
        ChunkerMode::Sentence => Box::new(SentenceChunker::new(config.clone())),
        ChunkerMode::Pipeline => Box::new(PipelineChunker::new(config.clone())),
    }
}

/// Recursively split `text` into units where each unit fits within `max_tokens`.
/// Separators are tried from coarsest to finest: paragraph → line → sentence → word → char.
fn recursive_split(text: &str, max_tokens: usize) -> Vec<String> {
    const SEPS: &[&str] = &["\n\n", "\n", ". ", "! ", "? ", " "];
    recursive_split_inner(text.trim(), SEPS, max_tokens)
}

fn recursive_split_inner(text: &str, seps: &[&str], max_tokens: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }
    if estimate_token_count(text) <= max_tokens {
        return vec![text.to_string()];
    }
    for (i, &sep) in seps.iter().enumerate() {
        let parts: Vec<&str> = text
            .split(sep)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() <= 1 {
            continue;
        }
        let rest = &seps[i + 1..];
        let mut result = Vec::new();
        for part in parts {
            if estimate_token_count(part) > max_tokens {
                result.extend(recursive_split_inner(part, rest, max_tokens));
            } else {
                result.push(part.to_string());
            }
        }
        return result;
    }
    // Hard char fallback — only reached when no separator splits the text at all.
    let char_budget = (max_tokens * 4).max(1);
    text.chars()
        .collect::<Vec<_>>()
        .chunks(char_budget)
        .map(|c| c.iter().collect::<String>())
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// Walk backward through `text` looking for the last `.`, `!`, or `?` followed by
/// whitespace (or EOS) within the final `tolerance * text.len()` bytes.
/// Returns `(head, tail)` split at that boundary; falls back to `(text, "")`.
fn sentence_snap_split(text: &str, tolerance: f32) -> (&str, &str) {
    if tolerance <= 0.0 || text.is_empty() {
        return (text, "");
    }
    let window = ((text.len() as f32) * tolerance) as usize;
    let search_from = text.len().saturating_sub(window.max(1));
    let bytes = text.as_bytes();
    let mut i = text.len().saturating_sub(1);
    loop {
        if i < search_from {
            break;
        }
        if matches!(bytes[i], b'.' | b'!' | b'?') {
            let after = i + 1;
            if after >= bytes.len() || bytes[after].is_ascii_whitespace() {
                // Skip leading whitespace in tail.
                let mut tail_start = after;
                while tail_start < bytes.len() && bytes[tail_start].is_ascii_whitespace() {
                    tail_start += 1;
                }
                if text.is_char_boundary(i + 1) && text.is_char_boundary(tail_start) {
                    return (&text[..i + 1], &text[tail_start..]);
                }
            }
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    (text, "")
}

/// Return the last `overlap_tokens` tokens from `text`, aligned to a word boundary.
fn overlap_tail(text: &str, overlap_tokens: usize) -> String {
    if overlap_tokens == 0 || text.is_empty() {
        return String::new();
    }
    // Walk backward word-by-word until we have enough tokens.
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut tail: Vec<&str> = Vec::new();
    let mut count = 0usize;
    for &word in words.iter().rev() {
        let wt = estimate_token_count(word);
        if count + wt > overlap_tokens && !tail.is_empty() {
            break;
        }
        tail.push(word);
        count += wt;
    }
    tail.reverse();
    tail.join(" ")
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

#[cfg(test)]
mod chunk_ir_tests {
    use super::*;
    use crate::doc_ir::{DocBlock, DocIR};
    use crate::memory::chunker::ChunkerConfig;

    fn chunker() -> FixedChunker {
        FixedChunker::new(ChunkerConfig::default())
    }

    #[test]
    fn breadcrumb_carries_full_heading_path() {
        let mut ir = DocIR::new("t.md", "text/markdown");
        ir.push(DocBlock::header(1, "AMD"));
        ir.push(DocBlock::header(2, "Financial Statements"));
        ir.push(DocBlock::header(3, "Cash Flows"));
        ir.push(DocBlock::text("Operating cash flow was $X."));

        let chunks = chunk_ir(&ir, &chunker());
        let (text, meta) = chunks.last().expect("at least one chunk");
        assert_eq!(
            meta.heading_path,
            vec![
                "AMD".to_string(),
                "Financial Statements".to_string(),
                "Cash Flows".to_string()
            ]
        );
        assert!(
            text.starts_with("AMD > Financial Statements > Cash Flows"),
            "chunk text should start with breadcrumb, got: {text}"
        );
    }

    #[test]
    fn heading_stack_pops_siblings_at_same_level() {
        // H1 A → H1 B should leave path == ["B"], not ["A", "B"].
        let mut ir = DocIR::new("t.md", "text/markdown");
        ir.push(DocBlock::header(1, "A"));
        ir.push(DocBlock::text("under A"));
        ir.push(DocBlock::header(1, "B"));
        ir.push(DocBlock::text("under B"));

        let chunks = chunk_ir(&ir, &chunker());
        let (_, last_meta) = chunks.last().unwrap();
        assert_eq!(last_meta.heading_path, vec!["B".to_string()]);
    }

    #[test]
    fn deeper_headers_extend_then_pop_correctly() {
        // H1 A → H2 a → H2 b: after entering H2 b, path should be ["A", "b"].
        let mut ir = DocIR::new("t.md", "text/markdown");
        ir.push(DocBlock::header(1, "A"));
        ir.push(DocBlock::header(2, "a"));
        ir.push(DocBlock::text("under a"));
        ir.push(DocBlock::header(2, "b"));
        ir.push(DocBlock::text("under b"));

        let chunks = chunk_ir(&ir, &chunker());
        let (_, last_meta) = chunks.last().unwrap();
        assert_eq!(
            last_meta.heading_path,
            vec!["A".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn chunks_in_same_section_share_section_id() {
        let mut ir = DocIR::new("t.md", "text/markdown");
        ir.push(DocBlock::header(1, "Section One"));
        ir.push(DocBlock::text("first paragraph"));
        ir.push(DocBlock::text("second paragraph"));

        let chunks = chunk_ir(&ir, &chunker());
        assert!(!chunks.is_empty());
        let first_section = &chunks[0].1.section_id;
        assert!(!first_section.is_empty());
        for (_, meta) in &chunks {
            assert_eq!(&meta.section_id, first_section);
        }
    }

    #[test]
    fn section_id_changes_across_header_boundary() {
        let mut ir = DocIR::new("t.md", "text/markdown");
        ir.push(DocBlock::header(1, "One"));
        ir.push(DocBlock::text("first section content"));
        ir.push(DocBlock::header(1, "Two"));
        ir.push(DocBlock::text("second section content"));

        let chunks = chunk_ir(&ir, &chunker());
        // At least two distinct section_ids, one per heading group.
        let ids: std::collections::HashSet<_> =
            chunks.iter().map(|(_, m)| m.section_id.clone()).collect();
        assert!(
            ids.len() >= 2,
            "expected at least 2 section_ids across headers, got {ids:?}"
        );
    }

    #[test]
    fn section_id_and_page_change_across_pagebreak_boundary() {
        // Mirrors pdf_paged_ir's actual emission shape for a 3-page PDF:
        // Text(page=1), PageBreak(page=2), Text(page=2), PageBreak(page=3),
        // Text(page=3). Each PageBreak is a strong boundary, so the chunker
        // should create three distinct section_ids and stamp the page on each.
        let mut ir = DocIR::new("t.pdf", "pdf");
        let mut b1 = DocBlock::text("page one content");
        b1.page = Some(1);
        ir.push(b1);
        ir.push(DocBlock::page_break(2));
        let mut b2 = DocBlock::text("page two content");
        b2.page = Some(2);
        ir.push(b2);
        ir.push(DocBlock::page_break(3));
        let mut b3 = DocBlock::text("page three content");
        b3.page = Some(3);
        ir.push(b3);

        let chunks = chunk_ir(&ir, &chunker());
        let ids: std::collections::HashSet<_> =
            chunks.iter().map(|(_, m)| m.section_id.clone()).collect();
        assert!(
            ids.len() >= 3,
            "expected >=3 section_ids across pagebreaks, got {ids:?} from {} chunks",
            chunks.len()
        );
        let pages: std::collections::HashSet<_> =
            chunks.iter().filter_map(|(_, m)| m.page).collect();
        assert!(
            pages.contains(&1) && pages.contains(&2) && pages.contains(&3),
            "expected pages 1,2,3 across chunks, got {pages:?}"
        );
    }

    /// A Col(0) block followed by a Col(1) block on the same page must
    /// produce two distinct chunks (one per column) — otherwise the
    /// resulting chunk would contain text from both columns and the
    /// renewal-fee question stays unanswerable. See docs/rag2.md §4.
    #[test]
    fn same_page_cross_column_transition_is_a_strong_boundary() {
        let mut ir = DocIR::new("t.pdf", "pdf");
        let mut left = DocBlock::text("Renewal fee EUR 200 one-time");
        left.page = Some(1);
        left.metadata
            .insert("column_position".to_string(), "col0".to_string());
        ir.push(left);

        let mut right = DocBlock::text("Late payment 75");
        right.page = Some(1);
        right
            .metadata
            .insert("column_position".to_string(), "col1".to_string());
        ir.push(right);

        let chunks = chunk_ir(&ir, &chunker());
        assert!(
            chunks.len() >= 2,
            "expected at least 2 column-pure chunks, got {} ({:?})",
            chunks.len(),
            chunks.iter().map(|(t, _)| t).collect::<Vec<_>>()
        );

        // Every chunk's column_position_set holds a single column (the
        // cross-column flush prevents Col(0)+Col(1) mixing).
        use crate::doc_ir::ColumnPosition;
        for (text, meta) in &chunks {
            let has_both = meta.column_position_set.contains(&ColumnPosition::Col(0))
                && meta.column_position_set.contains(&ColumnPosition::Col(1));
            assert!(
                !has_both,
                "chunk '{text}' mixes Col(0) and Col(1) content; \
                 column_position_set = {:?}",
                meta.column_position_set
            );
        }
    }

    /// Same logic generalises to 3+ columns: a page with Col(0), Col(1),
    /// Col(2) blocks must produce three column-pure chunks. Without the
    /// generalised crossing check this would only catch Left↔Right.
    #[test]
    fn three_column_page_produces_three_pure_chunks() {
        use crate::doc_ir::ColumnPosition;
        let mut ir = DocIR::new("t.pdf", "pdf");
        for (text, col) in [
            ("left col body", 0u8),
            ("middle col body", 1),
            ("right col body", 2),
        ] {
            let mut b = DocBlock::text(text);
            b.page = Some(1);
            b.metadata
                .insert("column_position".to_string(), format!("col{}", col));
            ir.push(b);
        }
        let chunks = chunk_ir(&ir, &chunker());
        assert!(
            chunks.len() >= 3,
            "expected at least 3 column-pure chunks, got {}",
            chunks.len()
        );
        // No chunk holds two different Col(_) values.
        for (text, meta) in &chunks {
            let cols: Vec<u8> = meta
                .column_position_set
                .iter()
                .filter_map(|c| {
                    if let ColumnPosition::Col(i) = c {
                        Some(*i)
                    } else {
                        None
                    }
                })
                .collect();
            assert!(cols.len() <= 1, "chunk '{text}' mixes columns {:?}", cols);
        }
    }

    #[test]
    fn cross_column_flush_does_not_fire_across_pages() {
        // Col(0) on page 1, then Col(1) on page 2: this should be split by
        // the PageBreak strong boundary, NOT by the column transition
        // (which is gated to same-page). We only emit the PageBreak when
        // the input includes one, so use that shape.
        let mut ir = DocIR::new("t.pdf", "pdf");
        let mut left = DocBlock::text("page one left text");
        left.page = Some(1);
        left.metadata
            .insert("column_position".to_string(), "col0".to_string());
        ir.push(left);

        ir.push(DocBlock::page_break(2));

        let mut right = DocBlock::text("page two right text");
        right.page = Some(2);
        right
            .metadata
            .insert("column_position".to_string(), "col1".to_string());
        ir.push(right);

        let chunks = chunk_ir(&ir, &chunker());
        let ids: std::collections::HashSet<_> =
            chunks.iter().map(|(_, m)| m.section_id.clone()).collect();
        assert!(ids.len() >= 2, "expected per-page sections, got {ids:?}");
    }
}
