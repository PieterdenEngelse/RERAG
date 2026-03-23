// ~/ag/backend/src/api/memory_routes.rs  v1.0
// RAG memory and manual observation endpoints

use super::*;


#[derive(serde::Deserialize)]
pub struct StoreRagRequest {
    pub agent_id: String,
    pub memory_type: String,
    pub content: String,
    pub timestamp: Option<String>,
}



#[derive(serde::Deserialize)]
pub struct SearchRagRequest {
    pub agent_id: String,
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}



#[derive(serde::Deserialize)]
pub struct RecallRagRequest {
    pub agent_id: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}



#[derive(serde::Deserialize)]
pub struct DeleteRagRequest {
    pub agent_id: String,
    pub ids: Vec<i64>,
}



#[derive(serde::Deserialize)]
pub struct ManualObservationRequest {
    pub entry_type: String,
    pub title: String,
    pub narrative: String,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub concepts: Vec<String>,
    #[serde(default)]
    pub files_read: Vec<String>,
    #[serde(default)]
    pub files_modified: Vec<String>,
    pub author: Option<String>,
    pub project: Option<String>,
}



#[derive(serde::Deserialize)]
pub struct ManualObservationSearchRequest {
    pub query: String,
    pub entry_type: Option<String>,
    pub project: Option<String>,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    #[serde(default)]
    pub order: ManualObservationOrder,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}



#[derive(serde::Deserialize)]
pub struct ManualObservationListQuery {
    pub entry_type: Option<String>,
    pub project: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}



#[derive(serde::Deserialize)]
pub struct ManualObservationTimelineRequest {
    pub anchor_id: Option<String>,
    pub query: Option<String>,
    #[serde(default = "default_limit")]
    pub depth_before: usize,
    #[serde(default = "default_limit")]
    pub depth_after: usize,
    pub entry_type: Option<String>,
    pub project: Option<String>,
}



#[derive(serde::Deserialize)]
pub struct ManualObservationFetchRequest {
    pub ids: Vec<String>,
}



pub(crate) fn validate_memory_type(memory_type: &str) -> Result<(), Error> {
    if VALID_MEMORY_TYPES.contains(&memory_type) {
        Ok(())
    } else {
        Err(actix_web::error::ErrorBadRequest(format!(
            "Invalid memory_type '{}'. Valid types are: {}",
            memory_type,
            VALID_MEMORY_TYPES.join(", ")
        )))
    }
}



pub(crate) async fn list_memory_types() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(json!({
        "core": ["fact", "preference", "instruction", "context", "summary", "task"],
        "extended": ["conversation", "decision", "correction", "feedback", "persona", "note"],
        "all": VALID_MEMORY_TYPES,
        "request_id": generate_request_id()
    })))
}



pub(crate) async fn store_rag_memory(req: web::Json<StoreRagRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    validate_memory_type(&req.memory_type)?;
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let ts = req
        .timestamp
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    mem.store_rag(&req.agent_id, &req.memory_type, &req.content, &ts)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "request_id": request_id
    })))
}



pub(crate) async fn search_rag_memory(req: web::Json<SearchRagRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let results: Vec<MemorySearchResult> = mem
        .search_rag(&req.agent_id, &req.query, req.top_k)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "results": results,
        "request_id": request_id
    })))
}



pub(crate) async fn recall_rag_memory(req: web::Json<RecallRagRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let items: Vec<MemoryItem> = mem
        .recall_rag(&req.agent_id, req.limit)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "items": items,
        "request_id": request_id
    })))
}



pub(crate) async fn delete_rag_memory(req: web::Json<DeleteRagRequest>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let mut mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let deleted = mem
        .delete_rag_by_ids(&req.agent_id, &req.ids)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "deleted": deleted,
        "request_id": request_id
    })))
}



pub(crate) fn validate_manual_observation(req: &ManualObservationRequest) -> Result<(), Error> {
    if req.title.trim().is_empty() || req.title.len() > 200 {
        return Err(actix_web::error::ErrorBadRequest(
            "title must be 1-200 characters",
        ));
    }
    if req.entry_type.trim().is_empty() || req.entry_type.len() > 100 {
        return Err(actix_web::error::ErrorBadRequest(
            "entry_type must be 1-100 characters",
        ));
    }
    if req.narrative.trim().is_empty() || req.narrative.len() > 10_000 {
        return Err(actix_web::error::ErrorBadRequest(
            "narrative must be 1-10000 characters",
        ));
    }
    if req.facts.len() > 32 || req.concepts.len() > 32 {
        return Err(actix_web::error::ErrorBadRequest(
            "facts/concepts limit is 32 items",
        ));
    }
    if req.files_read.len() > 32 || req.files_modified.len() > 32 {
        return Err(actix_web::error::ErrorBadRequest(
            "files_read/files_modified limit is 32 items",
        ));
    }
    Ok(())
}



pub(crate) async fn create_manual_observation(
    req: web::Json<ManualObservationRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    validate_manual_observation(&req)?;
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    let start = std::time::Instant::now();
    let result = (|| {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let id = mem
            .create_manual_observation(
                &req.entry_type,
                &req.title,
                &req.narrative,
                &req.facts,
                &req.concepts,
                &req.files_read,
                &req.files_modified,
                req.author.as_deref(),
                req.project.as_deref(),
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "id": id,
            "request_id": request_id
        })))
    })();
    crate::monitoring::metrics::record_manual_observation(
        "create",
        result.is_ok(),
        start.elapsed().as_secs_f64() * 1000.0,
    );
    result
}



pub(crate) async fn list_manual_observations(
    query: web::Query<ManualObservationListQuery>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_manual_endpoint("list", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let results = mem
            .list_manual_observations(
                query.entry_type.as_deref(),
                query.project.as_deref(),
                query.limit,
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "observations": results,
            "request_id": request_id
        })))
    })
}



pub(crate) async fn get_manual_observation(
    path: web::Path<String>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_manual_endpoint("get", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        match mem
            .get_manual_observation(&path)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
        {
            Some(obs) => Ok(HttpResponse::Ok().json(json!({
                "observation": obs,
                "request_id": request_id
            }))),
            None => Ok(HttpResponse::NotFound().json(json!({
                "error": "not_found",
                "request_id": request_id
            }))),
        }
    })
}



pub(crate) async fn update_manual_observation(
    path: web::Path<String>,
    req: web::Json<ManualObservationRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    validate_manual_observation(&req)?;
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_manual_endpoint("update", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let updated = mem
            .update_manual_observation(
                &path,
                &req.entry_type,
                &req.title,
                &req.narrative,
                &req.facts,
                &req.concepts,
                &req.files_read,
                &req.files_modified,
                req.author.as_deref(),
                req.project.as_deref(),
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        if updated {
            Ok(HttpResponse::Ok().json(json!({
                "status": "updated",
                "request_id": request_id
            })))
        } else {
            Ok(HttpResponse::NotFound().json(json!({
                "error": "not_found",
                "request_id": request_id
            })))
        }
    })
}



pub(crate) async fn delete_manual_observation(
    path: web::Path<String>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_manual_endpoint("delete", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let deleted = mem
            .delete_manual_observation(&path)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        if deleted {
            Ok(HttpResponse::Ok().json(json!({
                "status": "deleted",
                "request_id": request_id
            })))
        } else {
            Ok(HttpResponse::NotFound().json(json!({
                "error": "not_found",
                "request_id": request_id
            })))
        }
    })
}



pub(crate) async fn manual_observation_timeline(
    req: web::Json<ManualObservationTimelineRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_memory_search_layer("timeline", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let results = mem
            .timeline_manual_observations(
                req.anchor_id.as_deref(),
                req.query.as_deref(),
                req.entry_type.as_deref(),
                req.project.as_deref(),
                req.depth_before,
                req.depth_after,
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "timeline": results,
            "request_id": request_id
        })))
    })
}



pub(crate) async fn fetch_manual_observations(
    req: web::Json<ManualObservationFetchRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    if req.ids.is_empty() {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "empty_ids",
            "request_id": request_id
        })));
    }
    if req.ids.len() > 20 {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "too_many_ids",
            "request_id": request_id
        })));
    }
    observe_memory_search_layer("fetch", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let observations = mem
            .fetch_manual_observations(&req.ids)
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "observations": observations,
            "request_id": request_id
        })))
    })
}



pub(crate) async fn search_manual_observations(
    req: web::Json<ManualObservationSearchRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let config = http_req
        .app_data::<web::Data<ApiConfig>>()
        .map(|c| c.get_ref())
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("missing config"))?;
    require_admin(&http_req, config)?;
    observe_memory_search_layer("search", || {
        let mem = AgentMemory::new(path_resolver::agent_db_path_str())
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        let hits = mem
            .search_manual_observations(
                req.query.as_str(),
                req.entry_type.as_deref(),
                req.project.as_deref(),
                req.date_start.as_deref(),
                req.date_end.as_deref(),
                req.order,
                req.limit,
                req.offset,
            )
            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
        Ok(HttpResponse::Ok().json(json!({
            "results": hits,
            "offset": req.offset,
            "limit": req.limit,
            "request_id": request_id
        })))
    })
}



pub(crate) async fn get_manual_observation_metrics(_http_req: HttpRequest) -> Result<HttpResponse, Error> {
    // No admin auth required - this is read-only monitoring data
    let snapshot = metrics::manual_observation_metrics_snapshot();
    Ok(HttpResponse::Ok().json(json!({
        "metrics": snapshot,
        "request_id": generate_request_id()
    })))
}



/// GET /monitoring/memory/search/stats - 3-layer memory search metrics (SEARCH.md)
pub(crate) async fn get_memory_search_layer_stats(_http_req: HttpRequest) -> Result<HttpResponse, Error> {
    // No admin auth required - this is read-only monitoring data
    let layer_stats = metrics::memory_search_layer_stats();
    let tokens_saved = metrics::memory_search_tokens_saved_total();
    Ok(HttpResponse::Ok().json(json!({
        "layers": layer_stats,
        "tokens_saved_total": tokens_saved,
        "request_id": generate_request_id()
    })))
}



pub(crate) async fn get_recent_observations(
    query: web::Query<ManualObservationListQuery>,
) -> Result<HttpResponse, Error> {
    // No admin auth required - this is read-only monitoring data
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let results = mem
        .list_manual_observations(
            query.entry_type.as_deref(),
            query.project.as_deref(),
            query.limit,
        )
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "observations": results,
        "request_id": generate_request_id()
    })))
}



#[derive(serde::Deserialize)]
pub(crate) struct RagMemoriesQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub agent_id: Option<String>,
}



pub(crate) async fn get_recent_rag_memories(
    query: web::Query<RagMemoriesQuery>,
) -> Result<HttpResponse, Error> {
    // No admin auth required - this is read-only monitoring data
    let mem = AgentMemory::new(path_resolver::agent_db_path_str())
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    let agent_id = query.agent_id.as_deref().unwrap_or("default");
    let items: Vec<MemoryItem> = mem
        .recall_rag(agent_id, query.limit)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    Ok(HttpResponse::Ok().json(json!({
        "memories": items,
        "request_id": generate_request_id()
    })))
}


