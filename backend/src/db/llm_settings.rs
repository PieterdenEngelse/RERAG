use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};
use thiserror::Error;

// Default values
pub const DEFAULT_TEMPERATURE: f32 = 0.7;
pub const DEFAULT_TOP_P: f32 = 0.95;
pub const DEFAULT_TOP_K: usize = 40;
pub const DEFAULT_MAX_TOKENS: usize = 1024;
pub const DEFAULT_REPEAT_PENALTY: f32 = 1.1;
pub const DEFAULT_FREQUENCY_PENALTY: f32 = 0.0;
pub const DEFAULT_PRESENCE_PENALTY: f32 = 0.0;
pub const DEFAULT_MIN_P: f32 = 0.0;
pub const DEFAULT_TYPICAL_P: f32 = 1.0;
pub const DEFAULT_TFS_Z: f32 = 1.0;
pub const DEFAULT_MIROSTAT: i32 = 0;
pub const DEFAULT_MIROSTAT_ETA: f32 = 0.1;
pub const DEFAULT_MIROSTAT_TAU: f32 = 5.0;
pub const DEFAULT_REPEAT_LAST_N: usize = 64;
pub const DEFAULT_NUM_KEEP: i64 = 0;
pub const DEFAULT_PENALIZE_NEWLINE: bool = true;
pub const DEFAULT_IGNORE_EOS: bool = false;
pub const DEFAULT_DRY_MULTIPLIER: f32 = 0.0;
pub const DEFAULT_DRY_BASE: f32 = 1.75;
pub const DEFAULT_DRY_ALLOWED_LENGTH: usize = 2;
pub const DEFAULT_XTC_PROBABILITY: f32 = 0.0;
pub const DEFAULT_XTC_THRESHOLD: f32 = 0.1;

static GLOBAL_LLM_CONFIG: OnceLock<RwLock<LlmConfig>> = OnceLock::new();

static CONFIG_KEYS: LlmConfigKeys = LlmConfigKeys {
    temperature: "llm_temperature",
    top_p: "llm_top_p",
    top_k: "llm_top_k",
    max_tokens: "llm_max_tokens",
    repeat_penalty: "llm_repeat_penalty",
    frequency_penalty: "llm_frequency_penalty",
    presence_penalty: "llm_presence_penalty",
    stop_sequences: "llm_stop_sequences",
    seed: "llm_seed",
    min_p: "llm_min_p",
    typical_p: "llm_typical_p",
    tfs_z: "llm_tfs_z",
    mirostat: "llm_mirostat",
    mirostat_eta: "llm_mirostat_eta",
    mirostat_tau: "llm_mirostat_tau",
    repeat_last_n: "llm_repeat_last_n",
    num_keep: "llm_num_keep",
    penalize_newline: "llm_penalize_newline",
    ignore_eos: "llm_ignore_eos",
    dry_multiplier: "llm_dry_multiplier",
    dry_base: "llm_dry_base",
    dry_allowed_length: "llm_dry_allowed_length",
    xtc_probability: "llm_xtc_probability",
    xtc_threshold: "llm_xtc_threshold",
};

struct LlmConfigKeys {
    temperature: &'static str,
    top_p: &'static str,
    top_k: &'static str,
    max_tokens: &'static str,
    repeat_penalty: &'static str,
    frequency_penalty: &'static str,
    presence_penalty: &'static str,
    stop_sequences: &'static str,
    seed: &'static str,
    min_p: &'static str,
    typical_p: &'static str,
    tfs_z: &'static str,
    mirostat: &'static str,
    mirostat_eta: &'static str,
    mirostat_tau: &'static str,
    repeat_last_n: &'static str,
    num_keep: &'static str,
    penalize_newline: &'static str,
    ignore_eos: &'static str,
    dry_multiplier: &'static str,
    dry_base: &'static str,
    dry_allowed_length: &'static str,
    xtc_probability: &'static str,
    xtc_threshold: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    // Basic sampling
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: usize,
    pub max_tokens: usize,
    pub repeat_penalty: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
    pub stop_sequences: Vec<String>,
    pub seed: Option<i64>,
    pub min_p: f32,
    pub typical_p: f32,
    pub tfs_z: f32,

    // Mirostat (adaptive sampling)
    pub mirostat: i32,
    pub mirostat_eta: f32,
    pub mirostat_tau: f32,

    // Repetition control
    pub repeat_last_n: usize,
    pub penalize_newline: bool,

    // Generation limits
    pub num_keep: i64,
    pub ignore_eos: bool,

    // DRY (Don't Repeat Yourself) sampling
    pub dry_multiplier: f32,
    pub dry_base: f32,
    pub dry_allowed_length: usize,

    // XTC (eXtreme Token Control) sampling
    pub xtc_probability: f32,
    pub xtc_threshold: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            // Basic sampling
            temperature: DEFAULT_TEMPERATURE,
            top_p: DEFAULT_TOP_P,
            top_k: DEFAULT_TOP_K,
            max_tokens: DEFAULT_MAX_TOKENS,
            repeat_penalty: DEFAULT_REPEAT_PENALTY,
            frequency_penalty: DEFAULT_FREQUENCY_PENALTY,
            presence_penalty: DEFAULT_PRESENCE_PENALTY,
            stop_sequences: Vec::new(),
            seed: None,
            min_p: DEFAULT_MIN_P,
            typical_p: DEFAULT_TYPICAL_P,
            tfs_z: DEFAULT_TFS_Z,

            // Mirostat
            mirostat: DEFAULT_MIROSTAT,
            mirostat_eta: DEFAULT_MIROSTAT_ETA,
            mirostat_tau: DEFAULT_MIROSTAT_TAU,

            // Repetition control
            repeat_last_n: DEFAULT_REPEAT_LAST_N,
            penalize_newline: DEFAULT_PENALIZE_NEWLINE,

            // Generation limits
            num_keep: DEFAULT_NUM_KEEP,
            ignore_eos: DEFAULT_IGNORE_EOS,

            // DRY sampling
            dry_multiplier: DEFAULT_DRY_MULTIPLIER,
            dry_base: DEFAULT_DRY_BASE,
            dry_allowed_length: DEFAULT_DRY_ALLOWED_LENGTH,

            // XTC sampling
            xtc_probability: DEFAULT_XTC_PROBABILITY,
            xtc_threshold: DEFAULT_XTC_THRESHOLD,
        }
    }
}

impl LlmConfig {
    /// Mode 1: Documents Only (strict RAG)
    /// Factual, deterministic, sticks to retrieved content
    pub fn documents_only() -> Self {
        Self {
            temperature: 0.2,
            top_p: 0.85,
            top_k: 15,
            max_tokens: 512,
            repeat_penalty: 1.15,
            ..Default::default()
        }
    }

    /// Mode 2: LLM Only (pure generation)
    /// Creative, flexible, uses model knowledge
    pub fn llm_only() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.95,
            top_k: 40,
            max_tokens: 1024,
            repeat_penalty: 1.1,
            ..Default::default()
        }
    }

    /// Mode 3: Combined (RAG + LLM)
    /// Balanced - grounded but can expand
    pub fn combined() -> Self {
        Self {
            temperature: 0.4,
            top_p: 0.9,
            top_k: 30,
            max_tokens: 768,
            repeat_penalty: 1.12,
            ..Default::default()
        }
    }
}

#[derive(Debug, Error)]
pub enum LlmConfigError {
    #[error("database error: {0}")]
    Database(String),
    #[error("invalid value for {key}: {message}")]
    InvalidValue { key: String, message: String },
}

type Result<T> = std::result::Result<T, LlmConfigError>;

fn config_lock() -> &'static RwLock<LlmConfig> {
    GLOBAL_LLM_CONFIG.get_or_init(|| RwLock::new(LlmConfig::default()))
}

pub fn global_config() -> LlmConfig {
    config_lock().read().unwrap().clone()
}

pub fn load_active_config(conn: &Connection) {
    let cfg = load_llm_config(conn).expect("failed to load LLM settings");
    *config_lock().write().unwrap() = cfg;
}

pub fn load_llm_config(conn: &Connection) -> Result<LlmConfig> {
    let temperature = read_float(conn, CONFIG_KEYS.temperature)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_TEMPERATURE);
    let top_p = read_float(conn, CONFIG_KEYS.top_p)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_TOP_P);
    let top_k = read_int(conn, CONFIG_KEYS.top_k)?
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_TOP_K);
    let max_tokens = read_int(conn, CONFIG_KEYS.max_tokens)?
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_MAX_TOKENS);
    let repeat_penalty = read_float(conn, CONFIG_KEYS.repeat_penalty)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_REPEAT_PENALTY);
    let frequency_penalty = read_float(conn, CONFIG_KEYS.frequency_penalty)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_FREQUENCY_PENALTY);
    let presence_penalty = read_float(conn, CONFIG_KEYS.presence_penalty)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_PRESENCE_PENALTY);
    let stop_sequences = read_value(conn, CONFIG_KEYS.stop_sequences)?
        .map(|v| serde_json::from_str(&v).unwrap_or_default())
        .unwrap_or_default();
    let seed = read_int(conn, CONFIG_KEYS.seed)?;
    let min_p = read_float(conn, CONFIG_KEYS.min_p)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_MIN_P);
    let typical_p = read_float(conn, CONFIG_KEYS.typical_p)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_TYPICAL_P);
    let tfs_z = read_float(conn, CONFIG_KEYS.tfs_z)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_TFS_Z);
    let mirostat = read_int(conn, CONFIG_KEYS.mirostat)?
        .map(|v| v as i32)
        .unwrap_or(DEFAULT_MIROSTAT);
    let mirostat_eta = read_float(conn, CONFIG_KEYS.mirostat_eta)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_MIROSTAT_ETA);
    let mirostat_tau = read_float(conn, CONFIG_KEYS.mirostat_tau)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_MIROSTAT_TAU);
    let repeat_last_n = read_int(conn, CONFIG_KEYS.repeat_last_n)?
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_REPEAT_LAST_N);
    let num_keep = read_int(conn, CONFIG_KEYS.num_keep)?.unwrap_or(DEFAULT_NUM_KEEP);
    let penalize_newline = read_value(conn, CONFIG_KEYS.penalize_newline)?
        .map(|v| v == "true" || v == "1")
        .unwrap_or(DEFAULT_PENALIZE_NEWLINE);
    let ignore_eos = read_value(conn, CONFIG_KEYS.ignore_eos)?
        .map(|v| v == "true" || v == "1")
        .unwrap_or(DEFAULT_IGNORE_EOS);
    let dry_multiplier = read_float(conn, CONFIG_KEYS.dry_multiplier)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_DRY_MULTIPLIER);
    let dry_base = read_float(conn, CONFIG_KEYS.dry_base)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_DRY_BASE);
    let dry_allowed_length = read_int(conn, CONFIG_KEYS.dry_allowed_length)?
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_DRY_ALLOWED_LENGTH);
    let xtc_probability = read_float(conn, CONFIG_KEYS.xtc_probability)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_XTC_PROBABILITY);
    let xtc_threshold = read_float(conn, CONFIG_KEYS.xtc_threshold)?
        .map(|v| v as f32)
        .unwrap_or(DEFAULT_XTC_THRESHOLD);

    Ok(LlmConfig {
        temperature,
        top_p,
        top_k,
        max_tokens,
        repeat_penalty,
        frequency_penalty,
        presence_penalty,
        stop_sequences,
        seed,
        min_p,
        typical_p,
        tfs_z,
        mirostat,
        mirostat_eta,
        mirostat_tau,
        repeat_last_n,
        penalize_newline,
        num_keep,
        ignore_eos,
        dry_multiplier,
        dry_base,
        dry_allowed_length,
        xtc_probability,
        xtc_threshold,
    })
}

pub fn save_llm_config(conn: &Connection, cfg: &LlmConfig) -> Result<()> {
    conn.execute("BEGIN TRANSACTION", []).map_err(db_err)?;

    write_value(conn, CONFIG_KEYS.temperature, cfg.temperature.to_string())?;
    write_value(conn, CONFIG_KEYS.top_p, cfg.top_p.to_string())?;
    write_value(conn, CONFIG_KEYS.top_k, cfg.top_k.to_string())?;
    write_value(conn, CONFIG_KEYS.max_tokens, cfg.max_tokens.to_string())?;
    write_value(
        conn,
        CONFIG_KEYS.repeat_penalty,
        cfg.repeat_penalty.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.frequency_penalty,
        cfg.frequency_penalty.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.presence_penalty,
        cfg.presence_penalty.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.stop_sequences,
        serde_json::to_string(&cfg.stop_sequences).unwrap_or_default(),
    )?;

    if let Some(seed) = cfg.seed {
        write_value(conn, CONFIG_KEYS.seed, seed.to_string())?;
    } else {
        delete_value(conn, CONFIG_KEYS.seed)?;
    }
    write_value(conn, CONFIG_KEYS.min_p, cfg.min_p.to_string())?;
    write_value(conn, CONFIG_KEYS.typical_p, cfg.typical_p.to_string())?;
    write_value(conn, CONFIG_KEYS.tfs_z, cfg.tfs_z.to_string())?;
    write_value(conn, CONFIG_KEYS.mirostat, cfg.mirostat.to_string())?;
    write_value(conn, CONFIG_KEYS.mirostat_eta, cfg.mirostat_eta.to_string())?;
    write_value(conn, CONFIG_KEYS.mirostat_tau, cfg.mirostat_tau.to_string())?;
    write_value(
        conn,
        CONFIG_KEYS.repeat_last_n,
        cfg.repeat_last_n.to_string(),
    )?;
    write_value(conn, CONFIG_KEYS.num_keep, cfg.num_keep.to_string())?;
    write_value(
        conn,
        CONFIG_KEYS.penalize_newline,
        cfg.penalize_newline.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.ignore_eos,
        cfg.ignore_eos.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.dry_multiplier,
        cfg.dry_multiplier.to_string(),
    )?;
    write_value(conn, CONFIG_KEYS.dry_base, cfg.dry_base.to_string())?;
    write_value(
        conn,
        CONFIG_KEYS.dry_allowed_length,
        cfg.dry_allowed_length.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.xtc_probability,
        cfg.xtc_probability.to_string(),
    )?;
    write_value(
        conn,
        CONFIG_KEYS.xtc_threshold,
        cfg.xtc_threshold.to_string(),
    )?;

    conn.execute("COMMIT", []).map_err(db_err)?;
    *config_lock().write().unwrap() = cfg.clone();
    Ok(())
}

pub fn save_llm_config_default_db(cfg: &LlmConfig) -> Result<()> {
    let path = super::chunk_settings::get_db_path().expect("DB path not initialized");
    let conn = Connection::open(path).map_err(db_err)?;
    save_llm_config(&conn, cfg)
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

fn delete_value(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM config WHERE key = ?1", [key])
        .map_err(db_err)?;
    Ok(())
}

fn parse_int(key: &str, value: &str) -> Result<i64> {
    value
        .parse::<i64>()
        .map_err(|e| LlmConfigError::InvalidValue {
            key: key.to_string(),
            message: format!("{}", e),
        })
}

fn parse_float(key: &str, value: &str) -> Result<f64> {
    value
        .parse::<f64>()
        .map_err(|e| LlmConfigError::InvalidValue {
            key: key.to_string(),
            message: format!("{}", e),
        })
}

fn db_err<E: std::fmt::Display>(err: E) -> LlmConfigError {
    LlmConfigError::Database(err.to_string())
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
        let cfg = load_llm_config(&conn).unwrap();
        assert!((cfg.temperature - DEFAULT_TEMPERATURE).abs() < f32::EPSILON);
        assert!((cfg.top_p - DEFAULT_TOP_P).abs() < f32::EPSILON);
        assert_eq!(cfg.top_k, DEFAULT_TOP_K);
        assert_eq!(cfg.max_tokens, DEFAULT_MAX_TOKENS);
        assert!((cfg.repeat_penalty - DEFAULT_REPEAT_PENALTY).abs() < f32::EPSILON);
        assert!(cfg.seed.is_none());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let conn = setup_conn();
        let cfg = LlmConfig {
            temperature: 0.5,
            top_p: 0.9,
            top_k: 50,
            max_tokens: 2048,
            repeat_penalty: 1.2,
            frequency_penalty: 0.1,
            presence_penalty: 0.2,
            stop_sequences: vec!["\n".to_string()],
            seed: Some(42),
            min_p: 0.2,
            typical_p: 0.85,
            tfs_z: 0.7,
            mirostat: 1,
            mirostat_eta: 0.12,
            mirostat_tau: 6.0,
            repeat_last_n: 128,
            penalize_newline: DEFAULT_PENALIZE_NEWLINE,
            num_keep: DEFAULT_NUM_KEEP,
            ignore_eos: true,
            dry_multiplier: 0.5,
            dry_base: 2.0,
            dry_allowed_length: 3,
            xtc_probability: 0.1,
            xtc_threshold: 0.2,
        };
        save_llm_config(&conn, &cfg).unwrap();
        let loaded = load_llm_config(&conn).unwrap();
        assert!((loaded.temperature - 0.5).abs() < f32::EPSILON);
        assert!((loaded.top_p - 0.9).abs() < f32::EPSILON);
        assert_eq!(loaded.top_k, 50);
        assert_eq!(loaded.max_tokens, 2048);
        assert!((loaded.repeat_penalty - 1.2).abs() < f32::EPSILON);
        assert_eq!(loaded.seed, Some(42));
        assert!((loaded.min_p - 0.2).abs() < f32::EPSILON);
        assert!((loaded.typical_p - 0.85).abs() < f32::EPSILON);
        assert!((loaded.tfs_z - 0.7).abs() < f32::EPSILON);
        assert_eq!(loaded.mirostat, 1);
        assert!((loaded.mirostat_eta - 0.12).abs() < f32::EPSILON);
        assert!((loaded.mirostat_tau - 6.0).abs() < f32::EPSILON);
        assert_eq!(loaded.repeat_last_n, 128);
        // New fields
        assert!(loaded.ignore_eos);
        assert!((loaded.dry_multiplier - 0.5).abs() < f32::EPSILON);
        assert!((loaded.dry_base - 2.0).abs() < f32::EPSILON);
        assert_eq!(loaded.dry_allowed_length, 3);
        assert!((loaded.xtc_probability - 0.1).abs() < f32::EPSILON);
        assert!((loaded.xtc_threshold - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn save_with_none_seed() {
        let conn = setup_conn();
        let cfg = LlmConfig {
            seed: Some(123),
            ..Default::default()
        };
        save_llm_config(&conn, &cfg).unwrap();

        // Now save with None seed
        let cfg2 = LlmConfig {
            seed: None,
            ..Default::default()
        };
        save_llm_config(&conn, &cfg2).unwrap();

        let loaded = load_llm_config(&conn).unwrap();
        assert!(loaded.seed.is_none());
    }
}
