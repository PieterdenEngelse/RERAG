// backend/src/graph/config.rs
// Configuration for FalkorDB Knowledge Graph integration

use serde::{Deserialize, Serialize};
use std::env;

/// FalkorDB connection and feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Enable/disable FalkorDB integration
    pub enabled: bool,

    /// FalkorDB connection URI (e.g., "redis://localhost:6380")
    pub uri: String,

    /// FalkorDB password (not serialized for security)
    #[serde(skip_serializing)]
    pub password: String,

    /// Database name (default: "ag")
    pub database: String,

    /// Maximum connections in pool
    pub max_connections: usize,

    /// Connection timeout in milliseconds (time to *open* a connection)
    pub connection_timeout_ms: u64,

    /// Per-query command timeout in milliseconds (0 = no timeout). Caps how
    /// long a FalkorDB query may run; enforced server-side.
    pub command_timeout_ms: u64,

    /// Graph expansion settings
    pub expansion: GraphExpansionSettings,

    /// Entity extraction settings
    pub entity_extraction: EntityExtractionSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphExpansionSettings {
    /// Enable graph-based context expansion
    pub enabled: bool,

    /// Maximum hops for graph traversal
    pub max_hops: usize,

    /// Maximum chunks to add via expansion
    pub max_chunks: usize,

    /// Weight for entity-based expansion (0.0-1.0)
    pub entity_weight: f32,

    /// Weight for concept-based expansion (0.0-1.0)
    pub concept_weight: f32,

    /// Minimum relationship strength to follow
    pub min_relationship_strength: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExtractionSettings {
    /// Enable entity extraction during indexing
    pub enabled: bool,

    /// Minimum confidence for entity extraction
    pub confidence_threshold: f32,

    /// Fuzzy matching threshold for entity linking
    pub fuzzy_threshold: f32,

    /// Entity types to extract
    pub entity_types: Vec<String>,

    /// Entity reconciler: vector-similarity + optional LLM tiebreak to collapse
    /// "Sony Corp" / "Sony Corporation" / "Sony Interactive Entertainment" into
    /// a single canonical :Entity node.  Off by default — costs one embed +
    /// up to one LLM call per entity mention.
    pub reconcile_enabled: bool,
    /// Cosine score at or above which two candidates are merged with no LLM call.
    pub reconcile_auto_merge_threshold: f32,
    /// Cosine score at or above which the LLM is asked to decide.  Below this,
    /// the candidate becomes a new node with no LLM call.
    pub reconcile_llm_review_threshold: f32,
    /// Top-k neighbours pulled from the entity vector index per candidate.
    pub reconcile_vector_topk: usize,
    /// Hard cap on LLM tiebreak calls per ingested document.  Past this cap,
    /// borderline candidates short-circuit to auto-new.
    pub reconcile_llm_review_max_per_doc: usize,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            uri: "redis://localhost:6380".to_string(),
            password: "password".to_string(),
            database: "ag".to_string(),
            max_connections: 10,
            connection_timeout_ms: 5000,
            command_timeout_ms: 0,
            expansion: GraphExpansionSettings::default(),
            entity_extraction: EntityExtractionSettings::default(),
        }
    }
}

impl Default for GraphExpansionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_hops: 2,
            max_chunks: 10,
            entity_weight: 0.7,
            concept_weight: 0.5,
            min_relationship_strength: 0.3,
        }
    }
}

impl Default for EntityExtractionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            confidence_threshold: 0.5,
            fuzzy_threshold: 0.8,
            entity_types: vec![
                "PERSON".to_string(),
                "ORGANIZATION".to_string(),
                "LOCATION".to_string(),
                "CONCEPT".to_string(),
                "TECHNOLOGY".to_string(),
                "EVENT".to_string(),
            ],
            reconcile_enabled: false,
            reconcile_auto_merge_threshold: 0.92,
            reconcile_llm_review_threshold: 0.75,
            reconcile_vector_topk: 5,
            reconcile_llm_review_max_per_doc: 50,
        }
    }
}

/// Apply UI-saved overrides from `.env.graph` (written by the config page's
/// Save button) into the process environment.
///
/// Values here intentionally OVERRIDE existing environment variables: a UI
/// Save is the user's explicit, most-recent intent and should win over vars
/// inherited from `ag.env` / `.env`. Delete `.env.graph` to revert.
fn load_env_graph_file() {
    let Ok(content) = std::fs::read_to_string(".env.graph") else {
        return;
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            env::set_var(key.trim(), value.trim());
        }
    }
}

impl GraphConfig {
    /// Load configuration from environment variables (and `.env.graph` overrides)
    pub fn from_env() -> Self {
        // UI-saved overrides (.env.graph) win over inherited env vars.
        load_env_graph_file();
        Self {
            enabled: crate::settings::effective_bool("FALKOR_ENABLED", false),

            uri: crate::settings::effective_or("FALKOR_URI", "redis://localhost:6380"),

            password: env::var("FALKOR_PASSWORD").unwrap_or_else(|_| "password".to_string()),

            database: env::var("FALKOR_DATABASE").unwrap_or_else(|_| "ag".to_string()),

            max_connections: env::var("FALKOR_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),

            connection_timeout_ms: env::var("FALKOR_CONNECTION_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),

            command_timeout_ms: env::var("FALKOR_COMMAND_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),

            expansion: GraphExpansionSettings::from_env(),
            entity_extraction: EntityExtractionSettings::from_env(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.enabled {
            if self.uri.is_empty() {
                return Err("FALKOR_URI is required when FalkorDB is enabled".to_string());
            }
            if self.password.is_empty() {
                return Err("FALKOR_PASSWORD is required when FalkorDB is enabled".to_string());
            }
        }
        Ok(())
    }
}

impl GraphExpansionSettings {
    pub fn from_env() -> Self {
        Self {
            enabled: crate::settings::effective_bool("GRAPH_EXPANSION_ENABLED", true),

            max_hops: crate::settings::effective_u64("GRAPH_EXPANSION_MAX_HOPS", 2) as usize,

            max_chunks: crate::settings::effective_u64("GRAPH_EXPANSION_MAX_CHUNKS", 10) as usize,

            entity_weight: env::var("GRAPH_ENTITY_WEIGHT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.7),

            concept_weight: env::var("GRAPH_CONCEPT_WEIGHT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),

            min_relationship_strength: env::var("GRAPH_MIN_RELATIONSHIP_STRENGTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3),
        }
    }
}

impl EntityExtractionSettings {
    pub fn from_env() -> Self {
        Self {
            enabled: env::var("ENTITY_EXTRACTION_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),

            confidence_threshold: env::var("ENTITY_EXTRACTION_CONFIDENCE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),

            fuzzy_threshold: env::var("ENTITY_LINKING_FUZZY_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.8),

            entity_types: env::var("ENTITY_EXTRACTION_TYPES")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_else(|_| EntityExtractionSettings::default().entity_types),

            reconcile_enabled: crate::settings::effective_bool("RECONCILER_ENABLED", false),
            reconcile_auto_merge_threshold: env::var("RECONCILER_AUTO_MERGE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.92),
            reconcile_llm_review_threshold: env::var("RECONCILER_LLM_REVIEW_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.75),
            reconcile_vector_topk: env::var("RECONCILER_VECTOR_TOPK")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            reconcile_llm_review_max_per_doc: env::var("RECONCILER_LLM_MAX_PER_DOC")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GraphConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.uri, "redis://localhost:6380");
        assert_eq!(config.max_connections, 10);
    }

    #[test]
    fn test_validation_disabled() {
        let config = GraphConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_enabled_missing_uri() {
        let config = GraphConfig {
            enabled: true,
            uri: String::new(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
