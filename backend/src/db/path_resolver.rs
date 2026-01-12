use crate::path_manager::PathManager;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const DEFAULT_AGENT_DB_FILE: &str = "agent.db";
const ENV_AGENT_DB_PATH: &str = "AGENT_DB_PATH";

static PATH_MANAGER: OnceLock<PathManager> = OnceLock::new();
static AGENT_DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static AGENT_DB_STRING: OnceLock<String> = OnceLock::new();

fn path_manager() -> &'static PathManager {
    PATH_MANAGER.get_or_init(|| PathManager::new().expect("Failed to initialize PathManager"))
}

/// Resolve a database path taking into account absolute paths, the current working
/// directory, and the configured AG_HOME base directory.
pub fn resolve_db_path(db_path: &str) -> PathBuf {
    let target = if db_path.is_empty() {
        DEFAULT_AGENT_DB_FILE
    } else {
        db_path
    };

    let candidate = Path::new(target);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }

    if candidate.exists() {
        return candidate.to_path_buf();
    }

    let fallback = path_manager().base_dir().join(target);
    if !fallback.exists() {
        if let Some(parent) = fallback.parent() {
            let _ = fs::create_dir_all(parent);
        }
    }
    fallback
}

/// Return the resolved agent database path, honoring the optional AGENT_DB_PATH env var.
pub fn agent_db_path() -> &'static PathBuf {
    AGENT_DB_PATH.get_or_init(|| {
        let configured =
            env::var(ENV_AGENT_DB_PATH).unwrap_or_else(|_| DEFAULT_AGENT_DB_FILE.to_string());
        resolve_db_path(&configured)
    })
}

/// Convenience helper that returns the agent database path as an owned `String`.
pub fn agent_db_path_string() -> String {
    agent_db_path().to_string_lossy().into_owned()
}

/// Convenience helper that returns the agent database path as a `&'static str`.
pub fn agent_db_path_str() -> &'static str {
    AGENT_DB_STRING
        .get_or_init(|| agent_db_path_string())
        .as_str()
}
