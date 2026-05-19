//! File watcher for automatic document indexing
//!
//! Watches the documents folder for new files and automatically indexes them.

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::ChunkerMode;
use crate::index;
use crate::monitoring::metrics;
use crate::retriever::Retriever;

/// Configuration for the file watcher
#[derive(Debug, Clone)]
pub struct FileWatcherConfig {
    /// Whether file watching is enabled
    pub enabled: bool,
    /// Debounce duration to avoid processing the same file multiple times
    pub debounce_ms: u64,
    /// Global fallback chunker mode (used when no per-corpus override is set)
    pub chunker_mode: ChunkerMode,
    /// Corpus slug this watcher is responsible for
    pub corpus_slug: String,
    /// Path to the documents SQLite DB (for per-corpus chunker lookup)
    pub db_path: String,
}

impl FileWatcherConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("FILE_WATCHER_ENABLED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(true),
            debounce_ms: std::env::var("FILE_WATCHER_DEBOUNCE_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(500),
            chunker_mode: ChunkerMode::from_env(),
            corpus_slug: "default".to_string(),
            db_path: String::new(),
        }
    }
}

/// Start the file watcher in a background task
pub fn start_file_watcher(
    watch_dir: &str,
    retriever: Arc<Mutex<Retriever>>,
    config: FileWatcherConfig,
) -> Option<tokio::task::JoinHandle<()>> {
    if !config.enabled {
        info!("📁 File watcher disabled (set FILE_WATCHER_ENABLED=true to enable)");
        return None;
    }

    let watch_path = watch_dir.to_string();

    // Ensure the directory exists
    if let Err(e) = std::fs::create_dir_all(&watch_path) {
        error!("Failed to create watch directory {}: {}", watch_path, e);
        return None;
    }

    info!("👁️ Starting file watcher on: {}", watch_path);

    let handle = actix_web::rt::spawn(async move {
        if let Err(e) = run_watcher(&watch_path, retriever, config).await {
            error!("File watcher error: {}", e);
        }
    });

    Some(handle)
}

async fn run_watcher(
    watch_path: &str,
    retriever: Arc<Mutex<Retriever>>,
    config: FileWatcherConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, mut rx) = mpsc::channel::<Event>(100);

    // Create the watcher
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Only send create and modify events
                match &event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        let _ = tx.blocking_send(event);
                    }
                    _ => {}
                }
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(config.debounce_ms)),
    )?;

    // Start watching
    watcher.watch(Path::new(watch_path), RecursiveMode::NonRecursive)?;
    info!("👁️ File watcher active on: {}", watch_path);

    // Track recently processed files to debounce
    let mut recent_files: std::collections::HashMap<String, std::time::Instant> =
        std::collections::HashMap::new();
    let debounce_duration = Duration::from_millis(config.debounce_ms);

    // Process events
    while let Some(event) = rx.recv().await {
        for path in event.paths {
            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Skip temporary files and hidden files
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if filename.starts_with('.') || filename.ends_with(".tmp") || filename.ends_with("~") {
                continue;
            }

            // Check file extension
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let allowed_extensions = [
                // Documents
                "pdf", "txt", "text", "md", "markdown", "html", "htm", "xhtml", "xml", "json",
                // Code files
                "rs", "py", "pyw", "js", "mjs", "cjs", "ts", "tsx", "go", "java", "cs", "cpp", "cc",
                "cxx", "hpp", "c", "h", "rb", "php", "sh", "bash", "zsh", "sql", "yaml", "yml",
                "toml",
            ];

            if !allowed_extensions.contains(&ext.as_str()) {
                debug!("Skipping unsupported file type: {}", path.display());
                continue;
            }

            // Debounce: always record the latest event time so that bursts of
            // writes (each 500 ms apart) don't all slip through the window.
            let path_str = path.to_string_lossy().to_string();
            let now = std::time::Instant::now();
            let last = recent_files.insert(path_str.clone(), now);
            if let Some(last_event) = last {
                if now.duration_since(last_event) < debounce_duration {
                    debug!("Debouncing file: {}", path.display());
                    continue;
                }
            }

            // Clean up old entries from recent_files
            recent_files.retain(|_, v| now.duration_since(*v) < Duration::from_secs(60));

            // Wait a bit for the file to be fully written
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Check if file exists and is readable
            if !path.exists() {
                debug!("File no longer exists: {}", path.display());
                continue;
            }

            info!("📄 New file detected: {}", path.display());

            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            // Phase 0: quick already-indexed check — hold lock only long enough to read doc IDs.
            let already_indexed = retriever
                .lock()
                .ok()
                .and_then(|ret| ret.get_all_doc_ids().ok())
                .map(|ids| {
                    ids.iter()
                        .any(|id| id.split('#').next() == Some(filename.as_str()))
                })
                .unwrap_or(false);
            if already_indexed {
                debug!("Skipping already-indexed file: {}", filename);
                continue;
            }

            // Compute effective per-corpus chunker config (no lock needed).
            let effective_cfg = {
                let global = crate::db::chunk_settings::global_config();
                let settings = if !config.db_path.is_empty() {
                    rusqlite::Connection::open(&config.db_path)
                        .ok()
                        .and_then(|conn| {
                            crate::db::corpora::get_corpus_settings(&conn, &config.corpus_slug).ok()
                        })
                        .unwrap_or_default()
                } else {
                    crate::db::corpora::CorpusSettings::default()
                };
                crate::db::corpora::effective_chunker_config(&global, &settings)
            };
            let effective_mode = effective_cfg
                .mode
                .parse::<ChunkerMode>()
                .unwrap_or(config.chunker_mode);

            // Phase 1: Extract IR — async, no mutex held, no Actix thread blocked.
            let ir = index::extract_ir_async(&path, &config.corpus_slug).await;
            let ir = match ir {
                Some(ir) => ir,
                None => {
                    warn!("Failed to extract IR from: {}", path.display());
                    continue;
                }
            };

            // Phase 2: Chunk + embed — CPU-bound, offloaded to blocking thread pool.
            let path_clone = path.clone();
            let corpus_slug = config.corpus_slug.clone();
            let cfg_clone = effective_cfg.clone();
            let prepared = tokio::task::spawn_blocking(move || {
                let cp_enabled = crate::db::chunk_settings::global_config().context_prefix_enabled;
                let chunker = crate::memory::chunker_factory::create_chunker(
                    effective_mode.into(),
                    &cfg_clone,
                );
                index::prepare_doc(
                    &path_clone,
                    &ir,
                    effective_mode,
                    chunker.as_ref(),
                    &corpus_slug,
                    cp_enabled,
                )
            })
            .await;

            let prepared = match prepared {
                Ok(p) => p,
                Err(e) => {
                    warn!("prepare_doc panicked for {}: {}", path.display(), e);
                    continue;
                }
            };

            // Phase 3: Write to index — hold mutex only for the brief Tantivy write.
            match retriever.lock() {
                Ok(mut ret) => {
                    if let Err(e) = ret.begin_batch() {
                        warn!("begin_batch failed for {}: {}", path.display(), e);
                        continue;
                    }
                    match index::index_prepared_doc(&mut ret, prepared) {
                        Ok((chunks, _)) if chunks > 0 => {
                            if let Err(e) = ret.commit() {
                                warn!("Failed to commit after indexing {}: {}", path.display(), e);
                            } else {
                                info!(
                                    "✅ Auto-indexed {} ({} chunks)",
                                    path.file_name().unwrap_or_default().to_string_lossy(),
                                    chunks
                                );
                                metrics::refresh_retriever_gauges(&ret);
                            }
                        }
                        Ok(_) => {
                            warn!("No chunks produced for: {}", path.display());
                        }
                        Err(e) => {
                            warn!("Failed to index {}: {}", path.display(), e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to acquire retriever lock: {}", e);
                }
            }
        }
    }

    Ok(())
}
