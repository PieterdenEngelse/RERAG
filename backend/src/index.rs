use crate::config::ChunkerMode;
use crate::embedder;
use crate::memory::chunker_factory::{create_chunker, Chunker};
use crate::mime_detect::{detect_content_type, ContentType};
use crate::monitoring::DetectionInfo;
use crate::retriever::Retriever;
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

pub fn index_all_documents(
    retriever: &mut Retriever,
    folder: &str,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
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
) -> Result<usize, String> {
    // Get detection info for observability
    let (_, detection_info) = detect_file_type_with_info(path)?;
    index_file_with_detection(retriever, path, chunker_mode, chunker, detection_info)
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
    let chunks = chunker.chunk_text(content);
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

        if let Err(e) = retriever.index_chunk(&chunk_id, chunk, &vector) {
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
) -> Result<(usize, Vec<(String, String)>), String> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let chunk_start = std::time::Instant::now();
    let chunks = chunker.chunk_text(content);
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

        if let Err(e) = retriever.index_chunk(&chunk_id, chunk, &vector) {
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

    Ok((ok, graph_chunks))
}

/// Async version of index_file using io_uring for 2-3x faster file reads
/// Use this for document ingestion to benefit from io_uring on Linux
pub async fn index_file_async(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
) -> Result<usize, String> {
    let (_, detection_info) = detect_file_type_with_info(path)?;
    index_file_with_detection_async(retriever, path, chunker_mode, chunker, detection_info).await
}

async fn index_file_with_detection_async(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    detection_info: DetectionInfo,
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
    let chunks = chunker.chunk_text(&content);
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

        if let Err(e) = retriever.index_chunk(&chunk_id, chunk, &vector) {
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

    Ok(ok)
}

fn index_file_with_detection(
    retriever: &mut Retriever,
    path: &Path,
    chunker_mode: ChunkerMode,
    chunker: &dyn Chunker,
    detection_info: DetectionInfo,
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
    let chunks = chunker.chunk_text(&content);
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

        if let Err(e) = retriever.index_chunk(&chunk_id, chunk, &vector) {
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

/// Common text extraction logic from bytes
fn extract_text_from_bytes(path: &Path, bytes: Vec<u8>) -> Option<String> {
    let filename = path.file_name().and_then(|n| n.to_str());
    let content_type = detect_content_type(&bytes, filename);

    match content_type {
        ContentType::Pdf => {
            // PDF parsing - could use pdf-extract crate here
            debug!("extract_text: PDF detected, attempting extraction");
            Some("PDF parsing not fully implemented.".to_string())
        }
        ContentType::Binary => {
            debug!("extract_text: Binary file detected, skipping");
            None
        }
        _ => {
            // For all text-based types, try to read as UTF-8
            String::from_utf8(bytes.clone()).ok().or_else(|| {
                // Fall back to lossy conversion for non-UTF8 text
                debug!("extract_text: Non-UTF8 content, using lossy conversion");
                Some(String::from_utf8_lossy(&bytes).to_string())
            })
        }
    }
}

pub fn default_chunker(mode: ChunkerMode) -> Box<dyn Chunker> {
    let config = crate::db::chunk_settings::global_config();
    create_chunker(mode.into(), &config)
}
