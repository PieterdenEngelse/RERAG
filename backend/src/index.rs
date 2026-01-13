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
            // Use MIME detection to determine if file is indexable
            let (content_type, detection_info) = match detect_file_type_with_info(&path) {
                Ok(result) => result,
                Err(e) => {
                    debug!("index_all_documents: failed to detect type for '{}': {}", path_str, e);
                    continue;
                }
            };
            
            debug!(
                "index_all_documents: considering file='{}' content_type={:?} method={}",
                path_str, content_type, detection_info.detection_method
            );
            
            // Only index text-based files
            if content_type.is_text_based() {
                match index_file_with_detection(retriever, &path, chunker_mode, chunker, detection_info) {
                    Ok(chunks) => debug!("indexed file='{}' chunks={} type={:?}", path_str, chunks, content_type),
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

    retriever
        .commit()
        .map_err(|e| format!("commit failed: {}", e))
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
    debug!("index_file: start file='{}' detected_format={} strategy={}", 
        path.to_string_lossy(), detection_info.detected_format, detection_info.chosen_strategy);
    
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
        crate::monitoring::record_chunking_snapshot(
            crate::monitoring::ChunkingStatsSnapshot::with_detection(
                filename,
                chunker_mode,
                ok,
                total_tokens,
                chunk_duration.as_millis() as u64,
                Some(stats),
                detection_info,
            )
        );
    } else {
        crate::monitoring::record_chunking_snapshot(
            crate::monitoring::ChunkingStatsSnapshot::with_detection(
                filename,
                chunker_mode,
                ok,
                total_tokens,
                chunk_duration.as_millis() as u64,
                None,
                detection_info,
            )
        );
    }
    Ok(ok)
}

/// Detect file type using MIME magic bytes with extension fallback
/// Returns both the ContentType and DetectionInfo for observability
fn detect_file_type_with_info(path: &Path) -> Result<(ContentType, DetectionInfo), String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let filename = path.file_name().and_then(|n| n.to_str());
    let extension = path.extension().and_then(|e| e.to_str()).map(|s| s.to_string());
    
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
            String::from_utf8(bytes).ok().or_else(|| {
                // Fall back to lossy conversion for non-UTF8 text
                debug!("extract_text: Non-UTF8 content, using lossy conversion");
                Some(String::from_utf8_lossy(&fs::read(path).ok()?).to_string())
            })
        }
    }
}

pub fn default_chunker(mode: ChunkerMode) -> Box<dyn Chunker> {
    let config = crate::db::chunk_settings::global_config();
    create_chunker(mode.into(), &config)
}
