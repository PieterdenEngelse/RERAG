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
    export_to_json, get_runtime_graph, initialize_standalone, ChunkNode, GraphQuery, Relationship,
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

#[cfg(test)]
mod petgraph_runtime_test;
