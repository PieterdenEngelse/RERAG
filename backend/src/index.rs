use crate::config::ChunkerMode;
use crate::embedder;
use crate::memory::chunker_factory::{create_chunker, Chunker};
use crate::mime_detect::{detect_content_type, ContentType};
use crate::monitoring::{
    record_canon_store, record_extraction_format, record_ocr_attempted, record_ocr_no_pages,
    record_ocr_no_text, record_ocr_ok, record_ocr_unavailable, DetectionInfo,
    EXTRACTION_CHARS_TOTAL, EXTRACTION_OCR_TOTAL, EXTRACTION_TOTAL,
};
use crate::retriever::Retriever;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

pub fn index_all_documents(
    retriever: &mut Retriever,
    folder: &str,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    corpus_slug: &str,
    context_prefix_enabled: bool,
) -> Result<(), String> {
    debug!("index_all_documents: scanning folder='{}'", folder);
    let entries =
        fs::read_dir(folder).map_err(|e| format!("read_dir('{}') failed: {}", folder, e))?;

    // Collect existing doc_ids to skip already-indexed files
    let existing_docs = retriever.get_all_doc_ids().unwrap_or_default();
    let existing_files: std::collections::HashSet<String> = existing_docs
        .iter()
        .filter_map(|id| id.split('#').next().map(|s| s.to_string()))
        .collect();
    debug!(
        "index_all_documents: found {} already-indexed files",
        existing_files.len()
    );

    let mut indexed_count = 0usize;
    let mut skipped_count = 0usize;

    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(e) => {
                warn!("index_all_documents: failed to read directory entry: {}", e);
                continue;
            }
        };
        let path = entry.path();
        let path_str = path.to_string_lossy();
        if path.is_file() {
            // Skip already-indexed files
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            if existing_files.contains(filename) {
                debug!(
                    "index_all_documents: skipping already-indexed file='{}'",
                    filename
                );
                skipped_count += 1;
                continue;
            }

            // Use MIME detection to determine if file is indexable
            let (content_type, detection_info) = match detect_file_type_with_info(&path) {
                Ok(result) => result,
                Err(e) => {
                    debug!(
                        "index_all_documents: failed to detect type for '{}': {}",
                        path_str, e
                    );
                    continue;
                }
            };
            debug!(
                "index_all_documents: considering file='{}' content_type={:?} method={}",
                path_str, content_type, detection_info.detection_method
            );
            // Only index text-based files
            if content_type.is_text_based() {
                // Commit per-file so partial progress survives a restart.
                retriever
                    .begin_batch()
                    .map_err(|e| format!("begin_batch failed: {}", e))?;
                match index_file_with_detection(
                    retriever,
                    &path,
                    chunker_mode,
                    chunker,
                    detection_info,
                    corpus_slug,
                    context_prefix_enabled,
                ) {
                    Ok(chunks) => {
                        if let Err(e) = retriever.commit() {
                            warn!(
                                "index_all_documents: commit failed for '{}': {}",
                                path_str, e
                            );
                        } else {
                            debug!(
                                "indexed file='{}' chunks={} type={:?}",
                                path_str, chunks, content_type
                            );
                            indexed_count += 1;
                        }
                    }
                    Err(e) => {
                        warn!("index_file failed for '{}': {}", path_str, e);
                        // End the open batch even on failure so the writer is not left open.
                        let _ = retriever.commit();
                    }
                }
            } else {
                debug!(
                    "index_all_documents: skipping binary file='{}' type={:?}",
                    path_str, content_type
                );
            }
        } else {
            debug!("index_all_documents: skipping non-file path='{}'", path_str);
        }
    }

    info!(
        "index_all_documents: indexed={} skipped={} (already indexed)",
        indexed_count, skipped_count
    );

    Ok(())
}

/// Async version of index_all_documents using io_uring for 2-3x faster file reads
/// Reads all files in parallel with io_uring, then indexes them
pub async fn index_all_documents_async(
    retriever: &mut Retriever,
    folder: &str,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    _corpus_slug: &str,
    context_prefix_enabled: bool,
) -> Result<usize, String> {
    use crate::perf::io_uring as async_io;

    let io_backend = async_io::backend_name();
    info!(
        "index_all_documents_async: scanning folder='{}' backend={}",
        folder, io_backend
    );

    let entries =
        fs::read_dir(folder).map_err(|e| format!("read_dir('{}') failed: {}", folder, e))?;

    // Phase 1: Collect all indexable file paths
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();

    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    "index_all_documents_async: failed to read directory entry: {}",
                    e
                );
                continue;
            }
        };
        let path = entry.path();
        if path.is_file() {
            // Use MIME detection to determine if file is indexable
            let (content_type, _) = match detect_file_type_with_info(&path) {
                Ok(result) => result,
                Err(_) => continue,
            };

            if content_type.is_text_based() {
                file_paths.push(path);
            }
        }
    }

    let total_files = file_paths.len();
    info!(
        "index_all_documents_async: found {} indexable files",
        total_files
    );

    // Stream each file: read → index → drop. Never accumulate all content in RAM.
    let start = std::time::Instant::now();
    let mut indexed_count = 0usize;

    for path in file_paths {
        let content = extract_text_async(&path).await;
        if let Some(content) = content {
            match index_content_direct(
                retriever,
                &path,
                &content,
                chunker_mode,
                chunker,
                context_prefix_enabled,
            ) {
                Ok(chunks) => {
                    indexed_count += chunks;
                    debug!("indexed file='{}' chunks={}", path.display(), chunks);
                }
                Err(e) => warn!(
                    "index_content_direct failed for '{}': {}",
                    path.display(),
                    e
                ),
            }
        }
    }

    let elapsed = start.elapsed();

    retriever
        .commit()
        .map_err(|e| format!("commit failed: {}", e))?;

    info!(
        "index_all_documents_async: indexed {} chunks from {} files in {:?} via {}",
        indexed_count, total_files, elapsed, io_backend
    );

    Ok(indexed_count)
}

pub fn index_file(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    corpus_slug: &str,
    context_prefix_enabled: bool,
) -> Result<usize, String> {
    // Get detection info for observability
    let (_, detection_info) = detect_file_type_with_info(path)?;
    index_file_with_detection(
        retriever,
        path,
        chunker_mode,
        chunker,
        detection_info,
        corpus_slug,
        context_prefix_enabled,
    )
}

/// Index pre-read content directly (no file I/O)
/// Use this when you've already read the file content (e.g., with io_uring)
pub fn index_content_direct(
    retriever: &mut Retriever,
    path: &Path,
    content: &str,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    context_prefix_enabled: bool,
) -> Result<usize, String> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let chunk_start = std::time::Instant::now();
    let mut embed_file_in = 0usize;
    let mut embed_file_out = 0usize;
    let chunks: Vec<String> = apply_context_prefix(
        chunker.chunk_text(content),
        filename,
        context_prefix_enabled,
    )
    .into_iter()
    .map(|c| {
        let out = crate::normalizer::normalize(&c, crate::normalizer::NormalizeTarget::Embed);
        embed_file_in += c.len();
        embed_file_out += out.len();
        crate::monitoring::record_canon_embed_ingestion(c.len(), out.len());
        out
    })
    .collect();
    let mut index_file_in = 0usize;
    let mut index_file_out = 0usize;
    let index_chunks: Vec<String> = chunks
        .iter()
        .map(|c| {
            let out = crate::normalizer::to_index(c);
            index_file_in += c.len();
            index_file_out += out.len();
            crate::monitoring::record_canon_index_ingestion(c.len(), out.len());
            out
        })
        .collect();
    crate::monitoring::record_canon_file_embed(filename, embed_file_in, embed_file_out, "");
    crate::monitoring::record_canon_file_index(filename, index_file_in, index_file_out, "");
    let chunk_duration = chunk_start.elapsed();
    let mut ok = 0usize;
    let mut total_tokens = 0usize;

    let embed_start = std::time::Instant::now();
    let embeddings = embedder::embed_batch(&chunks);
    let embed_duration = embed_start.elapsed();
    debug!(
        "index_content_direct: embedding completed for '{}' chunks={} duration_ms={}",
        filename,
        chunks.len(),
        embed_duration.as_millis()
    );

    if embeddings.len() != chunks.len() {
        return Err("embedding batch size mismatch".into());
    }

    for (i, vector) in embeddings.into_iter().enumerate() {
        let chunk = &chunks[i];
        let chunk_id = format!("{}#{}", filename, i);

        total_tokens += chunk.split_whitespace().count();

        if let Err(e) = retriever.index_chunk(&chunk_id, &index_chunks[i], vector, None) {
            warn!(
                "index_content_direct: Failed to index chunk {}: {}",
                chunk_id, e
            );
        } else {
            ok += 1;
        }
    }

    info!(
        "index_content_direct: file='{}' mode={:?} chunks={} tokens={} chunk_ms={} embed_ms={}",
        filename,
        chunker_mode,
        ok,
        total_tokens,
        chunk_duration.as_millis(),
        embed_duration.as_millis()
    );

    let mut snap = crate::monitoring::ChunkingStatsSnapshot::new(
        filename,
        chunker_mode,
        ok,
        total_tokens,
        chunk_duration.as_millis() as u64,
        None,
    );
    snap.tokenizer_model = crate::api::get_token_counter().map(|h| h.model_name());
    crate::monitoring::record_chunking_snapshot(snap);

    Ok(ok)
}

/// Index content and return chunks for knowledge graph integration
/// Returns (chunk_count, Vec<(chunk_id, chunk_content)>)
/// All per-document work that does NOT need the retriever lock:
/// chunking, normalisation, and embedding.
pub struct PreparedDoc {
    pub filename: String,
    pub corpus: String,
    /// Embed-normalised chunks (used for HNSW/vector store).
    pub chunks: Vec<String>,
    /// Index-normalised chunks (used for Tantivy full-text).
    pub index_chunks: Vec<String>,
    pub chunk_metas: Vec<crate::doc_ir::ChunkMeta>,
    pub embeddings: Vec<crate::embedder::EmbeddingVector>,
    pub chunker_mode: ChunkerMode,
    pub chunk_duration_ms: u64,
    pub embed_duration_ms: u64,
    pub chunker_stats: Option<crate::memory::chunker_factory::ChunkingStats>,
}

/// Phase 1 of indexing: chunk + embed, no retriever needed.
/// Call this before acquiring the retriever mutex so searches are not blocked
/// during the (slow) ONNX embedding pass.
pub fn prepare_doc(
    path: &Path,
    ir: &crate::doc_ir::DocIR,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    corpus_slug: &str,
    context_prefix_enabled: bool,
) -> PreparedDoc {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    {
        let flat = ir.to_plain_text();
        let norm = crate::normalizer::normalize(&flat, crate::normalizer::NormalizeTarget::Store);
        record_canon_store(&filename, flat.len(), norm.len(), corpus_slug);
    }

    let chunk_start = std::time::Instant::now();
    let mut embed_file_in = 0usize;
    let mut embed_file_out = 0usize;
    let (raw_texts, chunk_metas): (Vec<String>, Vec<crate::doc_ir::ChunkMeta>) =
        crate::memory::chunker_factory::chunk_ir(ir, chunker)
            .into_iter()
            .unzip();
    let chunks: Vec<String> = apply_context_prefix(raw_texts, &filename, context_prefix_enabled)
        .into_iter()
        .map(|c| {
            let out = crate::normalizer::normalize(&c, crate::normalizer::NormalizeTarget::Embed);
            embed_file_in += c.len();
            embed_file_out += out.len();
            crate::monitoring::record_canon_embed_ingestion(c.len(), out.len());
            out
        })
        .collect();
    for c in &chunks {
        crate::db::golden_sample::offer_chunk(c, corpus_slug);
    }
    let mut index_file_in = 0usize;
    let mut index_file_out = 0usize;
    let index_chunks: Vec<String> = chunks
        .iter()
        .map(|c| {
            let out = crate::normalizer::to_index(c);
            index_file_in += c.len();
            index_file_out += out.len();
            crate::monitoring::record_canon_index_ingestion(c.len(), out.len());
            out
        })
        .collect();
    crate::monitoring::record_canon_file_embed(
        &filename,
        embed_file_in,
        embed_file_out,
        corpus_slug,
    );
    crate::monitoring::record_canon_file_index(
        &filename,
        index_file_in,
        index_file_out,
        corpus_slug,
    );
    let chunk_duration = chunk_start.elapsed();

    let embed_start = std::time::Instant::now();
    let embeddings = embedder::embed_batch(&chunks);
    let embed_duration = embed_start.elapsed();
    debug!(
        "prepare_doc: embedded '{}' chunks={} ms={}",
        filename,
        chunks.len(),
        embed_duration.as_millis()
    );

    PreparedDoc {
        chunker_stats: chunker.stats(),
        filename,
        corpus: corpus_slug.to_string(),
        chunks,
        index_chunks,
        chunk_metas,
        embeddings,
        chunker_mode,
        chunk_duration_ms: chunk_duration.as_millis() as u64,
        embed_duration_ms: embed_duration.as_millis() as u64,
    }
}

/// Phase 2 of indexing: write pre-computed chunks+embeddings into the retriever.
/// The caller must hold the retriever lock (begin_batch before, commit after).
pub fn index_prepared_doc(
    retriever: &mut Retriever,
    prepared: PreparedDoc,
) -> Result<(usize, Vec<(String, String)>), String> {
    let PreparedDoc {
        filename,
        corpus,
        chunks,
        index_chunks,
        chunk_metas,
        embeddings,
        chunker_mode,
        chunk_duration_ms,
        embed_duration_ms,
        chunker_stats,
    } = prepared;

    if embeddings.len() != chunks.len() {
        return Err("embedding batch size mismatch".into());
    }

    let mut ok = 0usize;
    let mut total_tokens = 0usize;
    let mut graph_chunks = Vec::new();

    for (i, vector) in embeddings.into_iter().enumerate() {
        let chunk = &chunks[i];
        let chunk_id = format!("{}#{}", filename, i);
        total_tokens += chunk.split_whitespace().count();
        if let Err(e) = retriever.index_chunk(
            &chunk_id,
            &index_chunks[i],
            vector,
            chunk_metas.get(i).cloned(),
        ) {
            warn!("index_prepared_doc: failed chunk {}: {}", chunk_id, e);
        } else {
            ok += 1;
            graph_chunks.push((chunk_id, chunk.clone()));
        }
    }

    info!(
        "index_prepared_doc: file='{}' mode={:?} chunks={} tokens={} chunk_ms={} embed_ms={}",
        filename, chunker_mode, ok, total_tokens, chunk_duration_ms, embed_duration_ms,
    );

    let mut snap = crate::monitoring::ChunkingStatsSnapshot::new(
        &filename,
        chunker_mode,
        ok,
        total_tokens,
        chunk_duration_ms,
        chunker_stats,
    );
    snap.tokenizer_model = crate::api::get_token_counter().map(|h| h.model_name());
    snap.corpus = corpus;
    crate::monitoring::record_chunking_snapshot(snap);

    Ok((ok, graph_chunks))
}

/// Convenience wrapper: chunk + embed + index in one call (holds retriever for the full duration).
/// Prefer `prepare_doc` + `index_prepared_doc` when the retriever is behind a mutex.
pub fn index_content_with_graph(
    retriever: &mut Retriever,
    path: &Path,
    ir: &crate::doc_ir::DocIR,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    corpus_slug: &str,
    context_prefix_enabled: bool,
) -> Result<(usize, Vec<(String, String)>), String> {
    let prepared = prepare_doc(
        path,
        ir,
        chunker_mode,
        chunker,
        corpus_slug,
        context_prefix_enabled,
    );
    index_prepared_doc(retriever, prepared)
}

/// Async version of index_file using io_uring for 2-3x faster file reads
/// Use this for document ingestion to benefit from io_uring on Linux
pub async fn index_file_async(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    corpus_slug: &str,
    context_prefix_enabled: bool,
) -> Result<usize, String> {
    let (_, detection_info) = detect_file_type_with_info(path)?;
    index_file_with_detection_async(
        retriever,
        path,
        chunker_mode,
        chunker,
        detection_info,
        corpus_slug,
        context_prefix_enabled,
    )
    .await
}

async fn index_file_with_detection_async(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    detection_info: DetectionInfo,
    corpus_slug: &str,
    context_prefix_enabled: bool,
) -> Result<usize, String> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    debug!(
        "index_file_async: start file='{}' detected_format={} strategy={} backend={}",
        path.to_string_lossy(),
        detection_info.detected_format,
        detection_info.chosen_strategy,
        crate::perf::io_uring::backend_name()
    );

    // Use async io_uring-backed file read, then produce IR
    let ir = match extract_ir_async(path, corpus_slug).await {
        Some(ir) => ir,
        None => {
            warn!(
                "index_file_async: extract_ir returned None for '{}'",
                filename
            );
            return Err("extract_text failed".into());
        }
    };
    {
        let flat = ir.to_plain_text();
        let norm = crate::normalizer::normalize(&flat, crate::normalizer::NormalizeTarget::Store);
        record_canon_store(filename, flat.len(), norm.len(), corpus_slug);
    }

    let chunk_start = std::time::Instant::now();
    let mut embed_file_in = 0usize;
    let mut embed_file_out = 0usize;
    let (raw_texts, chunk_metas): (Vec<String>, Vec<crate::doc_ir::ChunkMeta>) =
        crate::memory::chunker_factory::chunk_ir(&ir, chunker)
            .into_iter()
            .unzip();
    let prefixed = apply_context_prefix(raw_texts, filename, context_prefix_enabled);
    let chunks: Vec<String> = prefixed
        .into_iter()
        .map(|c| {
            let out = crate::normalizer::normalize(&c, crate::normalizer::NormalizeTarget::Embed);
            embed_file_in += c.len();
            embed_file_out += out.len();
            crate::monitoring::record_canon_embed_ingestion(c.len(), out.len());
            out
        })
        .collect();
    for c in &chunks {
        crate::db::golden_sample::offer_chunk(c, corpus_slug);
    }
    let mut index_file_in = 0usize;
    let mut index_file_out = 0usize;
    let index_chunks: Vec<String> = chunks
        .iter()
        .map(|c| {
            let out = crate::normalizer::to_index(c);
            index_file_in += c.len();
            index_file_out += out.len();
            crate::monitoring::record_canon_index_ingestion(c.len(), out.len());
            out
        })
        .collect();
    crate::monitoring::record_canon_file_embed(
        filename,
        embed_file_in,
        embed_file_out,
        corpus_slug,
    );
    crate::monitoring::record_canon_file_index(
        filename,
        index_file_in,
        index_file_out,
        corpus_slug,
    );
    let chunk_duration = chunk_start.elapsed();
    let mut ok = 0usize;
    let mut total_tokens = 0usize;

    let embed_start = std::time::Instant::now();
    let embeddings = embedder::embed_batch(&chunks);
    let embed_duration = embed_start.elapsed();
    debug!(
        "index_file_async: embedding completed for '{}' chunks={} duration_ms={}",
        filename,
        chunks.len(),
        embed_duration.as_millis()
    );

    if embeddings.len() != chunks.len() {
        return Err("embedding batch size mismatch".into());
    }

    for (i, vector) in embeddings.into_iter().enumerate() {
        let chunk = &chunks[i];
        let chunk_id = format!("{}#{}", filename, i);

        total_tokens += chunk.split_whitespace().count();

        if let Err(e) = retriever.index_chunk(
            &chunk_id,
            &index_chunks[i],
            vector,
            chunk_metas.get(i).cloned(),
        ) {
            warn!(
                "index_file_async: Failed to index chunk {}: {}",
                chunk_id, e
            );
        } else {
            ok += 1;
        }
    }

    info!(
        "index_file_async: file='{}' mode={:?} chunks={} tokens={} duration_ms={} backend={}",
        filename,
        chunker_mode,
        ok,
        total_tokens,
        chunk_duration.as_millis(),
        crate::perf::io_uring::backend_name()
    );

    let mut snap = crate::monitoring::ChunkingStatsSnapshot::with_detection(
        filename,
        chunker_mode,
        ok,
        total_tokens,
        chunk_duration.as_millis() as u64,
        chunker.stats(),
        detection_info,
    );
    snap.tokenizer_model = crate::api::get_token_counter().map(|h| h.model_name());
    snap.corpus = corpus_slug.to_string();
    crate::monitoring::record_chunking_snapshot(snap);

    Ok(ok)
}

fn index_file_with_detection(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    detection_info: DetectionInfo,
    corpus_slug: &str,
    context_prefix_enabled: bool,
) -> Result<usize, String> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    debug!(
        "index_file: start file='{}' detected_format={} strategy={}",
        path.to_string_lossy(),
        detection_info.detected_format,
        detection_info.chosen_strategy
    );

    let ir = match extract_ir(path, corpus_slug) {
        Some(ir) => ir,
        None => {
            warn!("index_file: extract_ir returned None for '{}'", filename);
            return Err("extract_text failed".into());
        }
    };
    // Record Store-normalization metrics on the flattened text (backward compat).
    {
        let flat = ir.to_plain_text();
        let norm = crate::normalizer::normalize(&flat, crate::normalizer::NormalizeTarget::Store);
        record_canon_store(filename, flat.len(), norm.len(), corpus_slug);
    }

    let chunk_start = std::time::Instant::now();
    let mut embed_file_in = 0usize;
    let mut embed_file_out = 0usize;
    let (raw_texts, chunk_metas): (Vec<String>, Vec<crate::doc_ir::ChunkMeta>) =
        crate::memory::chunker_factory::chunk_ir(&ir, chunker)
            .into_iter()
            .unzip();
    let prefixed = apply_context_prefix(raw_texts, filename, context_prefix_enabled);
    let chunks: Vec<String> = prefixed
        .into_iter()
        .map(|c| {
            let out = crate::normalizer::normalize(&c, crate::normalizer::NormalizeTarget::Embed);
            embed_file_in += c.len();
            embed_file_out += out.len();
            crate::monitoring::record_canon_embed_ingestion(c.len(), out.len());
            out
        })
        .collect();
    for c in &chunks {
        crate::db::golden_sample::offer_chunk(c, corpus_slug);
    }
    let mut index_file_in = 0usize;
    let mut index_file_out = 0usize;
    let index_chunks: Vec<String> = chunks
        .iter()
        .map(|c| {
            let out = crate::normalizer::to_index(c);
            index_file_in += c.len();
            index_file_out += out.len();
            crate::monitoring::record_canon_index_ingestion(c.len(), out.len());
            out
        })
        .collect();
    crate::monitoring::record_canon_file_embed(
        filename,
        embed_file_in,
        embed_file_out,
        corpus_slug,
    );
    crate::monitoring::record_canon_file_index(
        filename,
        index_file_in,
        index_file_out,
        corpus_slug,
    );
    let chunk_duration = chunk_start.elapsed();
    let mut ok = 0usize;
    let mut total_tokens = 0usize;

    let embed_start = std::time::Instant::now();
    let embeddings = embedder::embed_batch(&chunks);
    let embed_duration = embed_start.elapsed();
    debug!(
        "index_file: embedding completed for '{}' chunks={} duration_ms={}",
        filename,
        chunks.len(),
        embed_duration.as_millis()
    );

    if embeddings.len() != chunks.len() {
        return Err("embedding batch size mismatch".into());
    }

    for (i, vector) in embeddings.into_iter().enumerate() {
        let chunk = &chunks[i];
        let chunk_id = format!("{}#{}", filename, i);

        total_tokens += chunk.split_whitespace().count();

        if let Err(e) = retriever.index_chunk(
            &chunk_id,
            &index_chunks[i],
            vector,
            chunk_metas.get(i).cloned(),
        ) {
            warn!("index_file: Failed to index chunk {}: {}", chunk_id, e);
        } else {
            ok += 1;
        }
    }

    // Log with detection info for observability
    info!(
        "index_file: file='{}' mode={:?} chunks={} tokens={} duration_ms={} mime={:?} ext={:?} format={} strategy={} method={}",
        filename,
        chunker_mode,
        ok,
        total_tokens,
        chunk_duration.as_millis(),
        detection_info.mime_type,
        detection_info.extension,
        detection_info.detected_format,
        detection_info.chosen_strategy,
        detection_info.detection_method,
    );

    if let Some(stats) = chunker.stats() {
        info!(
            "index_file: file='{}' semantic_threshold={} semantic_flushes={} heading_flushes={} size_flushes={} total_segments={} avg_similarity={:?}",
            filename,
            stats.semantic_similarity_threshold,
            stats.semantic_flushes,
            stats.heading_flushes,
            stats.size_flushes,
            stats.total_segments,
            stats.average_similarity(),
        );
        let mut snap = crate::monitoring::ChunkingStatsSnapshot::with_detection(
            filename,
            chunker_mode,
            ok,
            total_tokens,
            chunk_duration.as_millis() as u64,
            Some(stats),
            detection_info,
        );
        snap.tokenizer_model = crate::api::get_token_counter().map(|h| h.model_name());
        snap.corpus = corpus_slug.to_string();
        crate::monitoring::record_chunking_snapshot(snap);
    } else {
        let mut snap = crate::monitoring::ChunkingStatsSnapshot::with_detection(
            filename,
            chunker_mode,
            ok,
            total_tokens,
            chunk_duration.as_millis() as u64,
            None,
            detection_info,
        );
        snap.tokenizer_model = crate::api::get_token_counter().map(|h| h.model_name());
        snap.corpus = corpus_slug.to_string();
        crate::monitoring::record_chunking_snapshot(snap);
    }
    Ok(ok)
}

/// Detect file type using MIME magic bytes with extension fallback
/// Returns both the ContentType and DetectionInfo for observability
fn detect_file_type_with_info(path: &Path) -> Result<(ContentType, DetectionInfo), String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let filename = path.file_name().and_then(|n| n.to_str());
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());

    // Try magic byte detection first
    let mime_type = infer::get(&bytes).map(|k| k.mime_type().to_string());

    let content_type = detect_content_type(&bytes, filename);

    // Determine detection method
    let detection_method = if mime_type.is_some() {
        "magic_bytes".to_string()
    } else if extension.is_some() {
        "extension".to_string()
    } else {
        "heuristic".to_string()
    };

    // Map content type to strategy
    let (detected_format, chosen_strategy) = match &content_type {
        ContentType::Pdf => ("PDF".to_string(), "character_split".to_string()),
        ContentType::Text => ("Plain Text".to_string(), "paragraph_split".to_string()),
        ContentType::Markdown => ("Markdown".to_string(), "header_aware".to_string()),
        ContentType::Html => ("HTML".to_string(), "tag_aware".to_string()),
        ContentType::Xml => ("XML".to_string(), "tag_aware".to_string()),
        ContentType::Json => ("JSON".to_string(), "structure_aware".to_string()),
        ContentType::Code(_) => ("Source Code".to_string(), "ast_based".to_string()),
        ContentType::Docx => ("Word Document".to_string(), "paragraph_split".to_string()),
        ContentType::Xlsx => ("Excel Spreadsheet".to_string(), "row_split".to_string()),
        ContentType::Csv => ("CSV".to_string(), "row_split".to_string()),
        ContentType::Odt => (
            "OpenDocument Text".to_string(),
            "paragraph_split".to_string(),
        ),
        ContentType::Ods => (
            "OpenDocument Spreadsheet".to_string(),
            "row_split".to_string(),
        ),
        ContentType::Epub => ("EPUB e-book".to_string(), "chapter_split".to_string()),
        ContentType::Pptx => (
            "PowerPoint Presentation".to_string(),
            "slide_split".to_string(),
        ),
        ContentType::Binary => ("Binary".to_string(), "skip".to_string()),
        ContentType::Unknown => ("Unknown".to_string(), "fallback_paragraph".to_string()),
    };

    let detection_info = DetectionInfo {
        mime_type,
        extension,
        detected_format,
        chosen_strategy,
        detection_method,
    };

    Ok((content_type, detection_info))
}

/// Extract text content from a file based on its detected type
#[allow(dead_code)]
fn extract_text(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let filename = path.file_name().and_then(|n| n.to_str());
    let ct = detect_content_type(&bytes, filename);
    extract_text_from_bytes(path, bytes, ct, "")
}

/// Async version of extract_text using io_uring for 2-3x faster file reads
pub async fn extract_text_async(path: &Path) -> Option<String> {
    use crate::perf::io_uring as async_io;

    let bytes = async_io::read_file(path).await.ok()?;
    let filename = path.file_name().and_then(|n| n.to_str());
    let ct = detect_content_type(&bytes, filename);
    extract_text_from_bytes(path, bytes, ct, "")
}

// ─────────────────────────────────────────────────────────────────────────────
// IR extraction — returns a typed DocIR instead of flat text.
// The pipeline: extract_ir() → chunk_ir() → normalize(Embed) → embed → index.
// Formats with known structure (Markdown, HTML, Code) get typed blocks.
// All other formats fall back to extract_text_from_bytes() wrapped in a single
// Text block, so downstream chunking behaves exactly as before.
// ─────────────────────────────────────────────────────────────────────────────

/// Sync IR extractor: reads bytes, tries external extractors first, then built-in.
fn extract_ir(path: &Path, corpus: &str) -> Option<crate::doc_ir::DocIR> {
    let bytes = fs::read(path).ok()?;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let ct = detect_content_type(&bytes, Some(filename));

    // Try registered external extractors (Docling, etc.)
    if let Some(reg) = crate::extractor::global_registry() {
        if reg.has_handler(&ct) {
            if let Some(ir) = reg.extract(bytes, filename, &ct) {
                return Some(ir);
            }
            // Extractor failed — re-read bytes for built-in fallback.
            let bytes = fs::read(path).ok()?;
            let ct2 = detect_content_type(&bytes, Some(filename));
            return extract_ir_from_bytes_typed(path, bytes, ct2, corpus);
        }
    }

    extract_ir_from_bytes_typed(path, bytes, ct, corpus)
}

/// Async IR extractor: io_uring read, then external extractors, then built-in.
pub async fn extract_ir_async(path: &Path, corpus: &str) -> Option<crate::doc_ir::DocIR> {
    use crate::perf::io_uring as async_io;
    let bytes = async_io::read_file(path).await.ok()?;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let ct = detect_content_type(&bytes, Some(&filename));
    let corpus = corpus.to_string();

    // Offload blocking HTTP call to thread pool.
    // Move `bytes` into the extractor rather than cloning — saves one full copy of the
    // file in RAM. If the external extractor fails we re-read from disk for the fallback
    // (one extra disk read is cheaper than holding two copies of a large file).
    if let Some(reg) = crate::extractor::global_registry() {
        if reg.has_handler(&ct) {
            let fname = filename.clone();
            let ct_clone = ct.clone();
            let path_buf = path.to_path_buf();
            let ir_opt = tokio::task::spawn_blocking(move || reg.extract(bytes, &fname, &ct_clone))
                .await
                .ok()
                .flatten();
            if let Some(ir) = ir_opt {
                let chars = ir.to_plain_text().len();
                crate::monitoring::record_extraction_format(
                    ir.extractor_tag(),
                    true,
                    chars,
                    &filename,
                    &path.to_string_lossy(),
                    &corpus,
                );
                crate::monitoring::record_preprocess_passthrough(&filename, &corpus, chars);
                return Some(ir);
            }
            // External extractor failed — re-read for built-in fallback.
            let bytes = async_io::read_file(&path_buf).await.ok()?;
            let ct2 = detect_content_type(&bytes, Some(&filename));
            let corpus2 = corpus.clone();
            return tokio::task::spawn_blocking(move || {
                extract_ir_from_bytes_typed(&path_buf, bytes, ct2, &corpus2)
            })
            .await
            .ok()
            .flatten();
        }
    }

    // Built-in extraction is blocking (subprocess calls for PDF, ONNX for others).
    let path_buf = path.to_path_buf();
    tokio::task::spawn_blocking(move || extract_ir_from_bytes_typed(&path_buf, bytes, ct, &corpus))
        .await
        .ok()
        .flatten()
}

/// Core IR dispatch: typed extraction for structured formats, flat-text fallback for others.
/// Accepts an already-detected `ContentType` so callers don't pay for a second magic-bytes scan.
fn extract_ir_from_bytes_typed(
    path: &Path,
    bytes: Vec<u8>,
    content_type: crate::mime_detect::ContentType,
    corpus: &str,
) -> Option<crate::doc_ir::DocIR> {
    use crate::doc_ir::{DocBlock, DocIR};
    use crate::mime_detect::ContentType;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    match content_type {
        ContentType::Markdown => {
            let raw = detect_and_decode(&bytes)?;
            crate::monitoring::record_preprocess_passthrough(filename, corpus, raw.len());
            let mut ir = extract_ir_from_markdown(&raw, filename);
            ir.tag_extractor("builtin/markdown");
            let chars = ir.to_plain_text().len();
            crate::monitoring::record_extraction_format(
                "builtin/markdown",
                true,
                chars,
                filename,
                &path.to_string_lossy(),
                corpus,
            );
            Some(ir)
        }
        ContentType::Html => {
            let chars_in = detect_and_decode(&bytes)
                .map(|s| s.len())
                .unwrap_or(bytes.len());
            let mut ir = extract_ir_from_html(&bytes, filename);
            ir.tag_extractor("builtin/html");
            let chars_out = ir.to_plain_text().len();
            crate::monitoring::record_preprocess_html(filename, corpus, chars_in, chars_out, 0);
            crate::monitoring::record_extraction_format(
                "builtin/html",
                true,
                chars_out,
                filename,
                &path.to_string_lossy(),
                corpus,
            );
            Some(ir)
        }
        ContentType::Code(ref lang) => {
            let raw = detect_and_decode(&bytes)?;
            crate::monitoring::record_preprocess_passthrough(filename, corpus, raw.len());
            let language = Some(format!("{:?}", lang).to_lowercase());
            let mut ir = DocIR::new(filename, "code");
            ir.push(DocBlock::code(language, raw));
            ir.tag_extractor("builtin/code");
            let chars = ir.to_plain_text().len();
            crate::monitoring::record_extraction_format(
                "builtin/code",
                true,
                chars,
                filename,
                &path.to_string_lossy(),
                corpus,
            );
            Some(ir)
        }
        // Structured extraction in Rust — no sidecar needed.
        // Structure is explicit in the file format (heading styles, XML elements).
        ContentType::Docx => extract_ir_from_docx(&bytes, filename)
            .map(|mut ir| {
                ir.tag_extractor("builtin/docx");
                let chars = ir.to_plain_text().len();
                crate::monitoring::record_preprocess_passthrough(filename, corpus, chars);
                crate::monitoring::record_extraction_format(
                    "builtin/docx",
                    true,
                    chars,
                    filename,
                    &path.to_string_lossy(),
                    corpus,
                );
                ir
            })
            .or_else(|| flat_text_ir(path, bytes, ContentType::Docx, "builtin/docx", corpus)),
        ContentType::Odt => extract_ir_from_docx(&bytes, filename)
            .map(|mut ir| {
                ir.tag_extractor("builtin/odt");
                let chars = ir.to_plain_text().len();
                crate::monitoring::record_preprocess_passthrough(filename, corpus, chars);
                crate::monitoring::record_extraction_format(
                    "builtin/odt",
                    true,
                    chars,
                    filename,
                    &path.to_string_lossy(),
                    corpus,
                );
                ir
            })
            .or_else(|| flat_text_ir(path, bytes, ContentType::Odt, "builtin/odt", corpus)),
        ContentType::Epub => extract_ir_from_epub(&bytes, filename)
            .map(|mut ir| {
                ir.tag_extractor("builtin/epub");
                let chars = ir.to_plain_text().len();
                crate::monitoring::record_preprocess_passthrough(filename, corpus, chars);
                crate::monitoring::record_extraction_format(
                    "builtin/epub",
                    true,
                    chars,
                    filename,
                    &path.to_string_lossy(),
                    corpus,
                );
                ir
            })
            .or_else(|| flat_text_ir(path, bytes, ContentType::Epub, "builtin/epub", corpus)),
        ContentType::Pptx => extract_ir_from_pptx(&bytes, filename)
            .map(|mut ir| {
                ir.tag_extractor("builtin/pptx");
                let chars = ir.to_plain_text().len();
                crate::monitoring::record_preprocess_passthrough(filename, corpus, chars);
                crate::monitoring::record_extraction_format(
                    "builtin/pptx",
                    true,
                    chars,
                    filename,
                    &path.to_string_lossy(),
                    corpus,
                );
                ir
            })
            .or_else(|| flat_text_ir(path, bytes, ContentType::Pptx, "builtin/pptx", corpus)),
        ContentType::Pdf => flat_text_ir(path, bytes, ContentType::Pdf, "builtin/pdf", corpus),
        ContentType::Xlsx => flat_text_ir(
            path,
            bytes,
            ContentType::Xlsx,
            "builtin/spreadsheet",
            corpus,
        ),
        ContentType::Ods => {
            flat_text_ir(path, bytes, ContentType::Ods, "builtin/spreadsheet", corpus)
        }
        ContentType::Csv => {
            flat_text_ir(path, bytes, ContentType::Csv, "builtin/spreadsheet", corpus)
        }
        ContentType::Xml => flat_text_ir(path, bytes, ContentType::Xml, "builtin/text", corpus),
        ContentType::Json => flat_text_ir(path, bytes, ContentType::Json, "builtin/text", corpus),
        ct => flat_text_ir(path, bytes, ct, "builtin/text", corpus),
    }
}

/// Fallback: run the existing flat-text extractor and wrap result in a single Text block.
fn flat_text_ir(
    path: &Path,
    bytes: Vec<u8>,
    content_type: ContentType,
    format: &str,
    corpus: &str,
) -> Option<crate::doc_ir::DocIR> {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let text = extract_text_from_bytes(path, bytes, content_type, corpus)?;
    let mut ir = crate::doc_ir::DocIR::new(filename, "text");
    ir.push(crate::doc_ir::DocBlock::text(text));
    ir.tag_extractor(format);
    Some(ir)
}

/// Parse Markdown text into typed DocIR blocks.
/// Handles: ATX headers (# through ######), fenced code blocks (``` and ~~~),
/// pipe-delimited tables, and plain text paragraphs.
fn extract_ir_from_markdown(text: &str, source: &str) -> crate::doc_ir::DocIR {
    use crate::doc_ir::{DocBlock, DocIR};

    let mut ir = DocIR::new(source, "markdown");
    let mut pending = String::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        // Fenced code block
        let fence = if line.starts_with("```") {
            Some("```")
        } else if line.starts_with("~~~") {
            Some("~~~")
        } else {
            None
        };
        if let Some(fence_str) = fence {
            ir_push_text(&mut ir, &mut pending);
            let lang_str = line.trim_start_matches(fence_str).trim();
            let lang = if lang_str.is_empty() {
                None
            } else {
                Some(lang_str.to_string())
            };
            let mut code = String::new();
            for code_line in lines.by_ref() {
                if code_line.starts_with(fence_str) {
                    break;
                }
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(code_line);
            }
            ir.push(DocBlock::code(lang, code));
            continue;
        }

        // ATX header (1–6 hashes followed by a space)
        let hash_count = line.len() - line.trim_start_matches('#').len();
        if (1..=6).contains(&hash_count) && line.chars().nth(hash_count) == Some(' ') {
            let content = line[hash_count + 1..].trim();
            ir_push_text(&mut ir, &mut pending);
            if !content.is_empty() {
                ir.push(DocBlock::header(hash_count as u8, content));
            }
            continue;
        }

        // Pipe-delimited table — collect all contiguous table rows
        if line.trim_start().starts_with('|') {
            ir_push_text(&mut ir, &mut pending);
            let mut table_lines: Vec<String> = vec![line.to_string()];
            while lines
                .peek()
                .is_some_and(|l| l.trim_start().starts_with('|'))
            {
                table_lines.push(lines.next().unwrap().to_string());
            }
            // Strip GFM separator lines (|---|---|)
            let content_rows: Vec<&str> = table_lines
                .iter()
                .filter(|l| !l.replace(['|', '-', ':', ' '], "").trim().is_empty())
                .map(String::as_str)
                .collect();
            if !content_rows.is_empty() {
                let rows = content_rows.len();
                let cols = content_rows[0]
                    .split('|')
                    .filter(|s| !s.trim().is_empty())
                    .count();
                ir.push(DocBlock::table(rows, cols, content_rows.join("\n")));
            }
            continue;
        }

        // Plain text — accumulate
        if !pending.is_empty() {
            pending.push('\n');
        }
        pending.push_str(line);
    }
    ir_push_text(&mut ir, &mut pending);
    ir
}

/// Parse HTML bytes into typed DocIR blocks.
/// Extracts h1–h6 headers, pre/code blocks, and table content as typed blocks;
/// everything else becomes plain Text.
fn extract_ir_from_html(bytes: &[u8], source: &str) -> crate::doc_ir::DocIR {
    use crate::doc_ir::{DocBlock, DocIR};

    let raw = match detect_and_decode(bytes) {
        Some(s) => s,
        None => return DocIR::new(source, "html"),
    };
    let cleaned = remove_html_blocks(&raw, &["script", "style", "head"]);

    let mut ir = DocIR::new(source, "html");
    let mut pending_text = String::new();

    // Context tracks what structured block we are currently inside.
    #[derive(PartialEq)]
    enum Ctx {
        Header(u8),
        Code,
        Table,
    }
    let mut ctx: Option<Ctx> = None;
    let mut ctx_buf = String::new();
    let mut in_tag = false;
    let mut tag_buf = String::new();
    let mut depth: usize = 0; // nesting depth inside current ctx block

    for ch in cleaned.chars() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }
        if ch == '>' && in_tag {
            in_tag = false;
            let raw_tag = tag_buf.trim();
            let is_close = raw_tag.starts_with('/');
            let tag_name = raw_tag
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace())
                .next()
                .unwrap_or("")
                .to_lowercase();
            let tag_name = tag_name.trim_end_matches('/'); // self-closing

            match tag_name {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = tag_name[1..].parse::<u8>().unwrap_or(1);
                    if !is_close && ctx.is_none() {
                        ir_push_html_text(&mut ir, &mut pending_text);
                        ctx = Some(Ctx::Header(level));
                        ctx_buf.clear();
                        depth = 0;
                    } else if is_close {
                        if let Some(Ctx::Header(lvl)) = ctx.take() {
                            let t = ctx_buf.split_whitespace().collect::<Vec<_>>().join(" ");
                            if !t.is_empty() {
                                ir.push(DocBlock::header(lvl, t));
                            }
                            ctx_buf.clear();
                        }
                    }
                }
                "pre" | "code" => {
                    if !is_close && ctx.is_none() {
                        ir_push_html_text(&mut ir, &mut pending_text);
                        ctx = Some(Ctx::Code);
                        ctx_buf.clear();
                        depth = 0;
                    } else if is_close {
                        if let Some(Ctx::Code) = ctx {
                            if depth == 0 {
                                ctx = None;
                                let t = ctx_buf.trim().to_string();
                                if !t.is_empty() {
                                    ir.push(DocBlock::code(None, t));
                                }
                                ctx_buf.clear();
                            } else {
                                depth -= 1;
                            }
                        }
                    }
                }
                "table" => {
                    if !is_close && ctx.is_none() {
                        ir_push_html_text(&mut ir, &mut pending_text);
                        ctx = Some(Ctx::Table);
                        ctx_buf.clear();
                        depth = 0;
                    } else if is_close {
                        if let Some(Ctx::Table) = ctx {
                            if depth == 0 {
                                ctx = None;
                                let t = ctx_buf.split_whitespace().collect::<Vec<_>>().join(" ");
                                if !t.is_empty() {
                                    ir.push(DocBlock::table(0, 0, t));
                                }
                                ctx_buf.clear();
                            } else {
                                depth -= 1;
                            }
                        }
                    } else if !is_close && ctx.is_some() {
                        depth += 1;
                    }
                }
                "p" | "div" | "section" | "article" => {
                    if is_close && ctx.is_none() && !pending_text.trim().is_empty() {
                        pending_text.push('\n');
                    }
                }
                _ => {}
            }
            tag_buf.clear();
            continue;
        }
        if in_tag {
            tag_buf.push(ch);
            continue;
        }
        if ctx.is_some() {
            ctx_buf.push(ch);
        } else {
            pending_text.push(ch);
        }
    }

    ir_push_html_text(&mut ir, &mut pending_text);
    ir
}

/// Flush accumulated plain text as a DocBlock::text, then clear the buffer.
fn ir_push_text(ir: &mut crate::doc_ir::DocIR, pending: &mut String) {
    let t = pending.trim().to_string();
    if !t.is_empty() {
        ir.push(crate::doc_ir::DocBlock::text(t));
    }
    pending.clear();
}

/// Flush HTML plain-text (collapse whitespace, decode entities).
fn ir_push_html_text(ir: &mut crate::doc_ir::DocIR, pending: &mut String) {
    let decoded = decode_html_entities(pending);
    let t = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    if !t.is_empty() {
        ir.push(crate::doc_ir::DocBlock::text(t));
    }
    pending.clear();
}

// ── DOCX IR extractor ─────────────────────────────────────────────────────────
//
// Parses word/document.xml using quick-xml to extract typed blocks:
//   w:pStyle Heading1–6 → Header   (always starts a new chunk)
//   w:tbl               → Table    (atomic, never split)
//   everything else     → Text     (accumulated for the chunker)

fn extract_ir_from_docx(bytes: &[u8], source: &str) -> Option<crate::doc_ir::DocIR> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut entry = archive.by_name("word/document.xml").ok()?;
    let mut xml = String::new();
    entry.read_to_string(&mut xml).ok()?;

    let mut ir = crate::doc_ir::DocIR::new(source, "docx");
    parse_docx_xml(&xml, &mut ir);
    Some(ir)
}

fn parse_docx_xml(xml: &str, ir: &mut crate::doc_ir::DocIR) {
    use crate::doc_ir::DocBlock;
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(xml);

    let mut tbl_depth: usize = 0; // nesting depth inside w:tbl (tables can nest)
    let mut in_para = false; // currently inside w:p
    let mut in_ppr = false; // currently inside w:pPr
    let mut in_run = false; // currently inside w:r
    let mut in_t = false; // currently inside w:t
    let mut para_style: Option<String> = None;
    let mut para_buf = String::new();
    let mut table_text = String::new();
    let mut table_rows = 0usize;
    let mut pending = String::new(); // accumulated normal-paragraph text

    loop {
        match reader.read_event() {
            // Start tags: open structural elements and set state flags.
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"tbl" => {
                        if tbl_depth == 0 {
                            docx_flush(ir, &mut pending);
                            table_text.clear();
                            table_rows = 0;
                        }
                        tbl_depth += 1;
                    }
                    b"tr" if tbl_depth > 0 => table_rows += 1,
                    b"tc" if tbl_depth > 0 => {
                        if !table_text.is_empty()
                            && !table_text.ends_with('\n')
                            && !table_text.ends_with('\t')
                        {
                            table_text.push('\t');
                        }
                    }
                    b"p" if !in_para => {
                        in_para = true;
                        in_ppr = false;
                        in_run = false;
                        in_t = false;
                        para_style = None;
                        para_buf.clear();
                    }
                    b"pPr" if in_para => in_ppr = true,
                    b"r" if in_para => in_run = true,
                    b"t" if in_run => in_t = true,
                    _ => {}
                }
            }
            // Empty (self-closing) tags: only capture attributes, never set open flags.
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"pStyle" if in_ppr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                if let Ok(v) = attr.unescape_value() {
                                    para_style = Some(v.into_owned());
                                }
                            }
                        }
                    }
                    b"br" | b"tab" if in_run => para_buf.push(' '),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"tbl" if tbl_depth > 0 => {
                        tbl_depth -= 1;
                        if tbl_depth == 0 {
                            let text = table_text.trim().to_string();
                            if !text.is_empty() {
                                let rows = table_rows;
                                let cols = text
                                    .lines()
                                    .map(|l| l.split('\t').count())
                                    .max()
                                    .unwrap_or(1);
                                ir.push(DocBlock::table(rows, cols, text));
                            }
                            table_text.clear();
                            table_rows = 0;
                        }
                    }
                    b"tr" if tbl_depth > 0 => {
                        if !table_text.ends_with('\n') {
                            table_text.push('\n');
                        }
                    }
                    b"p" if in_para => {
                        let text = para_buf.trim().to_string();
                        if !text.is_empty() {
                            if tbl_depth > 0 {
                                if !table_text.is_empty()
                                    && !table_text.ends_with('\t')
                                    && !table_text.ends_with('\n')
                                {
                                    table_text.push(' ');
                                }
                                table_text.push_str(&text);
                            } else if let Some(level) = docx_heading_level(&para_style) {
                                docx_flush(ir, &mut pending);
                                ir.push(DocBlock::header(level, text));
                            } else {
                                if !pending.is_empty() {
                                    pending.push('\n');
                                }
                                pending.push_str(&text);
                            }
                        }
                        in_para = false;
                        in_ppr = false;
                        in_run = false;
                        in_t = false;
                        para_buf.clear();
                        para_style = None;
                    }
                    b"pPr" => in_ppr = false,
                    b"r" => {
                        in_run = false;
                        in_t = false;
                    }
                    b"t" => in_t = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t && in_para {
                    if let Ok(text) = e.decode() {
                        para_buf.push_str(&text);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    docx_flush(ir, &mut pending);
}

fn docx_heading_level(style: &Option<String>) -> Option<u8> {
    let s = style.as_deref()?;
    let lower = s.to_lowercase();
    if let Some(rest) = lower.strip_prefix("heading") {
        return rest
            .trim_matches(|c: char| !c.is_ascii_digit())
            .parse::<u8>()
            .ok()
            .filter(|&l| (1..=6).contains(&l));
    }
    match lower.as_str() {
        "title" => Some(1),
        "subtitle" => Some(2),
        _ => None,
    }
}

fn docx_flush(ir: &mut crate::doc_ir::DocIR, pending: &mut String) {
    let t = pending.trim().to_string();
    if !t.is_empty() {
        ir.push(crate::doc_ir::DocBlock::text(t));
    }
    pending.clear();
}

// ── EPUB IR extractor ─────────────────────────────────────────────────────────
//
// EPUBs are ZIP archives of XHTML spine items.  Re-use the HTML IR extractor
// per item and merge all blocks into a single IR — so headings, code blocks,
// and tables in an e-book chapter land as typed blocks, not flat text.

fn extract_ir_from_epub(bytes: &[u8], source: &str) -> Option<crate::doc_ir::DocIR> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut ir = crate::doc_ir::DocIR::new(source, "epub");

    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .filter(|n| n.ends_with(".xhtml") || n.ends_with(".html") || n.ends_with(".htm"))
        .collect();

    for name in &names {
        if let Ok(mut entry) = archive.by_name(name) {
            let mut buf: Vec<u8> = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() {
                let sub = extract_ir_from_html(&buf, name);
                for block in sub.blocks {
                    ir.blocks.push(block);
                }
            }
        }
    }

    if ir.blocks.is_empty() {
        None
    } else {
        Some(ir)
    }
}

// ── PPTX IR extractor ────────────────────────────────────────────────────────
//
// Each slide's title shape (ph type="title") becomes a Header; body text and
// all other shapes become Text.  Slide numbers provide natural section order.

fn extract_ir_from_pptx(bytes: &[u8], source: &str) -> Option<crate::doc_ir::DocIR> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut ir = crate::doc_ir::DocIR::new(source, "pptx");

    let mut slide_names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .collect();
    // Sort slide1.xml < slide2.xml … (lexicographic is correct here)
    slide_names.sort_by(|a, b| {
        let num = |s: &str| -> usize {
            s.trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .parse()
                .unwrap_or(0)
        };
        num(a).cmp(&num(b))
    });

    for name in &slide_names {
        if let Ok(mut entry) = archive.by_name(name) {
            let mut xml = String::new();
            if entry.read_to_string(&mut xml).is_ok() {
                parse_pptx_slide(&xml, &mut ir);
            }
        }
    }

    if ir.blocks.is_empty() {
        None
    } else {
        Some(ir)
    }
}

fn parse_pptx_slide(xml: &str, ir: &mut crate::doc_ir::DocIR) {
    use crate::doc_ir::DocBlock;
    use quick_xml::{events::Event, Reader};

    // Two-pass approach: collect shape ph_type + text content per shape,
    // then emit blocks.  A shape is a <p:sp> element.
    let mut reader = Reader::from_str(xml);

    let mut in_sp = false; // inside a shape
    let mut ph_type: Option<String> = None; // placeholder type for current shape
    let mut in_nvsppr = false; // inside p:nvSpPr (for ph detection)
    let mut in_txbody = false; // inside p:txBody
    let mut in_para = false; // inside a:p
    let mut in_run = false; // inside a:r
    let mut in_t = false; // inside a:t
    let mut shape_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"sp" => {
                        in_sp = true;
                        ph_type = None;
                        shape_buf.clear();
                        in_nvsppr = false;
                        in_txbody = false;
                    }
                    b"nvSpPr" if in_sp => in_nvsppr = true,
                    b"ph" if in_nvsppr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"type" {
                                if let Ok(v) = attr.unescape_value() {
                                    ph_type = Some(v.into_owned());
                                }
                            }
                        }
                        // ph with no type attribute = body placeholder
                        if ph_type.is_none() {
                            ph_type = Some("body".to_string());
                        }
                    }
                    b"txBody" if in_sp => in_txbody = true,
                    b"p" if in_txbody => {
                        in_para = true;
                        in_run = false;
                        in_t = false;
                    }
                    b"r" if in_para => in_run = true,
                    b"t" if in_run => in_t = true,
                    b"br" if in_para => shape_buf.push('\n'),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"sp" if in_sp => {
                        let text = shape_buf.trim().to_string();
                        if !text.is_empty() {
                            let is_title = ph_type
                                .as_deref()
                                .map(|t| matches!(t, "title" | "ctrTitle" | "subTitle"))
                                .unwrap_or(false);
                            if is_title {
                                ir.push(DocBlock::header(1, text));
                            } else {
                                ir.push(DocBlock::text(text));
                            }
                        }
                        in_sp = false;
                        in_nvsppr = false;
                        in_txbody = false;
                        in_para = false;
                        in_run = false;
                        in_t = false;
                        shape_buf.clear();
                    }
                    b"nvSpPr" => in_nvsppr = false,
                    b"txBody" => in_txbody = false,
                    b"p" if in_para => {
                        if !shape_buf.ends_with('\n') {
                            shape_buf.push('\n');
                        }
                        in_para = false;
                        in_run = false;
                        in_t = false;
                    }
                    b"r" => {
                        in_run = false;
                        in_t = false;
                    }
                    b"t" => in_t = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    if let Ok(text) = e.decode() {
                        shape_buf.push_str(&text);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/// Strip HTML/XML tags from text, returning (clean_text, tag_count).
pub fn strip_html_tags(text: &str) -> (String, usize) {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    let mut count = 0usize;
    for ch in text.chars() {
        match ch {
            '<' => {
                in_tag = true;
                count += 1;
            }
            '>' => {
                in_tag = false;
                result.push(' ');
            }
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    (
        result.split_whitespace().collect::<Vec<_>>().join(" "),
        count,
    )
}

/// Common text extraction logic from bytes.
/// `content_type` must already be detected by the caller — avoids a second magic-bytes scan.
fn extract_text_from_bytes(
    path: &Path,
    bytes: Vec<u8>,
    content_type: ContentType,
    corpus: &str,
) -> Option<String> {
    let format_label = match &content_type {
        ContentType::Pdf => "pdf",
        ContentType::Docx => "docx",
        ContentType::Odt => "odt",
        ContentType::Xlsx => "xlsx",
        ContentType::Ods => "ods",
        ContentType::Csv => "csv",
        ContentType::Epub => "epub",
        ContentType::Pptx => "pptx",
        ContentType::Text => "text",
        ContentType::Markdown => "markdown",
        ContentType::Html => "html",
        ContentType::Json => "json",
        ContentType::Xml => "xml",
        ContentType::Code(_) => "code",
        ContentType::Binary => "binary",
        ContentType::Unknown => "unknown",
    };

    let needs_html_clean = matches!(content_type, ContentType::Html);
    let needs_unicode_clean = matches!(
        content_type,
        ContentType::Pdf
            | ContentType::Docx
            | ContentType::Odt
            | ContentType::Epub
            | ContentType::Pptx
            | ContentType::Html
    );

    let result = match content_type {
        ContentType::Pdf => {
            // PDF extraction only uses path — drop the bytes clone immediately to free memory.
            drop(bytes);
            debug!("extract_text: PDF detected, trying pdftotext → OCR");
            let file_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
            // pdf-extract is removed: it loads the full PDF DOM in-process and OOMs on
            // files >~10 MB with complex fonts or embedded images. pdftotext handles
            // text-layer PDFs better anyway; OCR covers image-only ones.
            let text = extract_text_from_pdf_pdftotext(path)
                .or_else(|| extract_text_from_pdf_ocr(path, file_bytes));
            // dedupe_pdf_noise can reduce header/footer-heavy PDFs to ""; treat that as None
            // so the file records as "empty" rather than "ok" with 0 indexable chars.
            text.map(dedupe_pdf_noise).filter(|t| !t.trim().is_empty())
        }
        ContentType::Html => {
            debug!("extract_text: HTML detected, using smart extractor");
            extract_text_from_html(&bytes)
        }
        ContentType::Docx => {
            debug!("extract_text: DOCX detected, extracting word/document.xml");
            extract_text_from_zip_xml(&bytes, "word/document.xml")
        }
        ContentType::Odt => {
            debug!("extract_text: ODT detected, extracting content.xml");
            extract_text_from_zip_xml(&bytes, "content.xml")
        }
        ContentType::Xlsx => {
            debug!("extract_text: XLSX detected, using calamine");
            extract_text_from_spreadsheet(&bytes)
        }
        ContentType::Ods => {
            debug!("extract_text: ODS detected, using calamine");
            extract_text_from_spreadsheet(&bytes)
        }
        ContentType::Csv => {
            // CSV is plain UTF-8 text
            String::from_utf8(bytes.clone())
                .ok()
                .or_else(|| Some(String::from_utf8_lossy(&bytes).to_string()))
        }
        ContentType::Epub => {
            debug!("extract_text: EPUB detected, extracting XHTML spine items");
            extract_text_from_epub(&bytes)
        }
        ContentType::Pptx => {
            debug!("extract_text: PPTX detected, extracting slide XML");
            extract_text_from_pptx(&bytes)
        }
        ContentType::Binary => {
            debug!("extract_text: Binary file detected, skipping");
            None
        }
        _ => detect_and_decode(&bytes),
    }
    .map(|text| {
        let fname = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let preprocessed =
            apply_text_preprocessing(text, needs_html_clean, needs_unicode_clean, corpus, fname);
        let normalized =
            crate::normalizer::normalize(&preprocessed, crate::normalizer::NormalizeTarget::Store);
        record_canon_store(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            preprocessed.len(),
            normalized.len(),
            corpus,
        );
        normalized
    });

    // Record extraction metrics
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let file_path = abs.to_string_lossy();
    match &result {
        Some(text) => {
            let chars = text.len();
            record_extraction_format(format_label, true, chars, file_name, &file_path, corpus);
            let _ = EXTRACTION_TOTAL
                .get_metric_with_label_values(&[format_label, "ok"])
                .map(|c| c.inc());
            let _ = EXTRACTION_CHARS_TOTAL
                .get_metric_with_label_values(&[format_label])
                .map(|c| c.inc_by(chars as u64));
        }
        None => {
            record_extraction_format(format_label, false, 0, file_name, &file_path, corpus);
            let _ = EXTRACTION_TOTAL
                .get_metric_with_label_values(&[format_label, "empty"])
                .map(|c| c.inc());
        }
    }

    result
}

pub fn apply_context_prefix(chunks: Vec<String>, filename: &str, enabled: bool) -> Vec<String> {
    if enabled {
        chunks
            .into_iter()
            .map(|c| format!("[Source: {}] {}", filename, c))
            .collect()
    } else {
        chunks
    }
}

/// Normalise typography-heavy Unicode to plain ASCII so tokenisers see consistent tokens.
/// Handles: curly quotes, em/en dashes, non-breaking hyphen, ellipsis, and PDF ligatures.
fn clean_unicode_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201C}' | '\u{201D}' => out.push('"'),
            '\u{2013}' | '\u{2014}' => out.push_str(" - "),
            '\u{2011}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB06}' => out.push_str("st"),
            _ => out.push(ch),
        }
    }
    out
}

/// Apply format-driven preprocessing: HTML tag stripping and Unicode normalisation.
/// Callers compute the two booleans from the detected ContentType before calling this.
pub fn apply_text_preprocessing(
    text: String,
    clean_html: bool,
    clean_unicode: bool,
    corpus: &str,
    filename: &str,
) -> String {
    let chars_in = text.len();
    let mut result = text;
    if clean_html {
        let (cleaned, count) = strip_html_tags(&result);
        if count > 0 {
            debug!("apply_text_preprocessing: stripped {} HTML tags", count);
        }
        crate::monitoring::record_preprocess_html(
            filename,
            corpus,
            chars_in,
            cleaned.len(),
            count as u64,
        );
        result = cleaned;
    } else if clean_unicode {
        let cleaned = clean_unicode_text(&result);
        crate::monitoring::record_preprocess_unicode(filename, corpus, chars_in, cleaned.len());
        result = cleaned;
    } else {
        crate::monitoring::record_preprocess_passthrough(filename, corpus, chars_in);
    }
    result
}

/// Extract text from a ZIP-based XML format (DOCX uses word/document.xml, ODT uses content.xml).
/// Strips all XML tags, leaving only text content.
fn extract_text_from_zip_xml(bytes: &[u8], entry_path: &str) -> Option<String> {
    use std::io::Read;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut entry = archive.by_name(entry_path).ok()?;
    let mut xml_content = String::new();
    entry.read_to_string(&mut xml_content).ok()?;
    Some(strip_xml_tags(&xml_content))
}

/// Strip XML/HTML tags from a string, returning just the text nodes joined by spaces.
fn strip_xml_tags(xml: &str) -> String {
    let mut result = String::with_capacity(xml.len() / 2);
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                result.push(' ');
            }
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    // Collapse whitespace
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract text from XLSX or ODS using calamine.
/// Concatenates all sheets: each row becomes a tab-separated line.
fn extract_text_from_spreadsheet(bytes: &[u8]) -> Option<String> {
    use calamine::{open_workbook_auto_from_rs, Data, Reader};
    let cursor = std::io::Cursor::new(bytes);
    let mut workbook = open_workbook_auto_from_rs(cursor).ok()?;
    let sheet_names = workbook.sheet_names().to_vec();
    let mut text = String::new();
    for sheet_name in &sheet_names {
        if sheet_names.len() > 1 {
            text.push_str(&format!("[Sheet: {}]\n", sheet_name));
        }
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            for row in range.rows() {
                let cols: Vec<String> = row
                    .iter()
                    .map(|cell| match cell {
                        Data::Empty => String::new(),
                        Data::String(s) => s.clone(),
                        Data::Float(f) => f.to_string(),
                        Data::Int(i) => i.to_string(),
                        Data::Bool(b) => b.to_string(),
                        Data::Error(e) => format!("{:?}", e),
                        Data::DateTime(dt) => dt.to_string(),
                        Data::DateTimeIso(s) => s.clone(),
                        Data::DurationIso(s) => s.clone(),
                    })
                    .collect();
                let line = cols.join("\t");
                if !line.trim().is_empty() {
                    text.push_str(&line);
                    text.push('\n');
                }
            }
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extract text from an EPUB file.
/// EPUBs are ZIP archives; text lives in XHTML/HTML spine items anywhere in the archive.
fn extract_text_from_epub(bytes: &[u8]) -> Option<String> {
    use std::io::Read;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut text = String::new();
    // Collect names first to avoid borrow issues
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .collect();
    for name in &names {
        if name.ends_with(".xhtml") || name.ends_with(".html") || name.ends_with(".htm") {
            if let Ok(mut entry) = archive.by_name(name) {
                let mut buf = String::new();
                if entry.read_to_string(&mut buf).is_ok() {
                    let extracted = strip_xml_tags(&buf);
                    if !extracted.is_empty() {
                        text.push_str(&extracted);
                        text.push('\n');
                    }
                }
            }
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extract text from a PPTX file.
/// PPTXs are ZIP archives; slide text lives in `ppt/slides/slide*.xml`.
fn extract_text_from_pptx(bytes: &[u8]) -> Option<String> {
    use std::io::Read;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut text = String::new();
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .collect();
    for name in &names {
        if let Ok(mut entry) = archive.by_name(name) {
            let mut buf = String::new();
            if entry.read_to_string(&mut buf).is_ok() {
                let extracted = strip_xml_tags(&buf);
                if !extracted.is_empty() {
                    text.push_str(&extracted);
                    text.push('\n');
                }
            }
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// OCR fallback for scanned/image-only PDFs.
///
/// Pipeline: `pdftoppm` (poppler-utils) renders each page to a PPM image,
/// then `tesseract` OCRs each image and concatenates the results.
///
/// Gracefully returns `None` if either tool is absent, with a one-time warning.
fn extract_text_from_pdf_ocr(path: &Path, file_bytes: u64) -> Option<String> {
    use std::process::Command;

    // Skip OCR for large files — rendering many pages as PPM eats ~6 MB/page.
    const OCR_MAX_BYTES: u64 = 25 * 1024 * 1024; // 25 MB
    if file_bytes > OCR_MAX_BYTES {
        warn!(
            "extract_text: skipping OCR for large file ({:.1} MB > 25 MB limit)",
            file_bytes as f64 / 1_048_576.0
        );
        record_ocr_unavailable();
        let _ = EXTRACTION_OCR_TOTAL
            .get_metric_with_label_values(&["skipped_large"])
            .map(|c| c.inc());
        return None;
    }

    // Check both tools are on PATH before doing any work
    let has_pdftoppm = Command::new("pdftoppm").arg("-v").output().is_ok();
    let has_tesseract = Command::new("tesseract").arg("--version").output().is_ok();

    if !has_pdftoppm || !has_tesseract {
        warn!(
            "extract_text: OCR fallback unavailable (pdftoppm={}, tesseract={}). \
            Install with: sudo apt install poppler-utils tesseract-ocr tesseract-ocr-eng",
            has_pdftoppm, has_tesseract
        );
        record_ocr_unavailable();
        let _ = EXTRACTION_OCR_TOTAL
            .get_metric_with_label_values(&["unavailable"])
            .map(|c| c.inc());
        return None;
    }

    record_ocr_attempted();
    let _ = EXTRACTION_OCR_TOTAL
        .get_metric_with_label_values(&["attempted"])
        .map(|c| c.inc());

    // Render PDF pages to PPM images in a temp directory.
    // 150 DPI: letter page ≈ 6 MB uncompressed; capped at 20 pages (~120 MB peak).
    let tmp = tempfile::tempdir().ok()?;
    let prefix = tmp.path().join("pg");

    let render = Command::new("pdftoppm")
        .args(["-r", "150", "-l", "20", path.to_str()?, prefix.to_str()?])
        .output()
        .ok()?;

    if !render.status.success() {
        warn!(
            "extract_text: pdftoppm failed: {}",
            String::from_utf8_lossy(&render.stderr)
        );
        return None;
    }

    // Collect and sort page images (pdftoppm names them pg-1.ppm, pg-2.ppm, …)
    let mut pages: Vec<_> = std::fs::read_dir(tmp.path())
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "ppm").unwrap_or(false))
        .collect();
    pages.sort();

    if pages.is_empty() {
        debug!("extract_text: pdftoppm produced no page images");
        record_ocr_no_pages();
        let _ = EXTRACTION_OCR_TOTAL
            .get_metric_with_label_values(&["no_pages"])
            .map(|c| c.inc());
        return None;
    }

    // OCR each page
    let mut text = String::new();
    for img in &pages {
        let ocr = Command::new("tesseract")
            .args([img.to_str()?, "stdout", "-l", "eng", "--psm", "3"])
            .output()
            .ok()?;
        if ocr.status.success() {
            let page_text = String::from_utf8_lossy(&ocr.stdout);
            if !page_text.trim().is_empty() {
                text.push_str(&page_text);
                text.push('\n');
            }
        }
    }

    if text.trim().is_empty() {
        debug!("extract_text: OCR produced no text");
        record_ocr_no_text();
        let _ = EXTRACTION_OCR_TOTAL
            .get_metric_with_label_values(&["no_text"])
            .map(|c| c.inc());
        None
    } else {
        record_ocr_ok();
        let _ = EXTRACTION_OCR_TOTAL
            .get_metric_with_label_values(&["ok"])
            .map(|c| c.inc());
        info!(
            "extract_text: OCR extracted {} chars from {} page(s)",
            text.len(),
            pages.len()
        );
        Some(text)
    }
}

/// Try pdftotext (poppler-utils) for higher-quality PDF text extraction.
/// Handles multi-column layouts, complex fonts, and non-standard encodings
/// better than pdf-extract. Returns None if pdftotext is not on PATH.
fn extract_text_from_pdf_pdftotext(path: &Path) -> Option<String> {
    use std::process::Command;
    let output = Command::new("pdftotext")
        .args(["-layout", "-enc", "UTF-8", path.to_str()?, "-"])
        .output()
        .ok()?;
    if !output.status.success() {
        debug!(
            "extract_text: pdftotext failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Remove repeated short lines that are likely PDF page headers/footers.
/// Lines appearing 4+ times that are ≤80 chars are treated as noise.
fn dedupe_pdf_noise(text: String) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 12 {
        return text;
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for line in &lines {
        let t = line.trim();
        if !t.is_empty() {
            *counts.entry(t).or_insert(0) += 1;
        }
    }
    let noise: std::collections::HashSet<&str> = counts
        .into_iter()
        .filter(|(line, count)| *count >= 4 && line.len() <= 80)
        .map(|(line, _)| line)
        .collect();
    if noise.is_empty() {
        return text;
    }
    let filtered: Vec<&str> = lines
        .iter()
        .filter(|l| !noise.contains(l.trim()))
        .copied()
        .collect();
    let removed = lines.len() - filtered.len();
    if removed > 0 {
        debug!(
            "dedupe_pdf_noise: removed {} repeated header/footer lines",
            removed
        );
    }
    filtered.join("\n")
}

/// Smart HTML text extractor:
/// 1. Strips `<script>`, `<style>`, and `<head>` blocks entirely (content included).
/// 2. Strips remaining HTML tags.
/// 3. Decodes HTML entities.
fn extract_text_from_html(bytes: &[u8]) -> Option<String> {
    let raw = detect_and_decode(bytes)?;
    let cleaned = remove_html_blocks(&raw, &["script", "style", "head"]);
    let (stripped, _) = strip_html_tags(&cleaned);
    Some(decode_html_entities(&stripped))
}

/// Remove named block-level HTML elements and their contents entirely.
fn remove_html_blocks(html: &str, block_tags: &[&str]) -> String {
    let mut result = html.to_string();
    for tag in block_tags {
        let open_pat = format!("<{}", tag);
        let close_pat = format!("</{}>", tag);
        loop {
            let lower = result.to_lowercase();
            let Some(start) = lower.find(&open_pat) else {
                break;
            };
            // find '>' to end the opening tag
            let tag_end = lower[start..]
                .find('>')
                .map(|i| start + i + 1)
                .unwrap_or(start + open_pat.len());
            if let Some(close_off) = lower[tag_end..].find(&close_pat) {
                let end = tag_end + close_off + close_pat.len();
                result.replace_range(start..end, " ");
            } else {
                result.truncate(start);
                break;
            }
        }
    }
    result
}

/// Decode common HTML named entities and numeric entities (&#NNN; / &#xHHH;).
pub fn decode_html_entities(text: &str) -> String {
    // Named entities
    let s = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&ldquo;", "\"")
        .replace("&rdquo;", "\"")
        .replace("&lsquo;", "'")
        .replace("&rsquo;", "'")
        .replace("&hellip;", "...")
        .replace("&copy;", "©")
        .replace("&reg;", "®")
        .replace("&trade;", "™");

    // Numeric entities: &#NNN; and &#xHHH;
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' && i + 2 < chars.len() && chars[i + 1] == '#' {
            let mut j = i + 2;
            let hex = j < chars.len() && (chars[j] == 'x' || chars[j] == 'X');
            if hex {
                j += 1;
            }
            let start = j;
            while j < chars.len() && chars[j] != ';' && j - start < 8 {
                j += 1;
            }
            if j < chars.len() && chars[j] == ';' {
                let num_str: String = chars[start..j].iter().collect();
                let codepoint = if hex {
                    u32::from_str_radix(&num_str, 16).ok()
                } else {
                    num_str.parse::<u32>().ok()
                };
                if let Some(cp) = codepoint.and_then(char::from_u32) {
                    out.push(cp);
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Decode bytes to a UTF-8 string, detecting encoding when not UTF-8.
/// Priority: UTF-8 (fast path) → BOM detection → chardetng detection → lossy UTF-8.
pub fn detect_and_decode(bytes: &[u8]) -> Option<String> {
    // Fast path: valid UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Some(s.to_string());
    }
    // BOM detection
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // UTF-8 BOM
        return std::str::from_utf8(&bytes[3..])
            .ok()
            .map(|s| s.to_string())
            .or_else(|| Some(String::from_utf8_lossy(&bytes[3..]).into_owned()));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (cow, _, _) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        return Some(cow.into_owned());
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (cow, _, _) = encoding_rs::UTF_16BE.decode(&bytes[2..]);
        return Some(cow.into_owned());
    }
    // chardetng: feed up to 4 KB for detection
    let mut det = chardetng::EncodingDetector::new();
    det.feed(&bytes[..bytes.len().min(4096)], true);
    let encoding = det.guess(None, true);
    debug!("detect_and_decode: detected encoding={}", encoding.name());
    let (cow, _, _) = encoding.decode(bytes);
    Some(cow.into_owned())
}

pub fn default_chunker(_hint: ChunkerMode) -> Box<dyn Chunker> {
    let config = crate::db::chunk_settings::global_config();
    let mode = crate::db::chunk_settings::global_chunker_mode();
    create_chunker(mode.into(), &config)
}
