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

    // Begin batch mode — single writer, single commit
    retriever
        .begin_batch()
        .map_err(|e| format!("begin_batch failed: {}", e))?;

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
                match index_file_with_detection(
                    retriever,
                    &path,
                    chunker_mode,
                    chunker,
                    detection_info,
                    corpus_slug,
                ) {
                    Ok(chunks) => {
                        debug!(
                            "indexed file='{}' chunks={} type={:?}",
                            path_str, chunks, content_type
                        );
                        indexed_count += 1;
                    }
                    Err(e) => warn!("index_file failed for '{}': {}", path_str, e),
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

    retriever
        .commit()
        .map_err(|e| format!("commit failed: {}", e))
}

/// Async version of index_all_documents using io_uring for 2-3x faster file reads
/// Reads all files in parallel with io_uring, then indexes them
pub async fn index_all_documents_async(
    retriever: &mut Retriever,
    folder: &str,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    _corpus_slug: &str,
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

    // Phase 2: Read all files asynchronously with io_uring
    let start = std::time::Instant::now();
    let mut file_contents: Vec<(std::path::PathBuf, Option<String>)> =
        Vec::with_capacity(total_files);

    for path in file_paths {
        let content = extract_text_async(&path).await;
        file_contents.push((path, content));
    }

    let read_duration = start.elapsed();
    info!(
        "index_all_documents_async: read {} files in {:?} via {}",
        total_files, read_duration, io_backend
    );

    // Phase 3: Index all content (CPU-bound, no I/O)
    let index_start = std::time::Instant::now();
    let mut indexed_count = 0usize;

    for (path, content_opt) in file_contents {
        if let Some(content) = content_opt {
            match index_content_direct(retriever, &path, &content, chunker_mode, chunker) {
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

    let index_duration = index_start.elapsed();

    retriever
        .commit()
        .map_err(|e| format!("commit failed: {}", e))?;

    info!(
        "index_all_documents_async: indexed {} chunks from {} files (read: {:?}, index: {:?}, backend: {})",
        indexed_count, total_files, read_duration, index_duration, io_backend
    );

    Ok(indexed_count)
}

pub fn index_file(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    corpus_slug: &str,
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
) -> Result<usize, String> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let chunk_start = std::time::Instant::now();
    let mut embed_file_in = 0usize;
    let mut embed_file_out = 0usize;
    let chunks: Vec<String> = apply_context_prefix(chunker.chunk_text(content), filename)
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
    crate::monitoring::record_canon_file_embed(filename, embed_file_in, embed_file_out);
    crate::monitoring::record_canon_file_index(filename, index_file_in, index_file_out);
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

        if let Err(e) = retriever.index_chunk(&chunk_id, &index_chunks[i], &vector) {
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
        chunker.stats(),
    );
    snap.tokenizer_model = crate::api::get_token_counter().map(|h| h.model_name());
    crate::monitoring::record_chunking_snapshot(snap);

    Ok(ok)
}

/// Index content and return chunks for knowledge graph integration
/// Returns (chunk_count, Vec<(chunk_id, chunk_content)>)
pub fn index_content_with_graph(
    retriever: &mut Retriever,
    path: &Path,
    content: &str,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    corpus_slug: &str,
) -> Result<(usize, Vec<(String, String)>), String> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let chunk_start = std::time::Instant::now();
    let mut embed_file_in = 0usize;
    let mut embed_file_out = 0usize;
    let chunks: Vec<String> = apply_context_prefix(chunker.chunk_text(content), filename)
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
    crate::monitoring::record_canon_file_embed(filename, embed_file_in, embed_file_out);
    crate::monitoring::record_canon_file_index(filename, index_file_in, index_file_out);
    let chunk_duration = chunk_start.elapsed();
    let mut ok = 0usize;
    let mut total_tokens = 0usize;
    let mut graph_chunks = Vec::new();

    let embed_start = std::time::Instant::now();
    let embeddings = embedder::embed_batch(&chunks);
    let embed_duration = embed_start.elapsed();
    debug!(
        "index_content_with_graph: embedding completed for '{}' chunks={} duration_ms={}",
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

        if let Err(e) = retriever.index_chunk(&chunk_id, &index_chunks[i], &vector) {
            warn!(
                "index_content_with_graph: Failed to index chunk {}: {}",
                chunk_id, e
            );
        } else {
            ok += 1;
            graph_chunks.push((chunk_id, chunk.clone()));
        }
    }

    info!(
        "index_content_with_graph: file='{}' mode={:?} chunks={} tokens={} chunk_ms={} embed_ms={}",
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
        chunker.stats(),
    );
    snap.tokenizer_model = crate::api::get_token_counter().map(|h| h.model_name());
    crate::monitoring::record_chunking_snapshot(snap);

    Ok((ok, graph_chunks))
}

/// Async version of index_file using io_uring for 2-3x faster file reads
/// Use this for document ingestion to benefit from io_uring on Linux
pub async fn index_file_async(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    corpus_slug: &str,
) -> Result<usize, String> {
    let (_, detection_info) = detect_file_type_with_info(path)?;
    index_file_with_detection_async(
        retriever,
        path,
        chunker_mode,
        chunker,
        detection_info,
        corpus_slug,
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

    // Use async io_uring-backed file read
    let content = match extract_text_async(path).await {
        Some(text) => text,
        None => {
            warn!(
                "index_file_async: extract_text returned None for '{}'",
                filename
            );
            return Err("extract_text failed".into());
        }
    };

    let chunk_start = std::time::Instant::now();
    let mut embed_file_in = 0usize;
    let mut embed_file_out = 0usize;
    let chunks: Vec<String> = apply_context_prefix(chunker.chunk_text(&content), filename)
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
    crate::monitoring::record_canon_file_embed(filename, embed_file_in, embed_file_out);
    crate::monitoring::record_canon_file_index(filename, index_file_in, index_file_out);
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

        if let Err(e) = retriever.index_chunk(&chunk_id, &index_chunks[i], &vector) {
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

    let content = match extract_text(path) {
        Some(text) => text,
        None => {
            warn!("index_file: extract_text returned None for '{}'", filename);
            return Err("extract_text failed".into());
        }
    };

    let chunk_start = std::time::Instant::now();
    let mut embed_file_in = 0usize;
    let mut embed_file_out = 0usize;
    let chunks: Vec<String> = apply_context_prefix(chunker.chunk_text(&content), filename)
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
    crate::monitoring::record_canon_file_embed(filename, embed_file_in, embed_file_out);
    crate::monitoring::record_canon_file_index(filename, index_file_in, index_file_out);
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

        if let Err(e) = retriever.index_chunk(&chunk_id, &index_chunks[i], &vector) {
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
fn extract_text(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    extract_text_from_bytes(path, bytes)
}

/// Async version of extract_text using io_uring for 2-3x faster file reads
pub async fn extract_text_async(path: &Path) -> Option<String> {
    use crate::perf::io_uring as async_io;

    let bytes = async_io::read_file(path).await.ok()?;
    extract_text_from_bytes(path, bytes)
}

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

/// Common text extraction logic from bytes
fn extract_text_from_bytes(path: &Path, bytes: Vec<u8>) -> Option<String> {
    let filename = path.file_name().and_then(|n| n.to_str());
    let content_type = detect_content_type(&bytes, filename);

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
            debug!("extract_text: PDF detected, trying pdftotext → pdf-extract → OCR");
            let text = extract_text_from_pdf_pdftotext(path).or_else(|| {
                match pdf_extract::extract_text(path) {
                    Ok(t) if !t.trim().is_empty() => Some(t),
                    Ok(_) => {
                        debug!("extract_text: pdf-extract found no text layer — trying OCR");
                        extract_text_from_pdf_ocr(path)
                    }
                    Err(e) => {
                        warn!("extract_text: pdf-extract failed ({}), trying OCR", e);
                        extract_text_from_pdf_ocr(path)
                    }
                }
            });
            text.map(dedupe_pdf_noise)
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
        let preprocessed = apply_text_preprocessing(text, needs_html_clean, needs_unicode_clean);
        let normalized =
            crate::normalizer::normalize(&preprocessed, crate::normalizer::NormalizeTarget::Store);
        record_canon_store(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            preprocessed.len(),
            normalized.len(),
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
            record_extraction_format(format_label, true, chars, file_name, &file_path);
            let _ = EXTRACTION_TOTAL
                .get_metric_with_label_values(&[format_label, "ok"])
                .map(|c| c.inc());
            let _ = EXTRACTION_CHARS_TOTAL
                .get_metric_with_label_values(&[format_label])
                .map(|c| c.inc_by(chars as u64));
        }
        None => {
            record_extraction_format(format_label, false, 0, file_name, &file_path);
            let _ = EXTRACTION_TOTAL
                .get_metric_with_label_values(&[format_label, "empty"])
                .map(|c| c.inc());
        }
    }

    result
}

/// Apply [Source: filename] context prefix to chunks if enabled in global config.
pub fn apply_context_prefix(chunks: Vec<String>, filename: &str) -> Vec<String> {
    let config = crate::db::chunk_settings::global_config();
    if config.context_prefix_enabled {
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
pub fn apply_text_preprocessing(text: String, clean_html: bool, clean_unicode: bool) -> String {
    let mut result = text;
    if clean_html {
        let (cleaned, count) = strip_html_tags(&result);
        if count > 0 {
            debug!("apply_text_preprocessing: stripped {} HTML tags", count);
        }
        result = cleaned;
    }
    if clean_unicode {
        result = clean_unicode_text(&result);
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
fn extract_text_from_pdf_ocr(path: &Path) -> Option<String> {
    use std::process::Command;

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

    // Render PDF pages to PPM images in a temp directory (150 dpi — fast + legible)
    let tmp = tempfile::tempdir().ok()?;
    let prefix = tmp.path().join("pg");

    let render = Command::new("pdftoppm")
        .args(["-r", "300", path.to_str()?, prefix.to_str()?])
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
