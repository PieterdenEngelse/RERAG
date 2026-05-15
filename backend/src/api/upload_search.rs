// ~/ag/backend/src/api/upload_search.rs  v1.0
// Upload, reindex, search, rerank, summarize endpoints

use super::*;
// Rate limiting is enforced by middleware (see monitoring/rate_limit_middleware.rs).
// The per-handler token-bucket implementation was removed to avoid double-limiting.

#[derive(serde::Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub corpus: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct RerankRequest {
    pub query: String,
    pub candidates: Vec<String>,
}

#[derive(serde::Deserialize)]
pub struct SummarizeRequest {
    pub query: String,
    pub candidates: Vec<String>,
}

pub(crate) async fn upload_document_inner(
    mut payload: Multipart,
    config: web::Data<ApiConfig>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let upload_dir = default_upload_dir();
    fs::create_dir_all(&upload_dir).ok();
    let mut uploaded_files = Vec::new();

    while let Some(item) = payload.next().await {
        let mut field = item?;
        let filename = field
            .content_disposition()
            .as_ref()
            .and_then(|cd| cd.get_filename())
            .ok_or_else(|| actix_web::error::ErrorBadRequest("No filename"))?
            .to_string();

        let ext = Path::new(&filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Allow documents and code files that mime_detect.rs supports
        let allowed_extensions = [
            // Documents
            "pdf", "txt", "text", "md", "markdown", "html", "htm", "xhtml", "xml", "json",
            // Office formats
            "docx", "xlsx", "csv", "odt", "ods", "epub", "pptx", // Code files
            "rs", "py", "pyw", "js", "mjs", "cjs", "ts", "tsx", "go", "java", "cs", "cpp", "cc",
            "cxx", "hpp", "c", "h", "rb", "php", "sh", "bash", "zsh", "sql", "yaml", "yml", "toml",
        ];

        if !allowed_extensions.contains(&ext.as_str()) {
            return Ok(HttpResponse::BadRequest().body(format!(
                "File type '{}' not supported. Allowed: documents (pdf, txt, md, html, xml, json), office formats (docx, xlsx, csv, odt, ods, epub, pptx), and code files (rs, py, js, ts, go, java, etc.)",
                ext
            )));
        }

        let filepath = format!("{}/{}", upload_dir, filename);
        let mut f = web::block(move || std::fs::File::create(&filepath)).await??;
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            f = web::block(move || f.write_all(&data).map(|_| f)).await??;
        }

        uploaded_files.push(filename);
    }

    let mut indexed_files = Vec::new();
    let mut index_errors = Vec::new();
    let io_backend = crate::perf::io_uring::backend_name();

    if !uploaded_files.is_empty() {
        if is_reindex_in_progress() {
            index_errors.push(json!({
                "file": null,
                "error": "Reindex already in progress; automatic indexing skipped",
            }));
        } else if let Some(handle) = RETRIEVER.get() {
            // Stream each file: extract → chunk+embed → index → drop.
            // Never accumulate all prepared docs in RAM simultaneously.
            let chunker_mode = config.chunker_mode;
            #[allow(clippy::type_complexity)]
            let mut graph_index_tasks: Vec<(String, String, Vec<(String, String)>)> = Vec::new();

            for filename in &uploaded_files {
                let path = Path::new(&upload_dir).join(filename);

                // Phase 1: Extract IR (I/O-bound, outside mutex)
                let ir = index::extract_ir_async(&path, "default").await;
                let ir = match ir {
                    Some(ir) => ir,
                    None => {
                        index_errors.push(json!({
                            "file": filename,
                            "error": "Failed to extract document IR from file",
                        }));
                        continue;
                    }
                };

                // Phase 2: Chunk + embed (CPU-bound, thread pool, outside mutex)
                let path_clone = path.clone();
                let prepared = tokio::task::spawn_blocking(move || {
                    let global = crate::db::chunk_settings::global_config();
                    let cp_enabled = global.context_prefix_enabled;
                    let chunker = crate::index::default_chunker(chunker_mode);
                    index::prepare_doc(&path_clone, &ir, chunker_mode, chunker.as_ref(), "default", cp_enabled)
                })
                .await
                .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

                // Phase 3: Write to index (brief mutex window per file)
                match handle.lock() {
                    Ok(mut retriever) => {
                        if let Err(e) = retriever.begin_batch() {
                            index_errors.push(
                                json!({ "file": filename, "error": format!("begin_batch: {e}") }),
                            );
                            continue;
                        }
                        match index::index_prepared_doc(&mut retriever, prepared) {
                            Ok((chunk_count, graph_chunks)) if chunk_count > 0 => {
                                if let Err(err) = retriever.commit() {
                                    index_errors.push(json!({
                                        "file": filename,
                                        "error": format!("commit failed: {}", err),
                                    }));
                                } else {
                                    indexed_files.push(json!({
                                        "file": filename.clone(),
                                        "chunks_indexed": chunk_count,
                                        "io_backend": io_backend,
                                    }));
                                    if !graph_chunks.is_empty() {
                                        graph_index_tasks.push((
                                            filename.clone(),
                                            path.to_string_lossy().to_string(),
                                            graph_chunks,
                                        ));
                                    }
                                }
                            }
                            Ok((_, _)) => {
                                let _ = retriever.commit();
                                index_errors.push(json!({
                                    "file": filename,
                                    "error": "Extraction returned no text — file is absent from the search index. Check the Parser tile: if status is 'empty', the PDF may be image-only (install pdftotext / tesseract) or use the Docling sidecar.",
                                }));
                            }
                            Err(err) => {
                                let _ = retriever.commit();
                                index_errors.push(json!({
                                    "file": filename,
                                    "error": err,
                                }));
                            }
                        }
                    }
                    Err(_) => {
                        index_errors.push(json!({
                            "file": filename,
                            "error": "Failed to lock retriever for indexing",
                        }));
                    }
                }
            }

            // Phase 4: Index to knowledge graph (outside mutex, async)
            for (filename, source, chunks) in graph_index_tasks {
                index_to_knowledge_graph(&filename, &filename, &source, &chunks).await;
            }
        } else {
            index_errors.push(json!({
                "file": null,
                "error": "Retriever not initialized; run /reindex manually",
            }));
        }
    }

    trigger_auto_export_after_upload(uploaded_files.len());

    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "uploaded_files": uploaded_files,
        "indexed_files": indexed_files,
        "index_errors": index_errors,
        "request_id": request_id
    })))
}

pub async fn list_documents() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(default_upload_dir()) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                if let Some(filename) = entry.file_name().to_str() {
                    files.push(filename.to_string());
                }
            }
        }
    }
    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "documents": files,
        "count": files.len(),
        "request_id": request_id
    })))
}

pub async fn delete_document(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let filename = path.into_inner();
    let filepath = format!("{}/{}", default_upload_dir(), filename);

    match fs::remove_file(&filepath) {
        Ok(_) => {
            // Incrementally delete from the live index so the deleted document
            // does not keep showing up in search results.
            let mut deleted_chunks: Option<usize> = None;
            if let Some(retriever) = RETRIEVER.get() {
                if let Ok(mut retriever) = retriever.lock() {
                    match retriever.delete_document_by_filename(&filename) {
                        Ok(count) => {
                            deleted_chunks = Some(count);
                        }
                        Err(e) => {
                            warn!(error = %e, filename, "Failed to delete document chunks from index");
                        }
                    }
                }
            }
            crate::monitoring::forget_extraction_file(&filename);
            crate::monitoring::forget_canon_file(&filename, "default");

            Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": format!("Deleted {}", filename),
                "deleted_chunks": deleted_chunks,
                "request_id": request_id
            })))
        }
        Err(_) => Ok(HttpResponse::NotFound().json(json!({
            "status": "error",
            "message": "File not found",
            "request_id": request_id
        }))),
    }
}

pub async fn reindex_handler(config: web::Data<ApiConfig>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let start = std::time::Instant::now();

    // Phase 15: Check concurrency
    if REINDEX_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "status": "busy",
            "message": "Reindex already in progress",
            "request_id": request_id
        })));
    }

    // Alerting config
    let hooks = crate::monitoring::alerting_hooks::AlertingHooksConfig::from_env();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let upload_dir = default_upload_dir();
        let global_cfg = crate::db::chunk_settings::global_config();
        let chunker = crate::index::default_chunker(config.chunker_mode);
        let res = index::index_all_documents(
            &mut retriever,
            &upload_dir,
            config.chunker_mode,
            chunker.as_ref(),
            "default",
            global_cfg.context_prefix_enabled,
        );
        let duration_ms = start.elapsed().as_millis() as u64;
        let vectors = retriever.metrics.total_vectors as u64;
        let mappings = retriever.metrics.total_documents_indexed as u64;
        REINDEX_IN_PROGRESS.store(false, Ordering::SeqCst);

        // Fire webhook (non-blocking)
        let event = match res {
            Ok(_) => crate::monitoring::alerting_hooks::ReindexCompletionEvent::success(
                duration_ms,
                vectors,
                mappings,
            ),
            Err(_) => crate::monitoring::alerting_hooks::ReindexCompletionEvent::error(
                duration_ms,
                vectors,
                mappings,
            ),
        };
        actix_web::rt::spawn(async move {
            crate::monitoring::alerting_hooks::send_alert(&hooks, event).await;
        });

        if res.is_ok() {
            tokio::spawn(async move {
                crate::api::graph_routes::rebuild_graph_from_index().await;
                crate::api::graph_routes::export_and_reload_graph().await;
            });
        }
        match res {
            Ok(_) => Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "Reindexing complete",
                "request_id": request_id
            }))),
            Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Reindex failed: {}", e),
                "request_id": request_id
            }))),
        }
    } else {
        REINDEX_IN_PROGRESS.store(false, Ordering::SeqCst);
        // Fire error webhook for missing retriever
        let hooks2 = crate::monitoring::alerting_hooks::AlertingHooksConfig::from_env();
        let event = crate::monitoring::alerting_hooks::ReindexCompletionEvent::error(0, 0, 0);
        actix_web::rt::spawn(async move {
            crate::monitoring::alerting_hooks::send_alert(&hooks2, event).await;
        });
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

#[allow(clippy::await_holding_lock)]
pub(crate) fn launch_async_reindex_job(
    config: web::Data<ApiConfig>,
) -> Result<String, (StatusCode, String)> {
    if REINDEX_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Reindex already in progress".to_string(),
        ));
    }

    let job_id = Uuid::new_v4().to_string();
    let job = AsyncJob {
        job_id: job_id.clone(),
        status: "pending".to_string(),
        started_at: Utc::now().to_rfc3339(),
        completed_at: None,
        vectors_indexed: None,
        mappings_indexed: None,
        error: None,
    };

    let jobs = get_jobs_map();
    jobs.lock().unwrap().insert(job_id.clone(), job);

    let job_id_clone = job_id.clone();
    let jobs_map = jobs.clone();
    let retriever_handle = RETRIEVER.get().map(Arc::clone);
    let config_clone = config.clone();
    let upload_dir_for_job = default_upload_dir();

    actix_web::rt::spawn(
        async move {
            let start = std::time::Instant::now();
            let hooks = crate::monitoring::alerting_hooks::AlertingHooksConfig::from_env();
            if let Some(retriever) = retriever_handle {
                let mut retriever = retriever.lock().unwrap();
                {
                    let mut job = jobs_map
                        .lock()
                        .unwrap()
                        .get(&job_id_clone)
                        .cloned()
                        .unwrap();
                    job.status = "running".to_string();
                    jobs_map.lock().unwrap().insert(job_id_clone.clone(), job);
                }

                let global_cfg2 = crate::db::chunk_settings::global_config();
                let chunker = crate::index::default_chunker(config_clone.chunker_mode);
                let res = index::index_all_documents(
                    &mut retriever,
                    &upload_dir_for_job,
                    config_clone.chunker_mode,
                    chunker.as_ref(),
                    "default",
                    global_cfg2.context_prefix_enabled,
                );

                let mut job = jobs_map
                    .lock()
                    .unwrap()
                    .get(&job_id_clone)
                    .cloned()
                    .unwrap();
                let duration_ms = start.elapsed().as_millis() as u64;
                let vectors = retriever.metrics.total_vectors as u64;
                let mappings = retriever.metrics.total_documents_indexed as u64;
                drop(retriever); // Release lock before async graph rebuild

                match res {
                    Ok(_) => {
                        job.status = "completed".to_string();
                        job.completed_at = Some(Utc::now().to_rfc3339());
                        job.vectors_indexed = Some(vectors as usize);
                        job.mappings_indexed = Some(mappings as usize);
                        let event =
                            crate::monitoring::alerting_hooks::ReindexCompletionEvent::success(
                                duration_ms,
                                vectors,
                                mappings,
                            );
                        crate::monitoring::alerting_hooks::send_alert(&hooks, event).await;
                    }
                    Err(ref e) => {
                        job.status = "failed".to_string();
                        job.completed_at = Some(Utc::now().to_rfc3339());
                        job.error = Some(e.to_string());
                        let event =
                            crate::monitoring::alerting_hooks::ReindexCompletionEvent::error(
                                duration_ms,
                                vectors,
                                mappings,
                            );
                        crate::monitoring::alerting_hooks::send_alert(&hooks, event).await;
                    }
                }
                jobs_map.lock().unwrap().insert(job_id_clone.clone(), job);
                // v1.3.0: Rebuild knowledge graph after successful reindex
                if res.is_ok() {
                    let graph_result = crate::api::graph_routes::rebuild_graph_from_index().await;
                    info!(
                        "Post-reindex graph rebuild: {} docs, {} chunks",
                        graph_result.documents_processed, graph_result.chunks_processed
                    );
                    // Auto-export petgraph after graph rebuild
                    tokio::spawn(async move {
                        crate::api::graph_routes::export_and_reload_graph().await;
                    });
                }
            } else {
                let mut job = jobs_map
                    .lock()
                    .unwrap()
                    .get(&job_id_clone)
                    .cloned()
                    .unwrap();
                job.status = "failed".to_string();
                job.completed_at = Some(Utc::now().to_rfc3339());
                job.error = Some("Retriever not initialized".to_string());
                jobs_map
                    .lock()
                    .unwrap()
                    .insert(job_id_clone.clone(), job.clone());
                let event =
                    crate::monitoring::alerting_hooks::ReindexCompletionEvent::error(0, 0, 0);
                crate::monitoring::alerting_hooks::send_alert(&hooks, event).await;
            }
            REINDEX_IN_PROGRESS.store(false, Ordering::SeqCst);
        },
    );

    Ok(job_id)
}

/// Phase 15: Async reindex endpoint
pub async fn reindex_async_handler(config: web::Data<ApiConfig>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    match launch_async_reindex_job(config) {
        Ok(job_id) => Ok(HttpResponse::Accepted().json(json!({
            "status": "accepted",
            "job_id": job_id,
            "request_id": request_id
        }))),
        Err((status, message)) => Ok(HttpResponse::build(status).json(json!({
            "status": "busy",
            "message": message,
            "request_id": request_id
        }))),
    }
}

/// Phase 15: Check async job status
pub async fn reindex_status_handler(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let job_id = path.into_inner();

    let jobs = get_jobs_map();
    let jobs_lock = jobs.lock().unwrap();

    if let Some(job) = jobs_lock.get(&job_id) {
        Ok(HttpResponse::Ok().json(json!({
            "status": job.status,
            "job_id": job.job_id,
            "started_at": job.started_at,
            "completed_at": job.completed_at,
            "vectors_indexed": job.vectors_indexed,
            "mappings_indexed": job.mappings_indexed,
            "error": job.error,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::NotFound().json(json!({
            "status": "not_found",
            "message": format!("Job {} not found", job_id),
            "request_id": request_id
        })))
    }
}

/// Phase 15: Index info endpoint
fn human_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let mut val = bytes as f64;
    let mut i = 0;
    while val >= 1024.0 && i < units.len() - 1 {
        val /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{} B", bytes) } else { format!("{:.1} {}", val, units[i]) }
}

fn process_rss_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse::<u64>().ok())
        })
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

pub async fn index_info_handler() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let in_ram = std::env::var("INDEX_IN_RAM")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        let doc_count = retriever.metrics.total_documents_indexed;

        let (mem_bytes, mem_human, mem_label) = if in_ram {
            let rss = process_rss_bytes();
            (rss, human_bytes(rss), "Process RSS")
        } else {
            let bytes = retriever.metrics.get_index_size_bytes().unwrap_or(0);
            let human = retriever.metrics.get_index_size_human().unwrap_or_else(|_| "?".into());
            (bytes, human, "Est. RAM if active")
        };

        Ok(HttpResponse::Ok().json(json!({
            "index_in_ram": in_ram,
            "mode": if in_ram { "RAM (fast)" } else { "Disk (standard)" },
            "warning": if in_ram && mem_bytes > 100_000_000 {
                json!(format!("High memory usage: process RSS is {}.", mem_human))
            } else if in_ram {
                json!("INDEX_IN_RAM active. Avoid when index exceeds ~100 MB on disk.")
            } else {
                json!(null)
            },
            "total_documents": doc_count,
            "total_vectors": retriever.metrics.total_vectors,
            "index_size_bytes": mem_bytes,
            "index_size_human": mem_human,
            "memory_label": mem_label,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub(crate) async fn search_documents_inner(
    query: web::Query<SearchQuery>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let start = std::time::Instant::now();
    let corpus_slug = query.corpus.as_deref().unwrap_or("default");
    let retriever_handle = get_corpus_retriever(corpus_slug);
    if let Some(retriever) = retriever_handle {
        // Normalize query for each use: Embed for vector search, Index for BM25
        let embed_q =
            crate::normalizer::normalize(&query.q, crate::normalizer::NormalizeTarget::Embed);
        crate::monitoring::record_canon_embed_query(query.q.len(), embed_q.len());
        let index_q = crate::normalizer::to_index(&embed_q);
        crate::monitoring::record_canon_index_query(embed_q.len(), index_q.len());
        let query_vector = if let Some(svc) = get_embedding_service() {
            svc.embed_query(&embed_q).await
        } else {
            crate::embedder::embed(&embed_q)
        };
        // Entity extraction for graph search (use raw query — NER has its own tokenizer)
        let extractor = crate::tools::entity_extractor::EntityExtractorTool::new();
        let extraction = extractor.extract(&query.q);
        let entity_texts: Vec<String> = extraction
            .entities
            .iter()
            .filter(|e| e.confidence >= 0.5)
            .map(|e| e.text.clone())
            .collect();
        let mut retriever = retriever.lock().unwrap();
        // Graph search (petgraph)
        let graph_results = retriever.graph_search(&entity_texts);
        // Hybrid BM25 + vector search
        let hybrid_results = retriever
            .hybrid_search(&index_q, Some(&query_vector))
            .unwrap_or_default();
        // 3-way RRF merge
        let k = 60.0_f32;
        let mut score_map: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();
        for (rank, content) in hybrid_results.iter().enumerate() {
            *score_map.entry(content.clone()).or_insert(0.0) += 1.0 / (k + rank as f32 + 1.0);
        }
        for (rank, content) in graph_results.iter().enumerate() {
            *score_map.entry(content.clone()).or_insert(0.0) += 1.0 / (k + rank as f32 + 1.0);
        }
        let mut merged: Vec<(String, f32)> = score_map.into_iter().collect();
        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top: Vec<String> = merged.into_iter().take(10).map(|(c, _)| c).collect();

        let results: Vec<serde_json::Value> = top
            .iter()
            .map(|content| {
                let meta = retriever.meta_for_content(content);
                json!({
                    "text": content,
                    "block_type": meta.map(|m| m.block_type.as_str()).unwrap_or("Text"),
                    "page": meta.and_then(|m| m.page),
                    "extractor": meta.map(|m| m.extractor.as_str()).unwrap_or("builtin"),
                })
            })
            .collect();

        let elapsed = start.elapsed().as_millis() as u64;

        // Record tool execution
        crate::monitoring::record_tool_execution(
            "SemanticSearch",
            &query.q,
            true,
            &format!("{} results", results.len()),
            elapsed,
            1.0,
            Some("api/search"),
        );

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "results": results,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub async fn rerank(request: web::Json<RerankRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        let reranked = retriever.rerank_by_similarity(&request.query, &request.candidates);
        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "results": reranked,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub async fn summarize(request: web::Json<SummarizeRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        let summary = retriever.summarize_chunks(&request.query, &request.candidates);
        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "summary": summary,
            "request_id": request_id
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub async fn save_vectors_handler() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        match retriever.force_save() {
            Ok(_) => Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "message": "Vectors saved successfully",
                "vector_count": retriever.vectors.len(),
                "request_id": request_id
            }))),
            Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save vectors: {}", e),
                "request_id": request_id
            }))),
        }
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

#[derive(serde::Deserialize)]
pub(crate) struct ChunkPreviewRequest {
    pub text: String,
    pub filename: Option<String>,
}

pub(crate) async fn chunk_preview_handler(
    payload: web::Json<ChunkPreviewRequest>,
) -> Result<HttpResponse, Error> {
    use crate::db::chunk_settings;
    use crate::memory::chunker_factory::create_chunker;

    let request_id = generate_request_id();
    let body = payload.into_inner();

    if body.text.is_empty() {
        return Ok(HttpResponse::BadRequest().json(json!({
            "status": "invalid",
            "message": "text must not be empty",
            "request_id": request_id,
        })));
    }

    let config = chunk_settings::global_config();
    let mode = chunk_settings::global_chunker_mode();

    // Semantic and Pipeline (with sem stage) require the ONNX embedder.
    // Check availability before attempting to chunk — the embedder panics on
    // first use if the model file is absent, which produces an empty 500 body.
    let needs_embedder = matches!(mode, crate::config::ChunkerMode::Semantic)
        || (matches!(mode, crate::config::ChunkerMode::Pipeline)
            && config.pipeline_stages.split(',').any(|s| s.trim() == "sem"));
    if needs_embedder {
        if !crate::perf::onnx_embedder::is_onnx_enabled() {
            return Ok(HttpResponse::UnprocessableEntity().json(json!({
                "status": "error",
                "message": "The backend was compiled without the 'onnx' feature. Rebuild with: cargo build (onnx is on by default).",
                "request_id": request_id,
            })));
        }
        let model_path = std::env::var("ONNX_MODEL_PATH")
            .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());
        if !std::path::Path::new(&model_path).exists() {
            // Derive the companion .data path for the diagnostic message
            let data_path = format!("{}.data", model_path);
            let data_exists = std::path::Path::new(&data_path).exists();
            let detail = if data_exists {
                format!(
                    "The weight file '{}' exists but the model graph '{}' is missing.",
                    data_path, model_path
                )
            } else {
                format!(
                    "Neither '{}' nor '{}.data' were found. \
                     Both files are required — the graph file is ~600 KB and \
                     the weight file is ~87 MB.",
                    model_path, model_path
                )
            };
            return Ok(HttpResponse::UnprocessableEntity().json(json!({
                "status": "error",
                "message": format!(
                    "ONNX model not found. {} \
                     Place both files in the models/ directory at the repo root, \
                     or set ONNX_MODEL_PATH in .env to point to the graph file. \
                     See /config/onnx for path configuration, then restart the backend.",
                    detail
                ),
                "request_id": request_id,
            })));
        }
    }

    let chunker = create_chunker(mode.into(), &config);

    let filename = body.filename.as_deref().unwrap_or("preview");
    let preview_ct = crate::mime_detect::detect_from_extension(filename);
    let needs_html_clean = matches!(preview_ct, crate::mime_detect::ContentType::Html);
    let needs_unicode_clean = matches!(
        preview_ct,
        crate::mime_detect::ContentType::Pdf
            | crate::mime_detect::ContentType::Docx
            | crate::mime_detect::ContentType::Odt
            | crate::mime_detect::ContentType::Epub
            | crate::mime_detect::ContentType::Pptx
            | crate::mime_detect::ContentType::Html
    );
    let preprocessed = crate::index::apply_text_preprocessing(
        body.text.clone(),
        needs_html_clean,
        needs_unicode_clean,
        "default",
        filename,
    );
    let mut chunks = chunker.chunk_text(&preprocessed);
    if config.context_prefix_enabled {
        chunks = chunks
            .into_iter()
            .map(|c| format!("[Source: {}] {}", filename, c))
            .collect();
    }

    let stats = chunker.stats();
    let mode_str = config.mode.clone();

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "chunk_count": chunks.len(),
        "chunks": chunks,
        "stats": stats,
        "mode": mode_str,
        "config": {
            "target_size": config.target_size,
            "min_size": config.min_size,
            "max_size": config.max_size,
            "overlap": config.overlap,
            "semantic_similarity_threshold": config.semantic_similarity_threshold,
            "clean_html": config.clean_html,
            "clean_unicode": config.clean_unicode,
            "context_prefix_enabled": config.context_prefix_enabled,
            "context_prefix_tokens": config.context_prefix_tokens,
        }
    })))
}
