// ~/ag/backend/src/api/monitor_routes.rs  v1.0
// Monitoring, health, metrics, cache, rate limiting, logs endpoints

use super::*;

#[derive(Clone)]
pub(crate) struct RateLimitSharedState {
    pub limiter: Arc<RateLimiter>,
    pub opts: RateLimitOptions,
}

#[derive(Serialize)]
pub(crate) struct L1CacheSnapshot {
    pub enabled: bool,
    pub total_searches: u64,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[derive(Serialize)]
pub(crate) struct L2CacheSnapshot {
    pub enabled: bool,
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub total_items: u64,
    pub hit_rate: f64,
}

#[derive(Serialize)]
pub(crate) struct CacheCountersSnapshot {
    pub hits_total: i64,
    pub misses_total: i64,
}

#[derive(Serialize)]
pub(crate) struct CacheMonitorResponse {
    pub request_id: String,
    pub l1: L1CacheSnapshot,
    pub l2: L2CacheSnapshot,
    pub redis: crate::cache::redis_cache::RedisCacheSummary,
    pub counters: CacheCountersSnapshot,
}

#[derive(Serialize)]
pub(crate) struct RouteDropStat {
    pub route: String,
    pub drops: i64,
}

#[derive(Serialize)]
pub(crate) struct ServerRouteDrop {
    pub server: String,
    pub route: String,
    pub drops: i64,
}

#[derive(Serialize)]
pub(crate) struct RateLimitConfigSnapshot {
    pub enabled: bool,
    pub trust_proxy: bool,
    pub search_qps: f64,
    pub search_burst: f64,
    pub upload_qps: f64,
    pub upload_burst: f64,
    pub exempt_prefixes: Vec<String>,
    pub rules: Vec<RouteRule>,
}

#[derive(Serialize)]
pub(crate) struct RateLimitMonitorResponse {
    pub request_id: String,
    pub total_drops: i64,
    pub drops_by_route: Vec<RouteDropStat>,
    pub drops_by_server_route: Vec<ServerRouteDrop>,
    pub config: RateLimitConfigSnapshot,
    pub upload_config: Option<RateLimitConfigSnapshot>,
    pub limiter_state: RateLimiterState,
}

#[derive(serde::Deserialize)]
pub(crate) struct LogsQuery {
    pub limit: Option<usize>,
}

#[derive(serde::Deserialize)]
pub(crate) struct ChunkingQuery {
    pub limit: Option<usize>,
    pub capacity: Option<usize>,
    pub corpus: Option<String>,
}

#[derive(serde::Deserialize)]
pub(crate) struct CorpusFilter {
    pub corpus: Option<String>,
}

#[derive(serde::Deserialize)]
pub(crate) struct LoggingQuery {
    pub enabled: Option<bool>,
}

#[derive(Serialize)]
pub(crate) struct LogEntry {
    pub timestamp: Option<String>,
    pub level: Option<String>,
    pub target: Option<String>,
    pub message: Option<String>,
    pub raw: String,
    pub fields: Option<Value>,
}

#[derive(Serialize)]
pub(crate) struct LogsResponse {
    pub request_id: String,
    pub file: Option<String>,
    pub entries: Vec<LogEntry>,
    pub note: Option<String>,
}

// Track previous health status for change detection

/// Get status log file content
pub async fn get_systemd_logs(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let unit = query
        .get("unit")
        .cloned()
        .unwrap_or_else(|| "ag.service".to_string());
    let limit = query
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(100);

    // Validate unit name to prevent injection
    if unit.contains("..") || unit.contains('/') || unit.contains(';') {
        return Ok(HttpResponse::BadRequest().json(json!({"error": "Invalid unit name"})));
    }

    let scope = query
        .get("scope")
        .cloned()
        .unwrap_or_else(|| "system".to_string());
    let mut args: Vec<String> = Vec::new();
    if scope == "user" {
        args.push("--user".to_string());
    }
    args.extend([
        "-u".to_string(),
        unit.clone(),
        "-n".to_string(),
        limit.to_string(),
        "--no-pager".to_string(),
        "--output=short-iso".to_string(),
    ]);

    let output = tokio::process::Command::new("journalctl")
        .args(&args)
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let content = if stdout.is_empty() { stderr } else { stdout };
            let lines: Vec<&str> = content.lines().collect();
            Ok(HttpResponse::Ok().json(json!({
                "unit": unit,
                "limit": limit,
                "total_lines": lines.len(),
                "content": content,
            })))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to run journalctl: {}", e)
        }))),
    }
}

pub async fn get_status_log(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let status = path.into_inner();

    // Validate status name to prevent path traversal
    let valid_statuses = [
        "healthy",
        "busy",
        "degraded",
        "unhealthy",
        "offline",
        "checking",
        "unknown",
        "initial",
    ];
    if !valid_statuses.contains(&status.as_str()) {
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "Invalid status name",
            "valid_statuses": valid_statuses
        })));
    }

    // Get log directory
    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/.agentic-rag/logs", home)
    });

    let filename = format!("status_{}.log", status);
    let log_path = format!("{}/{}", log_dir, filename);

    // Read log file
    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            // Return last 100 lines (most recent entries)
            let lines: Vec<&str> = content.lines().collect();
            let start = if lines.len() > 100 {
                lines.len() - 100
            } else {
                0
            };
            let recent_lines = lines[start..].join("\n");

            Ok(HttpResponse::Ok().json(json!({
                "status": status,
                "log_path": log_path,
                "total_lines": lines.len(),
                "showing_lines": lines.len() - start,
                "content": recent_lines
            })))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HttpResponse::Ok().json(json!({
            "status": status,
            "log_path": log_path,
            "total_lines": 0,
            "showing_lines": 0,
            "content": "",
            "message": "No log entries yet for this status"
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to read log file: {}", e),
            "log_path": log_path
        }))),
    }
}

pub async fn health_check() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // Check ONNX model exists
    let onnx_model_path = std::env::var("ONNX_MODEL_PATH")
        .unwrap_or_else(|_| "models/embedding_model.onnx".to_string());
    let onnx_ready = std::path::Path::new(&onnx_model_path).exists();

    if !onnx_ready {
        let reason = format!("ONNX model not found at: {}", onnx_model_path);
        log_status_change("unhealthy", &reason);
        return Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "unhealthy",
            "error": reason,
            "request_id": request_id
        })));
    }

    // Get load metrics from global health tracker
    let (load, is_busy, message) = if let Some(tracker) = crate::monitoring::get_health_tracker() {
        let load = tracker.get_load_metrics();
        let is_busy = tracker.is_busy();
        let message = if is_busy {
            Some(format!(
                "System busy: {} active tasks{}{}",
                load.active_tasks,
                if load.indexing { ", indexing" } else { "" },
                if load.llm_active {
                    ", LLM processing"
                } else {
                    ""
                }
            ))
        } else {
            None
        };
        (Some(load), is_busy, message)
    } else {
        (None, false, None)
    };

    // Check FalkorDB status if enabled
    #[cfg(feature = "graph")]
    let graph_status: Option<(bool, bool)> = {
        let config = crate::graph::config::GraphConfig::from_env();
        if config.enabled {
            if let Some(client) = get_graph_client() {
                match client.health_check().await {
                    Ok(connected) => Some((true, connected)),
                    Err(_) => Some((true, false)),
                }
            } else {
                Some((true, false)) // Enabled but client not initialized
            }
        } else {
            None // Disabled
        }
    };

    #[cfg(not(feature = "graph"))]
    let graph_status: Option<(bool, bool)> = None;

    if let Some(retriever) = RETRIEVER.get() {
        let retriever = match retriever.try_lock() {
            Ok(r) => r,
            Err(_) => {
                // Retriever mutex is held by an indexing task — respond immediately
                // rather than blocking for potentially minutes.
                let msg = message.unwrap_or_else(|| "Indexing in progress".to_string());
                log_status_change("busy", &msg);
                let mut response = json!({
                    "status": "busy",
                    "message": msg,
                    "request_id": request_id
                });
                if let Some(l) = load {
                    response["load"] = json!({
                        "cpu_percent": l.cpu_percent,
                        "memory_percent": l.memory_percent,
                        "active_tasks": l.active_tasks,
                        "queue_depth": l.queue_depth,
                        "indexing": l.indexing,
                        "llm_active": l.llm_active
                    });
                }
                return Ok(HttpResponse::Ok().json(response));
            }
        };
        match retriever.health_check() {
            Ok(()) => {
                // FalkorDB is ingestion-only — not running is normal.
                // Still report its status in the response, but never downgrade health.

                // Check if Redis is enabled but backend not connected
                // We check the env config directly because if connection failed at startup,
                // l3_cache is None and summary() returns enabled=false, masking the issue.
                let redis_configured = std::env::var("REDIS_ENABLED")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(false);
                let redis_summary = retriever.get_l3_cache_summary();
                let redis_issue = redis_configured && !redis_summary.connected;

                let status = if redis_issue {
                    "degraded"
                } else if is_busy {
                    "busy"
                } else {
                    "healthy"
                };

                let reason = if redis_issue {
                    "Redis enabled but backend not connected".to_string()
                } else if is_busy {
                    message.clone().unwrap_or_else(|| "System busy".to_string())
                } else {
                    format!(
                        "All systems operational ({} docs, {} vectors)",
                        retriever.metrics.total_documents_indexed, retriever.metrics.total_vectors
                    )
                };
                log_status_change(status, &reason);

                let mut response = json!({
                    "status": status,
                    "server": "search",
                    "documents": retriever.metrics.total_documents_indexed,
                    "vectors": retriever.metrics.total_vectors,
                    "index_path": retriever.metrics.index_path,
                    "request_id": request_id
                });

                // Add load metrics if available
                if let Some(load) = load {
                    response["load"] = json!({
                        "cpu_percent": load.cpu_percent,
                        "memory_percent": load.memory_percent,
                        "active_tasks": load.active_tasks,
                        "queue_depth": load.queue_depth,
                        "indexing": load.indexing,
                        "llm_active": load.llm_active
                    });
                }

                // Add message if busy or degraded
                if redis_issue {
                    response["message"] = json!(reason);
                } else if let Some(msg) = message {
                    response["message"] = json!(msg);
                }

                // Add FalkorDB status
                if let Some((enabled, connected)) = graph_status {
                    response["graph"] = json!({
                        "enabled": enabled,
                        "connected": connected
                    });
                }

                Ok(HttpResponse::Ok().json(response))
            }
            Err(e) => {
                let reason = format!("Retriever health check failed: {}", e);
                log_status_change("unhealthy", &reason);
                error!("[{}] {}", request_id, reason);
                Ok(HttpResponse::ServiceUnavailable().json(json!({
                    "status": "unhealthy",
                    "error": e.to_string(),
                    "request_id": request_id
                })))
            }
        }
    } else {
        let reason = "Retriever not initialized";
        log_status_change("unhealthy", reason);
        error!("[{}] Health check failed: {}", request_id, reason);
        Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "unhealthy",
            "error": reason,
            "request_id": request_id
        })))
    }
}

pub async fn upload_health_check() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    // ONNX pool must be initialized before any uploads can embed
    if !crate::embedder::upload_pool_ready() {
        return Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "unhealthy",
            "server": "upload",
            "error": "Upload ONNX blocking pool not initialized",
            "request_id": request_id
        })));
    }

    let (load, is_busy, message) = if let Some(tracker) = crate::monitoring::get_health_tracker() {
        let load = tracker.get_load_metrics();
        let is_busy = tracker.is_busy();
        let msg = if is_busy {
            Some(format!(
                "System busy: {} active tasks{}",
                load.active_tasks,
                if load.indexing { ", indexing" } else { "" }
            ))
        } else {
            None
        };
        (Some(load), is_busy, msg)
    } else {
        (None, false, None)
    };

    if let Some(retriever) = RETRIEVER.get() {
        let (status, detail) = match retriever.try_lock() {
            Ok(_) => {
                if is_busy {
                    ("busy", message.unwrap_or_else(|| "System busy".to_string()))
                } else {
                    ("healthy", "Upload server ready".to_string())
                }
            }
            Err(_) => ("busy", "Indexing in progress".to_string()),
        };

        let mut response = json!({
            "status": status,
            "server": "upload",
            "message": detail,
            "onnx_pool": "ready",
            "request_id": request_id
        });
        if let Some(l) = load {
            response["load"] = json!({
                "cpu_percent": l.cpu_percent,
                "memory_percent": l.memory_percent,
                "active_tasks": l.active_tasks,
                "queue_depth": l.queue_depth,
                "indexing": l.indexing,
                "llm_active": l.llm_active
            });
        }
        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "unhealthy",
            "server": "upload",
            "error": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub(crate) async fn root_handler() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body("✅ Backend is running (Actix Web)\n\nTry /health or /ready\n"))
}

pub(crate) async fn ready_check() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    if let Some(retriever) = RETRIEVER.get() {
        match retriever.try_lock() {
            Ok(retriever) => match retriever.ready_check() {
                Ok(_) => Ok(HttpResponse::Ok().json(json!({
                    "status": "ready",
                    "server": "search",
                    "timestamp": Utc::now().to_rfc3339(),
                    "request_id": request_id
                }))),
                Err(e) => Ok(HttpResponse::ServiceUnavailable().json(json!({
                    "status": "not ready",
                    "server": "search",
                    "error": e.to_string(),
                    "timestamp": Utc::now().to_rfc3339(),
                    "request_id": request_id
                }))),
            },
            Err(_) => Ok(HttpResponse::ServiceUnavailable().json(json!({
                "status": "not ready",
                "server": "search",
                "error": "Retriever locked — indexing in progress",
                "timestamp": Utc::now().to_rfc3339(),
                "request_id": request_id
            }))),
        }
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "not ready",
            "server": "search",
            "message": "Retriever not initialized",
            "timestamp": Utc::now().to_rfc3339(),
            "request_id": request_id
        })))
    }
}

pub(crate) async fn upload_ready_check() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if !crate::embedder::upload_pool_ready() {
        return Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "not ready",
            "server": "upload",
            "reason": "onnx_pool_not_initialized",
            "timestamp": Utc::now().to_rfc3339(),
            "request_id": request_id
        })));
    }

    if RETRIEVER.get().is_none() {
        return Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "not ready",
            "server": "upload",
            "reason": "retriever_not_initialized",
            "timestamp": Utc::now().to_rfc3339(),
            "request_id": request_id
        })));
    }

    let available_slots = crate::api::get_upload_semaphore()
        .map(|s| s.available_permits())
        .unwrap_or(0);

    if available_slots == 0 {
        return Ok(HttpResponse::ServiceUnavailable().json(json!({
            "status": "not ready",
            "server": "upload",
            "reason": "at_capacity",
            "available_slots": 0,
            "timestamp": Utc::now().to_rfc3339(),
            "request_id": request_id
        })));
    }

    Ok(HttpResponse::Ok().json(json!({
        "status": "ready",
        "server": "upload",
        "available_slots": available_slots,
        "timestamp": Utc::now().to_rfc3339(),
        "request_id": request_id
    })))
}

/// Phase 16: Export metrics in Prometheus text format
/// GET /monitoring/metrics
/// Returns: Prometheus-compliant text format metrics
pub(crate) async fn get_metrics() -> Result<HttpResponse, Error> {
    // Export metrics in Prometheus text format (not JSON)
    // Phase 16 Step 3: OTLP Exporting - Prometheus format compliance
    let prometheus_text = crate::monitoring::metrics::export_prometheus();

    Ok(HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(prometheus_text))
}

/// GET /monitoring/optimizations
/// Returns: Statistics about all performance optimizations
pub(crate) async fn get_optimization_stats() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let retriever = retriever.lock().unwrap();
        let stats = retriever.get_optimization_stats();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "optimizations": stats,
            "modules": {
                "simd": "4-8x faster cosine similarity",
                "bloom_filter": "O(1) document existence checks",
                "hnsw_index": "O(log n) approximate nearest neighbor",
                "semantic_cache": "Cache similar queries",
                "hybrid_search": "BM25 + vector fusion",
                "sqlite_wal": "10-100x faster concurrent writes",
                "mmap": "Zero-copy vector loading",
                "rkyv": "20-40x faster serialization",
                "lz4": "2x compression for vectors",
            }
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// GET /monitoring/io-uring
/// Returns: io_uring async I/O statistics, configuration, and availability
pub(crate) async fn get_io_uring_stats() -> Result<HttpResponse, Error> {
    use crate::perf::io_uring as async_io;

    let request_id = generate_request_id();
    let stats = async_io::get_stats();
    let config = async_io::get_config();

    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "request_id": request_id,
        "io_uring": {
            "available": async_io::is_available(),
            "feature_enabled": async_io::is_feature_enabled(),
            "backend": async_io::backend_name(),
            "config": {
                // Category 1: Queue & Buffers
                "ring_size": config.ring_size,
                "cq_size": config.cq_size,
                "buffer_size": config.buffer_size,
                "buffer_pool_size": config.buffer_pool_size,
                "clamp": config.clamp,
                // Category 2: Polling
                "sqpoll": config.sqpoll,
                "sqpoll_idle_ms": config.sqpoll_idle_ms,
                "sqpoll_cpu": config.sqpoll_cpu,
                "iopoll": config.iopoll,
                // Category 3: Optimization
                "single_issuer": config.single_issuer,
                "coop_taskrun": config.coop_taskrun,
                "defer_taskrun": config.defer_taskrun,
                "submit_all": config.submit_all,
                "taskrun_flag": config.taskrun_flag,
                // Category 4: Advanced
                "r_disabled": config.r_disabled,
                "attach_wq_fd": config.attach_wq_fd,
                "dontfork": config.dontfork
            },
            "stats": {
                "reads": stats.get_reads(),
                "writes": stats.get_writes(),
                "bytes_read": stats.get_bytes_read(),
                "bytes_written": stats.get_bytes_written(),
                "read_errors": stats.get_read_errors(),
                "write_errors": stats.get_write_errors(),
                "total_errors": stats.get_total_errors()
            },
            "env_vars": {
                "IO_URING_RING_SIZE": "Submission/completion queue size (1-32768, power of 2)",
                "IO_URING_BUFFER_SIZE": "Read/write buffer size in bytes (4096-16MB)",
                "IO_URING_SQPOLL": "Enable kernel SQ polling thread (true/false)",
                "IO_URING_SQPOLL_IDLE_MS": "SQ poll thread idle timeout in ms",
                "IO_URING_BUFFER_POOL_SIZE": "Number of pre-allocated buffers (1-4096)",
                "IO_URING_SINGLE_ISSUER": "Single issuer optimization (true/false)"
            },
            "startup": async_io::get_startup_io(),
            "description": "io_uring provides 2-3x faster file I/O on Linux 5.1+",
            "available_functions": {
                "vector_loading": "load_vectors_rkyv_async() / load_vectors_auto_async()",
                "cache_loading": "load_search_cache_async()",
                "document_ingestion": "index_file_async() / extract_text_async()",
                "file_read": "perf::io_uring::read_file()",
                "file_write": "perf::io_uring::write_file()",
                "batch_read": "perf::io_uring::read_files()"
            },
            "current_usage": {
                "startup_vector_load": "io_uring bulk read (mmap fallback)",
                "upload_indexing": "io_uring via extract_text_async()",
                "reindex": "io_uring via index_all_documents_async()",
                "note": "All file reads now use io_uring on Linux 5.1+ for 2-3x speedup"
            }
        }
    })))
}

/// POST /monitoring/log-frontend-error
/// Log frontend errors so they appear in the log viewer
/// This allows page errors to be visible when filtering logs by color (red for errors)
pub(crate) async fn log_frontend_error(
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, Error> {
    let page = body
        .get("page")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let error = body
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown error");
    let level = body
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("error");

    // Log at the appropriate level so it appears in log filtering
    match level {
        "warn" | "warning" => {
            tracing::warn!(page = %page, "Frontend error: {}", error);
        }
        _ => {
            tracing::error!(page = %page, "Frontend error: {}", error);
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "logged",
        "page": page,
        "level": level
    })))
}

/// POST /monitoring/optimizations/build-hnsw
/// Build HNSW index for O(log n) search
pub(crate) async fn build_hnsw_index() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let start = std::time::Instant::now();
        retriever.build_hnsw_index();
        let elapsed = start.elapsed().as_millis();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "message": "HNSW index built",
            "index_size": retriever.hnsw_index_size(),
            "build_time_ms": elapsed
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// POST /monitoring/optimizations/build-pq
/// Build Product Quantization index for 16x memory reduction
pub(crate) async fn build_pq_index() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let start = std::time::Instant::now();
        retriever.build_pq_index();
        let elapsed = start.elapsed().as_millis();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "message": "PQ index built (16x compression)",
            "build_time_ms": elapsed
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// POST /monitoring/optimizations/build-fp16
/// Build FP16 vector store for 2x memory reduction
pub(crate) async fn build_fp16_store() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let start = std::time::Instant::now();
        retriever.build_fp16_store();
        let elapsed = start.elapsed().as_millis();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "message": "FP16 store built (2x compression)",
            "build_time_ms": elapsed
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

/// POST /monitoring/optimizations/build-all
/// Build all optimization indexes
pub(crate) async fn build_all_indexes() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    if let Some(retriever) = RETRIEVER.get() {
        let mut retriever = retriever.lock().unwrap();
        let start = std::time::Instant::now();

        retriever.build_hnsw_index();
        retriever.build_pq_index();
        retriever.build_fp16_store();

        let elapsed = start.elapsed().as_millis();
        let stats = retriever.get_optimization_stats();

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "request_id": request_id,
            "message": "All optimization indexes built",
            "build_time_ms": elapsed,
            "stats": stats
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Retriever not initialized",
            "request_id": request_id
        })))
    }
}

pub(crate) async fn get_cache_monitor_info() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let retriever = match RETRIEVER.get() {
        Some(handle) => handle,
        None => {
            return Ok(HttpResponse::ServiceUnavailable().json(json!({
                "status": "unavailable",
                "error": "Retriever not initialized",
                "request_id": request_id,
            })));
        }
    };

    let (metrics_snapshot, l2_stats, redis_summary, l1_enabled, l2_enabled) = {
        let guard = retriever.lock().unwrap();
        (
            guard.metrics.clone(),
            guard.get_l2_cache_stats(),
            guard.get_l3_cache_summary(),
            guard.l1_cache_enabled(),
            guard.l2_cache_enabled(),
        )
    };

    let l1_snapshot = L1CacheSnapshot {
        enabled: l1_enabled,
        total_searches: metrics_snapshot.total_searches as u64,
        hits: metrics_snapshot.cache_hits as u64,
        misses: metrics_snapshot.cache_misses as u64,
        hit_rate: metrics_snapshot.cache_hit_rate(),
    };
    let l2_snapshot = L2CacheSnapshot {
        enabled: l2_enabled,
        l1_hits: l2_stats.l1_hits,
        l1_misses: l2_stats.l1_misses,
        l2_hits: l2_stats.l2_hits,
        l2_misses: l2_stats.l2_misses,
        total_items: l2_stats.total_items as u64,
        hit_rate: l2_stats.hit_rate(),
    };
    let counters = metrics::cache_hit_miss_counts();
    let counters_snapshot = CacheCountersSnapshot {
        hits_total: counters.0,
        misses_total: counters.1,
    };

    let response = CacheMonitorResponse {
        request_id,
        l1: l1_snapshot,
        l2: l2_snapshot,
        redis: redis_summary,
        counters: counters_snapshot,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// POST /cache/clear
/// Clear all caches (L1, L2, and optionally L3/Redis)
pub(crate) async fn clear_cache() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();

    let retriever = match RETRIEVER.get() {
        Some(handle) => handle,
        None => {
            return Ok(HttpResponse::ServiceUnavailable().json(json!({
                "status": "error",
                "error": "Retriever not initialized",
                "request_id": request_id,
            })));
        }
    };

    // Clear caches
    {
        let mut guard = retriever.lock().unwrap();
        guard.clear_cache();
        guard.clear_l2_cache();
    }

    info!("[{}] Cache cleared via API", request_id);

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "message": "Cache cleared",
        "request_id": request_id,
    })))
}

pub(crate) async fn get_rate_limit_monitor_info(
    state: web::Data<RateLimitSharedState>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let limiter_state = state.limiter.snapshot();
    let total_drops = metrics::rate_limit_drop_total();
    let drops_by_route = metrics::rate_limit_drops_by_route_snapshot()
        .into_iter()
        .map(|(route, drops)| RouteDropStat { route, drops })
        .collect();
    let drops_by_server_route = metrics::rate_limit_drops_by_server_route_snapshot()
        .into_iter()
        .map(|(server, route, drops)| ServerRouteDrop {
            server,
            route,
            drops,
        })
        .collect();
    let config = state.config_snapshot(limiter_state.enabled);

    let upload_config = crate::api::get_upload_rate_limiter().map(|ul| {
        let upload_state = ul.snapshot();
        let t = ul.thresholds.snapshot();
        RateLimitConfigSnapshot {
            enabled: upload_state.enabled,
            trust_proxy: state.opts.trust_proxy, // populated from upload opts below
            search_qps: t.search_qps,
            search_burst: t.search_burst,
            upload_qps: t.upload_qps,
            upload_burst: t.upload_burst,
            exempt_prefixes: vec!["/monitoring".into()],
            rules: vec![],
        }
    });

    let response = RateLimitMonitorResponse {
        request_id,
        total_drops,
        drops_by_route,
        drops_by_server_route,
        config,
        upload_config,
        limiter_state,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Get inference gateway statistics
pub(crate) async fn get_inference_gateway_stats() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let stats = crate::inference_gateway::gateway_stats();

    // Also refresh the Prometheus gauges
    metrics::refresh_inference_gateway_gauges();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "request_id": request_id,
        "gateway": stats
    })))
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SetRateLimitEnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct SetRateLimitEnabledResponse {
    pub request_id: String,
    pub enabled: bool,
    pub message: String,
}

fn persist_rate_limits(
    enabled: bool,
    search_qps: f64,
    search_burst: f64,
    upload_qps: f64,
    upload_burst: f64,
) {
    let content = format!(
        "# Rate limit settings — written by UI\n\
         RATE_LIMIT_ENABLED={enabled}\n\
         RATE_LIMIT_SEARCH_QPS={search_qps}\n\
         RATE_LIMIT_SEARCH_BURST={}\n\
         RATE_LIMIT_UPLOAD_QPS={upload_qps}\n\
         RATE_LIMIT_UPLOAD_BURST={}\n",
        search_burst as u64, upload_burst as u64,
    );
    let _ = std::fs::write(".env.rate_limits", content);
}

pub(crate) async fn set_rate_limit_enabled(
    state: web::Data<RateLimitSharedState>,
    body: web::Json<SetRateLimitEnabledRequest>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let new_state = state.limiter.set_enabled(body.enabled);
    if let Some(ul) = crate::api::get_upload_rate_limiter() {
        ul.set_enabled(body.enabled);
    }
    let t = state.limiter.thresholds.snapshot();
    persist_rate_limits(
        new_state,
        t.search_qps,
        t.search_burst,
        t.upload_qps,
        t.upload_burst,
    );

    let message = if new_state {
        "Rate limiter enabled".to_string()
    } else {
        "Rate limiter disabled".to_string()
    };

    tracing::info!("[{}] Rate limiter set to: {}", request_id, new_state);

    Ok(HttpResponse::Ok().json(SetRateLimitEnabledResponse {
        request_id,
        enabled: new_state,
        message,
    }))
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UpdateThresholdsRequest {
    pub search_qps: Option<f64>,
    pub search_burst: Option<f64>,
    pub upload_qps: Option<f64>,
    pub upload_burst: Option<f64>,
}

pub(crate) async fn update_rate_limit_thresholds(
    state: web::Data<RateLimitSharedState>,
    body: web::Json<UpdateThresholdsRequest>,
) -> Result<HttpResponse, Error> {
    let current = state.limiter.thresholds.snapshot();
    let search_qps = body.search_qps.unwrap_or(current.search_qps).max(0.0);
    let search_burst = body.search_burst.unwrap_or(current.search_burst).max(0.0);
    let upload_qps = body.upload_qps.unwrap_or(current.upload_qps).max(0.0);
    let upload_burst = body.upload_burst.unwrap_or(current.upload_burst).max(0.0);

    state
        .limiter
        .thresholds
        .set(search_qps, search_burst, upload_qps, upload_burst);

    let enabled = state.limiter.is_enabled();
    persist_rate_limits(enabled, search_qps, search_burst, upload_qps, upload_burst);

    tracing::info!(
        search_qps,
        search_burst,
        upload_qps,
        upload_burst,
        "Rate limit thresholds updated"
    );
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "search_qps": search_qps,
        "search_burst": search_burst,
        "upload_qps": upload_qps,
        "upload_burst": upload_burst,
    })))
}

pub(crate) async fn get_recent_logs(query: web::Query<LogsQuery>) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let limit = query
        .limit
        .unwrap_or(DEFAULT_LOG_LIMIT)
        .clamp(1, MAX_LOG_LIMIT);
    let config = MonitoringConfig::from_env();
    let log_dir = config.log_dir;

    let file = latest_log_file(&log_dir);
    let (entries, note) = if let Some(path) = file.clone() {
        match read_recent_lines(&path, limit) {
            Ok(lines) => {
                let entries = lines
                    .into_iter()
                    .map(|line| parse_log_line(&line))
                    .collect();
                (entries, None)
            }
            Err(err) => {
                warn!(error = %err, path = %path.display(), "Failed to read logs");
                (Vec::new(), Some(format!("Failed to read logs: {}", err)))
            }
        }
    } else {
        (Vec::new(), Some("No backend log files found".to_string()))
    };

    let response = LogsResponse {
        request_id,
        file: file.and_then(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        }),
        entries,
        note,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// GET /monitoring/onnx
/// Returns ONNX embedding runtime statistics.
pub(crate) async fn get_onnx_status() -> Result<HttpResponse, Error> {
    let snap = crate::monitoring::onnx_metrics::snapshot();
    Ok(HttpResponse::Ok().json(snap))
}

/// GET /monitoring/ollama
/// Returns Ollama service status fetched directly from the Ollama API
pub(crate) async fn get_ollama_status() -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    // Check version
    let version_resp = client
        .get("http://localhost:11434/api/version")
        .send()
        .await;

    let available = version_resp
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    let version = if let Ok(resp) = version_resp {
        resp.json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v["version"].as_str().map(|s| s.to_string()))
    } else {
        None
    };

    // Get loaded/available models
    let tags_resp = client.get("http://localhost:11434/api/tags").send().await;

    let (loaded_model, model_count) = if let Ok(resp) = tags_resp {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            let models = json["models"].as_array();
            let count = models.map(|m| m.len()).unwrap_or(0);
            let first = models
                .and_then(|m| m.first())
                .and_then(|m| m["name"].as_str())
                .map(|s| s.to_string());
            (first, count)
        } else {
            (None, 0)
        }
    } else {
        (None, 0)
    };

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "available": available,
        "version": version,
        "loaded_model": loaded_model,
        "model_count": model_count
    })))
}

/// GET /monitoring/docker/inspect?name=<container>
#[allow(dead_code)]
pub(crate) async fn get_container_inspect(
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let request_id = generate_request_id();
    let name = match query.get("name") {
        Some(n) => n.clone(),
        None => return Ok(HttpResponse::BadRequest().json(json!({"error": "name is required"}))),
    };

    // docker inspect
    let inspect_out = tokio::process::Command::new("docker")
        .args(["inspect", "--format", "{{json .State}}", &name])
        .env("DOCKER_HOST", "unix:///var/run/docker.sock")
        .output()
        .await;

    let (restart_count, exit_code, started_at, finished_at) = if let Ok(out) = inspect_out {
        let text = String::from_utf8_lossy(&out.stdout);
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        (
            json["RestartCount"].as_u64().unwrap_or(0),
            json["ExitCode"].as_i64().unwrap_or(0),
            json["StartedAt"].as_str().unwrap_or("").to_string(),
            json["FinishedAt"].as_str().unwrap_or("").to_string(),
        )
    } else {
        (0, 0, String::new(), String::new())
    };

    // docker logs --tail 20
    let logs_out = tokio::process::Command::new("docker")
        .args(["logs", "--tail", "20", "--timestamps", &name])
        .env("DOCKER_HOST", "unix:///var/run/docker.sock")
        .output()
        .await;

    let logs = if let Ok(out) = logs_out {
        // docker logs writes to stderr by default
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if stderr.is_empty() {
            stdout
        } else {
            stderr
        }
    } else {
        "Failed to fetch logs".to_string()
    };

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "request_id": request_id,
        "name": name,
        "restart_count": restart_count,
        "exit_code": exit_code,
        "started_at": started_at,
        "finished_at": finished_at,
        "logs": logs
    })))
}

pub(crate) async fn get_parser_stats(
    query: actix_web::web::Query<CorpusFilter>,
) -> Result<HttpResponse, Error> {
    let corpus = query.corpus.as_deref().filter(|s| !s.is_empty());
    let stats = crate::monitoring::get_extraction_stats_for(corpus);
    Ok(HttpResponse::Ok().json(stats))
}

pub(crate) async fn get_canon_stats(
    query: actix_web::web::Query<CorpusFilter>,
) -> Result<HttpResponse, Error> {
    let corpus = query.corpus.as_deref().filter(|s| !s.is_empty());
    let stats = crate::monitoring::get_canon_stats_for(corpus);
    Ok(HttpResponse::Ok().json(stats))
}

pub(crate) async fn get_preprocess_stats(
    query: actix_web::web::Query<CorpusFilter>,
) -> Result<HttpResponse, Error> {
    let corpus = query.corpus.as_deref().filter(|s| !s.is_empty());
    let stats = crate::monitoring::get_preprocess_stats(corpus);
    Ok(HttpResponse::Ok().json(stats))
}

pub(crate) async fn get_chunk_meta_stats(
    query: actix_web::web::Query<CorpusFilter>,
) -> Result<HttpResponse, Error> {
    let corpus = query.corpus.as_deref().filter(|s| !s.is_empty());
    let (block_types, extractors, total) = if let Some(slug) = corpus {
        // Use the corpus-specific retriever from the registry when a filter is set.
        crate::corpus_registry::get_registry()
            .and_then(|reg| reg.get(slug))
            .map(|handle| {
                handle
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .chunk_meta_distribution()
            })
            .unwrap_or_default()
    } else if let Some(retriever) = RETRIEVER.get() {
        retriever
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .chunk_meta_distribution()
    } else {
        Default::default()
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "block_types": block_types,
        "extractors": extractors,
        "total": total,
    })))
}

#[derive(serde::Serialize)]
pub(crate) struct GoldenSampleResponse {
    pub status: Option<crate::db::golden_sample::GoldenSampleStatus>,
    pub entries: Vec<crate::db::golden_sample::GoldenSampleEntry>,
}

pub(crate) async fn get_golden_sample(
    query: actix_web::web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let limit = query
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50);
    let entries = crate::db::golden_sample::list(limit);
    let status = crate::db::golden_sample::status();
    Ok(HttpResponse::Ok().json(GoldenSampleResponse { status, entries }))
}

#[derive(serde::Deserialize, Default)]
pub(crate) struct RecaptureRequest {
    #[serde(default)]
    pub rotate_seed: bool,
}

pub(crate) async fn post_golden_sample_recapture(
    body: Option<actix_web::web::Json<RecaptureRequest>>,
) -> Result<HttpResponse, Error> {
    let rotate_seed = body.map(|b| b.rotate_seed).unwrap_or(false);
    let ok = crate::db::golden_sample::recapture(rotate_seed);
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "ok": ok,
        "rotate_seed": rotate_seed,
        "note": "Sample cleared. It will repopulate as you ingest documents."
    })))
}

/// POST /monitor/tokenizer/diff
/// Body: { candidate_path: string } OR { candidate_ollama_model: string }
/// Optional: { limit: number } (defaults to 50, capped at 1000)
pub(crate) async fn post_tokenizer_diff(
    body: actix_web::web::Json<crate::tokenizer_diff::DiffRequest>,
) -> Result<HttpResponse, Error> {
    let req = body.into_inner();
    // Run the diff on a blocking thread — GGUF load + per-entry encode are CPU
    // bound and would block the actix worker otherwise.
    let result = actix_web::web::block(move || crate::tokenizer_diff::compute_diff(&req)).await;
    match result {
        Ok(Ok(report)) => Ok(HttpResponse::Ok().json(report)),
        Ok(Err(e)) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("{:#}", e),
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("blocking task failed: {}", e),
        }))),
    }
}
