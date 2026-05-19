// backend/src/graph/mod.rs
// FalkorDB Knowledge Graph integration for GraphRAG (Phase 27)
//
// This module provides graph-augmented retrieval capabilities:
// - Entity extraction and linking
// - Multi-hop reasoning through graph traversal
// - Context expansion via entity relationships
// - Agent memory as a knowledge graph
//
// Enable FalkorDB with: cargo build --features graph
// Petgraph runtime is ALWAYS available (no FalkorDB needed)

pub mod config;

// Petgraph runtime is ALWAYS available (no FalkorDB needed at runtime)
pub mod petgraph_runtime;

// Only compile FalkorDB modules when the feature is enabled
#[cfg(feature = "graph")]
pub mod agent_memory_graph;
#[cfg(feature = "graph")]
pub mod client;
#[cfg(feature = "graph")]
pub mod graph_retriever;
#[cfg(feature = "graph")]
pub mod knowledge_builder;

// Re-exports: Petgraph (always available)
pub use petgraph_runtime::{
    export_to_json, get_runtime_graph, initialize_standalone, reload_from_json_path,
    set_runtime_graph, ChunkNode, GraphQuery, Relationship, RuntimeGraph,
};

// Re-exports: FalkorDB (only when feature enabled)
#[cfg(feature = "graph")]
pub use agent_memory_graph::{AgentMemoryGraph, AgentStats, Pattern, SimilarEpisode};
#[cfg(feature = "graph")]
pub use client::{GraphClient, GraphClientError, GraphHandle, GraphStats};
#[cfg(feature = "graph")]
pub use graph_retriever::{ExpandedChunk, GraphExpansionConfig, GraphRetriever, RelatedEntity};
#[cfg(feature = "graph")]
pub use knowledge_builder::{ChunkMeta, DocumentMeta, GraphBuildStats, KnowledgeBuilder};

/// Check if FalkorDB feature is enabled at compile time
pub fn is_graph_compiled() -> bool {
    cfg!(feature = "graph")
}

/// Check if FalkorDB is enabled via configuration
pub fn is_graph_enabled() -> bool {
    config::GraphConfig::from_env().enabled
}

/// Process a document and its chunks through the knowledge graph.
/// Extracts entities and stores them in FalkorDB.
#[cfg(feature = "graph")]
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
    let ner_batch_size = crate::db::ner_settings::global_config().batch_size.max(1);

    // Phase 1: add all chunks to the graph, collect the ones that succeeded.
    let mut valid: Vec<(&str, &str)> = Vec::with_capacity(chunks.len());
    for (chunk_id, chunk_content) in chunks {
        tokio::task::yield_now().await;
        let chunk_meta = knowledge_builder::ChunkMeta {
            id: chunk_id.clone(),
            document_id: doc_id.to_string(),
            content: chunk_content.clone(),
            embedding_id: chunk_id.clone(),
            position: chunk_id
                .split('#')
                .next_back()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            token_count: chunk_content.split_whitespace().count(),
        };
        if let Err(e) = kb.add_chunk(&chunk_meta).await {
            warn!(error = %e, chunk_id = %chunk_id, "Failed to add chunk to knowledge graph");
        } else {
            valid.push((chunk_id, chunk_content));
        }
    }

    // Phase 2: batched NER — one ONNX call per batch of ner_batch_size chunks.
    let texts: Vec<&str> = valid.iter().map(|(_, c)| *c).collect();
    let ner_results: Vec<Vec<crate::tools::ner_extractor::NerEntity>> = texts
        .chunks(ner_batch_size)
        .flat_map(crate::tools::ner_extractor::extract_entities_batch)
        .collect();

    // Phase 3: entity mentions, regex fallback, co-occurrence links.
    for (i, (chunk_id, chunk_content)) in valid.iter().enumerate() {
        let ner_entities = &ner_results[i];
        let use_ner = !ner_entities.is_empty();
        if use_ner {
            for ner_entity in ner_entities {
                if let Err(e) = kb
                    .add_entity_mention(
                        chunk_id,
                        &ner_entity.text,
                        &ner_entity.label,
                        ner_entity.score,
                    )
                    .await
                {
                    debug!(error = %e, entity = %ner_entity.text, "Failed to add NER entity");
                }
            }
        }
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

/// No-op when graph feature is disabled
#[cfg(not(feature = "graph"))]
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
