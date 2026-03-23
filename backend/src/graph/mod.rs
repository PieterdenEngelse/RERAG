// backend/src/graph/mod.rs
// Neo4j Knowledge Graph integration for GraphRAG (Phase 27)
//
// This module provides graph-augmented retrieval capabilities:
// - Entity extraction and linking
// - Multi-hop reasoning through graph traversal
// - Context expansion via entity relationships
// - Agent memory as a knowledge graph
//
// Enable Neo4j with: cargo build --features neo4j
// Petgraph runtime is ALWAYS available (no Neo4j needed)

pub mod config;

// Petgraph runtime is ALWAYS available (no Neo4j needed at runtime)
pub mod petgraph_runtime;

// Only compile Neo4j modules when the feature is enabled
#[cfg(feature = "neo4j")]
pub mod agent_memory_graph;
#[cfg(feature = "neo4j")]
pub mod client;
#[cfg(feature = "neo4j")]
pub mod graph_retriever;
#[cfg(feature = "neo4j")]
pub mod knowledge_builder;
#[cfg(feature = "neo4j")]
pub mod schema;

// Re-exports: Petgraph (always available)
pub use petgraph_runtime::{
    export_to_json, get_runtime_graph, set_runtime_graph, initialize_standalone, reload_from_json_path, ChunkNode, GraphQuery, Relationship,
    RuntimeGraph,
};

// Re-exports: Neo4j (only when feature enabled)
#[cfg(feature = "neo4j")]
pub use agent_memory_graph::{AgentMemoryGraph, AgentStats, Pattern, SimilarEpisode};
#[cfg(feature = "neo4j")]
pub use client::{GraphStats, Neo4jClient, Neo4jError};
#[cfg(feature = "neo4j")]
pub use graph_retriever::{ExpandedChunk, GraphExpansionConfig, GraphRetriever, RelatedEntity};
#[cfg(feature = "neo4j")]
pub use knowledge_builder::{ChunkMeta, DocumentMeta, GraphBuildStats, KnowledgeBuilder};

/// Check if Neo4j feature is enabled at compile time
pub fn is_neo4j_compiled() -> bool {
    cfg!(feature = "neo4j")
}

/// Check if Neo4j is enabled via configuration
pub fn is_neo4j_enabled() -> bool {
    config::GraphConfig::from_env().enabled
}

/// Process a document and its chunks through the knowledge graph.
/// Extracts entities and stores them in Neo4j.
#[cfg(feature = "neo4j")]
pub async fn index_to_knowledge_graph(
    kb: &KnowledgeBuilder,
    doc_id: &str,
    title: &str,
    source: &str,
    chunks: &[(String, String)], // (chunk_id, chunk_content)
) {
    use crate::tools::entity_extractor::EntityExtractorTool;
    use tracing::{debug, warn};

    // Check if entity extraction is enabled
    let graph_config = config::GraphConfig::from_env();
    if !graph_config.entity_extraction.enabled {
        debug!("Entity extraction disabled, skipping knowledge graph indexing");
        return;
    }

    // Add document to graph
    let doc_meta = knowledge_builder::DocumentMeta {
        id: doc_id.to_string(),
        title: title.to_string(),
        source: source.to_string(),
        content_hash: {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            title.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        },
        mime_type: "text/plain".to_string(),
        chunk_count: chunks.len(),
    };

    if let Err(e) = kb.add_document(&doc_meta).await {
        warn!(error = %e, doc_id = %doc_id, "Failed to add document to knowledge graph");
        return;
    }

    // Process each chunk
    let extractor = EntityExtractorTool::new();
    let confidence_threshold = graph_config.entity_extraction.confidence_threshold;

    for (chunk_id, chunk_content) in chunks {
        // Yield between chunks to prevent CPU starvation
        tokio::task::yield_now().await;

        // Add chunk to graph
        let chunk_meta = knowledge_builder::ChunkMeta {
            id: chunk_id.clone(),
            document_id: doc_id.to_string(),
            content: chunk_content.clone(),
            embedding_id: chunk_id.clone(),
            position: chunk_id
                .split('#')
                .last()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            token_count: chunk_content.split_whitespace().count(),
        };

        if let Err(e) = kb.add_chunk(&chunk_meta).await {
            warn!(error = %e, chunk_id = %chunk_id, "Failed to add chunk to knowledge graph");
            continue;
        }

        // Extract entities - try ONNX NER first, fall back to regex
        let ner_entities = crate::tools::ner_extractor::extract_entities(chunk_content);
        let use_ner = !ner_entities.is_empty();
        if use_ner {
            for ner_entity in &ner_entities {
                if let Err(e) = kb.add_entity_mention(
                    chunk_id, &ner_entity.text, &ner_entity.label, ner_entity.score,
                ).await {
                    debug!(error = %e, entity = %ner_entity.text, "Failed to add NER entity");
                }
            }
        }
        // Fallback regex extraction
        let extraction = extractor.extract(chunk_content);

        for entity in &extraction.entities {
            if !use_ner && entity.confidence >= confidence_threshold {
                if let Err(e) = kb
                    .add_entity_mention(
                        chunk_id,
                        &entity.text,
                        entity.entity_type.label(),
                        entity.confidence,
                    )
                    .await
                {
                    debug!(error = %e, entity = %entity.text, "Failed to add entity mention");
                }
            }
        }

        // Link co-occurring entities (entities in the same chunk are related)
        let high_confidence_entities: Vec<_> = extraction
            .entities
            .iter()
            .filter(|e| e.confidence >= confidence_threshold)
            .collect();

        for i in 0..high_confidence_entities.len() {
            for j in (i + 1)..high_confidence_entities.len() {
                let e1 = &high_confidence_entities[i];
                let e2 = &high_confidence_entities[j];
                let _ = kb
                    .link_entities(
                        &e1.text,
                        &e2.text,
                        "co_occurs_with",
                        (e1.confidence + e2.confidence) / 2.0,
                    )
                    .await;
            }
        }
    }

    debug!(
        doc_id = %doc_id,
        chunks = chunks.len(),
        "Indexed document to knowledge graph"
    );
}

/// No-op when neo4j feature is disabled
#[cfg(not(feature = "neo4j"))]
pub async fn index_to_knowledge_graph(
    _doc_id: &str,
    _title: &str,
    _source: &str,
    _chunks: &[(String, String)],
) {
    // No-op
}

#[cfg(test)]
mod petgraph_runtime_test;
