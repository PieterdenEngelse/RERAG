use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, error, info, warn};

use ag::api::start_api_server;
use ag::cache::redis_cache::RedisCache;
use ag::config::ApiConfig;
use ag::db::schema_init::SchemaInitializer;
#[cfg(feature = "neo4j")]
use ag::graph::{config::GraphConfig, Neo4jClient};
use ag::index;
use ag::monitoring::metrics;
use ag::monitoring::tracing_config::init_tracing;
use ag::monitoring::MonitoringConfig;
use ag::retriever::Retriever;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let startup_instant = Instant::now();

    // ─────────────────────────────────────────────────────────────
    // PHASE 1: Load Environment & Initialize Monitoring
    // ─────────────────────────────────────────────────────────────

    dotenvy::dotenv().ok();

    // ─────────────────────────────────────────────────────────────
    // PHASE 0.1: Set up Ctrl+C handler for clean shutdown
    // ─────────────────────────────────────────────────────────────
    ctrlc::set_handler(move || {
        eprintln!("\n🛑 Received Ctrl+C, shutting down...");
        std::process::exit(0);
    })
    .expect("Failed to set Ctrl+C handler");

    // ─────────────────────────────────────────────────────────────
    // PHASE 0.5: Clean up stale locks from previous crashes
    // ─────────────────────────────────────────────────────────────
    cleanup_stale_locks();

    // Load monitoring config from environment
    let monitoring_config = MonitoringConfig::from_env();

    // Create logs directory
    std::fs::create_dir_all(&monitoring_config.log_dir).expect("Failed to create log directory");
    info!("📝 Log directory: {}", monitoring_config.log_dir.display());

    // Initialize tracing/logging
    let _tracing_guard = init_tracing(&monitoring_config).expect("Failed to initialize tracing");

    // Initialize global health tracker for load metrics
    ag::monitoring::init_health_tracker();

    info!("🚀 Starting agentic-rag v{}", env!("CARGO_PKG_VERSION"));

    // Initialize OpenTelemetry for distributed tracing (Phase 16)
    let otel_config = ag::monitoring::otel_config::OtelConfig::from_env();
    let _otel_guard = ag::monitoring::otel_config::init_otel(&otel_config)
        .expect("Failed to initialize OpenTelemetry");
    info!("🔍 OpenTelemetry initialized");
    debug!("Monitoring enabled: {}", monitoring_config.enabled);

    // ─────────────────────────────────────────────────────────────
    // PHASE 1.5: Start Trace-Based Alerting (Background Task)
    // ─────────────────────────────────────────────────────────────

    let trace_alert_config = ag::monitoring::TraceAlertingConfig::from_env();
    if trace_alert_config.is_enabled() {
        let _alert_handle = ag::monitoring::start_trace_alerting(trace_alert_config);
        info!("🔔 Trace-based alerting started");
    } else {
        debug!("Trace-based alerting disabled (set TEMPO_ENABLED=true to enable)");
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 1.6: Start Resource Attribution (Background Task)
    // ─────────────────────────────────────────────────────────────

    let resource_config = ag::monitoring::ResourceAttributionConfig::from_env();
    if resource_config.is_enabled() {
        let _resource_handle = ag::monitoring::start_resource_attribution(resource_config);
        info!("📊 Resource attribution started");
    } else {
        debug!("Resource attribution disabled (set RESOURCE_ATTRIBUTION_ENABLED=false to disable)");
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 2: Load Configuration with Tracing
    // ─────────────────────────────────────────────────────────────
    debug!(
        "Monitoring config: enabled={}, file_logging={}",
        monitoring_config.enabled, monitoring_config.enable_file_logging
    );

    let _config_start = Instant::now();
    debug!("Loading configuration with PathManager...");

    let config = ApiConfig::from_env();
    ag::monitoring::set_chunking_logging_enabled(config.chunking_log_enabled);

    let pm = &config.path_manager;
    info!("🏠 AG_HOME: {}", pm.base_dir().display());
    debug!("DB path: {}", pm.db_path("documents").display());
    debug!("Index path: {}", pm.index_path("tantivy").display());

    // ─────────────────────────────────────────────────────────────
    // PHASE 3: Initialize Database
    // ─────────────────────────────────────────────────────────────

    let db_start = Instant::now();
    info!("📦 Initializing database schema...");

    let _db_conn = match (|| -> std::io::Result<rusqlite::Connection> {
        let conn = rusqlite::Connection::open(pm.db_path("documents"))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        SchemaInitializer::init(&conn)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        ag::db::param_store::init_table(&conn)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        Ok(conn)
    })() {
        Ok(conn) => {
            let duration_ms = db_start.elapsed().as_millis() as u64;
            info!(duration_ms = duration_ms, "✓ Database initialized");
            conn
        }
        Err(e) => {
            error!(error = %e, "Failed to initialize database");
            return Err(e);
        }
    };

    ag::db::chunk_settings::set_global_db_path(pm.db_path("documents"));
    ag::db::chunk_settings::load_active_config(&_db_conn);
    ag::db::llm_settings::load_active_config(&_db_conn);
    ag::db::param_hardware::load_active_config(&_db_conn);

    // ─────────────────────────────────────────────────────────────
    // PHASE 4: Initialize Retriever with PathManager
    // ─────────────────────────────────────────────────────────────

    let retriever_start = Instant::now();
    info!("📦 Initializing Retriever with PathManager...");

    let mut retriever =
        match Retriever::new_with_paths(pm.index_path("tantivy"), pm.vector_store_path()) {
            Ok(mut ret) => {
                ret.set_search_top_k(config.search_top_k);
                let duration_ms = retriever_start.elapsed().as_millis() as u64;
                info!(duration_ms = duration_ms, "✓ Retriever initialized");
                // Initialize Prometheus app_info and initial gauges
                metrics::APP_INFO.set(1);
                metrics::refresh_retriever_gauges(&ret);
                ret
            }
            Err(e) => {
                error!(error = %e, "Failed to initialize retriever");
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
            }
        };

    // ─────────────────────────────────────────────────────────────
    // PHASE 5: Initialize Redis L3 Cache (if enabled)
    // ─────────────────────────────────────────────────────────────

    if config.redis_enabled {
        let redis_start = Instant::now();
        info!("📡 Initializing Redis L3 cache...");

        match RedisCache::new(
            config
                .redis_url
                .as_deref()
                .unwrap_or("redis://127.0.0.1:6379/"),
            config.redis_ttl,
        )
        .await
        {
            Ok(redis_cache) => {
                let duration_ms = redis_start.elapsed().as_millis() as u64;
                retriever.set_l3_cache(redis_cache);
                info!(duration_ms = duration_ms, "✅ Redis L3 cache initialized");
            }
            Err(e) => {
                warn!(error = %e, "Failed to initialize Redis L3 cache");
                warn!("Continuing without L3 cache...");
            }
        }
    } else {
        debug!("Redis L3 cache disabled (set REDIS_ENABLED=true to enable)");
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 5.5: Initialize Neo4j Knowledge Graph (if enabled)
    // ─────────────────────────────────────────────────────────────

    #[cfg(feature = "neo4j")]
    {
        let graph_config = GraphConfig::from_env();
        if graph_config.enabled {
            let neo4j_start = Instant::now();
            info!("🔗 Initializing Neo4j Knowledge Graph...");

            match Neo4jClient::new(graph_config.clone()).await {
                Ok(client) => {
                    let duration_ms = neo4j_start.elapsed().as_millis() as u64;
                    info!(
                        duration_ms = duration_ms,
                        "✅ Neo4j Knowledge Graph initialized"
                    );

                    // Initialize schema
                    if let Err(e) = client.init_schema().await {
                        warn!(error = %e, "Failed to initialize Neo4j schema");
                    } else {
                        info!("✅ Neo4j schema initialized");
                    }

                    // Initialize KnowledgeBuilder for graph integration during indexing
                    let kb_config = GraphConfig::from_env();
                    let knowledge_builder =
                        ag::graph::KnowledgeBuilder::new(client.graph(), kb_config);
                    ag::api::set_knowledge_builder(std::sync::Arc::new(knowledge_builder));
                    info!("✅ KnowledgeBuilder initialized for entity extraction");

                    // Store client globally for API access
                    ag::api::set_neo4j_client(client);

                    // Spawn background task to compile Neo4j → petgraph runtime
                    // Create a NEW Neo4j connection for the background task
                    let graph_config_for_petgraph = GraphConfig::from_env();
                    actix_web::rt::spawn(async move {
                        info!("ParallelGroup: Compiling Neo4j → petgraph runtime (background)...");

                        match Neo4jClient::new(graph_config_for_petgraph).await {
                            Ok(client_for_graph) => {
                                ag::graph::petgraph_runtime::initialize_from_neo4j(
                                    client_for_graph,
                                )
                                .await;

                                // Log the result
                                let runtime = ag::graph::petgraph_runtime::get_runtime_graph();
                                info!(
                                    "ParallelGroup: Runtime graph ready ({} nodes, {} edges)",
                                    runtime.node_count(),
                                    runtime.edge_count()
                                );
                            }
                            Err(e) => {
                                warn!("ParallelGroup: Failed to create Neo4j client for graph compilation: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    warn!(error = %e, "Failed to initialize Neo4j Knowledge Graph");
                    warn!("Continuing without Neo4j...");
                }
            }
        } else {
            debug!("Neo4j Knowledge Graph disabled (set NEO4J_ENABLED=true to enable)");
        }
    }

    #[cfg(not(feature = "neo4j"))]
    {
        debug!("Neo4j feature not compiled (build with --features neo4j)");
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 5.6: Initialize Petgraph Runtime (always, from files)
    // ─────────────────────────────────────────────────────────────

    {
        let petgraph_start = Instant::now();
        info!("📊 Initializing Petgraph runtime graph...");

        let data_dir = std::env::var("AG_DATA_DIR").unwrap_or_else(|_| "data".to_string());
        ag::graph::petgraph_runtime::initialize_standalone(&data_dir).await;

        let runtime = ag::graph::petgraph_runtime::get_runtime_graph();
        let duration_ms = petgraph_start.elapsed().as_millis() as u64;

        if runtime.is_empty() {
            info!(
                duration_ms = duration_ms,
                "📊 Petgraph runtime initialized (empty - export from Neo4j with POST /graph/export)"
            );
        } else {
            info!(
                duration_ms = duration_ms,
                nodes = runtime.node_count(),
                edges = runtime.edge_count(),
                "✅ Petgraph runtime initialized from file"
            );
        }
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 6: Prepare Retriever for API
    // ─────────────────────────────────────────────────────────────

    let retriever = Arc::new(Mutex::new(retriever));
    ag::api::set_retriever_handle(Arc::clone(&retriever));
    // Initialize shared EmbeddingService for cached query embedding
    let embedding_svc = std::sync::Arc::new(ag::embedder::EmbeddingService::new(
        ag::embedder::EmbeddingConfig::from_env(),
    ));
    ag::api::set_embedding_service(embedding_svc);

    // ─────────────────────────────────────────────────────────────
    // PHASE 6.5: Initialize Optimized Search Engine (all perf optimizations)
    // ─────────────────────────────────────────────────────────────
    info!("🚀 Initializing optimized search engine (SIMD, HNSW, semantic cache, hybrid search)...");
    let _search_engine = ag::perf::integration::init_search_engine();
    info!("✅ Optimized search engine initialized");

    // ─────────────────────────────────────────────────────────────
    // PHASE 7: Spawn Background Indexing (NON-BLOCKING) - v2.1.0
    // ─────────────────────────────────────────────────────────────

    let skip_initial = config.skip_initial_indexing;

    if skip_initial {
        info!("⏭️  Skipping initial indexing due to SKIP_INITIAL_INDEXING=true");
    } else {
        info!("📚 Starting background indexing (non-blocking)...");

        let retriever_clone = Arc::clone(&retriever);
        let upload_dir = ag::api::UPLOAD_DIR.to_string();

        // Spawn as background task - doesn't block server startup
        tokio::task::spawn_blocking(move || {
            let indexing_start = Instant::now();
            debug!("Background indexing task started");

            // Mark indexing as started for health status
            ag::monitoring::mark_indexing_started();

            match retriever_clone.lock() {
                Ok(mut ret) => {
                    // Call indexing synchronously within the async task
                    let chunker = index::default_chunker(config.chunker_mode);
                    if let Err(e) = index::index_all_documents(
                        &mut *ret,
                        &upload_dir,
                        config.chunker_mode,
                        chunker.as_ref(),
                    ) {
                        error!("Background indexing failed: {}", e);
                    } else {
                        let duration_ms = indexing_start.elapsed().as_millis() as u64;
                        info!(duration_ms = duration_ms, "✓ Background indexing completed");
                        metrics::refresh_retriever_gauges(&ret);
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to acquire retriever lock for background indexing: {}",
                        e
                    );
                }
            }

            // Mark indexing as finished for health status
            ag::monitoring::mark_indexing_finished();
        });
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 7.5: Start File Watcher for Auto-Indexing
    // ─────────────────────────────────────────────────────────────

    let file_watcher_config = ag::file_watcher::FileWatcherConfig::from_env();
    let _file_watcher_handle = ag::file_watcher::start_file_watcher(
        ag::api::UPLOAD_DIR,
        Arc::clone(&retriever),
        file_watcher_config,
    );

    // ─────────────────────────────────────────────────────────────
    // PHASE 8: Start Server Immediately (Server Ready Before Indexing Done)
    // ─────────────────────────────────────────────────────────────

    let total_startup_ms = startup_instant.elapsed().as_millis() as u64;

    info!("═══════════════════════════════════════════════════════════");
    info!("🎉 Application Started Successfully!");
    info!("   Version: {}", env!("CARGO_PKG_VERSION"));
    info!(
        "   Startup Time: {}ms (server ready, indexing in background)",
        total_startup_ms
    );
    metrics::STARTUP_DURATION_MS.set(total_startup_ms as i64);
    info!("   Server: http://{}", config.bind_addr());
    info!("   Health: http://{}/monitoring/health", config.bind_addr());
    info!(
        "   Metrics: http://{}/monitoring/metrics",
        config.bind_addr()
    );
    info!("   Ready: http://{}/monitoring/ready", config.bind_addr());
    if skip_initial {
        info!("   Note: Initial indexing skipped. Use POST /reindex/async to index.");
    } else {
        info!("   Note: Background indexing in progress. Check /index/info for status.");
    }
    info!("═══════════════════════════════════════════════════════════");

    info!(
        "🚀 Starting API server on http://{} ...",
        config.bind_addr()
    );

    start_api_server(&config).await
}

/// Clean up stale lock files from previous crashes or kill -9
/// This runs before any other initialization to ensure clean startup
fn cleanup_stale_locks() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let data_dir = format!("{}/.local/share/ag", home);
    let project_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let lock_patterns = [
        // Tantivy index locks
        format!("{}/tantivy_index/*.lock", data_dir),
        format!("{}/tantivy_index/.tantivy-*", data_dir),
        // Data directory locks
        format!("{}/*.lock", data_dir),
        format!("{}/data/*.lock", data_dir),
        // SQLite WAL/SHM files (data dir)
        format!("{}/*.db-wal", data_dir),
        format!("{}/*.db-shm", data_dir),
        // Project directory locks
        format!("{}/*.lock", project_dir),
        // SQLite WAL/SHM files (project dir)
        format!("{}/*.db-wal", project_dir),
        format!("{}/*.db-shm", project_dir),
    ];

    let mut cleaned = 0;
    for pattern in &lock_patterns {
        if let Ok(entries) = glob::glob(pattern) {
            for entry in entries.flatten() {
                if entry.is_file() {
                    match std::fs::remove_file(&entry) {
                        Ok(_) => {
                            cleaned += 1;
                            eprintln!("  Cleaned stale lock: {}", entry.display());
                        }
                        Err(e) => {
                            eprintln!("  Warning: Could not remove {}: {}", entry.display(), e);
                        }
                    }
                }
            }
        }
    }

    // Also clean /tmp/ag_* files
    if let Ok(entries) = glob::glob("/tmp/ag_*") {
        for entry in entries.flatten() {
            if entry.is_file() {
                if let Ok(_) = std::fs::remove_file(&entry) {
                    cleaned += 1;
                    eprintln!("  Cleaned temp file: {}", entry.display());
                }
            }
        }
    }

    if cleaned > 0 {
        eprintln!("🧹 Cleaned {} stale lock/temp files", cleaned);
    }
}
