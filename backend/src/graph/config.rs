// backend/src/graph/config.rs
// Configuration for Neo4j Knowledge Graph integration

use serde::{Deserialize, Serialize};
use std::env;

/// Neo4j connection and feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Enable/disable Neo4j integration
    pub enabled: bool,

    /// Neo4j Bolt URI (e.g., "bolt://localhost:7687")
    pub uri: String,

    /// Neo4j username
    pub user: String,

    /// Neo4j password (not serialized for security)
    #[serde(skip_serializing)]
    pub password: String,

    /// Database name (default: "neo4j")
    pub database: String,

    /// Maximum connections in pool
    pub max_connections: usize,

    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,

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
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            uri: "bolt://localhost:7687".to_string(),
            user: "neo4j".to_string(),
            password: "password".to_string(),
            database: "neo4j".to_string(),
            max_connections: 10,
            connection_timeout_ms: 5000,
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
        }
    }
}

impl GraphConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: env::var("NEO4J_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),

            uri: env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string()),

            user: env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string()),

            password: env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "password".to_string()),

            database: env::var("NEO4J_DATABASE").unwrap_or_else(|_| "neo4j".to_string()),

            max_connections: env::var("NEO4J_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),

            connection_timeout_ms: env::var("NEO4J_CONNECTION_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),

            expansion: GraphExpansionSettings::from_env(),
            entity_extraction: EntityExtractionSettings::from_env(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.enabled {
            if self.uri.is_empty() {
                return Err("NEO4J_URI is required when Neo4j is enabled".to_string());
            }
            if self.user.is_empty() {
                return Err("NEO4J_USER is required when Neo4j is enabled".to_string());
            }
            if self.password.is_empty() {
                return Err("NEO4J_PASSWORD is required when Neo4j is enabled".to_string());
            }
        }
        Ok(())
    }
}

impl GraphExpansionSettings {
    pub fn from_env() -> Self {
        Self {
            enabled: env::var("GRAPH_EXPANSION_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),

            max_hops: env::var("GRAPH_EXPANSION_MAX_HOPS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),

            max_chunks: env::var("GRAPH_EXPANSION_MAX_CHUNKS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),

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
        assert_eq!(config.uri, "bolt://localhost:7687");
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
