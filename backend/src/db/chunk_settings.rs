use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use thiserror::Error;

use crate::memory::chunker::{
    ChunkerConfig, DEFAULT_MAX_SIZE, DEFAULT_MIN_SIZE, DEFAULT_OVERLAP, DEFAULT_PIPELINE_STAGES,
    DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD, DEFAULT_TARGET_SIZE,
};
const DEFAULT_MODE: &str = "fixed";
const DEFAULT_CLEAN_HTML: bool = false;
const DEFAULT_CLEAN_UNICODE: bool = false;
const DEFAULT_CONTEXT_PREFIX: bool = false;
const DEFAULT_CONTEXT_PREFIX_TOKENS: usize = 32;

static GLOBAL_CHUNK_CONFIG: OnceLock<RwLock<ChunkerConfig>> = OnceLock::new();
static DB_PATH: OnceLock<PathBuf> = OnceLock::new();

static CONFIG_KEYS: ChunkConfigKeys = ChunkConfigKeys {
    target: "chunk_target_size",
    min: "chunk_min_size",
    max: "chunk_max_size",
    overlap: "chunk_overlap",
    semantic_threshold: "semantic_similarity_threshold",
    mode: "chunker_mode",
    clean_html: "chunk_clean_html",
    clean_unicode: "chunk_clean_unicode",
    context_prefix: "chunk_context_prefix",
    context_prefix_tokens: "chunk_context_prefix_tokens",
    pipeline_stages: "pipeline_stages",
};

struct ChunkConfigKeys {
    target: &'static str,
    min: &'static str,
    max: &'static str,
    overlap: &'static str,
    semantic_threshold: &'static str,
    mode: &'static str,
    clean_html: &'static str,
    clean_unicode: &'static str,
    context_prefix: &'static str,
    context_prefix_tokens: &'static str,
    pipeline_stages: &'static str,
}

#[derive(Debug, Error)]
pub enum ChunkConfigError {
    #[error("database error: {0}")]
    Database(String),
    #[error("invalid value for {key}: {message}")]
    InvalidValue { key: String, message: String },
}

type Result<T> = std::result::Result<T, ChunkConfigError>;

fn config_lock() -> &'static RwLock<ChunkerConfig> {
    GLOBAL_CHUNK_CONFIG.get_or_init(|| RwLock::new(ChunkerConfig::from_env()))
}

pub fn global_config() -> ChunkerConfig {
    config_lock().read().unwrap().clone()
}

/// Returns the active chunker mode from SQLite-backed config.
pub fn global_chunker_mode() -> crate::config::ChunkerMode {
    config_lock()
        .read()
        .unwrap()
        .mode
        .parse()
        .unwrap_or(crate::config::ChunkerMode::Fixed)
}

pub fn set_global_db_path(path: PathBuf) {
    let _ = DB_PATH.set(path);
}

pub fn get_db_path() -> Option<PathBuf> {
    DB_PATH.get().cloned()
}

pub fn load_active_config(conn: &Connection) {
    let cfg = load_chunker_config(conn).expect("failed to load chunk settings");
    *config_lock().write().unwrap() = cfg;
}

pub fn load_chunker_config(conn: &Connection) -> Result<ChunkerConfig> {
    let target = read_int(conn, CONFIG_KEYS.target)?.unwrap_or(DEFAULT_TARGET_SIZE as i64);
    let min = read_int(conn, CONFIG_KEYS.min)?.unwrap_or(DEFAULT_MIN_SIZE as i64);
    let max = read_int(conn, CONFIG_KEYS.max)?.unwrap_or(DEFAULT_MAX_SIZE as i64);
    let overlap = read_int(conn, CONFIG_KEYS.overlap)?.unwrap_or(DEFAULT_OVERLAP as i64);
    let semantic = read_float(conn, CONFIG_KEYS.semantic_threshold)?
        .unwrap_or(DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD as f64);
    let mode = read_value(conn, CONFIG_KEYS.mode)?.unwrap_or_else(|| DEFAULT_MODE.to_string());
    let clean_html = read_bool(conn, CONFIG_KEYS.clean_html)?.unwrap_or(DEFAULT_CLEAN_HTML);
    let clean_unicode =
        read_bool(conn, CONFIG_KEYS.clean_unicode)?.unwrap_or(DEFAULT_CLEAN_UNICODE);
    let context_prefix_enabled =
        read_bool(conn, CONFIG_KEYS.context_prefix)?.unwrap_or(DEFAULT_CONTEXT_PREFIX);
    let context_prefix_tokens = read_int(conn, CONFIG_KEYS.context_prefix_tokens)?
        .unwrap_or(DEFAULT_CONTEXT_PREFIX_TOKENS as i64) as usize;
    let pipeline_stages = read_value(conn, CONFIG_KEYS.pipeline_stages)?
        .unwrap_or_else(|| DEFAULT_PIPELINE_STAGES.to_string());

    let cfg = ChunkerConfig {
        target_size: target as usize,
        min_size: min as usize,
        max_size: max as usize,
        overlap: overlap as usize,
        semantic_similarity_threshold: semantic as f32,
        mode,
        clean_html,
        clean_unicode,
        context_prefix_enabled,
        context_prefix_tokens,
        pipeline_stages,
    };
    Ok(cfg)
}

pub fn save_chunker_config(conn: &Connection, cfg: &ChunkerConfig) -> Result<()> {
    conn.execute("BEGIN TRANSACTION", []).map_err(db_err)?;

    write_value(conn, CONFIG_KEYS.target, cfg.target_size.to_string())?;
    write_value(conn, CONFIG_KEYS.min, cfg.min_size.to_string())?;
    write_value(conn, CONFIG_KEYS.max, cfg.max_size.to_string())?;
    write_value(conn, CONFIG_KEYS.overlap, cfg.overlap.to_string())?;
    write_value(
        conn,
        CONFIG_KEYS.semantic_threshold,
        cfg.semantic_similarity_threshold.to_string(),
    )?;
    write_value(conn, CONFIG_KEYS.mode, cfg.mode.clone())?;
    write_value(conn, CONFIG_KEYS.clean_html, cfg.clean_html.to_string())?;
    write_value(
        conn,
        CONFIG_KEYS.clean_unicode,
        cfg.clean_unicode.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.context_prefix,
        cfg.context_prefix_enabled.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.context_prefix_tokens,
        cfg.context_prefix_tokens.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.pipeline_stages,
        cfg.pipeline_stages.clone(),
    )?;

    conn.execute("COMMIT", []).map_err(db_err)?;
    *config_lock().write().unwrap() = cfg.clone();
    Ok(())
}

pub fn save_chunker_config_default_db(cfg: &ChunkerConfig) -> Result<()> {
    let path = DB_PATH
        .get()
        .expect("DB path not initialized for chunk settings")
        .clone();
    let conn = Connection::open(path).map_err(db_err)?;
    save_chunker_config(&conn, cfg)
}

fn read_bool(conn: &Connection, key: &str) -> Result<Option<bool>> {
    Ok(read_value(conn, key)?.map(|v| v == "true" || v == "1"))
}

fn read_int(conn: &Connection, key: &str) -> Result<Option<i64>> {
    read_value(conn, key)?
        .map(|v| parse_int(key, &v))
        .transpose()
}

fn read_float(conn: &Connection, key: &str) -> Result<Option<f64>> {
    read_value(conn, key)?
        .map(|v| parse_float(key, &v))
        .transpose()
}

fn read_value(conn: &Connection, key: &str) -> Result<Option<String>> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM config WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(db_err)?;
    Ok(value)
}

fn write_value(conn: &Connection, key: &str, value: String) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO config(key, value, value_type, description, updated_at)
         VALUES(?1, ?2, 'string', NULL, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now],
    )
    .map_err(db_err)?;
    Ok(())
}

fn parse_int(key: &str, value: &str) -> Result<i64> {
    value
        .parse::<i64>()
        .map_err(|e| ChunkConfigError::InvalidValue {
            key: key.to_string(),
            message: format!("{}", e),
        })
}

fn parse_float(key: &str, value: &str) -> Result<f64> {
    value
        .parse::<f64>()
        .map_err(|e| ChunkConfigError::InvalidValue {
            key: key.to_string(),
            message: format!("{}", e),
        })
}

fn db_err<E: std::fmt::Display>(err: E) -> ChunkConfigError {
    ChunkConfigError::Database(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                value_type TEXT,
                description TEXT,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn load_returns_defaults_when_empty() {
        let conn = setup_conn();
        let cfg = load_chunker_config(&conn).unwrap();
        assert_eq!(cfg.target_size, DEFAULT_TARGET_SIZE);
        assert_eq!(cfg.min_size, DEFAULT_MIN_SIZE);
        assert_eq!(cfg.max_size, DEFAULT_MAX_SIZE);
        assert_eq!(cfg.overlap, DEFAULT_OVERLAP);
        assert!(
            (cfg.semantic_similarity_threshold - DEFAULT_SEMANTIC_SIMILARITY_THRESHOLD).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn save_then_load_roundtrip() {
        let conn = setup_conn();
        let cfg = ChunkerConfig {
            target_size: 512,
            min_size: 256,
            max_size: 768,
            overlap: 80,
            semantic_similarity_threshold: 0.9,
            mode: "fixed".to_string(),
            clean_html: false,
            clean_unicode: false,
            context_prefix_enabled: false,
            context_prefix_tokens: 32,
            pipeline_stages: "lw,sent,sem".to_string(),
        };
        save_chunker_config(&conn, &cfg).unwrap();
        let loaded = load_chunker_config(&conn).unwrap();
        assert_eq!(loaded.target_size, 512);
        assert_eq!(loaded.min_size, 256);
        assert_eq!(loaded.max_size, 768);
        assert_eq!(loaded.overlap, 80);
        assert!((loaded.semantic_similarity_threshold - 0.9).abs() < f32::EPSILON);
    }
}
