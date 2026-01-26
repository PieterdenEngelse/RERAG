# Neo4j RAG Improvements for AG

## Overview

This document outlines improvements to the AG RAG system by integrating Neo4j as a knowledge graph layer. The integration enables **GraphRAG** - combining vector similarity search with graph traversal for enhanced retrieval and reasoning.

## Current Architecture Limitations

| Component | Current State | Limitation |
|-----------|--------------|------------|
| **Retrieval** | Vector + BM25 hybrid | No relationship awareness between chunks |
| **Entity Handling** | EntityExtractorTool exists but isolated | Entities not connected across documents |
| **Agent Memory** | SQLite tables | No pattern discovery, flat structure |
| **Context Expansion** | Top-K similarity only | Can't follow conceptual relationships |
| **Multi-hop Reasoning** | Not supported | "How is X related to Y?" fails |

## Proposed Neo4j Integration

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           AG RAG Pipeline                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────────┐  │
│  │   Document   │───▶│   Chunker    │───▶│  Parallel Processing     │  │
│  │   Upload     │    │              │    │                          │  │
│  └──────────────┘    └──────────────┘    │  ┌────────────────────┐  │  │
│                                          │  │ Tantivy (BM25)     │  │  │
│                                          │  └────────────────────┘  │  │
│                                          │  ┌────────────────────┐  │  │
│                                          │  │ Vector Store       │  │  │
│                                          │  │ (Embeddings)       │  │  │
│                                          │  └────────────────────┘  │  │
│                                          │  ┌────────────────────┐  │  │
│                                          │  │ Neo4j Graph  [NEW] │  │  │
│                                          │  │ (Entities/Rels)    │  │  │
│                                          │  └────────────────────┘  │  │
│                                          └──────────────────────────┘  │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                      Query Pipeline                               │  │
│  │                                                                    │  │
│  │  Query ──▶ Entity Recognition ──▶ ┌─────────────────────────┐    │  │
│  │                                    │  Parallel Retrieval     │    │  │
│  │                                    │  • Vector Similarity    │    │  │
│  │                                    │  • BM25 Keywords        │    │  │
│  │                                    │  • Graph Expansion [NEW]│    │  │
│  │                                    └─────────────────────────┘    │  │
│  │                                              │                     │  │
│  │                                              ▼                     │  │
│  │                                    ┌─────────────────────────┐    │  │
│  │                                    │  Fusion + Reranking     │    │  │
│  │                                    │  (Graph-aware) [NEW]    │    │  │
│  │                                    └─────────────────────────┘    │  │
│  │                                              │                     │  │
│  │                                              ▼                     │  │
│  │                                    ┌─────────────────────────┐    │  │
│  │                                    │  LLM Generation         │    │  │
│  │                                    └─────────────────────────┘    │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

### Graph Schema

```cypher
// ═══════════════════════════════════════════════════════════════════════
// DOCUMENT KNOWLEDGE GRAPH
// ═══════════════════════════════════════════════════════════════════════

// Core Nodes
CREATE CONSTRAINT doc_id IF NOT EXISTS FOR (d:Document) REQUIRE d.id IS UNIQUE;
CREATE CONSTRAINT chunk_id IF NOT EXISTS FOR (c:Chunk) REQUIRE c.id IS UNIQUE;
CREATE CONSTRAINT entity_id IF NOT EXISTS FOR (e:Entity) REQUIRE e.id IS UNIQUE;
CREATE CONSTRAINT concept_id IF NOT EXISTS FOR (c:Concept) REQUIRE c.id IS UNIQUE;

// Document Node
(:Document {
    id: String,           // UUID
    title: String,
    source: String,       // File path or URL
    content_hash: String, // For deduplication
    mime_type: String,
    created_at: DateTime,
    indexed_at: DateTime,
    chunk_count: Integer
})

// Chunk Node (links to vector store via embedding_id)
(:Chunk {
    id: String,           // UUID (same as vector store)
    content: String,      // Chunk text
    embedding_id: String, // Reference to vector store
    position: Integer,    // Position in document
    token_count: Integer,
    char_count: Integer,
    created_at: DateTime
})

// Entity Node (extracted from chunks)
(:Entity {
    id: String,
    name: String,              // Original mention
    normalized_name: String,   // Lowercased, trimmed
    entity_type: String,       // PERSON, ORG, LOCATION, CONCEPT, etc.
    description: String,       // Optional description
    mention_count: Integer,    // How many times mentioned
    first_seen: DateTime,
    last_seen: DateTime
})

// Concept Node (higher-level abstractions)
(:Concept {
    id: String,
    name: String,
    description: String,
    domain: String,        // e.g., "rust", "observability", "rag"
    importance: Float      // Computed centrality score
})

// ═══════════════════════════════════════════════════════════════════════
// RELATIONSHIPS
// ═══════════════════════════════════════════════════════════════════════

// Document → Chunk
(Document)-[:HAS_CHUNK {position: Integer}]->(Chunk)

// Chunk → Entity (with context)
(Chunk)-[:MENTIONS {
    confidence: Float,     // NER confidence
    context: String,       // Surrounding text
    start_offset: Integer,
    end_offset: Integer
}]->(Entity)

// Chunk → Concept
(Chunk)-[:DISCUSSES {
    relevance: Float
}]->(Concept)

// Entity → Entity (relationships)
(Entity)-[:RELATED_TO {
    relation_type: String,  // "works_for", "located_in", "part_of", etc.
    strength: Float,
    evidence_count: Integer
}]->(Entity)

// Concept → Concept (taxonomy)
(Concept)-[:BROADER_THAN]->(Concept)
(Concept)-[:RELATED_TO {strength: Float}]->(Concept)

// Document → Document (citations, references)
(Document)-[:REFERENCES {context: String}]->(Document)
(Document)-[:SIMILAR_TO {score: Float}]->(Document)

// ═══════════════════════════════════════════════════════════════════════
// AGENT MEMORY GRAPH
// ═══════════════════════════════════════════════════════════════════════

(:Agent {
    id: String,
    name: String,
    created_at: DateTime
})

(:Goal {
    id: String,
    description: String,
    status: String,        // active, completed, failed, paused, abandoned
    priority: Integer,
    created_at: DateTime,
    completed_at: DateTime
})

(:Task {
    id: String,
    description: String,
    status: String,
    created_at: DateTime,
    completed_at: DateTime
})

(:Episode {
    id: String,
    query: String,
    response: String,
    success: Boolean,
    chunks_used: Integer,
    latency_ms: Integer,
    created_at: DateTime
})

(:Reflection {
    id: String,
    reflection_type: String,  // success, failure, pattern, improvement
    insight: String,
    created_at: DateTime
})

// Agent Memory Relationships
(Agent)-[:HAS_GOAL]->(Goal)
(Goal)-[:HAS_TASK]->(Task)
(Task)-[:DEPENDS_ON]->(Task)
(Agent)-[:EXPERIENCED]->(Episode)
(Episode)-[:USED_CHUNK]->(Chunk)
(Episode)-[:MENTIONED_ENTITY]->(Entity)
(Episode)-[:LED_TO]->(Reflection)
(Reflection)-[:ABOUT]->(Goal)
(Episode)-[:SIMILAR_TO {score: Float}]->(Episode)
```

---

## Implementation Plan

### Phase 1: Core Infrastructure (Week 1-2)

#### 1.1 Add Dependencies

```toml
# backend/Cargo.toml additions
[dependencies]
neo4rs = "0.7"              # Async Neo4j Bolt driver
deadpool = "0.10"           # Connection pooling

[features]
neo4j = ["neo4rs", "deadpool"]
```

#### 1.2 Create Graph Module Structure

```
backend/src/graph/
├── mod.rs                  # Module exports
├── config.rs               # Neo4j configuration
├── client.rs               # Connection pool & client
├── schema.rs               # Graph schema initialization
├── entity_linker.rs        # Entity extraction & linking
├── graph_retriever.rs      # Graph-augmented retrieval
├── knowledge_builder.rs    # Build graph from documents
└── agent_memory_graph.rs   # Agent memory in graph
```

#### 1.3 Neo4j Client Implementation

```rust
// backend/src/graph/client.rs

use neo4rs::{Graph, ConfigBuilder, Query};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

pub struct Neo4jConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
    pub database: String,
    pub max_connections: usize,
    pub enabled: bool,
}

impl Neo4jConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("NEO4J_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            uri: std::env::var("NEO4J_URI")
                .unwrap_or_else(|_| "bolt://localhost:7687".to_string()),
            user: std::env::var("NEO4J_USER")
                .unwrap_or_else(|_| "neo4j".to_string()),
            password: std::env::var("NEO4J_PASSWORD")
                .unwrap_or_else(|_| "password".to_string()),
            database: std::env::var("NEO4J_DATABASE")
                .unwrap_or_else(|_| "neo4j".to_string()),
            max_connections: std::env::var("NEO4J_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        }
    }
}

pub struct Neo4jClient {
    graph: Arc<Graph>,
    config: Neo4jConfig,
}

impl Neo4jClient {
    pub async fn new(config: Neo4jConfig) -> Result<Self, neo4rs::Error> {
        let graph = Graph::new(
            &config.uri,
            &config.user,
            &config.password,
        ).await?;
        
        info!(uri = %config.uri, "Connected to Neo4j");
        
        Ok(Self {
            graph: Arc::new(graph),
            config,
        })
    }
    
    pub async fn init_schema(&self) -> Result<(), neo4rs::Error> {
        // Create constraints and indexes
        let queries = vec![
            "CREATE CONSTRAINT doc_id IF NOT EXISTS FOR (d:Document) REQUIRE d.id IS UNIQUE",
            "CREATE CONSTRAINT chunk_id IF NOT EXISTS FOR (c:Chunk) REQUIRE c.id IS UNIQUE",
            "CREATE CONSTRAINT entity_id IF NOT EXISTS FOR (e:Entity) REQUIRE e.id IS UNIQUE",
            "CREATE INDEX chunk_embedding IF NOT EXISTS FOR (c:Chunk) ON (c.embedding_id)",
            "CREATE INDEX entity_name IF NOT EXISTS FOR (e:Entity) ON (e.normalized_name)",
            "CREATE FULLTEXT INDEX entity_search IF NOT EXISTS FOR (e:Entity) ON EACH [e.name, e.description]",
        ];
        
        for query in queries {
            self.graph.run(Query::new(query.to_string())).await?;
        }
        
        info!("Neo4j schema initialized");
        Ok(())
    }
    
    pub fn graph(&self) -> Arc<Graph> {
        Arc::clone(&self.graph)
    }
}
```

### Phase 2: Entity Extraction & Linking (Week 2-3)

#### 2.1 Enhanced Entity Extractor

```rust
// backend/src/graph/entity_linker.rs

use crate::tools::entity_extractor::EntityExtractorTool;
use neo4rs::{Graph, Query, Node};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: String,
    pub confidence: f32,
    pub start_offset: usize,
    pub end_offset: usize,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedEntity {
    pub id: String,
    pub name: String,
    pub normalized_name: String,
    pub entity_type: String,
    pub is_new: bool,
}

pub struct EntityLinker {
    graph: Arc<Graph>,
    extractor: EntityExtractorTool,
    // Cache for entity normalization
    entity_cache: HashMap<String, String>,
}

impl EntityLinker {
    pub fn new(graph: Arc<Graph>) -> Self {
        Self {
            graph,
            extractor: EntityExtractorTool::new(),
            entity_cache: HashMap::new(),
        }
    }
    
    /// Extract entities from chunk and link to graph
    pub async fn extract_and_link(
        &mut self,
        chunk_id: &str,
        chunk_content: &str,
    ) -> Result<Vec<LinkedEntity>, Box<dyn std::error::Error>> {
        // Step 1: Extract entities using existing tool
        let extracted = self.extractor.extract(chunk_content)?;
        
        let mut linked = Vec::new();
        
        for entity in extracted {
            // Step 2: Normalize entity name
            let normalized = self.normalize_entity(&entity.name);
            
            // Step 3: Find or create entity in graph
            let linked_entity = self.find_or_create_entity(
                &entity.name,
                &normalized,
                &entity.entity_type,
            ).await?;
            
            // Step 4: Create MENTIONS relationship
            self.create_mention(
                chunk_id,
                &linked_entity.id,
                entity.confidence,
                &entity.context,
                entity.start_offset,
                entity.end_offset,
            ).await?;
            
            linked.push(linked_entity);
        }
        
        Ok(linked)
    }
    
    fn normalize_entity(&self, name: &str) -> String {
        name.trim()
            .to_lowercase()
            .replace("  ", " ")
    }
    
    async fn find_or_create_entity(
        &self,
        name: &str,
        normalized: &str,
        entity_type: &str,
    ) -> Result<LinkedEntity, neo4rs::Error> {
        let query = Query::new(
            "MERGE (e:Entity {normalized_name: $normalized})
             ON CREATE SET 
                e.id = randomUUID(),
                e.name = $name,
                e.entity_type = $type,
                e.mention_count = 1,
                e.first_seen = datetime(),
                e.last_seen = datetime()
             ON MATCH SET
                e.mention_count = e.mention_count + 1,
                e.last_seen = datetime()
             RETURN e.id as id, e.name as name, e.normalized_name as normalized,
                    e.entity_type as type, 
                    e.mention_count = 1 as is_new"
                .to_string()
        )
        .param("normalized", normalized)
        .param("name", name)
        .param("type", entity_type);
        
        let mut result = self.graph.execute(query).await?;
        let row = result.next().await?.unwrap();
        
        Ok(LinkedEntity {
            id: row.get("id")?,
            name: row.get("name")?,
            normalized_name: row.get("normalized")?,
            entity_type: row.get("type")?,
            is_new: row.get("is_new")?,
        })
    }
    
    async fn create_mention(
        &self,
        chunk_id: &str,
        entity_id: &str,
        confidence: f32,
        context: &str,
        start: usize,
        end: usize,
    ) -> Result<(), neo4rs::Error> {
        let query = Query::new(
            "MATCH (c:Chunk {id: $chunk_id})
             MATCH (e:Entity {id: $entity_id})
             MERGE (c)-[m:MENTIONS]->(e)
             SET m.confidence = $confidence,
                 m.context = $context,
                 m.start_offset = $start,
                 m.end_offset = $end"
                .to_string()
        )
        .param("chunk_id", chunk_id)
        .param("entity_id", entity_id)
        .param("confidence", confidence)
        .param("context", context)
        .param("start", start as i64)
        .param("end", end as i64);
        
        self.graph.run(query).await
    }
}
```

### Phase 3: Graph-Augmented Retrieval (Week 3-4)

#### 3.1 Graph Retriever

```rust
// backend/src/graph/graph_retriever.rs

use neo4rs::{Graph, Query};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedChunk {
    pub chunk_id: String,
    pub content: String,
    pub expansion_score: f32,
    pub expansion_path: Vec<String>,  // How we got here
    pub shared_entities: Vec<String>,
}

pub struct GraphRetriever {
    graph: Arc<Graph>,
    config: GraphExpansionConfig,
}

impl GraphRetriever {
    pub fn new(graph: Arc<Graph>, config: GraphExpansionConfig) -> Self {
        Self { graph, config }
    }
    
    /// Expand retrieved chunks using graph relationships
    pub async fn expand_context(
        &self,
        seed_chunk_ids: &[String],
        query_entities: &[String],
    ) -> Result<Vec<ExpandedChunk>, neo4rs::Error> {
        let mut expanded = Vec::new();
        let seed_set: HashSet<_> = seed_chunk_ids.iter().collect();
        
        // Strategy 1: Find chunks that share entities with seed chunks
        let entity_expanded = self.expand_via_entities(seed_chunk_ids).await?;
        
        // Strategy 2: Find chunks discussing related concepts
        let concept_expanded = self.expand_via_concepts(seed_chunk_ids).await?;
        
        // Strategy 3: Find chunks mentioning query entities
        let query_entity_chunks = self.find_chunks_by_entities(query_entities).await?;
        
        // Merge and deduplicate
        let mut seen = seed_set.clone();
        
        for chunk in entity_expanded.into_iter()
            .chain(concept_expanded)
            .chain(query_entity_chunks)
        {
            if !seen.contains(&chunk.chunk_id) && expanded.len() < self.config.max_expanded_chunks {
                seen.insert(chunk.chunk_id.clone());
                expanded.push(chunk);
            }
        }
        
        // Sort by expansion score
        expanded.sort_by(|a, b| b.expansion_score.partial_cmp(&a.expansion_score).unwrap());
        
        Ok(expanded)
    }
    
    /// Find related chunks through shared entities
    async fn expand_via_entities(
        &self,
        seed_chunk_ids: &[String],
    ) -> Result<Vec<ExpandedChunk>, neo4rs::Error> {
        let query = Query::new(
            "UNWIND $chunk_ids AS seed_id
             MATCH (seed:Chunk {id: seed_id})-[:MENTIONS]->(e:Entity)<-[m:MENTIONS]-(related:Chunk)
             WHERE related.id <> seed_id
             WITH related, e, m, count(DISTINCT seed_id) as shared_count
             RETURN related.id as chunk_id,
                    related.content as content,
                    collect(DISTINCT e.name) as shared_entities,
                    shared_count * avg(m.confidence) as score
             ORDER BY score DESC
             LIMIT $limit"
                .to_string()
        )
        .param("chunk_ids", seed_chunk_ids.to_vec())
        .param("limit", self.config.max_expanded_chunks as i64);
        
        let mut result = self.graph.execute(query).await?;
        let mut expanded = Vec::new();
        
        while let Some(row) = result.next().await? {
            expanded.push(ExpandedChunk {
                chunk_id: row.get("chunk_id")?,
                content: row.get("content")?,
                expansion_score: row.get::<f64>("score")? as f32 * self.config.entity_weight,
                expansion_path: vec!["entity_link".to_string()],
                shared_entities: row.get("shared_entities")?,
            });
        }
        
        Ok(expanded)
    }
    
    /// Find related chunks through concept relationships
    async fn expand_via_concepts(
        &self,
        seed_chunk_ids: &[String],
    ) -> Result<Vec<ExpandedChunk>, neo4rs::Error> {
        let query = Query::new(
            "UNWIND $chunk_ids AS seed_id
             MATCH (seed:Chunk {id: seed_id})-[:DISCUSSES]->(c:Concept)
             MATCH (c)-[:RELATED_TO|BROADER_THAN*1..2]-(related_concept:Concept)
             MATCH (related:Chunk)-[:DISCUSSES]->(related_concept)
             WHERE related.id <> seed_id
             WITH related, collect(DISTINCT related_concept.name) as concepts
             RETURN related.id as chunk_id,
                    related.content as content,
                    concepts,
                    size(concepts) as score
             ORDER BY score DESC
             LIMIT $limit"
                .to_string()
        )
        .param("chunk_ids", seed_chunk_ids.to_vec())
        .param("limit", self.config.max_expanded_chunks as i64);
        
        let mut result = self.graph.execute(query).await?;
        let mut expanded = Vec::new();
        
        while let Some(row) = result.next().await? {
            expanded.push(ExpandedChunk {
                chunk_id: row.get("chunk_id")?,
                content: row.get("content")?,
                expansion_score: row.get::<i64>("score")? as f32 * self.config.concept_weight,
                expansion_path: vec!["concept_link".to_string()],
                shared_entities: row.get("concepts")?,
            });
        }
        
        Ok(expanded)
    }
    
    /// Find chunks that mention specific entities
    async fn find_chunks_by_entities(
        &self,
        entity_names: &[String],
    ) -> Result<Vec<ExpandedChunk>, neo4rs::Error> {
        if entity_names.is_empty() {
            return Ok(Vec::new());
        }
        
        let query = Query::new(
            "UNWIND $entities AS entity_name
             MATCH (e:Entity)
             WHERE toLower(e.name) CONTAINS toLower(entity_name)
                OR toLower(e.normalized_name) CONTAINS toLower(entity_name)
             MATCH (c:Chunk)-[m:MENTIONS]->(e)
             WITH c, collect(DISTINCT e.name) as matched_entities, avg(m.confidence) as conf
             RETURN c.id as chunk_id,
                    c.content as content,
                    matched_entities,
                    size(matched_entities) * conf as score
             ORDER BY score DESC
             LIMIT $limit"
                .to_string()
        )
        .param("entities", entity_names.to_vec())
        .param("limit", self.config.max_expanded_chunks as i64);
        
        let mut result = self.graph.execute(query).await?;
        let mut expanded = Vec::new();
        
        while let Some(row) = result.next().await? {
            expanded.push(ExpandedChunk {
                chunk_id: row.get("chunk_id")?,
                content: row.get("content")?,
                expansion_score: row.get::<f64>("score")? as f32,
                expansion_path: vec!["query_entity".to_string()],
                shared_entities: row.get("matched_entities")?,
            });
        }
        
        Ok(expanded)
    }
    
    /// Get reasoning path between two entities
    pub async fn find_entity_path(
        &self,
        entity1: &str,
        entity2: &str,
        max_hops: usize,
    ) -> Result<Vec<String>, neo4rs::Error> {
        let query = Query::new(
            format!(
                "MATCH (e1:Entity), (e2:Entity)
                 WHERE toLower(e1.name) CONTAINS toLower($entity1)
                   AND toLower(e2.name) CONTAINS toLower($entity2)
                 MATCH path = shortestPath((e1)-[*..{}]-(e2))
                 RETURN [n IN nodes(path) | 
                    CASE WHEN n:Entity THEN n.name
                         WHEN n:Chunk THEN 'chunk:' + n.id
                         WHEN n:Concept THEN 'concept:' + n.name
                         ELSE 'unknown'
                    END
                 ] as path_nodes
                 LIMIT 1",
                max_hops
            )
        )
        .param("entity1", entity1)
        .param("entity2", entity2);
        
        let mut result = self.graph.execute(query).await?;
        
        if let Some(row) = result.next().await? {
            Ok(row.get("path_nodes")?)
        } else {
            Ok(Vec::new())
        }
    }
}
```

### Phase 4: Integration with Existing Pipeline (Week 4-5)

#### 4.1 Enhanced RAG Query Pipeline

```rust
// Modifications to backend/src/memory/query.rs

use crate::graph::{GraphRetriever, EntityLinker, ExpandedChunk};

pub struct EnhancedRagQueryPipeline {
    // Existing fields
    embedding_service: Arc<EmbeddingService>,
    vector_store: Arc<RwLock<VectorStore>>,
    llm_provider: Arc<dyn LLMProvider>,
    config: RagConfig,
    
    // NEW: Graph components
    graph_retriever: Option<Arc<GraphRetriever>>,
    entity_linker: Option<Arc<RwLock<EntityLinker>>>,
}

impl EnhancedRagQueryPipeline {
    pub async fn query_with_graph(
        &self,
        req: &RagQueryRequest,
    ) -> Result<RagQueryResponse, RagError> {
        info!(query = %req.query, "Starting GraphRAG query");
        
        // Step 1: Extract entities from query
        let query_entities = if let Some(linker) = &self.entity_linker {
            linker.read().await.extract_entities(&req.query)?
        } else {
            Vec::new()
        };
        
        // Step 2: Standard vector retrieval
        let query_embedding = self.embedding_service.embed_query(&req.query).await;
        let vector_results = self.vector_store.read().await
            .search(&query_embedding, req.top_k)?;
        
        // Step 3: Graph expansion (if enabled)
        let expanded_chunks = if let Some(graph_ret) = &self.graph_retriever {
            let seed_ids: Vec<_> = vector_results.iter()
                .map(|r| r.chunk_id.clone())
                .collect();
            let entity_names: Vec<_> = query_entities.iter()
                .map(|e| e.name.clone())
                .collect();
            
            graph_ret.expand_context(&seed_ids, &entity_names).await?
        } else {
            Vec::new()
        };
        
        // Step 4: Merge and rerank
        let mut all_chunks = self.merge_results(vector_results, expanded_chunks);
        
        // Step 5: Build context and generate answer
        let context = self.build_context(&all_chunks);
        let answer = self.llm_provider.generate(&req.query, &context).await?;
        
        Ok(RagQueryResponse {
            query: req.query.clone(),
            answer,
            context_chunks: all_chunks.into_iter().map(|c| c.into()).collect(),
            total_chunks_used: all_chunks.len(),
            sources: self.extract_sources(&all_chunks),
            // NEW: Graph metadata
            graph_expansion_used: self.graph_retriever.is_some(),
            entities_found: query_entities.iter().map(|e| e.name.clone()).collect(),
        })
    }
    
    fn merge_results(
        &self,
        vector_results: Vec<SearchResult>,
        graph_expanded: Vec<ExpandedChunk>,
    ) -> Vec<MergedChunk> {
        // Reciprocal Rank Fusion with graph boost
        let mut scores: HashMap<String, f32> = HashMap::new();
        
        // Vector results (primary)
        for (rank, result) in vector_results.iter().enumerate() {
            let rrf_score = 1.0 / (60.0 + rank as f32);
            *scores.entry(result.chunk_id.clone()).or_default() += rrf_score;
        }
        
        // Graph expanded (secondary with boost for shared entities)
        for chunk in &graph_expanded {
            let graph_boost = 0.3 + (chunk.shared_entities.len() as f32 * 0.1);
            *scores.entry(chunk.chunk_id.clone()).or_default() += 
                chunk.expansion_score * graph_boost;
        }
        
        // Sort by combined score
        let mut merged: Vec<_> = scores.into_iter().collect();
        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        merged.into_iter()
            .take(self.config.top_k)
            .map(|(id, score)| MergedChunk { chunk_id: id, score })
            .collect()
    }
}
```

### Phase 5: Agent Memory Graph (Week 5-6)

#### 5.1 Graph-Based Agent Memory

```rust
// backend/src/graph/agent_memory_graph.rs

use neo4rs::{Graph, Query};
use crate::memory::{Episode, Goal, Reflection, Task};

pub struct AgentMemoryGraph {
    graph: Arc<Graph>,
    agent_id: String,
}

impl AgentMemoryGraph {
    pub async fn record_episode(
        &self,
        episode: &Episode,
        used_chunk_ids: &[String],
        mentioned_entities: &[String],
    ) -> Result<(), neo4rs::Error> {
        // Create episode node
        let query = Query::new(
            "MATCH (a:Agent {id: $agent_id})
             CREATE (e:Episode {
                id: $id,
                query: $query,
                response: $response,
                success: $success,
                chunks_used: $chunks_used,
                created_at: datetime()
             })
             CREATE (a)-[:EXPERIENCED]->(e)
             WITH e
             UNWIND $chunk_ids AS chunk_id
             MATCH (c:Chunk {id: chunk_id})
             CREATE (e)-[:USED_CHUNK]->(c)
             WITH e
             UNWIND $entity_names AS entity_name
             MATCH (ent:Entity {normalized_name: toLower($entity_name)})
             CREATE (e)-[:MENTIONED_ENTITY]->(ent)"
                .to_string()
        )
        .param("agent_id", &self.agent_id)
        .param("id", &episode.id)
        .param("query", &episode.query)
        .param("response", &episode.response)
        .param("success", episode.success)
        .param("chunks_used", episode.context_chunks_used as i64)
        .param("chunk_ids", used_chunk_ids.to_vec())
        .param("entity_names", mentioned_entities.to_vec());
        
        self.graph.run(query).await
    }
    
    /// Find similar past episodes using graph structure
    pub async fn find_similar_episodes(
        &self,
        query_entities: &[String],
        limit: usize,
    ) -> Result<Vec<SimilarEpisode>, neo4rs::Error> {
        let query = Query::new(
            "MATCH (a:Agent {id: $agent_id})-[:EXPERIENCED]->(e:Episode)
             OPTIONAL MATCH (e)-[:MENTIONED_ENTITY]->(ent:Entity)
             WHERE ent.normalized_name IN $entities
             WITH e, count(DISTINCT ent) as entity_overlap
             OPTIONAL MATCH (e)-[:USED_CHUNK]->(c:Chunk)-[:MENTIONS]->(ent2:Entity)
             WHERE ent2.normalized_name IN $entities
             WITH e, entity_overlap, count(DISTINCT ent2) as chunk_entity_overlap
             WITH e, entity_overlap + chunk_entity_overlap * 0.5 as similarity
             WHERE similarity > 0
             RETURN e.id as id, e.query as query, e.response as response,
                    e.success as success, similarity
             ORDER BY similarity DESC
             LIMIT $limit"
                .to_string()
        )
        .param("agent_id", &self.agent_id)
        .param("entities", query_entities.iter().map(|e| e.to_lowercase()).collect::<Vec<_>>())
        .param("limit", limit as i64);
        
        let mut result = self.graph.execute(query).await?;
        let mut episodes = Vec::new();
        
        while let Some(row) = result.next().await? {
            episodes.push(SimilarEpisode {
                id: row.get("id")?,
                query: row.get("query")?,
                response: row.get("response")?,
                success: row.get("success")?,
                similarity: row.get::<f64>("similarity")? as f32,
            });
        }
        
        Ok(episodes)
    }
    
    /// Discover patterns across episodes
    pub async fn discover_patterns(&self) -> Result<Vec<Pattern>, neo4rs::Error> {
        let query = Query::new(
            "MATCH (a:Agent {id: $agent_id})-[:EXPERIENCED]->(e:Episode)
             MATCH (e)-[:MENTIONED_ENTITY]->(ent:Entity)
             WITH ent, count(e) as episode_count, 
                  sum(CASE WHEN e.success THEN 1 ELSE 0 END) as success_count
             WHERE episode_count >= 3
             RETURN ent.name as entity,
                    episode_count,
                    success_count,
                    toFloat(success_count) / episode_count as success_rate
             ORDER BY episode_count DESC
             LIMIT 20"
                .to_string()
        )
        .param("agent_id", &self.agent_id);
        
        let mut result = self.graph.execute(query).await?;
        let mut patterns = Vec::new();
        
        while let Some(row) = result.next().await? {
            patterns.push(Pattern {
                entity: row.get("entity")?,
                episode_count: row.get::<i64>("episode_count")? as usize,
                success_count: row.get::<i64>("success_count")? as usize,
                success_rate: row.get::<f64>("success_rate")? as f32,
            });
        }
        
        Ok(patterns)
    }
}
```

---

## Configuration

### Environment Variables

```bash
# .env additions for Neo4j

# ═══════════════════════════════════════════════════════════════════════
# Neo4j Knowledge Graph Configuration
# ═══════════════════════════════════════════════════════════════════════

# Enable/disable Neo4j integration
NEO4J_ENABLED=true

# Connection settings
NEO4J_URI=bolt://localhost:7687
NEO4J_USER=neo4j
NEO4J_PASSWORD=your_secure_password
NEO4J_DATABASE=ag

# Connection pool
NEO4J_MAX_CONNECTIONS=10
NEO4J_CONNECTION_TIMEOUT_MS=5000

# Graph expansion settings
GRAPH_EXPANSION_ENABLED=true
GRAPH_EXPANSION_MAX_HOPS=2
GRAPH_EXPANSION_MAX_CHUNKS=10
GRAPH_ENTITY_WEIGHT=0.7
GRAPH_CONCEPT_WEIGHT=0.5
GRAPH_MIN_RELATIONSHIP_STRENGTH=0.3

# Entity extraction settings
ENTITY_EXTRACTION_ENABLED=true
ENTITY_EXTRACTION_CONFIDENCE_THRESHOLD=0.5
ENTITY_LINKING_FUZZY_THRESHOLD=0.8
```

---

## API Endpoints

### New Endpoints

```rust
// backend/src/api/graph_routes.rs

/// GET /graph/entities?query=<text>
/// Extract and return entities from text
pub async fn extract_entities(query: web::Query<EntityQuery>) -> HttpResponse;

/// GET /graph/entity/{name}/related
/// Get entities related to the given entity
pub async fn get_related_entities(path: web::Path<String>) -> HttpResponse;

/// GET /graph/chunk/{id}/graph
/// Get the subgraph around a chunk (entities, concepts, related chunks)
pub async fn get_chunk_graph(path: web::Path<String>) -> HttpResponse;

/// GET /graph/path?from=<entity>&to=<entity>
/// Find reasoning path between two entities
pub async fn find_entity_path(query: web::Query<PathQuery>) -> HttpResponse;

/// GET /graph/stats
/// Get graph statistics (node counts, relationship counts)
pub async fn get_graph_stats() -> HttpResponse;

/// POST /graph/rebuild
/// Rebuild graph from existing chunks (reprocess all documents)
pub async fn rebuild_graph() -> HttpResponse;
```

---

## Expected Benefits

| Metric | Current | With Neo4j | Improvement |
|--------|---------|------------|-------------|
| Multi-hop reasoning accuracy | ~40% | ~70% | +75% |
| "How/Why" question handling | Poor | Good | Significant |
| Entity disambiguation | Basic | Strong | +50% |
| Context relevance | Good | Excellent | +25% |
| Explainability | Limited | Full paths | Major |
| Pattern discovery | None | Automatic | New capability |

---

## Migration Path

### Step 1: Install Neo4j (Optional Feature)
```bash
# Docker
docker run -d --name neo4j \
  -p 7474:7474 -p 7687:7687 \
  -e NEO4J_AUTH=neo4j/password \
  neo4j:5.15

# Or use Neo4j Desktop / Aura
```

### Step 2: Enable Feature
```bash
# Build with neo4j feature
cargo build --features neo4j

# Or add to default features in Cargo.toml
```

### Step 3: Initialize Graph
```bash
# Set environment variables
export NEO4J_ENABLED=true
export NEO4J_URI=bolt://localhost:7687

# Start AG - schema will be created automatically
cargo run
```

### Step 4: Rebuild Graph from Existing Data
```bash
# POST to rebuild endpoint
curl -X POST http://localhost:3010/graph/rebuild
```

---

## Monitoring

### Prometheus Metrics

```rust
// New metrics for graph operations
lazy_static! {
    static ref GRAPH_QUERY_DURATION: HistogramVec = register_histogram_vec!(
        "ag_graph_query_duration_seconds",
        "Duration of graph queries",
        &["query_type"]
    ).unwrap();
    
    static ref GRAPH_EXPANSION_CHUNKS: Histogram = register_histogram!(
        "ag_graph_expansion_chunks",
        "Number of chunks added via graph expansion"
    ).unwrap();
    
    static ref ENTITY_EXTRACTION_COUNT: Counter = register_counter!(
        "ag_entity_extraction_total",
        "Total entities extracted"
    ).unwrap();
}
```

### Grafana Dashboard

Add panels for:
- Graph query latency
- Entity extraction rate
- Graph expansion effectiveness
- Cache hit rates for entity lookups

---

## Future Enhancements

1. **Vector Embeddings in Neo4j** - Use Neo4j's native vector index (5.11+) for unified storage
2. **Temporal Reasoning** - Add time-based relationships for "what changed since X"
3. **Multi-Agent Graphs** - Share knowledge graphs across agent instances
4. **Federated Graphs** - Connect to external knowledge graphs (Wikidata, DBpedia)
5. **Graph Neural Networks** - Use GNNs for advanced entity/relation prediction

---

## References

- [Neo4j Rust Driver (neo4rs)](https://github.com/neo4j-labs/neo4rs)
- [GraphRAG Paper](https://arxiv.org/abs/2404.16130)
- [Microsoft GraphRAG](https://github.com/microsoft/graphrag)
- [Neo4j Vector Search](https://neo4j.com/docs/cypher-manual/current/indexes-for-vector-search/)
