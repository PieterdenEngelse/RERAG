// backend/src/graph/schema.rs
// Neo4j schema initialization and migrations
//
// This module handles:
// - Creating constraints and indexes
// - Schema migrations for version upgrades
// - Schema validation

use crate::graph::client::Neo4jError;
use neo4rs::{Graph, Query};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Schema version for migrations
pub const SCHEMA_VERSION: u32 = 1;

/// Initialize the Neo4j schema
pub async fn init_schema(graph: &Arc<Graph>) -> Result<(), Neo4jError> {
    info!("Initializing Neo4j schema v{}", SCHEMA_VERSION);

    // Create constraints
    create_constraints(graph).await?;

    // Create indexes
    create_indexes(graph).await?;

    // Create full-text indexes
    create_fulltext_indexes(graph).await?;

    // Store schema version
    store_schema_version(graph, SCHEMA_VERSION).await?;

    info!("Neo4j schema initialization complete");
    Ok(())
}

async fn create_constraints(graph: &Arc<Graph>) -> Result<(), Neo4jError> {
    let constraints = vec![
        (
            "doc_id",
            "CREATE CONSTRAINT doc_id IF NOT EXISTS FOR (d:Document) REQUIRE d.id IS UNIQUE",
        ),
        (
            "chunk_id",
            "CREATE CONSTRAINT chunk_id IF NOT EXISTS FOR (c:Chunk) REQUIRE c.id IS UNIQUE",
        ),
        (
            "entity_id",
            "CREATE CONSTRAINT entity_id IF NOT EXISTS FOR (e:Entity) REQUIRE e.id IS UNIQUE",
        ),
        (
            "concept_id",
            "CREATE CONSTRAINT concept_id IF NOT EXISTS FOR (c:Concept) REQUIRE c.id IS UNIQUE",
        ),
        (
            "agent_id",
            "CREATE CONSTRAINT agent_id IF NOT EXISTS FOR (a:Agent) REQUIRE a.id IS UNIQUE",
        ),
        (
            "goal_id",
            "CREATE CONSTRAINT goal_id IF NOT EXISTS FOR (g:Goal) REQUIRE g.id IS UNIQUE",
        ),
        (
            "episode_id",
            "CREATE CONSTRAINT episode_id IF NOT EXISTS FOR (e:Episode) REQUIRE e.id IS UNIQUE",
        ),
    ];

    for (name, query_str) in constraints {
        match graph.run(Query::new(query_str.to_string())).await {
            Ok(_) => debug!("Created constraint: {}", name),
            Err(e) => {
                if !e.to_string().contains("already exists") {
                    warn!("Failed to create constraint {}: {}", name, e);
                }
            }
        }
    }

    Ok(())
}

async fn create_indexes(graph: &Arc<Graph>) -> Result<(), Neo4jError> {
    let indexes = vec![
        ("chunk_embedding_idx", "CREATE INDEX chunk_embedding_idx IF NOT EXISTS FOR (c:Chunk) ON (c.embedding_id)"),
        ("entity_normalized_idx", "CREATE INDEX entity_normalized_idx IF NOT EXISTS FOR (e:Entity) ON (e.normalized_name)"),
        ("entity_type_idx", "CREATE INDEX entity_type_idx IF NOT EXISTS FOR (e:Entity) ON (e.entity_type)"),
        ("concept_domain_idx", "CREATE INDEX concept_domain_idx IF NOT EXISTS FOR (c:Concept) ON (c.domain)"),
        ("episode_created_idx", "CREATE INDEX episode_created_idx IF NOT EXISTS FOR (e:Episode) ON (e.created_at)"),
        ("goal_status_idx", "CREATE INDEX goal_status_idx IF NOT EXISTS FOR (g:Goal) ON (g.status)"),
    ];

    for (name, query_str) in indexes {
        match graph.run(Query::new(query_str.to_string())).await {
            Ok(_) => debug!("Created index: {}", name),
            Err(e) => {
                if !e.to_string().contains("already exists") {
                    warn!("Failed to create index {}: {}", name, e);
                }
            }
        }
    }

    Ok(())
}

async fn create_fulltext_indexes(graph: &Arc<Graph>) -> Result<(), Neo4jError> {
    let fulltext_indexes = vec![
        ("entity_search", "CREATE FULLTEXT INDEX entity_search IF NOT EXISTS FOR (e:Entity) ON EACH [e.name, e.description]"),
        ("concept_search", "CREATE FULLTEXT INDEX concept_search IF NOT EXISTS FOR (c:Concept) ON EACH [c.name, c.description]"),
        ("chunk_content", "CREATE FULLTEXT INDEX chunk_content IF NOT EXISTS FOR (c:Chunk) ON EACH [c.content]"),
    ];

    for (name, query_str) in fulltext_indexes {
        match graph.run(Query::new(query_str.to_string())).await {
            Ok(_) => debug!("Created fulltext index: {}", name),
            Err(e) => {
                if !e.to_string().contains("already exists") {
                    warn!("Failed to create fulltext index {}: {}", name, e);
                }
            }
        }
    }

    Ok(())
}

async fn store_schema_version(graph: &Arc<Graph>, version: u32) -> Result<(), Neo4jError> {
    let query = Query::new(
        "MERGE (m:_Meta {key: 'schema_version'})
         SET m.version = $version, m.updated_at = datetime()"
            .to_string(),
    )
    .param("version", version as i64);

    graph.run(query).await?;
    Ok(())
}

/// Get current schema version
pub async fn get_schema_version(graph: &Arc<Graph>) -> Result<Option<u32>, Neo4jError> {
    let query = Query::new(
        "MATCH (m:_Meta {key: 'schema_version'})
         RETURN m.version as version"
            .to_string(),
    );

    let mut result = graph.execute(query).await?;

    if let Some(row) = result.next().await? {
        let version: i64 = row
            .get("version")
            .map_err(|e| Neo4jError::Deserialization(e.to_string()))?;
        Ok(Some(version as u32))
    } else {
        Ok(None)
    }
}

/// Check if schema needs migration
pub async fn needs_migration(graph: &Arc<Graph>) -> Result<bool, Neo4jError> {
    let current = get_schema_version(graph).await?;
    Ok(current.map(|v| v < SCHEMA_VERSION).unwrap_or(true))
}
