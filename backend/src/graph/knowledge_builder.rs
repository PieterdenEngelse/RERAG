// backend/src/graph/knowledge_builder.rs
// Build knowledge graph from documents during indexing
//
// This module integrates with the indexing pipeline to:
// - Create Document and Chunk nodes
// - Extract and link entities
// - Build concept relationships

use crate::graph::client::Neo4jError;
use crate::graph::config::GraphConfig;
use neo4rs::{query, Graph};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
    graph: Arc<Graph>,
    #[allow(dead_code)]
    config: GraphConfig,
}

impl KnowledgeBuilder {
    /// Create a new knowledge builder
    pub fn new(graph: Arc<Graph>, config: GraphConfig) -> Self {
        info!("Initializing KnowledgeBuilder");
        Self { graph, config }
    }

    /// Add a document to the knowledge graph
    pub async fn add_document(&self, doc: &DocumentMeta) -> Result<(), Neo4jError> {
        let q = query(
            "MERGE (d:Document {id: $id})
             SET d.title = $title,
                 d.source = $source,
                 d.content_hash = $content_hash,
                 d.mime_type = $mime_type,
                 d.chunk_count = $chunk_count,
                 d.indexed_at = datetime()",
        )
        .param("id", doc.id.clone())
        .param("title", doc.title.clone())
        .param("source", doc.source.clone())
        .param("content_hash", doc.content_hash.clone())
        .param("mime_type", doc.mime_type.clone())
        .param("chunk_count", doc.chunk_count as i64);

        self.graph.run(q).await?;
        debug!(doc_id = %doc.id, "Added document to graph");
        Ok(())
    }

    /// Add a chunk to the knowledge graph
    pub async fn add_chunk(&self, chunk: &ChunkMeta) -> Result<(), Neo4jError> {
        let q = query(
            "MATCH (d:Document {id: $doc_id})
             MERGE (c:Chunk {id: $id})
             SET c.content = $content,
                 c.embedding_id = $embedding_id,
                 c.position = $position,
                 c.token_count = $token_count,
                 c.created_at = datetime()
             MERGE (d)-[:HAS_CHUNK {position: $position}]->(c)",
        )
        .param("id", chunk.id.clone())
        .param("doc_id", chunk.document_id.clone())
        .param("content", chunk.content.clone())
        .param("embedding_id", chunk.embedding_id.clone())
        .param("position", chunk.position as i64)
        .param("token_count", chunk.token_count as i64);

        self.graph.run(q).await?;
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
    ) -> Result<String, Neo4jError> {
        let normalized = entity_name.trim().to_lowercase();

        let q = query(
            "MATCH (c:Chunk {id: $chunk_id})
             MERGE (e:Entity {normalized_name: $normalized})
             ON CREATE SET 
                e.id = randomUUID(),
                e.name = $name,
                e.entity_type = $type,
                e.mention_count = 1,
                e.first_seen = datetime()
             ON MATCH SET
                e.mention_count = e.mention_count + 1,
                e.last_seen = datetime()
             MERGE (c)-[m:MENTIONS]->(e)
             SET m.confidence = $confidence
             RETURN e.id as entity_id",
        )
        .param("chunk_id", chunk_id.to_string())
        .param("normalized", normalized)
        .param("name", entity_name.to_string())
        .param("type", entity_type.to_string())
        .param("confidence", confidence as f64);

        let mut result = self.graph.execute(q).await?;

        if let Ok(Some(row)) = result.next().await {
            let entity_id: String = row.get("entity_id").unwrap_or_default();
            Ok(entity_id)
        } else {
            Ok(String::new())
        }
    }

    /// Create relationship between two entities
    pub async fn link_entities(
        &self,
        entity1_name: &str,
        entity2_name: &str,
        relation_type: &str,
        strength: f32,
    ) -> Result<(), Neo4jError> {
        let q = query(
            "MATCH (e1:Entity {normalized_name: $name1})
             MATCH (e2:Entity {normalized_name: $name2})
             MERGE (e1)-[r:RELATED_TO]->(e2)
             SET r.relation_type = $relation_type,
                 r.strength = $strength,
                 r.evidence_count = coalesce(r.evidence_count, 0) + 1",
        )
        .param("name1", entity1_name.to_lowercase())
        .param("name2", entity2_name.to_lowercase())
        .param("relation_type", relation_type.to_string())
        .param("strength", strength as f64);

        self.graph.run(q).await?;
        Ok(())
    }

    /// Add a concept to the knowledge graph
    pub async fn add_concept(
        &self,
        name: &str,
        description: &str,
        domain: &str,
    ) -> Result<String, Neo4jError> {
        let q = query(
            "MERGE (c:Concept {name: $name})
             SET c.description = $description,
                 c.domain = $domain,
                 c.updated_at = datetime()
             ON CREATE SET c.id = randomUUID()
             RETURN c.id as id",
        )
        .param("name", name.to_string())
        .param("description", description.to_string())
        .param("domain", domain.to_string());

        let mut result = self.graph.execute(q).await?;

        if let Ok(Some(row)) = result.next().await {
            let id: String = row.get("id").unwrap_or_default();
            Ok(id)
        } else {
            Ok(String::new())
        }
    }

    /// Link a chunk to a concept
    pub async fn link_chunk_to_concept(
        &self,
        chunk_id: &str,
        concept_name: &str,
        relevance: f32,
    ) -> Result<(), Neo4jError> {
        let q = query(
            "MATCH (c:Chunk {id: $chunk_id})
             MATCH (concept:Concept {name: $concept_name})
             MERGE (c)-[r:DISCUSSES]->(concept)
             SET r.relevance = $relevance",
        )
        .param("chunk_id", chunk_id.to_string())
        .param("concept_name", concept_name.to_string())
        .param("relevance", relevance as f64);

        self.graph.run(q).await?;
        Ok(())
    }

    /// Delete a document and all its chunks from the graph
    pub async fn delete_document(&self, doc_id: &str) -> Result<usize, Neo4jError> {
        // First count chunks
        let count_q = query(
            "MATCH (d:Document {id: $doc_id})-[:HAS_CHUNK]->(c:Chunk)
             RETURN count(c) as count",
        )
        .param("doc_id", doc_id.to_string());

        let mut result = self.graph.execute(count_q).await?;
        let chunk_count: i64 = if let Ok(Some(row)) = result.next().await {
            row.get("count").unwrap_or(0)
        } else {
            0
        };

        // Delete document and chunks
        let delete_q = query(
            "MATCH (d:Document {id: $doc_id})
             OPTIONAL MATCH (d)-[:HAS_CHUNK]->(c:Chunk)
             DETACH DELETE d, c",
        )
        .param("doc_id", doc_id.to_string());

        self.graph.run(delete_q).await?;

        info!(doc_id = %doc_id, chunks_deleted = chunk_count, "Deleted document from graph");
        Ok(chunk_count as usize)
    }

    /// Get graph statistics
    pub async fn get_stats(&self) -> Result<GraphBuildStats, Neo4jError> {
        let q = query(
            "MATCH (d:Document) WITH count(d) as docs
             MATCH (c:Chunk) WITH docs, count(c) as chunks
             MATCH (e:Entity) WITH docs, chunks, count(e) as entities
             MATCH ()-[r]->() WITH docs, chunks, entities, count(r) as rels
             RETURN docs, chunks, entities, rels",
        );

        let mut result = self.graph.execute(q).await?;

        if let Ok(Some(row)) = result.next().await {
            Ok(GraphBuildStats {
                documents: row.get::<i64>("docs").unwrap_or(0) as usize,
                chunks: row.get::<i64>("chunks").unwrap_or(0) as usize,
                entities: row.get::<i64>("entities").unwrap_or(0) as usize,
                relationships: row.get::<i64>("rels").unwrap_or(0) as usize,
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
