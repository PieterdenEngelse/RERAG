// Corpus CRUD + corpus-scoped upload/search/reindex/documents routes.
// Existing top-level routes (/upload, /search, etc.) are unchanged and default to "default".

use super::*;
use crate::db::corpora::{self, AgentMemorySettings, CorporaError, CorpusSettings};

#[derive(Deserialize)]
pub struct CreateCorpusBody {
    pub slug: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct RenameCorpusBody {
    pub name: String,
}

fn open_db(config: &ApiConfig) -> Result<rusqlite::Connection, Error> {
    rusqlite::Connection::open(config.path_manager.db_path("documents"))
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))
}

fn corpus_not_found(slug: &str) -> HttpResponse {
    HttpResponse::NotFound().json(json!({
        "status": "error",
        "message": format!("Corpus '{}' not found", slug),
    }))
}

// ─── Corpus CRUD ─────────────────────────────────────────────────────────────

pub async fn create_corpus_handler(
    config: web::Data<ApiConfig>,
    body: web::Json<CreateCorpusBody>,
) -> Result<HttpResponse, Error> {
    let conn = open_db(&config)?;
    match corpora::create_corpus(&conn, &body.slug, &body.name) {
        Ok(corpus) => {
            // Pre-warm the registry entry and ensure the upload dir exists.
            if let Some(reg) = crate::corpus_registry::get_registry() {
                let _ = reg.get_or_create(&body.slug);
            } else {
                config.path_manager.corpus_upload_dir(&body.slug);
            }
            Ok(HttpResponse::Created().json(json!({ "status": "created", "corpus": corpus })))
        }
        Err(CorporaError::InvalidSlug(s)) => Ok(HttpResponse::BadRequest().json(json!({
            "status": "error",
            "message": format!("Invalid slug '{}'. Use 1-64 lowercase alphanumeric chars and hyphens; must start and end with alphanumeric.", s),
        }))),
        Err(CorporaError::Db(rusqlite::Error::SqliteFailure(err, _)))
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Ok(HttpResponse::Conflict().json(json!({
                "status": "error",
                "message": format!("Corpus '{}' already exists", body.slug),
            })))
        }
        Err(e) => Ok(HttpResponse::InternalServerError()
            .json(json!({ "status": "error", "message": e.to_string() }))),
    }
}

pub async fn list_corpora_handler(config: web::Data<ApiConfig>) -> Result<HttpResponse, Error> {
    let conn = open_db(&config)?;
    let corpora = corpora::list_corpora(&conn)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    // Attach file counts from upload dirs.
    let items: Vec<serde_json::Value> = corpora
        .into_iter()
        .map(|c| {
            let dir = config.path_manager.corpus_upload_dir(&c.slug);
            let doc_count = std::fs::read_dir(&dir)
                .map(|entries| entries.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).count())
                .unwrap_or(0);
            json!({ "id": c.id, "slug": c.slug, "name": c.name, "created_at": c.created_at, "doc_count": doc_count })
        })
        .collect();
    Ok(HttpResponse::Ok().json(json!({ "status": "ok", "corpora": items, "count": items.len() })))
}

pub async fn get_corpus_handler(
    config: web::Data<ApiConfig>,
    slug: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();
    let conn = open_db(&config)?;
    match corpora::get_corpus_by_slug(&conn, &slug)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
    {
        None => Ok(corpus_not_found(&slug)),
        Some(c) => {
            let dir = config.path_manager.corpus_upload_dir(&c.slug);
            let doc_count = std::fs::read_dir(&dir)
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_file())
                        .count()
                })
                .unwrap_or(0);
            Ok(HttpResponse::Ok().json(json!({
                "status": "ok",
                "corpus": { "id": c.id, "slug": c.slug, "name": c.name, "created_at": c.created_at, "doc_count": doc_count }
            })))
        }
    }
}

pub async fn rename_corpus_handler(
    config: web::Data<ApiConfig>,
    slug: web::Path<String>,
    body: web::Json<RenameCorpusBody>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();
    let conn = open_db(&config)?;
    match corpora::rename_corpus(&conn, &slug, &body.name) {
        Ok(()) => {
            Ok(HttpResponse::Ok().json(json!({ "status": "ok", "slug": slug, "name": body.name })))
        }
        Err(CorporaError::NotFound(_)) => Ok(corpus_not_found(&slug)),
        Err(e) => Ok(HttpResponse::InternalServerError()
            .json(json!({ "status": "error", "message": e.to_string() }))),
    }
}

pub async fn delete_corpus_handler(
    config: web::Data<ApiConfig>,
    slug: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();
    if slug == "default" {
        return Ok(HttpResponse::BadRequest()
            .json(json!({ "status": "error", "message": "Cannot delete the default corpus" })));
    }
    let conn = open_db(&config)?;
    match corpora::delete_corpus(&conn, &slug) {
        Ok(()) => {
            // Remove from registry (drops the in-memory retriever).
            if let Some(reg) = crate::corpus_registry::get_registry() {
                reg.remove(&slug);
            }
            // Delete upload dir (best-effort).
            let upload_dir = config.path_manager.corpus_upload_dir(&slug);
            let _ = std::fs::remove_dir_all(&upload_dir);
            // Delete Tantivy index dir (best-effort).
            let index_dir = config.path_manager.corpus_index_dir(&slug);
            let _ = std::fs::remove_dir_all(&index_dir);
            Ok(HttpResponse::Ok().json(json!({ "status": "deleted", "slug": slug })))
        }
        Err(CorporaError::NotFound(_)) => Ok(corpus_not_found(&slug)),
        Err(e) => Ok(HttpResponse::InternalServerError()
            .json(json!({ "status": "error", "message": e.to_string() }))),
    }
}

// ─── Corpus settings ──────────────────────────────────────────────────────────

pub async fn get_corpus_settings_handler(
    config: web::Data<ApiConfig>,
    slug: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();
    let conn = open_db(&config)?;
    match corpora::get_corpus_by_slug(&conn, &slug)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
    {
        None => Ok(corpus_not_found(&slug)),
        Some(_) => {
            let settings = corpora::get_corpus_settings(&conn, &slug)
                .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
            let build_meta = corpora::get_corpus_build_meta(&conn, &slug).unwrap_or_default();
            Ok(HttpResponse::Ok().json(json!({
                "status": "ok",
                "slug": slug,
                "settings": settings,
                "build_meta": build_meta,
            })))
        }
    }
}

pub async fn patch_corpus_settings_handler(
    config: web::Data<ApiConfig>,
    slug: web::Path<String>,
    body: web::Json<CorpusSettings>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();
    let conn = open_db(&config)?;
    match corpora::get_corpus_by_slug(&conn, &slug)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
    {
        None => Ok(corpus_not_found(&slug)),
        Some(_) => {
            corpora::set_corpus_settings(&conn, &slug, &body)
                .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
            // Apply scalar settings immediately to the live retriever.
            if let Some(handle) = get_corpus_retriever(&slug) {
                if let Ok(mut ret) = handle.lock() {
                    if let Some(top_k) = body.search_top_k {
                        ret.set_search_top_k(top_k);
                    }
                    if let Some(metric) = body.distance_metric {
                        ret.distance_metric = metric;
                    }
                    if let Some(ef_c) = body.hnsw_ef_construction {
                        ret.hnsw_ef_construction = ef_c;
                    }
                    if let Some(ef_s) = body.hnsw_ef_search {
                        ret.hnsw_ef_search = ef_s;
                    }
                    if let Some(pq) = body.pq_subvectors {
                        ret.pq_subvectors = pq;
                    }
                }
            }
            Ok(
                HttpResponse::Ok()
                    .json(json!({ "status": "ok", "slug": slug, "settings": &*body })),
            )
        }
    }
}

// ─── Corpus-scoped document operations ───────────────────────────────────────

pub async fn corpus_list_documents_handler(
    config: web::Data<ApiConfig>,
    slug: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();
    let upload_dir = config.path_manager.corpus_upload_dir(&slug);
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&upload_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    files.push(name.to_string());
                }
            }
        }
    }
    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "corpus": slug,
        "documents": files,
        "count": files.len(),
    })))
}

pub async fn corpus_delete_document_handler(
    config: web::Data<ApiConfig>,
    params: web::Path<(String, String)>,
) -> Result<HttpResponse, Error> {
    let (slug, filename) = params.into_inner();
    let upload_dir = config.path_manager.corpus_upload_dir(&slug);
    let filepath = upload_dir.join(&filename);
    match std::fs::remove_file(&filepath) {
        Ok(()) => {
            if let Some(handle) = get_corpus_retriever(&slug) {
                if let Ok(mut ret) = handle.lock() {
                    if let Err(e) = ret.delete_document_by_filename(&filename) {
                        tracing::warn!(error = %e, filename, "Failed to delete corpus document chunks from index");
                    }
                }
            }
            Ok(HttpResponse::Ok()
                .json(json!({ "status": "ok", "deleted": filename, "corpus": slug })))
        }
        Err(_) => Ok(HttpResponse::NotFound()
            .json(json!({ "status": "error", "message": "File not found" }))),
    }
}

// ─── Corpus-scoped upload ─────────────────────────────────────────────────────

pub(crate) async fn corpus_upload_handler(
    mut payload: actix_multipart::Multipart,
    config: web::Data<ApiConfig>,
    slug: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();
    let upload_dir = config.path_manager.corpus_upload_dir(&slug);
    let upload_dir_str = upload_dir.to_string_lossy().to_string();

    let mut uploaded_files: Vec<String> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field = item?;
        let filename = field
            .content_disposition()
            .as_ref()
            .and_then(|cd| cd.get_filename())
            .ok_or_else(|| actix_web::error::ErrorBadRequest("No filename"))?
            .to_string();

        let filepath = upload_dir.join(&filename);
        let mut f = web::block(move || std::fs::File::create(&filepath)).await??;
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            f = web::block(move || f.write_all(&data).map(|_| f)).await??;
        }
        uploaded_files.push(filename);
    }

    let mut indexed_files = Vec::new();
    let mut index_errors = Vec::new();

    if !uploaded_files.is_empty() {
        let retriever_handle = if let Some(reg) = crate::corpus_registry::get_registry() {
            reg.get_or_create(&slug).ok()
        } else {
            get_corpus_retriever(&slug)
        };

        if let Some(handle) = retriever_handle {
            // Use per-corpus chunker_mode if set, fall back to global.
            let effective_chunker_mode = {
                let conn = open_db(&config).ok();
                conn.as_ref()
                    .and_then(|c| crate::db::corpora::get_corpus_settings(c, &slug).ok())
                    .and_then(|s| s.chunker_mode)
                    .and_then(|m| m.parse::<crate::config::ChunkerMode>().ok())
                    .unwrap_or(config.chunker_mode)
            };
            let chunker = crate::index::default_chunker(effective_chunker_mode);
            let mut file_contents = Vec::new();
            for filename in &uploaded_files {
                let path = upload_dir.join(filename);
                let ir = crate::index::extract_ir_async(&path).await;
                file_contents.push((filename.clone(), path, ir));
            }
            match handle.lock() {
                Ok(mut retriever) => {
                    for (filename, path, ir_opt) in file_contents {
                        match ir_opt {
                            Some(ir) => match crate::index::index_content_with_graph(
                                &mut *retriever,
                                &path,
                                &ir,
                                effective_chunker_mode,
                                chunker.as_ref(),
                                &slug,
                            ) {
                                Ok((n, _)) if n > 0 => indexed_files
                                    .push(json!({ "file": filename, "chunks_indexed": n })),
                                Ok(_) => index_errors.push(json!({
                                    "file": filename,
                                    "error": "Extraction returned no text — file is absent from the search index. Check the Parser tile: if status is 'empty', the PDF may be image-only (install pdftotext / tesseract) or use the Docling sidecar.",
                                })),
                                Err(e) => {
                                    index_errors.push(json!({ "file": filename, "error": e }))
                                }
                            },
                            None => index_errors.push(json!({
                                "file": filename,
                                "error": "Failed to extract text",
                            })),
                        }
                    }
                    let _ = retriever.commit();
                }
                Err(_) => {
                    index_errors.push(json!({ "file": null, "error": "Failed to lock retriever" }))
                }
            }
        } else {
            index_errors.push(
                json!({ "file": null, "error": format!("No retriever for corpus '{}'", slug) }),
            );
        }
    }

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "corpus": slug,
        "upload_dir": upload_dir_str,
        "uploaded_files": uploaded_files,
        "indexed_files": indexed_files,
        "index_errors": index_errors,
    })))
}

// ─── Corpus-scoped search ─────────────────────────────────────────────────────

pub async fn corpus_search_handler(
    query: web::Query<SearchQuery>,
    slug: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();
    let handle = match get_corpus_retriever(&slug) {
        Some(h) => h,
        None => {
            return Ok(HttpResponse::NotFound().json(json!({
                "status": "error",
                "message": format!("No retriever for corpus '{}'. Create the corpus first.", slug),
            })));
        }
    };

    let embed_q = crate::normalizer::normalize(&query.q, crate::normalizer::NormalizeTarget::Embed);
    let index_q = crate::normalizer::to_index(&embed_q);
    let query_vector = if let Some(svc) = get_embedding_service() {
        svc.embed_query(&embed_q).await
    } else {
        crate::embedder::embed(&embed_q)
    };

    let mut retriever = handle.lock().unwrap();
    let results = retriever
        .hybrid_search(&index_q, Some(&query_vector))
        .unwrap_or_default();

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "corpus": slug,
        "results": results,
    })))
}

// ─── Corpus-scoped reindex ────────────────────────────────────────────────────

pub async fn corpus_reindex_handler(
    config: web::Data<ApiConfig>,
    slug: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let slug = slug.into_inner();

    let handle = {
        let h = get_corpus_retriever(&slug);
        if h.is_some() {
            h
        } else {
            crate::corpus_registry::get_registry().and_then(|reg| reg.get_or_create(&slug).ok())
        }
    };
    let handle = match handle {
        Some(h) => h,
        None => {
            return Ok(HttpResponse::NotFound().json(json!({
                "status": "error",
                "message": format!("No retriever for corpus '{}'", slug),
            })));
        }
    };

    let upload_dir = config
        .path_manager
        .corpus_upload_dir(&slug)
        .to_string_lossy()
        .to_string();

    // Use per-corpus chunker_mode if set, fall back to global.
    let effective_chunker_mode = {
        let conn = open_db(&config).ok();
        conn.as_ref()
            .and_then(|c| crate::db::corpora::get_corpus_settings(c, &slug).ok())
            .and_then(|s| s.chunker_mode)
            .and_then(|m| m.parse::<crate::config::ChunkerMode>().ok())
            .unwrap_or(config.chunker_mode)
    };
    let chunker = crate::index::default_chunker(effective_chunker_mode);
    let result = match handle.lock() {
        Ok(mut retriever) => crate::index::index_all_documents(
            &mut *retriever,
            &upload_dir,
            effective_chunker_mode,
            chunker.as_ref(),
            &slug,
        ),
        Err(_) => Err("Failed to lock retriever".to_string()),
    };
    match result {
        Ok(()) => {
            // Record build metadata so the UI can detect settings drift.
            if let Ok(conn) = open_db(&config) {
                let settings = corpora::get_corpus_settings(&conn, &slug).unwrap_or_default();
                let meta = corpora::CorpusBuildMeta {
                    chunker_mode: settings
                        .chunker_mode
                        .or_else(|| Some(format!("{:?}", config.chunker_mode).to_lowercase())),
                    distance_metric: settings
                        .distance_metric
                        .map(|m| format!("{:?}", m).to_lowercase()),
                    hnsw_ef_construction: settings.hnsw_ef_construction.or(Some(100)),
                    hnsw_ef_search: settings.hnsw_ef_search.or(Some(100)),
                    pq_subvectors: settings.pq_subvectors.or(Some(48)),
                    built_at: Some(chrono::Utc::now().to_rfc3339()),
                };
                let _ = corpora::set_corpus_build_meta(&conn, &slug, &meta);
            }
            Ok(HttpResponse::Ok().json(json!({
                "status": "ok",
                "corpus": slug,
                "message": "Reindex complete",
            })))
        }
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(json!({ "status": "error", "message": e }))
        ),
    }
}

// ─── Agent memory settings ────────────────────────────────────────────────────

pub async fn get_agent_memory_settings_handler(
    config: web::Data<ApiConfig>,
) -> Result<HttpResponse, Error> {
    let conn = open_db(&config)?;
    let settings = corpora::get_agent_memory_settings(&conn);
    Ok(HttpResponse::Ok().json(json!({ "status": "ok", "settings": settings })))
}

pub async fn patch_agent_memory_settings_handler(
    config: web::Data<ApiConfig>,
    body: web::Json<AgentMemorySettings>,
) -> Result<HttpResponse, Error> {
    let conn = open_db(&config)?;
    corpora::set_agent_memory_settings(&conn, &body)
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let settings = corpora::get_agent_memory_settings(&conn);
    Ok(HttpResponse::Ok().json(json!({ "status": "ok", "settings": settings })))
}
