// ag/src/path_manager.rs v13.1.2
use std::env;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PathError {
    #[error("Failed to create directory: {0}")]
    CreateDirFailed(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Environment variable not set: {0}")]
    EnvVarNotSet(String),
}
#[derive(Debug, Clone)]
pub struct PathManager {
    base_dir: PathBuf,
    data_dir: PathBuf,
    index_dir: PathBuf,
    db_dir: PathBuf,
    logs_dir: PathBuf,
    cache_dir: PathBuf,
}

impl PathManager {
    pub fn new() -> Result<Self, PathError> {
        let base_dir = Self::get_base_dir()?;
        let data_dir = base_dir.join("data");
        let index_dir = base_dir.join("index");
        let db_dir = base_dir.join("db");
        let logs_dir = base_dir.join("logs");
        let cache_dir = base_dir.join("cache");

        Self::ensure_dir(&data_dir)?;
        Self::ensure_dir(&index_dir)?;
        Self::ensure_dir(&db_dir)?;
        Self::ensure_dir(&logs_dir)?;
        Self::ensure_dir(&cache_dir)?;

        Ok(Self {
            base_dir,
            data_dir,
            index_dir,
            db_dir,
            logs_dir,
            cache_dir,
        })
    }

    fn get_base_dir() -> Result<PathBuf, PathError> {
        match env::var("AG_HOME") {
            Ok(path) => Ok(PathBuf::from(path)),
            Err(_) => dirs::data_local_dir()
                .ok_or_else(|| PathError::EnvVarNotSet("AG_HOME or platform data dir".into()))
                .map(|p| p.join("ag")),
        }
    }

    fn ensure_dir(path: &Path) -> Result<(), PathError> {
        if !path.exists() {
            std::fs::create_dir_all(path)
                .map_err(|e| PathError::CreateDirFailed(format!("{}: {}", path.display(), e)))?;
        }
        Ok(())
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn index_dir(&self) -> &Path {
        &self.index_dir
    }

    pub fn db_dir(&self) -> &Path {
        &self.db_dir
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    // Added: locks directory under AG_HOME (e.g., ~/.local/share/ag/locks)
    pub fn locks_dir(&self) -> PathBuf {
        let p = self.base_dir.join("locks");
        let _ = std::fs::create_dir_all(&p);
        p
    }

    pub fn db_path(&self, name: &str) -> PathBuf {
        self.db_dir.join(format!("{}.db", name))
    }

    pub fn index_path(&self, name: &str) -> PathBuf {
        self.index_dir.join(name)
    }

    pub fn vector_store_path(&self) -> PathBuf {
        self.data_dir.join("vectors.json")
    }

    pub fn log_path(&self, name: &str) -> PathBuf {
        self.logs_dir.join(format!("{}.log", name))
    }

    pub fn cache_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(key)
    }

    /// Tantivy index directory for a named corpus.
    /// The 'default' corpus uses the legacy `index/tantivy` path for zero-migration.
    /// All other corpora land in `index/{slug}`.
    pub fn corpus_index_dir(&self, slug: &str) -> PathBuf {
        let p = if slug == "default" {
            self.index_dir.join("tantivy")
        } else {
            self.index_dir.join(slug)
        };
        let _ = std::fs::create_dir_all(&p);
        p
    }

    /// Upload directory for a named corpus (absolute path).
    /// All corpora store documents at `{data_dir}/corpora/{slug}/documents/`.
    pub fn corpus_upload_dir(&self, slug: &str) -> PathBuf {
        let p = self.data_dir.join("corpora").join(slug).join("documents");
        let _ = std::fs::create_dir_all(&p);
        p
    }

    /// Vector store JSON file for a named corpus.
    /// The 'default' corpus reuses the existing `data/vectors.json` for zero-migration.
    pub fn corpus_vector_file(&self, slug: &str) -> PathBuf {
        if slug == "default" {
            self.data_dir.join("vectors.json")
        } else {
            let dir = self.data_dir.join("corpora").join(slug);
            let _ = std::fs::create_dir_all(&dir);
            dir.join("vectors.json")
        }
    }
}

impl Default for PathManager {
    fn default() -> Self {
        Self::new().expect("Failed to initialize PathManager")
    }
}
