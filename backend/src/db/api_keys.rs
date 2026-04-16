//! API key storage for cloud providers (OpenAI, Anthropic).
//!
//! Keys are stored encrypted in the database using the param_store module.
//! Environment variables take precedence over stored keys.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use super::param_store::{self, ParamStoreError};

const CONFIG_TYPE: &str = "api_keys";

/// API keys configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeys {
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default)]
    pub anthropic_api_key: String,
    #[serde(default)]
    pub openrouter_api_key: String,
}

impl ApiKeys {
    /// Check if OpenAI key is configured (env var or stored)
    pub fn has_openai_key(&self) -> bool {
        std::env::var("OPENAI_API_KEY").is_ok() || !self.openai_api_key.is_empty()
    }

    /// Check if Anthropic key is configured (env var or stored)
    pub fn has_anthropic_key(&self) -> bool {
        std::env::var("ANTHROPIC_API_KEY").is_ok() || !self.anthropic_api_key.is_empty()
    }

    /// Check if OpenRouter key is configured (env var or stored)
    pub fn has_openrouter_key(&self) -> bool {
        std::env::var("OPENROUTER_API_KEY").is_ok() || !self.openrouter_api_key.is_empty()
    }

    /// Get the effective OpenAI API key (env var takes precedence)
    pub fn get_openai_key(&self) -> Option<String> {
        std::env::var("OPENAI_API_KEY").ok().or_else(|| {
            if self.openai_api_key.is_empty() {
                None
            } else {
                Some(self.openai_api_key.clone())
            }
        })
    }

    /// Get the effective Anthropic API key (env var takes precedence)
    pub fn get_anthropic_key(&self) -> Option<String> {
        std::env::var("ANTHROPIC_API_KEY").ok().or_else(|| {
            if self.anthropic_api_key.is_empty() {
                None
            } else {
                Some(self.anthropic_api_key.clone())
            }
        })
    }

    /// Get the effective OpenRouter API key (env var takes precedence)
    pub fn get_openrouter_key(&self) -> Option<String> {
        std::env::var("OPENROUTER_API_KEY").ok().or_else(|| {
            if self.openrouter_api_key.is_empty() {
                None
            } else {
                Some(self.openrouter_api_key.clone())
            }
        })
    }

    /// Mask a key for display (show first 4 and last 4 chars)
    pub fn mask_key(key: &str) -> String {
        if key.len() <= 12 {
            "*".repeat(key.len())
        } else {
            format!("{}...{}", &key[..4], &key[key.len() - 4..])
        }
    }
}

// Global in-memory cache
static API_KEYS: OnceLock<RwLock<ApiKeys>> = OnceLock::new();

fn get_cache() -> &'static RwLock<ApiKeys> {
    API_KEYS.get_or_init(|| RwLock::new(ApiKeys::default()))
}

/// Load API keys from database, falling back to defaults
fn open_default_conn() -> Result<rusqlite::Connection, ParamStoreError> {
    let path = crate::db::chunk_settings::get_db_path().ok_or_else(|| {
        ParamStoreError::Database(
            "Database path not initialized for API keys. Did you call set_global_db_path()?".into(),
        )
    })?;
    rusqlite::Connection::open(&path).map_err(|e| ParamStoreError::Database(e.to_string()))
}

pub fn load_from_db() -> Result<ApiKeys, ParamStoreError> {
    let conn = open_default_conn()?;

    param_store::init_table(&conn)?;
    let keys = param_store::load_or_default::<ApiKeys>(&conn, CONFIG_TYPE)?;

    // Update cache
    *get_cache().write() = keys.clone();

    Ok(keys)
}

/// Save API keys to database
pub fn save_to_db(keys: &ApiKeys) -> Result<(), ParamStoreError> {
    let conn = open_default_conn()?;

    param_store::init_table(&conn)?;
    param_store::save(&conn, CONFIG_TYPE, keys)?;

    // Update cache
    *get_cache().write() = keys.clone();

    Ok(())
}

/// Get current API keys (from cache, loads from DB if needed)
pub fn global_config() -> ApiKeys {
    let cache = get_cache();
    let keys = cache.read().clone();

    // If cache is empty, try loading from DB
    if keys.openai_api_key.is_empty()
        && keys.anthropic_api_key.is_empty()
        && keys.openrouter_api_key.is_empty()
    {
        if let Ok(loaded) = load_from_db() {
            return loaded;
        }
    }

    keys
}

/// Update API keys in cache and database
pub fn update_config(keys: ApiKeys) -> Result<(), ParamStoreError> {
    save_to_db(&keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_key() {
        assert_eq!(ApiKeys::mask_key("sk-1234567890abcdef"), "sk-1...cdef");
        assert_eq!(ApiKeys::mask_key("short"), "*****");
        assert_eq!(ApiKeys::mask_key(""), "");
    }

    #[test]
    fn test_default_keys() {
        let keys = ApiKeys::default();
        assert!(keys.openai_api_key.is_empty());
        assert!(keys.anthropic_api_key.is_empty());
        assert!(!keys.has_openai_key());
        assert!(!keys.has_anthropic_key());
    }
}
