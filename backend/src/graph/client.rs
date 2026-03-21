// backend/src/graph/client.rs
// Neo4j Bolt driver client with connection pooling
//
// Requires: cargo build --features neo4j

use crate::graph::config::GraphConfig;
use neo4rs::{ConfigBuilder, Graph, Query};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Neo4j client wrapper with connection management
#[derive(Clone)]
pub struct Neo4jClient {
    graph: Arc<Graph>,
    #[allow(dead_code)]
    config: GraphConfig,
    connected: bool,
}

/// Error type for Neo4j operations
#[derive(Debug, thiserror::Error)]
pub enum Neo4jError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Feature disabled")]
    Disabled,
}

impl From<neo4rs::Error> for Neo4jError {
    fn from(err: neo4rs::Error) -> Self {
        Neo4jError::Query(err.to_string())
    }
}

impl From<neo4rs::DeError> for Neo4jError {
    fn from(err: neo4rs::DeError) -> Self {
        Neo4jError::Deserialization(err.to_string())
    }
}

impl Neo4jClient {
    /// Create a new Neo4j client and establish connection
    pub async fn new(config: GraphConfig) -> Result<Self, Neo4jError> {
        if !config.enabled {
            return Err(Neo4jError::Disabled);
        }

        info!(uri = %config.uri, database = %config.database, "Connecting to Neo4j");

        let graph_config = ConfigBuilder::default()
            .uri(&config.uri)
            .user(&config.user)
            .password(&config.password)
            .db(config.database.as_str())
            .max_connections(config.max_connections)
            .build()
            .map_err(|e| Neo4jError::Connection(e.to_string()))?;

        let graph = Graph::connect(graph_config)
            .await
            .map_err(|e| Neo4jError::Connection(e.to_string()))?;

        info!("Successfully connected to Neo4j");

        Ok(Self {
            graph: Arc::new(graph),
            config,
            connected: true,
        })
    }

    /// Check if client is connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get the underlying graph handle
    pub fn graph(&self) -> Arc<Graph> {
        Arc::clone(&self.graph)
    }

    /// Initialize the graph schema (constraints and indexes)
    pub async fn init_schema(&self) -> Result<(), Neo4jError> {
        info!("Initializing Neo4j schema");

        // Constraints for unique IDs
        let constraints = vec![
            "CREATE CONSTRAINT doc_id IF NOT EXISTS FOR (d:Document) REQUIRE d.id IS UNIQUE",
            "CREATE CONSTRAINT chunk_id IF NOT EXISTS FOR (c:Chunk) REQUIRE c.id IS UNIQUE",
            "CREATE CONSTRAINT entity_id IF NOT EXISTS FOR (e:Entity) REQUIRE e.id IS UNIQUE",
            "CREATE CONSTRAINT concept_id IF NOT EXISTS FOR (c:Concept) REQUIRE c.id IS UNIQUE",
            "CREATE CONSTRAINT agent_id IF NOT EXISTS FOR (a:Agent) REQUIRE a.id IS UNIQUE",
            "CREATE CONSTRAINT goal_id IF NOT EXISTS FOR (g:Goal) REQUIRE g.id IS UNIQUE",
            "CREATE CONSTRAINT episode_id IF NOT EXISTS FOR (e:Episode) REQUIRE e.id IS UNIQUE",
        ];

        // Indexes for common lookups
        let indexes = vec![
            "CREATE INDEX chunk_embedding_idx IF NOT EXISTS FOR (c:Chunk) ON (c.embedding_id)",
            "CREATE INDEX entity_normalized_idx IF NOT EXISTS FOR (e:Entity) ON (e.normalized_name)",
            "CREATE INDEX entity_type_idx IF NOT EXISTS FOR (e:Entity) ON (e.entity_type)",
            "CREATE INDEX concept_domain_idx IF NOT EXISTS FOR (c:Concept) ON (c.domain)",
            "CREATE INDEX episode_created_idx IF NOT EXISTS FOR (e:Episode) ON (e.created_at)",
        ];

        // Full-text indexes for search
        let fulltext_indexes = vec![
            "CREATE FULLTEXT INDEX entity_search IF NOT EXISTS FOR (e:Entity) ON EACH [e.name, e.description]",
            "CREATE FULLTEXT INDEX concept_search IF NOT EXISTS FOR (c:Concept) ON EACH [c.name, c.description]",
            "CREATE FULLTEXT INDEX chunk_content IF NOT EXISTS FOR (c:Chunk) ON EACH [c.content]",
        ];

        // Execute constraints
        for query_str in constraints {
            match self.graph.run(Query::new(query_str.to_string())).await {
                Ok(_) => debug!("Created constraint: {}", query_str),
                Err(e) => {
                    // Constraint might already exist, which is fine
                    if !e.to_string().contains("already exists") {
                        warn!("Failed to create constraint: {} - {}", query_str, e);
                    }
                }
            }
        }

        // Execute indexes
        for query_str in indexes {
            match self.graph.run(Query::new(query_str.to_string())).await {
                Ok(_) => debug!("Created index: {}", query_str),
                Err(e) => {
                    if !e.to_string().contains("already exists") {
                        warn!("Failed to create index: {} - {}", query_str, e);
                    }
                }
            }
        }

        // Execute full-text indexes
        for query_str in fulltext_indexes {
            match self.graph.run(Query::new(query_str.to_string())).await {
                Ok(_) => debug!("Created fulltext index: {}", query_str),
                Err(e) => {
                    if !e.to_string().contains("already exists") {
                        warn!("Failed to create fulltext index: {} - {}", query_str, e);
                    }
                }
            }
        }

        info!("Neo4j schema initialization complete");
        Ok(())
    }

    /// Get graph statistics
    pub async fn get_stats(&self) -> Result<GraphStats, Neo4jError> {
        let query = Query::new(
            "MATCH (n)
             WITH labels(n) as labels, count(*) as count
             UNWIND labels as label
             RETURN label, sum(count) as node_count
             ORDER BY node_count DESC"
                .to_string(),
        );

        let mut result = self.graph.execute(query).await?;
        let mut node_counts = std::collections::HashMap::new();

        while let Some(row) = result.next().await? {
            let label: String = row.get("label").map_err(Neo4jError::from)?;
            let count: i64 = row.get("node_count").map_err(Neo4jError::from)?;
            node_counts.insert(label, count as usize);
        }

        // Get relationship counts
        let rel_query = Query::new(
            "MATCH ()-[r]->()
             RETURN type(r) as rel_type, count(*) as count
             ORDER BY count DESC"
                .to_string(),
        );

        let mut rel_result = self.graph.execute(rel_query).await?;
        let mut rel_counts = std::collections::HashMap::new();

        while let Some(row) = rel_result.next().await? {
            let rel_type: String = row.get("rel_type").map_err(Neo4jError::from)?;
            let count: i64 = row.get("count").map_err(Neo4jError::from)?;
            rel_counts.insert(rel_type, count as usize);
        }

        let total_nodes = node_counts.values().sum();
        let total_relationships = rel_counts.values().sum();

        Ok(GraphStats {
            node_counts,
            relationship_counts: rel_counts,
            total_nodes,
            total_relationships,
        })
    }

    /// Health check - verify connection is alive
    pub async fn health_check(&self) -> Result<bool, Neo4jError> {
        let query = Query::new("RETURN 1 as health".to_string());
        match self.graph.execute(query).await {
            Ok(_) => Ok(true),
            Err(e) => {
                error!("Neo4j health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Execute a raw Cypher query (for advanced use cases)
    pub async fn execute_query(&self, cypher: &str) -> Result<Vec<neo4rs::Row>, Neo4jError> {
        let query = Query::new(cypher.to_string());
        let mut result = self.graph.execute(query).await?;

        let mut rows = Vec::new();
        while let Some(row) = result.next().await? {
            rows.push(row);
        }

        Ok(rows)
    }

    /// Run a query without returning results
    pub async fn run_query(&self, cypher: &str) -> Result<(), Neo4jError> {
        let query = Query::new(cypher.to_string());
        self.graph.run(query).await?;
        Ok(())
    }
}

/// Graph statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphStats {
    pub node_counts: std::collections::HashMap<String, usize>,
    pub relationship_counts: std::collections::HashMap<String, usize>,
    pub total_nodes: usize,
    pub total_relationships: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disabled_client() {
        let config = GraphConfig::default(); // disabled by default
        let result = Neo4jClient::new(config).await;
        assert!(matches!(result, Err(Neo4jError::Disabled)));
    }
}
