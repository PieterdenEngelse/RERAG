use crate::path_manager::PathManager;
use crate::retriever::Retriever;
use dashmap::DashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::info;

pub struct CorpusRegistry {
    retrievers: DashMap<String, Arc<Mutex<Retriever>>>,
    pm: Arc<PathManager>,
}

impl CorpusRegistry {
    pub fn new(pm: Arc<PathManager>) -> Self {
        Self {
            retrievers: DashMap::new(),
            pm,
        }
    }

    pub fn get(&self, slug: &str) -> Option<Arc<Mutex<Retriever>>> {
        self.retrievers.get(slug).map(|r| Arc::clone(&*r))
    }

    /// Insert (or replace) a corpus retriever. Called at startup for known corpora.
    pub fn insert(&self, slug: impl Into<String>, handle: Arc<Mutex<Retriever>>) {
        self.retrievers.insert(slug.into(), handle);
    }

    pub fn remove(&self, slug: &str) {
        self.retrievers.remove(slug);
    }

    pub fn slugs(&self) -> Vec<String> {
        self.retrievers.iter().map(|e| e.key().clone()).collect()
    }

    /// Return an existing retriever or create a new one at `corpus_index_dir(slug)`.
    pub fn get_or_create(&self, slug: &str) -> Result<Arc<Mutex<Retriever>>, String> {
        if let Some(handle) = self.get(slug) {
            return Ok(handle);
        }
        let index_dir = self.pm.corpus_index_dir(slug);
        let vector_file = self.pm.corpus_vector_file(slug);
        let mut retriever =
            Retriever::new_with_paths(index_dir, vector_file).map_err(|e| e.to_string())?;
        // Apply per-corpus settings (best-effort — don't fail if DB unavailable).
        let db_path = self.pm.db_path("documents");
        if let Ok(conn) = rusqlite::Connection::open(&db_path) {
            if let Ok(settings) = crate::db::corpora::get_corpus_settings(&conn, slug) {
                if let Some(top_k) = settings.search_top_k {
                    retriever.set_search_top_k(top_k);
                }
                if let Some(metric) = settings.distance_metric {
                    retriever.distance_metric = metric;
                }
                if let Some(ef_c) = settings.hnsw_ef_construction {
                    retriever.hnsw_ef_construction = ef_c;
                }
                if let Some(ef_s) = settings.hnsw_ef_search {
                    retriever.hnsw_ef_search = ef_s;
                }
                if let Some(pq) = settings.pq_subvectors {
                    retriever.pq_subvectors = pq;
                }
            }
        }
        let handle = Arc::new(Mutex::new(retriever));
        self.retrievers
            .insert(slug.to_string(), Arc::clone(&handle));
        info!("corpus_registry: created retriever for slug='{}'", slug);
        Ok(handle)
    }
}

static CORPUS_REGISTRY: OnceLock<CorpusRegistry> = OnceLock::new();

pub fn init(pm: Arc<PathManager>) {
    let _ = CORPUS_REGISTRY.set(CorpusRegistry::new(pm));
}

pub fn get_registry() -> Option<&'static CorpusRegistry> {
    CORPUS_REGISTRY.get()
}
