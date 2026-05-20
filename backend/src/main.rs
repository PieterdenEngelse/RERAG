use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, error, info, warn};

use ag::api::start_api_server;
use ag::cache::redis_cache::RedisCache;
use ag::config::ApiConfig;
use ag::db::schema_init::SchemaInitializer;
#[cfg(feature = "graph")]
use ag::graph::{config::GraphConfig, GraphClient};
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
    // Runtime overrides saved by the UI — must load after .env so they win.
    dotenvy::from_filename_override(".env.rate_limits").ok();
    dotenvy::from_filename_override(".env.index").ok();
    dotenvy::from_filename_override(".env.server").ok();

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

    // ── Settings + boot-failure recovery ──────────────────────────────
    // Initialize before ApiConfig so any reader that goes through
    // settings::effective_*() sees the right values from the first call.
    {
        let early_pm = ag::path_manager::PathManager::new()
            .expect("Failed to initialize PathManager for settings bootstrap");
        let base_dir = early_pm.base_dir().to_path_buf();
        let overrides_path = base_dir.join("overrides.json");
        let (overrides_path, recovery) =
            ag::settings::Recovery::boot_check(&base_dir, &overrides_path);
        let settings = ag::settings::Settings::load(overrides_path);
        ag::settings::install_global(settings, std::sync::Arc::new(recovery));

        // Mark the boot "known good" after a short window of uptime — enough
        // to clear startup hazards while remaining well within the user's
        // patience window if they're staring at a broken page.
        tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            ag::settings::mark_healthy();
        });
    }

    let config = ApiConfig::from_env();
    ag::monitoring::set_chunking_logging_enabled(config.chunking_log_enabled);

    ag::embedder::init_upload_blocking_pool(
        std::env::var("UPLOAD_ONNX_THREADS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4),
    );

    let pm = &config.path_manager;

    // ─────────────────────────────────────────────────────────────
    // PHASE 2.5: Migrate legacy documents/ dir and set upload path
    // ─────────────────────────────────────────────────────────────
    {
        // Compute target path without triggering create_dir_all (migration must run first).
        let new_upload_path = pm
            .data_dir()
            .join("corpora")
            .join("default")
            .join("documents");

        if !new_upload_path.exists() {
            let legacy = std::path::Path::new("documents");
            if legacy.exists() && legacy.is_dir() && !legacy.is_symlink() {
                match std::fs::rename(legacy, &new_upload_path) {
                    Ok(()) => {
                        // Symlink old path back so external tools still work.
                        #[cfg(unix)]
                        let _ = std::os::unix::fs::symlink(&new_upload_path, legacy);
                        info!(
                            "corpus migration: moved documents/ → {}",
                            new_upload_path.display()
                        );
                    }
                    Err(e) => {
                        // Cross-device rename; create_dir_all will handle the new path.
                        warn!(
                            "corpus migration: rename failed ({}), will create fresh dir",
                            e
                        );
                    }
                }
            }
        }

        // corpus_upload_dir creates the dir if it doesn't already exist.
        ag::api::set_default_upload_dir(pm.corpus_upload_dir("default"));
        info!("📂 Upload dir: {}", ag::api::default_upload_dir());
    }
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
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        SchemaInitializer::init(&conn).map_err(|e| std::io::Error::other(e.to_string()))?;
        ag::db::param_store::init_table(&conn).map_err(|e| std::io::Error::other(e.to_string()))?;

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
    ag::db::ner_settings::load_active_config(&_db_conn);
    ag::db::extraction_records::init(pm.db_path("documents"));
    ag::db::golden_sample::init(pm.db_path("documents"));
    ag::monitoring::load_extraction_history();
    ag::monitoring::init_preprocess_stats(pm.data_dir().join("preprocess_stats.json"));
    ag::monitoring::init_canon_stats(pm.data_dir().join("canon_stats.json"));
    ag::monitoring::init_chunking_stats(pm.data_dir().join("chunking_stats.json"));
    ag::perf::io_uring::init_stats(pm.data_dir().join("io_stats.json"));
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        interval.tick().await; // skip the immediate first tick
        loop {
            interval.tick().await;
            ag::monitoring::flush_preprocess_stats();
            ag::monitoring::flush_canon_stats();
            ag::monitoring::flush_chunking_stats();
            ag::perf::io_uring::flush_stats();
        }
    });

    // ─────────────────────────────────────────────────────────────
    // PHASE 3.5: Initialize GGUF Token Counter
    // ─────────────────────────────────────────────────────────────
    {
        use ag::db::param_hardware;
        use ag::gguf_tokenizer::*;

        let handle = std::sync::Arc::new(TokenCounterHandle::new_heuristic());

        // Try to load exact tokenizer from active model's GGUF
        let hw = param_hardware::global_config();
        let (gguf_result, expected_local_gguf) = match hw.backend_type {
            param_hardware::BackendType::Ollama => {
                if !hw.model.is_empty() {
                    info!("🔢 Resolving Ollama GGUF for tokenizer: {}", hw.model);
                    (resolve_ollama_gguf_path(&hw.model), true)
                } else {
                    handle.mark_fallback(FallbackReason::NoModelConfigured, None);
                    (Err(anyhow::anyhow!("No model configured")), false)
                }
            }
            param_hardware::BackendType::LlamaCpp => {
                info!("🔢 Resolving llama-server GGUF for tokenizer");
                (resolve_llama_server_gguf_path(), true)
            }
            _ => {
                handle.mark_fallback(FallbackReason::CloudBackend, None);
                (Err(anyhow::anyhow!("Cloud backend, no local GGUF")), false)
            }
        };

        if expected_local_gguf {
            match gguf_result {
                Ok(path) => {
                    if let Ok(()) = handle.load_from_gguf(&path) {
                        info!(
                            "✅ Exact token counter loaded (model={}, vocab={})",
                            handle.model_name(),
                            handle.vocab_size()
                        );
                    } // else: load_from_gguf already recorded the fallback + warned
                }
                Err(e) => handle.mark_fallback(
                    FallbackReason::PathNotFound {
                        detail: format!("{:#}", e),
                    },
                    None,
                ),
            }
        }

        ag::api::set_token_counter(handle);
        info!(
            "🔢 Token counter ready (exact={})",
            ag::api::get_token_counter()
                .map(|h| h.is_exact())
                .unwrap_or(false)
        );
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 4: Initialize Retriever with PathManager
    // ─────────────────────────────────────────────────────────────

    let retriever_start = Instant::now();
    info!("📦 Initializing Retriever with PathManager...");

    let mut retriever = match Retriever::new_with_paths(
        pm.index_path("tantivy"),
        pm.vector_store_path(),
        config.index_in_ram,
    ) {
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
            return Err(std::io::Error::other(e));
        }
    };

    // Restore the persisted L1 search cache so warm queries survive a restart.
    let search_cache_path = config
        .path_manager
        .search_cache_path()
        .to_string_lossy()
        .to_string();
    match retriever.load_search_cache_async(&search_cache_path).await {
        Ok(0) => {}
        Ok(n) => info!(entries = n, "✓ Search cache restored from disk"),
        Err(e) => warn!("Failed to restore search cache: {e:?}"),
    }

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
    // PHASE 5.5: Initialize FalkorDB Knowledge Graph (if enabled)
    // ─────────────────────────────────────────────────────────────

    #[cfg(feature = "graph")]
    {
        let graph_config = GraphConfig::from_env();
        if graph_config.enabled {
            let graph_start = Instant::now();
            info!("🔗 Initializing FalkorDB Knowledge Graph...");

            match GraphClient::new(graph_config.clone()).await {
                Ok(client) => {
                    let duration_ms = graph_start.elapsed().as_millis() as u64;
                    info!(
                        duration_ms = duration_ms,
                        "✅ FalkorDB Knowledge Graph initialized"
                    );

                    // Initialize schema
                    if let Err(e) = client.init_schema().await {
                        warn!(error = %e, "Failed to initialize FalkorDB schema");
                    } else {
                        info!("✅ FalkorDB schema initialized");
                    }

                    // Initialize KnowledgeBuilder for graph integration during indexing
                    let kb_config = GraphConfig::from_env();
                    let knowledge_builder =
                        ag::graph::KnowledgeBuilder::new(client.graph(), kb_config);
                    ag::api::set_knowledge_builder(std::sync::Arc::new(knowledge_builder));
                    info!("✅ KnowledgeBuilder initialized for entity extraction");

                    // Store client globally for API access
                    ag::api::set_graph_client(client);

                    // Spawn background task to compile FalkorDB → petgraph runtime
                    // Create a NEW FalkorDB connection for the background task
                    let graph_config_for_petgraph = GraphConfig::from_env();
                    actix_web::rt::spawn(async move {
                        info!(
                            "ParallelGroup: Compiling FalkorDB → petgraph runtime (background)..."
                        );

                        match GraphClient::new(graph_config_for_petgraph).await {
                            Ok(client_for_graph) => {
                                ag::graph::petgraph_runtime::initialize_from_graph(
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
                                warn!("ParallelGroup: Failed to create FalkorDB client for graph compilation: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    warn!(error = %e, "Failed to initialize FalkorDB Knowledge Graph");
                    warn!("Continuing without FalkorDB...");
                }
            }
        } else {
            debug!("FalkorDB Knowledge Graph disabled (set FALKOR_ENABLED=true to enable)");
        }
    }

    #[cfg(not(feature = "graph"))]
    {
        debug!("FalkorDB feature not compiled (build with --features graph)");
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
                "📊 Petgraph runtime initialized (empty - export from FalkorDB with POST /graph/export)"
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
    // PHASE 5.7: Initialize External Document Extractors (Docling)
    // ─────────────────────────────────────────────────────────────

    {
        let enabled = std::env::var("DOCLING_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        if enabled {
            let url = std::env::var("DOCLING_URL")
                .unwrap_or_else(|_| "http://localhost:5001".to_string());
            info!("🔬 Connecting to Docling sidecar at {}", url);
            let ext = ag::extractor::DoclingExtractor::new(url);
            match ext.health_check() {
                Ok(()) => {
                    ag::extractor::init_registry(vec![Box::new(ext)]);
                    info!("✅ Docling extractor registered (PDF/DOCX/PPTX structural extraction)");
                }
                Err(e) => {
                    warn!(error = %e, "Docling sidecar unreachable — falling back to built-in extraction");
                }
            }
        } else {
            debug!("Docling extraction disabled (set DOCLING_ENABLED=true to enable)");
        }
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 5.8: Native In-Process PDF Extractor (layout_ml feature)
    // ─────────────────────────────────────────────────────────────

    #[cfg(feature = "layout_ml")]
    {
        let enabled = std::env::var("LAYOUT_ML_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        if enabled {
            // Pre-warm models on a blocking thread so they're ready before first upload.
            tokio::task::spawn_blocking(|| {
                ag::pdf::layout_model::LayoutModel::load_or_heuristic();
                ag::pdf::table_model::TableModel::load_or_text();
            });
            let native = ag::pdf::native_extractor::NativePdfExtractor;
            ag::extractor::init_registry(vec![Box::new(native)]);
            info!("✅ NativePdfExtractor registered (native in-process PDF extraction)");
        } else {
            debug!("Native PDF extraction disabled (set LAYOUT_ML_ENABLED=true to enable)");
        }
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 6: Prepare Retriever for API
    // ─────────────────────────────────────────────────────────────

    let retriever = Arc::new(Mutex::new(retriever));
    ag::api::set_retriever_handle(Arc::clone(&retriever));

    // ── L3 hot-reload: rebuild the RedisCache when any REDIS_* setting
    //    override changes. The subscriber runs synchronously; the rebuild
    //    happens in a tokio task that locks the retriever briefly to swap
    //    the handle.
    if let Some(s) = ag::settings::global() {
        let retriever_for_l3 = Arc::clone(&retriever);
        let rebuild = std::sync::Arc::new(move || {
            let retriever = Arc::clone(&retriever_for_l3);
            tokio::spawn(async move {
                let enabled = ag::settings::effective_bool("REDIS_ENABLED", false);
                let new_cache = if enabled {
                    let url = ag::settings::effective_or(
                        "REDIS_URL",
                        "redis://127.0.0.1:6379/",
                    );
                    let ttl = ag::settings::effective_u64("REDIS_TTL", 3600);
                    match ag::cache::redis_cache::RedisCache::new(&url, ttl).await {
                        Ok(c) => Some(c),
                        Err(e) => {
                            warn!("L3 rebuild failed: {e}");
                            None
                        }
                    }
                } else {
                    None
                };
                if let Ok(mut ret) = retriever.lock() {
                    ret.swap_l3_cache(new_cache);
                }
            });
        });
        let rebuild_a = std::sync::Arc::clone(&rebuild);
        let rebuild_b = std::sync::Arc::clone(&rebuild);
        let rebuild_c = std::sync::Arc::clone(&rebuild);
        s.subscribe("REDIS_ENABLED", move |_| rebuild_a());
        s.subscribe("REDIS_URL", move |_| rebuild_b());
        s.subscribe("REDIS_TTL", move |_| rebuild_c());
        info!("✓ L3 hot-reload subscribers registered (REDIS_ENABLED / URL / TTL)");
    }

    // Periodically persist the L1 search cache so it survives a crash or
    // SIGKILL, not just a graceful shutdown.
    {
        let retriever_for_cache = Arc::clone(&retriever);
        let cache_path = search_cache_path.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(300));
            if let Ok(ret) = retriever_for_cache.lock() {
                if let Err(e) = ret.save_search_cache(&cache_path) {
                    warn!("Periodic search cache save failed: {e:?}");
                }
            }
        });
    }

    // Initialize corpus registry and register the default corpus
    ag::corpus_registry::init(Arc::new(config.path_manager.clone()), config.index_in_ram);
    if let Some(registry) = ag::corpus_registry::get_registry() {
        registry.insert("default", Arc::clone(&retriever));
        info!("✓ Corpus registry initialized (default corpus registered)");
    }

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
        let upload_dir = ag::api::default_upload_dir();

        // Spawn as background task - doesn't block server startup
        let db_path_for_idx = pm.db_path("documents").to_string_lossy().to_string();
        let pm_for_idx = config.path_manager.clone();
        tokio::task::spawn_blocking(move || {
            let indexing_start = Instant::now();
            debug!("Background indexing task started");
            ag::monitoring::mark_indexing_started();

            // Index default corpus
            match retriever_clone.lock() {
                Ok(mut ret) => {
                    // Check for per-corpus chunker override on default
                    let settings_default = rusqlite::Connection::open(&db_path_for_idx)
                        .ok()
                        .and_then(|conn| {
                            ag::db::corpora::get_corpus_settings(&conn, "default").ok()
                        });
                    let global_cfg = ag::db::chunk_settings::global_config();
                    let effective_cfg_default = settings_default
                        .map(|s| ag::db::corpora::effective_chunker_config(&global_cfg, &s))
                        .unwrap_or(global_cfg.clone());
                    let effective_mode = effective_cfg_default
                        .mode
                        .parse::<ag::config::ChunkerMode>()
                        .unwrap_or(config.chunker_mode);
                    let chunker = ag::memory::chunker_factory::create_chunker(
                        effective_mode.into(),
                        &effective_cfg_default,
                    );
                    if let Err(e) = index::index_all_documents(
                        &mut ret,
                        &upload_dir,
                        effective_mode,
                        chunker.as_ref(),
                        "default",
                        effective_cfg_default.context_prefix_enabled,
                    ) {
                        error!("Background indexing (default) failed: {}", e);
                    } else {
                        metrics::refresh_retriever_gauges(&ret);
                    }
                }
                Err(e) => error!(
                    "Failed to acquire retriever lock for background indexing: {}",
                    e
                ),
            }

            // Index non-default corpora
            if let Ok(conn) = rusqlite::Connection::open(&db_path_for_idx) {
                if let Ok(corpora) = ag::db::corpora::list_corpora(&conn) {
                    for corpus in corpora.iter().filter(|c| c.slug != "default") {
                        let slug = &corpus.slug;
                        let corpus_dir = pm_for_idx
                            .corpus_upload_dir(slug)
                            .to_string_lossy()
                            .to_string();
                        let corpus_settings =
                            ag::db::corpora::get_corpus_settings(&conn, slug).unwrap_or_default();
                        let global_cfg2 = ag::db::chunk_settings::global_config();
                        let eff_cfg = ag::db::corpora::effective_chunker_config(
                            &global_cfg2,
                            &corpus_settings,
                        );
                        let effective_mode = eff_cfg
                            .mode
                            .parse::<ag::config::ChunkerMode>()
                            .unwrap_or(config.chunker_mode);
                        if let Some(handle) = ag::corpus_registry::get_registry()
                            .and_then(|reg| reg.get_or_create(slug).ok())
                        {
                            if let Ok(mut ret) = handle.lock() {
                                let chunker = ag::memory::chunker_factory::create_chunker(
                                    effective_mode.into(),
                                    &eff_cfg,
                                );
                                if let Err(e) = index::index_all_documents(
                                    &mut ret,
                                    &corpus_dir,
                                    effective_mode,
                                    chunker.as_ref(),
                                    slug,
                                    eff_cfg.context_prefix_enabled,
                                ) {
                                    error!("Background indexing ({}) failed: {}", slug, e);
                                }
                            }
                        }
                    }
                }
            }

            let duration_ms = indexing_start.elapsed().as_millis() as u64;
            info!(
                duration_ms = duration_ms,
                "✓ Background indexing completed (all corpora)"
            );
            ag::monitoring::mark_indexing_finished();
        });
    }

    // ─────────────────────────────────────────────────────────────
    // PHASE 7.5: Start File Watcher for Auto-Indexing
    // ─────────────────────────────────────────────────────────────

    let db_path_str = pm.db_path("documents").to_string_lossy().to_string();

    // Start file watcher for the default corpus
    let file_watcher_dir = ag::api::default_upload_dir();
    let mut file_watcher_config = ag::file_watcher::FileWatcherConfig::from_env();
    file_watcher_config.corpus_slug = "default".to_string();
    file_watcher_config.db_path = db_path_str.clone();
    let _file_watcher_handle = ag::file_watcher::start_file_watcher(
        &file_watcher_dir,
        Arc::clone(&retriever),
        file_watcher_config.clone(),
    );

    // Start file watchers for non-default corpora
    if let Ok(conn) = rusqlite::Connection::open(&db_path_str) {
        if let Ok(corpora) = ag::db::corpora::list_corpora(&conn) {
            for corpus in corpora.into_iter().filter(|c| c.slug != "default") {
                let slug = corpus.slug.clone();
                let corpus_dir = pm.corpus_upload_dir(&slug).to_string_lossy().to_string();
                if let Some(handle) = ag::corpus_registry::get_registry()
                    .and_then(|reg| reg.get_or_create(&slug).ok())
                {
                    let mut cfg = file_watcher_config.clone();
                    cfg.corpus_slug = slug.clone();
                    ag::file_watcher::start_file_watcher(&corpus_dir, handle, cfg);
                }
            }
        }
    }

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
    info!("   Search server: http://{}", config.bind_addr());
    info!("   Upload server: http://{}", config.upload_bind_addr());
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
        "🚀 Starting API servers on http://{} (search) and http://{} (upload) ...",
        config.bind_addr(),
        config.upload_bind_addr()
    );

    let result = start_api_server(&config).await;
    // Persist the L1 search cache so warm queries survive the next restart.
    if let Ok(ret) = retriever.lock() {
        if let Err(e) = ret.save_search_cache(&search_cache_path) {
            warn!("Failed to persist search cache: {e:?}");
        }
    }
    // Flush all stats to disk before process exits (catches snapshots from uploads
    // that completed during graceful shutdown after SIGTERM).
    ag::monitoring::flush_preprocess_stats();
    ag::monitoring::flush_canon_stats();
    ag::monitoring::flush_chunking_stats();
    ag::perf::io_uring::flush_stats();
    result
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
        // Tantivy index locks — all corpus subdirs under index/
        format!("{}/index/*/.tantivy-writer.lock", data_dir),
        format!("{}/index/*/.tantivy-meta.lock", data_dir),
        format!("{}/index/*/.tantivy-*", data_dir),
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
            if entry.is_file() && std::fs::remove_file(&entry).is_ok() {
                cleaned += 1;
                eprintln!("  Cleaned temp file: {}", entry.display());
            }
        }
    }

    if cleaned > 0 {
        eprintln!("🧹 Cleaned {} stale lock/temp files", cleaned);
    }
}
