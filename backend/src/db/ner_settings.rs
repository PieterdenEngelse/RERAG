//! SQLite-backed persistence for NER (Named Entity Recognition) configuration.
//!
//! All settings are stored as key-value rows in the shared `config` table,
//! mirroring the pattern used by `chunk_settings`.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::{OnceLock, RwLock};
use thiserror::Error;

// ── Defaults ─────────────────────────────────────────────────────────────────

const DEFAULT_EXTRACTION_ENABLED: bool = true;
const DEFAULT_TYPE_ALLOWLIST: &str = "PERSON,ORGANIZATION,LOCATION,PRODUCT";
const DEFAULT_CONFIDENCE_THRESHOLD: f64 = 0.85;
const DEFAULT_TYPE_THRESHOLDS: &str = r#"{"PERSON":0.75,"ORGANIZATION":0.95,"PRODUCT":0.95}"#;
const DEFAULT_FUZZY_THRESHOLD: f64 = 0.8;
const DEFAULT_MIN_LENGTH: usize = 2;
const DEFAULT_MAX_LENGTH: usize = 100;
const DEFAULT_DEDUP_CASE_INSENSITIVE: bool = true;
const DEFAULT_NESTING_STRATEGY: &str = "KeepLongest";
const DEFAULT_BATCH_SIZE: usize = 4;
const DEFAULT_QUANTIZATION_ENABLED: bool = false;
const DEFAULT_MODEL_CACHE_ENABLED: bool = true;
const DEFAULT_GRAPH_STORAGE_ENABLED: bool = true;

// ── Config struct ─────────────────────────────────────────────────────────────

/// NER runtime configuration — persisted in SQLite, hot-reloaded on save.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NerConfig {
    pub extraction_enabled: bool,
    pub type_allowlist: String,
    pub confidence_threshold: f64,
    pub type_thresholds: String,
    pub fuzzy_threshold: f64,
    pub min_length: usize,
    pub max_length: usize,
    pub dedup_case_insensitive: bool,
    pub nesting_strategy: String,
    pub batch_size: usize,
    pub quantization_enabled: bool,
    pub model_cache_enabled: bool,
    pub graph_storage_enabled: bool,
}

impl Default for NerConfig {
    fn default() -> Self {
        Self {
            extraction_enabled: std::env::var("ENTITY_EXTRACTION_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(DEFAULT_EXTRACTION_ENABLED),
            type_allowlist: std::env::var("ENTITY_CONTROL_TYPE_ALLOWLIST")
                .unwrap_or_else(|_| DEFAULT_TYPE_ALLOWLIST.to_string()),
            confidence_threshold: std::env::var("ENTITY_QUALITY_CONFIDENCE_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_CONFIDENCE_THRESHOLD),
            type_thresholds: std::env::var("ENTITY_QUALITY_TYPE_THRESHOLDS")
                .unwrap_or_else(|_| DEFAULT_TYPE_THRESHOLDS.to_string()),
            fuzzy_threshold: std::env::var("ENTITY_LINKING_FUZZY_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_FUZZY_THRESHOLD),
            min_length: std::env::var("ENTITY_FILTER_MIN_LENGTH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_MIN_LENGTH),
            max_length: std::env::var("ENTITY_FILTER_MAX_LENGTH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_MAX_LENGTH),
            dedup_case_insensitive: std::env::var("ENTITY_FILTER_DEDUPLICATE_CASE_INSENSITIVE")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(DEFAULT_DEDUP_CASE_INSENSITIVE),
            nesting_strategy: std::env::var("ENTITY_FILTER_NESTING_STRATEGY")
                .unwrap_or_else(|_| DEFAULT_NESTING_STRATEGY.to_string()),
            batch_size: std::env::var("ENTITY_PERFORMANCE_BATCH_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_BATCH_SIZE),
            quantization_enabled: std::env::var("ENTITY_PERFORMANCE_QUANTIZATION_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(DEFAULT_QUANTIZATION_ENABLED),
            model_cache_enabled: std::env::var("ENTITY_PERFORMANCE_MODEL_CACHE_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(DEFAULT_MODEL_CACHE_ENABLED),
            graph_storage_enabled: std::env::var("ENTITY_INTEGRATION_GRAPH_STORAGE_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(DEFAULT_GRAPH_STORAGE_ENABLED),
        }
    }
}

// ── Key names ─────────────────────────────────────────────────────────────────

struct NerConfigKeys {
    extraction_enabled: &'static str,
    type_allowlist: &'static str,
    confidence_threshold: &'static str,
    type_thresholds: &'static str,
    fuzzy_threshold: &'static str,
    min_length: &'static str,
    max_length: &'static str,
    dedup_case_insensitive: &'static str,
    nesting_strategy: &'static str,
    batch_size: &'static str,
    quantization_enabled: &'static str,
    model_cache_enabled: &'static str,
    graph_storage_enabled: &'static str,
}

static CONFIG_KEYS: NerConfigKeys = NerConfigKeys {
    extraction_enabled: "ner_extraction_enabled",
    type_allowlist: "ner_type_allowlist",
    confidence_threshold: "ner_confidence_threshold",
    type_thresholds: "ner_type_thresholds",
    fuzzy_threshold: "ner_fuzzy_threshold",
    min_length: "ner_min_length",
    max_length: "ner_max_length",
    dedup_case_insensitive: "ner_dedup_case_insensitive",
    nesting_strategy: "ner_nesting_strategy",
    batch_size: "ner_batch_size",
    quantization_enabled: "ner_quantization_enabled",
    model_cache_enabled: "ner_model_cache_enabled",
    graph_storage_enabled: "ner_graph_storage_enabled",
};

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum NerConfigError {
    #[error("database error: {0}")]
    Database(String),
    #[error("invalid value for {key}: {message}")]
    InvalidValue { key: String, message: String },
}

type Result<T> = std::result::Result<T, NerConfigError>;

// ── Global in-process cache ───────────────────────────────────────────────────

static GLOBAL_NER_CONFIG: OnceLock<RwLock<NerConfig>> = OnceLock::new();

fn config_lock() -> &'static RwLock<NerConfig> {
    GLOBAL_NER_CONFIG.get_or_init(|| RwLock::new(NerConfig::default()))
}

pub fn global_config() -> NerConfig {
    config_lock().read().unwrap().clone()
}

pub fn load_active_config(conn: &Connection) {
    match load_ner_config(conn) {
        Ok(cfg) => *config_lock().write().unwrap() = cfg,
        Err(e) => tracing::warn!("Failed to load NER config from DB, using defaults: {}", e),
    }
}

// ── DB helpers ────────────────────────────────────────────────────────────────

pub fn load_ner_config(conn: &Connection) -> Result<NerConfig> {
    let extraction_enabled =
        read_bool(conn, CONFIG_KEYS.extraction_enabled)?.unwrap_or(DEFAULT_EXTRACTION_ENABLED);
    let type_allowlist = read_value(conn, CONFIG_KEYS.type_allowlist)?
        .unwrap_or_else(|| DEFAULT_TYPE_ALLOWLIST.to_string());
    let confidence_threshold =
        read_float(conn, CONFIG_KEYS.confidence_threshold)?.unwrap_or(DEFAULT_CONFIDENCE_THRESHOLD);
    let type_thresholds = read_value(conn, CONFIG_KEYS.type_thresholds)?
        .unwrap_or_else(|| DEFAULT_TYPE_THRESHOLDS.to_string());
    let fuzzy_threshold =
        read_float(conn, CONFIG_KEYS.fuzzy_threshold)?.unwrap_or(DEFAULT_FUZZY_THRESHOLD);
    let min_length =
        read_int(conn, CONFIG_KEYS.min_length)?.unwrap_or(DEFAULT_MIN_LENGTH as i64) as usize;
    let max_length =
        read_int(conn, CONFIG_KEYS.max_length)?.unwrap_or(DEFAULT_MAX_LENGTH as i64) as usize;
    let dedup_case_insensitive = read_bool(conn, CONFIG_KEYS.dedup_case_insensitive)?
        .unwrap_or(DEFAULT_DEDUP_CASE_INSENSITIVE);
    let nesting_strategy = read_value(conn, CONFIG_KEYS.nesting_strategy)?
        .unwrap_or_else(|| DEFAULT_NESTING_STRATEGY.to_string());
    let batch_size =
        read_int(conn, CONFIG_KEYS.batch_size)?.unwrap_or(DEFAULT_BATCH_SIZE as i64) as usize;
    let quantization_enabled =
        read_bool(conn, CONFIG_KEYS.quantization_enabled)?.unwrap_or(DEFAULT_QUANTIZATION_ENABLED);
    let model_cache_enabled =
        read_bool(conn, CONFIG_KEYS.model_cache_enabled)?.unwrap_or(DEFAULT_MODEL_CACHE_ENABLED);
    let graph_storage_enabled = read_bool(conn, CONFIG_KEYS.graph_storage_enabled)?
        .unwrap_or(DEFAULT_GRAPH_STORAGE_ENABLED);

    Ok(NerConfig {
        extraction_enabled,
        type_allowlist,
        confidence_threshold,
        type_thresholds,
        fuzzy_threshold,
        min_length,
        max_length,
        dedup_case_insensitive,
        nesting_strategy,
        batch_size,
        quantization_enabled,
        model_cache_enabled,
        graph_storage_enabled,
    })
}

pub fn save_ner_config(conn: &Connection, cfg: &NerConfig) -> Result<()> {
    conn.execute("BEGIN TRANSACTION", []).map_err(db_err)?;

    write_value(
        conn,
        CONFIG_KEYS.extraction_enabled,
        cfg.extraction_enabled.to_string(),
    )?;
    write_value(conn, CONFIG_KEYS.type_allowlist, cfg.type_allowlist.clone())?;
    write_value(
        conn,
        CONFIG_KEYS.confidence_threshold,
        cfg.confidence_threshold.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.type_thresholds,
        cfg.type_thresholds.clone(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.fuzzy_threshold,
        cfg.fuzzy_threshold.to_string(),
    )?;
    write_value(conn, CONFIG_KEYS.min_length, cfg.min_length.to_string())?;
    write_value(conn, CONFIG_KEYS.max_length, cfg.max_length.to_string())?;
    write_value(
        conn,
        CONFIG_KEYS.dedup_case_insensitive,
        cfg.dedup_case_insensitive.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.nesting_strategy,
        cfg.nesting_strategy.clone(),
    )?;
    write_value(conn, CONFIG_KEYS.batch_size, cfg.batch_size.to_string())?;
    write_value(
        conn,
        CONFIG_KEYS.quantization_enabled,
        cfg.quantization_enabled.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.model_cache_enabled,
        cfg.model_cache_enabled.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.graph_storage_enabled,
        cfg.graph_storage_enabled.to_string(),
    )?;

    conn.execute("COMMIT", []).map_err(db_err)?;
    *config_lock().write().unwrap() = cfg.clone();
    Ok(())
}

pub fn save_ner_config_default_db(cfg: &NerConfig) -> Result<()> {
    let path =
        crate::db::chunk_settings::get_db_path().expect("DB path not initialized for NER settings");
    let conn = Connection::open(path).map_err(db_err)?;
    save_ner_config(&conn, cfg)
}

// ── Low-level read/write ──────────────────────────────────────────────────────

fn read_bool(conn: &Connection, key: &str) -> Result<Option<bool>> {
    Ok(read_value(conn, key)?.map(|v| v == "true" || v == "1"))
}

fn read_int(conn: &Connection, key: &str) -> Result<Option<i64>> {
    read_value(conn, key)?
        .map(|v| {
            v.parse::<i64>().map_err(|e| NerConfigError::InvalidValue {
                key: key.to_string(),
                message: e.to_string(),
            })
        })
        .transpose()
}

fn read_float(conn: &Connection, key: &str) -> Result<Option<f64>> {
    read_value(conn, key)?
        .map(|v| {
            v.parse::<f64>().map_err(|e| NerConfigError::InvalidValue {
                key: key.to_string(),
                message: e.to_string(),
            })
        })
        .transpose()
}

fn read_value(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row("SELECT value FROM config WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    })
    .optional()
    .map_err(db_err)
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

fn db_err<E: std::fmt::Display>(err: E) -> NerConfigError {
    NerConfigError::Database(err.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

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
        let cfg = load_ner_config(&conn).unwrap();
        assert_eq!(cfg.extraction_enabled, DEFAULT_EXTRACTION_ENABLED);
        assert_eq!(cfg.type_allowlist, DEFAULT_TYPE_ALLOWLIST);
        assert!((cfg.confidence_threshold - DEFAULT_CONFIDENCE_THRESHOLD).abs() < f64::EPSILON);
        assert_eq!(cfg.min_length, DEFAULT_MIN_LENGTH);
        assert_eq!(cfg.max_length, DEFAULT_MAX_LENGTH);
        assert_eq!(cfg.nesting_strategy, DEFAULT_NESTING_STRATEGY);
        assert_eq!(cfg.batch_size, DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let conn = setup_conn();
        let cfg = NerConfig {
            extraction_enabled: false,
            type_allowlist: "PERSON,LOCATION".to_string(),
            confidence_threshold: 0.92,
            type_thresholds: r#"{"PERSON":0.8}"#.to_string(),
            fuzzy_threshold: 0.75,
            min_length: 3,
            max_length: 50,
            dedup_case_insensitive: false,
            nesting_strategy: "KeepAll".to_string(),
            batch_size: 8,
            quantization_enabled: true,
            model_cache_enabled: false,
            graph_storage_enabled: false,
        };
        save_ner_config(&conn, &cfg).unwrap();
        let loaded = load_ner_config(&conn).unwrap();
        assert!(!loaded.extraction_enabled);
        assert_eq!(loaded.type_allowlist, "PERSON,LOCATION");
        assert!((loaded.confidence_threshold - 0.92).abs() < f64::EPSILON);
        assert_eq!(loaded.min_length, 3);
        assert_eq!(loaded.max_length, 50);
        assert!(!loaded.dedup_case_insensitive);
        assert_eq!(loaded.nesting_strategy, "KeepAll");
        assert_eq!(loaded.batch_size, 8);
        assert!(loaded.quantization_enabled);
        assert!(!loaded.model_cache_enabled);
        assert!(!loaded.graph_storage_enabled);
    }
}
