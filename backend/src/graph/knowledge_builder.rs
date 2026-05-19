// backend/src/graph/knowledge_builder.rs
// Build the knowledge graph from documents during indexing.
//
// This module integrates with the indexing pipeline to:
// - Create Document and Chunk nodes
// - Extract and link entities
// - Build concept relationships
//
// FalkorDB notes: `datetime()` is replaced by app-supplied epoch-millis
// (`$now`), and `randomUUID()` by app-supplied UUIDs (`$new_id`).

use crate::graph::client::{lit, now_millis, row_str, GraphClientError, GraphHandle};
use crate::graph::config::GraphConfig;
use crate::params;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Document metadata for graph storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: String,
    pub title: String,
    pub source: String,
    pub content_hash: String,
    pub mime_type: String,
    pub chunk_count: usize,
}

/// Chunk metadata for graph storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMeta {
    pub id: String,
    pub document_id: String,
    pub content: String,
    pub embedding_id: String,
    pub position: usize,
    pub token_count: usize,
}

/// Knowledge graph builder
pub struct KnowledgeBuilder {
    graph: GraphHandle,
    #[allow(dead_code)]
    config: GraphConfig,
}

impl KnowledgeBuilder {
    /// Create a new knowledge builder
    pub fn new(graph: GraphHandle, config: GraphConfig) -> Self {
        info!("Initializing KnowledgeBuilder");
        Self { graph, config }
    }

    /// Add a document to the knowledge graph
    pub async fn add_document(&self, doc: &DocumentMeta) -> Result<(), GraphClientError> {
        let params = params! {
            "id" => lit::str(&doc.id),
            "title" => lit::str(&doc.title),
            "source" => lit::str(&doc.source),
            "content_hash" => lit::str(&doc.content_hash),
            "mime_type" => lit::str(&doc.mime_type),
            "chunk_count" => lit::int(doc.chunk_count as i64),
            "now" => lit::int(now_millis()),
        };
        self.graph
            .run(
                "MERGE (d:Document {id: $id})
                 SET d.title = $title,
                     d.source = $source,
                     d.content_hash = $content_hash,
                     d.mime_type = $mime_type,
                     d.chunk_count = $chunk_count,
                     d.indexed_at = $now",
                &params,
            )
            .await?;
        debug!(doc_id = %doc.id, "Added document to graph");
        Ok(())
    }

    /// Add a chunk to the knowledge graph
    pub async fn add_chunk(&self, chunk: &ChunkMeta) -> Result<(), GraphClientError> {
        let params = params! {
            "id" => lit::str(&chunk.id),
            "doc_id" => lit::str(&chunk.document_id),
            "content" => lit::str(&chunk.content),
            "embedding_id" => lit::str(&chunk.embedding_id),
            "position" => lit::int(chunk.position as i64),
            "token_count" => lit::int(chunk.token_count as i64),
            "now" => lit::int(now_millis()),
        };
        self.graph
            .run(
                "MATCH (d:Document {id: $doc_id})
                 MERGE (c:Chunk {id: $id})
                 SET c.content = $content,
                     c.embedding_id = $embedding_id,
                     c.position = $position,
                     c.token_count = $token_count,
                     c.created_at = $now
                 MERGE (d)-[:HAS_CHUNK {position: $position}]->(c)",
                &params,
            )
            .await?;
        debug!(chunk_id = %chunk.id, "Added chunk to graph");
        Ok(())
    }

    /// Add an entity and link it to a chunk
    pub async fn add_entity_mention(
        &self,
        chunk_id: &str,
        entity_name: &str,
        entity_type: &str,
        confidence: f32,
    ) -> Result<String, GraphClientError> {
        let normalized = entity_name.trim().to_lowercase();
        let params = params! {
            "chunk_id" => lit::str(chunk_id),
            "normalized" => lit::str(&normalized),
            "name" => lit::str(entity_name),
            "type" => lit::str(entity_type),
            "confidence" => lit::float(confidence as f64),
            "new_id" => lit::str(&uuid::Uuid::new_v4().to_string()),
            "now" => lit::int(now_millis()),
        };
        let rows = self
            .graph
            .query(
                "MATCH (c:Chunk {id: $chunk_id})
                 MERGE (e:Entity {normalized_name: $normalized})
                 ON CREATE SET
                    e.id = $new_id,
                    e.name = $name,
                    e.entity_type = $type,
                    e.mention_count = 1,
                    e.first_seen = $now
                 ON MATCH SET
                    e.mention_count = e.mention_count + 1,
                    e.last_seen = $now
                 MERGE (c)-[m:MENTIONS]->(e)
                 SET m.confidence = $confidence
                 RETURN e.id AS entity_id",
                &params,
            )
            .await?;

        Ok(rows.first().map(|r| row_str(r, 0)).unwrap_or_default())
    }

    /// Create a relationship between two entities
    pub async fn link_entities(
        &self,
        entity1_name: &str,
        entity2_name: &str,
        relation_type: &str,
        strength: f32,
    ) -> Result<(), GraphClientError> {
        let params = params! {
            "name1" => lit::str(&entity1_name.to_lowercase()),
            "name2" => lit::str(&entity2_name.to_lowercase()),
            "relation_type" => lit::str(relation_type),
            "strength" => lit::float(strength as f64),
        };
        self.graph
            .run(
                "MATCH (e1:Entity {normalized_name: $name1})
                 MATCH (e2:Entity {normalized_name: $name2})
                 MERGE (e1)-[r:RELATED_TO]->(e2)
                 SET r.relation_type = $relation_type,
                     r.strength = $strength,
                     r.evidence_count = coalesce(r.evidence_count, 0) + 1",
                &params,
            )
            .await?;
        Ok(())
    }

    /// Add a concept to the knowledge graph
    pub async fn add_concept(
        &self,
        name: &str,
        description: &str,
        domain: &str,
    ) -> Result<String, GraphClientError> {
        let params = params! {
            "name" => lit::str(name),
            "description" => lit::str(description),
            "domain" => lit::str(domain),
            "new_id" => lit::str(&uuid::Uuid::new_v4().to_string()),
            "now" => lit::int(now_millis()),
        };
        let rows = self
            .graph
            .query(
                "MERGE (c:Concept {name: $name})
                 ON CREATE SET c.id = $new_id
                 SET c.description = $description,
                     c.domain = $domain,
                     c.updated_at = $now
                 RETURN c.id AS id",
                &params,
            )
            .await?;

        Ok(rows.first().map(|r| row_str(r, 0)).unwrap_or_default())
    }

    /// Link a chunk to a concept
    pub async fn link_chunk_to_concept(
        &self,
        chunk_id: &str,
        concept_name: &str,
        relevance: f32,
    ) -> Result<(), GraphClientError> {
        let params = params! {
            "chunk_id" => lit::str(chunk_id),
            "concept_name" => lit::str(concept_name),
            "relevance" => lit::float(relevance as f64),
        };
        self.graph
            .run(
                "MATCH (c:Chunk {id: $chunk_id})
                 MATCH (concept:Concept {name: $concept_name})
                 MERGE (c)-[r:DISCUSSES]->(concept)
                 SET r.relevance = $relevance",
                &params,
            )
            .await?;
        Ok(())
    }

    /// Delete a document and all its chunks from the graph
    pub async fn delete_document(&self, doc_id: &str) -> Result<usize, GraphClientError> {
        let params = params! { "doc_id" => lit::str(doc_id) };

        // First count chunks
        let rows = self
            .graph
            .query(
                "MATCH (d:Document {id: $doc_id})-[:HAS_CHUNK]->(c:Chunk)
                 RETURN count(c) AS count",
                &params,
            )
            .await?;
        let chunk_count = rows
            .first()
            .map(|r| crate::graph::client::row_i64(r, 0))
            .unwrap_or(0);

        // Delete document and chunks
        self.graph
            .run(
                "MATCH (d:Document {id: $doc_id})
                 OPTIONAL MATCH (d)-[:HAS_CHUNK]->(c:Chunk)
                 DETACH DELETE d, c",
                &params,
            )
            .await?;

        info!(doc_id = %doc_id, chunks_deleted = chunk_count, "Deleted document from graph");
        Ok(chunk_count as usize)
    }

    /// Get graph statistics
    pub async fn get_stats(&self) -> Result<GraphBuildStats, GraphClientError> {
        let rows = self
            .graph
            .query(
                "MATCH (d:Document) WITH count(d) AS docs
                 MATCH (c:Chunk) WITH docs, count(c) AS chunks
                 MATCH (e:Entity) WITH docs, chunks, count(e) AS entities
                 MATCH ()-[r]->() WITH docs, chunks, entities, count(r) AS rels
                 RETURN docs, chunks, entities, rels",
                &std::collections::HashMap::new(),
            )
            .await?;

        if let Some(row) = rows.first() {
            use crate::graph::client::row_i64;
            Ok(GraphBuildStats {
                documents: row_i64(row, 0) as usize,
                chunks: row_i64(row, 1) as usize,
                entities: row_i64(row, 2) as usize,
                relationships: row_i64(row, 3) as usize,
            })
        } else {
            Ok(GraphBuildStats::default())
        }
    }
}

/// Statistics from graph building
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphBuildStats {
    pub documents: usize,
    pub chunks: usize,
    pub entities: usize,
    pub relationships: usize,
}
