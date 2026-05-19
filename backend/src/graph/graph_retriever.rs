// backend/src/graph/graph_retriever.rs
// Graph-augmented retrieval for context expansion.
//
// This module provides graph-based context expansion to enhance RAG retrieval:
// - Find related chunks through shared entities
// - Expand context via concept relationships
// - Multi-hop reasoning paths

use crate::graph::client::{lit, row_f64, row_i64, row_str, row_str_vec, GraphHandle, GraphClientError};
use crate::graph::config::GraphExpansionSettings;
use crate::params;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{debug, info};

/// Configuration for graph expansion
#[derive(Debug, Clone)]
pub struct GraphExpansionConfig {
    pub max_hops: usize,
    pub max_expanded_chunks: usize,
    pub entity_weight: f32,
    pub concept_weight: f32,
    pub min_relationship_strength: f32,
}

impl Default for GraphExpansionConfig {
    fn default() -> Self {
        Self {
            max_hops: 2,
            max_expanded_chunks: 10,
            entity_weight: 0.7,
            concept_weight: 0.5,
            min_relationship_strength: 0.3,
        }
    }
}

impl From<GraphExpansionSettings> for GraphExpansionConfig {
    fn from(settings: GraphExpansionSettings) -> Self {
        Self {
            max_hops: settings.max_hops,
            max_expanded_chunks: settings.max_chunks,
            entity_weight: settings.entity_weight,
            concept_weight: settings.concept_weight,
            min_relationship_strength: settings.min_relationship_strength,
        }
    }
}

/// A chunk discovered through graph expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedChunk {
    /// Chunk ID (matches vector store)
    pub chunk_id: String,
    /// Chunk content
    pub content: String,
    /// Score from graph expansion (higher = more relevant)
    pub expansion_score: f32,
    /// How this chunk was discovered
    pub expansion_path: Vec<String>,
    /// Entities/concepts shared with seed chunks
    pub shared_entities: Vec<String>,
    /// Source document
    pub source: Option<String>,
}

/// Graph-based retriever for context expansion
pub struct GraphRetriever {
    graph: GraphHandle,
    config: GraphExpansionConfig,
}

impl GraphRetriever {
    /// Create a new graph retriever
    pub fn new(graph: GraphHandle, config: GraphExpansionConfig) -> Self {
        info!(
            max_hops = config.max_hops,
            max_chunks = config.max_expanded_chunks,
            "Initializing GraphRetriever"
        );
        Self { graph, config }
    }

    /// Expand context using graph relationships
    pub async fn expand_context(
        &self,
        seed_chunk_ids: &[String],
        query_entities: &[String],
    ) -> Result<Vec<ExpandedChunk>, GraphClientError> {
        if seed_chunk_ids.is_empty() {
            return Ok(Vec::new());
        }

        debug!(
            seed_count = seed_chunk_ids.len(),
            query_entities = query_entities.len(),
            "Expanding context via graph"
        );

        let mut expanded = Vec::new();
        let seed_set: HashSet<_> = seed_chunk_ids.iter().cloned().collect();

        // Strategy 1: Find chunks that share entities with seed chunks
        let entity_expanded = self.expand_via_entities(seed_chunk_ids).await?;
        debug!(count = entity_expanded.len(), "Entity expansion results");

        // Strategy 2: Find chunks mentioning query entities directly
        let query_entity_chunks = if !query_entities.is_empty() {
            self.find_chunks_by_entities(query_entities).await?
        } else {
            Vec::new()
        };
        debug!(count = query_entity_chunks.len(), "Query entity results");

        // Merge and deduplicate, excluding seed chunks
        let mut seen: HashSet<String> = seed_set;

        for chunk in entity_expanded.into_iter().chain(query_entity_chunks) {
            if !seen.contains(&chunk.chunk_id) && expanded.len() < self.config.max_expanded_chunks {
                seen.insert(chunk.chunk_id.clone());
                expanded.push(chunk);
            }
        }

        // Sort by expansion score (descending)
        expanded.sort_by(|a, b| {
            b.expansion_score
                .partial_cmp(&a.expansion_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit to max chunks
        expanded.truncate(self.config.max_expanded_chunks);

        info!(expanded_count = expanded.len(), "Graph expansion complete");

        Ok(expanded)
    }

    /// Find related chunks through shared entities
    async fn expand_via_entities(
        &self,
        seed_chunk_ids: &[String],
    ) -> Result<Vec<ExpandedChunk>, GraphClientError> {
        let params = params! {
            "chunk_ids" => lit::str_list(seed_chunk_ids),
            "limit" => lit::int((self.config.max_expanded_chunks * 2) as i64),
        };
        let rows = self
            .graph
            .query(
                "UNWIND $chunk_ids AS seed_id
                 MATCH (seed:Chunk {id: seed_id})-[:MENTIONS]->(e:Entity)<-[m:MENTIONS]-(related:Chunk)
                 WHERE related.id <> seed_id
                 WITH related, e, m, count(DISTINCT seed_id) AS shared_count
                 RETURN related.id AS chunk_id,
                        related.content AS content,
                        related.source AS source,
                        collect(DISTINCT e.name) AS shared_entities,
                        shared_count AS score
                 ORDER BY score DESC
                 LIMIT $limit",
                &params,
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let score = row_i64(&row, 4);
                ExpandedChunk {
                    chunk_id: row_str(&row, 0),
                    content: row_str(&row, 1),
                    source: opt_str(row_str(&row, 2)),
                    expansion_score: (score as f32) * self.config.entity_weight,
                    expansion_path: vec!["entity_link".to_string()],
                    shared_entities: row_str_vec(&row, 3),
                }
            })
            .collect())
    }

    /// Find chunks that mention specific entities
    async fn find_chunks_by_entities(
        &self,
        entity_names: &[String],
    ) -> Result<Vec<ExpandedChunk>, GraphClientError> {
        if entity_names.is_empty() {
            return Ok(Vec::new());
        }

        // Normalize entity names for matching
        let normalized: Vec<String> = entity_names.iter().map(|e| e.to_lowercase()).collect();
        let params = params! {
            "entities" => lit::str_list(&normalized),
            "limit" => lit::int((self.config.max_expanded_chunks * 2) as i64),
        };
        let rows = self
            .graph
            .query(
                "UNWIND $entities AS entity_name
                 MATCH (e:Entity)
                 WHERE toLower(e.name) CONTAINS entity_name
                    OR toLower(e.normalized_name) CONTAINS entity_name
                 MATCH (c:Chunk)-[m:MENTIONS]->(e)
                 WITH c, collect(DISTINCT e.name) AS matched_entities
                 RETURN c.id AS chunk_id,
                        c.content AS content,
                        c.source AS source,
                        matched_entities AS shared_entities,
                        size(matched_entities) AS score
                 ORDER BY score DESC
                 LIMIT $limit",
                &params,
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| ExpandedChunk {
                chunk_id: row_str(&row, 0),
                content: row_str(&row, 1),
                source: opt_str(row_str(&row, 2)),
                expansion_score: row_i64(&row, 4) as f32,
                expansion_path: vec!["query_entity".to_string()],
                shared_entities: row_str_vec(&row, 3),
            })
            .collect())
    }

    /// Get entities related to a given entity
    pub async fn get_related_entities(
        &self,
        entity_name: &str,
        limit: usize,
    ) -> Result<Vec<RelatedEntity>, GraphClientError> {
        let params = params! {
            "name" => lit::str(entity_name),
            "limit" => lit::int(limit as i64),
        };
        let rows = self
            .graph
            .query(
                "MATCH (e:Entity)
                 WHERE toLower(e.name) CONTAINS toLower($name)
                 MATCH (e)-[r:RELATED_TO]-(related:Entity)
                 RETURN related.name AS name,
                        related.entity_type AS entity_type,
                        r.relation_type AS relation_type,
                        r.strength AS strength
                 ORDER BY r.strength DESC
                 LIMIT $limit",
                &params,
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| RelatedEntity {
                name: row_str(&row, 0),
                entity_type: opt_str(row_str(&row, 1)),
                relation_type: opt_str(row_str(&row, 2)),
                strength: row_f64(&row, 3, 0.5) as f32,
            })
            .collect())
    }
}

/// Map an empty string (an absent FalkorDB property) to `None`.
fn opt_str(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// An entity related to another entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedEntity {
    pub name: String,
    pub entity_type: Option<String>,
    pub relation_type: Option<String>,
    pub strength: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expansion_config_default() {
        let config = GraphExpansionConfig::default();
        assert_eq!(config.max_hops, 2);
        assert_eq!(config.max_expanded_chunks, 10);
        assert!(config.entity_weight > 0.0);
    }
}
