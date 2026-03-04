//! Graph API routes for knowledge graph visualization and search
//! v1.3.0 - Extracted rebuild_graph_from_index() helper for reuse
//!
//! Provides endpoints for:
//! - Graph statistics (Neo4j and Petgraph)
//! - Graph data sampling for visualization
//! - Entity search
//! - Graph-enhanced search (vector + graph)
//! - Graph rebuild from indexed documents
//! - Petgraph runtime endpoints (work without Neo4j)
//!
//! INSTALLER IMPACT (v1.3.0): None - same routes, same response types

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tantivy::schema::Value;
use tracing::{debug, info, warn};

#[cfg(feature = "neo4j")]
use crate::api::get_neo4j_client;

// Import petgraph runtime (always available)
use crate::graph::petgraph_runtime::{get_runtime_graph, ChunkNode, GraphQuery, Relationship};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    pub document_count: usize,
    pub chunk_count: usize,
    pub entity_count: usize,
    pub relationship_count: usize,
    pub entity_types: Vec<EntityTypeCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeCount {
    pub entity_type: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub label: String,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSearchResult {
    pub chunk_id: String,
    pub content: String,
    pub score: f32,
    pub entities: Vec<String>,
    pub related_chunks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSearchResponse {
    pub results: Vec<GraphSearchResult>,
    pub total_results: usize,
    pub graph_enhanced: bool,
}

/// Result of a knowledge graph rebuild operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphBuildResult {
    pub status: String,
    pub documents_processed: usize,
    pub chunks_processed: usize,
    pub entities_extracted: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SampleQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
}

// ============================================================================
// Petgraph Runtime Endpoints (Option C: No Neo4j required at runtime)
// ============================================================================

/// GET /graph/rt/stats - Get petgraph runtime stats (no Neo4j needed)
pub async fn get_petgraph_stats() -> HttpResponse {
    let runtime = get_runtime_graph();

    HttpResponse::Ok().json(serde_json::json!({
        "source": "petgraph_runtime",
        "node_count": runtime.node_count(),
        "edge_count": runtime.edge_count(),
        "is_empty": runtime.is_empty()
    }))
}

/// GET /graph/rt/node/{id} - Get node by ID from petgraph
pub async fn get_petgraph_node(path: web::Path<String>) -> HttpResponse {
    let node_id = path.into_inner();
    let runtime = get_runtime_graph();
    let query = GraphQuery::new(&runtime);

    match query.get_node(&node_id) {
        Some(node) => HttpResponse::Ok().json(node),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Node not found",
            "id": node_id
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct TraverseRequest {
    pub seed_id: String,
    pub max_hops: Option<usize>,
    pub rel_filter: Option<String>,
}

/// POST /graph/rt/traverse - BFS traversal from petgraph
pub async fn petgraph_traverse(body: web::Json<TraverseRequest>) -> HttpResponse {
    let runtime = get_runtime_graph();
    let query = GraphQuery::new(&runtime);

    let max_hops = body.max_hops.unwrap_or(2).min(5);
    let rel_filter = body.rel_filter.as_deref().unwrap_or("");

    let results = query.constrained_bfs(&body.seed_id, max_hops, rel_filter);

    HttpResponse::Ok().json(serde_json::json!({
        "seed_id": body.seed_id,
        "max_hops": max_hops,
        "rel_filter": rel_filter,
        "results_count": results.len(),
        "results": results
    }))
}

/// GET /graph/rt/neighbors/{id} - Get neighbors from petgraph
pub async fn get_petgraph_neighbors(path: web::Path<String>) -> HttpResponse {
    let node_id = path.into_inner();
    let runtime = get_runtime_graph();
    let query = GraphQuery::new(&runtime);

    let neighbors = query.get_neighbors(&node_id);

    #[derive(Serialize)]
    struct NeighborResult {
        node: ChunkNode,
        relationship: Relationship,
    }

    let results: Vec<NeighborResult> = neighbors
        .into_iter()
        .map(|(node, rel)| NeighborResult {
            node: node.clone(),
            relationship: rel.clone(),
        })
        .collect();

    HttpResponse::Ok().json(serde_json::json!({
        "node_id": node_id,
        "neighbors_count": results.len(),
        "neighbors": results
    }))
}

// ============================================================================
// Export for Petgraph (Option C: Neo4j design-time → Petgraph runtime)
// ============================================================================

/// POST /graph/export - Export Neo4j graph to JSON for petgraph runtime
#[cfg(feature = "neo4j")]
pub async fn export_graph_to_json() -> HttpResponse {
    let Some(client) = get_neo4j_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Neo4j not connected. Start Neo4j to export."
        }));
    };

    let graph = client.graph();

    #[derive(Serialize)]
    struct ExportFormat {
        nodes: Vec<ChunkNode>,
        relationships: Vec<ExportRelationship>,
    }

    #[derive(Serialize)]
    struct ExportRelationship {
        from_id: String,
        to_id: String,
        #[serde(rename = "type")]
        rel_type: String,
        confidence: f32,
        meta: serde_json::Value,
    }

    let mut export = ExportFormat {
        nodes: Vec::new(),
        relationships: Vec::new(),
    };

    // Fetch all Chunk nodes
    let nodes_query = neo4rs::query(
        "MATCH (n:Chunk)
         RETURN n.id AS id, n.content AS content, 
                coalesce(n.entities, []) AS entities,
                coalesce(n.source, 'unknown') AS source",
    );

    match graph.execute(nodes_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let id: String = row.get("id").unwrap_or_default();
                let content: String = row.get("content").unwrap_or_default();
                let entities: Vec<String> = row.get("entities").unwrap_or_default();
                let source: String = row.get("source").unwrap_or_default();

                export.nodes.push(ChunkNode {
                    id,
                    content,
                    entities,
                    source,
                });
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch nodes for export");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch nodes: {}", e)
            }));
        }
    }

    // Fetch all relationships between Chunks
    let rels_query = neo4rs::query(
        "MATCH (n:Chunk)-[r]->(m:Chunk)
         RETURN n.id AS from_id, m.id AS to_id, type(r) AS rel_type,
                coalesce(r.confidence, 0.8) AS confidence,
                coalesce(r.metadata, {}) AS meta",
    );

    match graph.execute(rels_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let from_id: String = row.get("from_id").unwrap_or_default();
                let to_id: String = row.get("to_id").unwrap_or_default();
                let rel_type: String = row.get("rel_type").unwrap_or_default();
                let confidence: f32 = row.get("confidence").unwrap_or(0.8);
                let meta: serde_json::Value = row.get("meta").unwrap_or(serde_json::json!({}));

                export.relationships.push(ExportRelationship {
                    from_id,
                    to_id,
                    rel_type,
                    confidence,
                    meta,
                });
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch relationships for export");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch relationships: {}", e)
            }));
        }
    }

    // Save to file
    let data_dir = std::env::var("AG_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let json_path = format!("{}/knowledge_graph.json", data_dir);

    if let Some(parent) = std::path::Path::new(&json_path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!(error = %e, "Failed to create data directory");
        }
    }

    match serde_json::to_string_pretty(&export) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&json_path, &json) {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to write file: {}", e)
                }));
            }

            info!(
                nodes = export.nodes.len(),
                relationships = export.relationships.len(),
                path = %json_path,
                "Exported graph to JSON"
            );

            HttpResponse::Ok().json(serde_json::json!({
                "status": "exported",
                "nodes": export.nodes.len(),
                "relationships": export.relationships.len(),
                "path": json_path
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to serialize: {}", e)
        })),
    }
}

#[cfg(not(feature = "neo4j"))]
pub async fn export_graph_to_json() -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "Neo4j feature not enabled. Build with --features neo4j to export."
    }))
}

// ============================================================================
// Neo4j Handlers
// ============================================================================

/// GET /graph/stats - Get graph statistics
#[cfg(feature = "neo4j")]
pub async fn get_graph_stats() -> HttpResponse {
    let Some(client) = get_neo4j_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Neo4j not connected"
        }));
    };

    let graph = client.graph();

    let stats_query = neo4rs::query(
        "MATCH (d:Document) WITH count(d) as docs
         MATCH (c:Chunk) WITH docs, count(c) as chunks
         MATCH (e:Entity) WITH docs, chunks, count(e) as entities
         MATCH ()-[r]->() WITH docs, chunks, entities, count(r) as rels
         RETURN docs, chunks, entities, rels",
    );

    let mut stats = GraphStats::default();

    match graph.execute(stats_query).await {
        Ok(mut result) => {
            if let Ok(Some(row)) = result.next().await {
                stats.document_count = row.get::<i64>("docs").unwrap_or(0) as usize;
                stats.chunk_count = row.get::<i64>("chunks").unwrap_or(0) as usize;
                stats.entity_count = row.get::<i64>("entities").unwrap_or(0) as usize;
                stats.relationship_count = row.get::<i64>("rels").unwrap_or(0) as usize;
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch graph stats");
        }
    }

    let types_query = neo4rs::query(
        "MATCH (e:Entity)
         RETURN e.entity_type as type, count(*) as count
         ORDER BY count DESC",
    );

    match graph.execute(types_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                if let (Some(entity_type), Some(count)) =
                    (row.get::<String>("type").ok(), row.get::<i64>("count").ok())
                {
                    stats.entity_types.push(EntityTypeCount {
                        entity_type,
                        count: count as usize,
                    });
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch entity types");
        }
    }

    HttpResponse::Ok().json(stats)
}

#[cfg(not(feature = "neo4j"))]
pub async fn get_graph_stats() -> HttpResponse {
    // Fallback to petgraph stats when Neo4j not available
    get_petgraph_stats().await
}

/// GET /graph/sample - Get sample graph data for visualization
/// v1.1.0: Fixed - fetches all node types + edges + uses elementId() for reliable IDs
#[cfg(feature = "neo4j")]
pub async fn get_graph_sample(query: web::Query<SampleQuery>) -> HttpResponse {
    let limit = query.limit.unwrap_or(50).min(200);

    let Some(client) = get_neo4j_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Neo4j not connected"
        }));
    };

    let graph = client.graph();
    let mut data = GraphData::default();
    let mut all_node_ids: Vec<String> = Vec::new();

    // Proportional limits per node type
    let doc_limit = ((limit as f64) * 0.2).ceil() as i64;
    let chunk_limit = ((limit as f64) * 0.3).ceil() as i64;
    let entity_limit = ((limit as f64) * 0.5).ceil() as i64;

    // ── 1. Fetch Document nodes ──────────────────────────────────
    let docs_query = neo4rs::query(
        "MATCH (d:Document)
         RETURN elementId(d) AS id,
                coalesce(d.title, d.name, 'Untitled') AS label
         LIMIT $limit",
    )
    .param("limit", doc_limit);

    match graph.execute(docs_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let id = row.get::<String>("id").unwrap_or_default();
                let label = row.get::<String>("label").unwrap_or_default();

                if !id.is_empty() {
                    all_node_ids.push(id.clone());
                    data.nodes.push(GraphNode {
                        id,
                        label: truncate_label(&label, 30),
                        node_type: "Document".to_string(),
                        properties: HashMap::new(),
                    });
                }
            }
        }
        Err(e) => warn!(error = %e, "Failed to fetch Document nodes"),
    }

    // ── 2. Fetch Chunk nodes ─────────────────────────────────────
    let chunks_query = neo4rs::query(
        "MATCH (c:Chunk)
         RETURN elementId(c) AS id,
                coalesce(c.chunk_id, left(c.content, 40), 'chunk') AS label
         LIMIT $limit",
    )
    .param("limit", chunk_limit);

    match graph.execute(chunks_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let id = row.get::<String>("id").unwrap_or_default();
                let label = row.get::<String>("label").unwrap_or_default();

                if !id.is_empty() {
                    all_node_ids.push(id.clone());
                    data.nodes.push(GraphNode {
                        id,
                        label: truncate_label(&label, 40),
                        node_type: "Chunk".to_string(),
                        properties: HashMap::new(),
                    });
                }
            }
        }
        Err(e) => warn!(error = %e, "Failed to fetch Chunk nodes"),
    }

    // ── 3. Fetch Entity nodes ────────────────────────────────────
    let entities_query = neo4rs::query(
        "MATCH (e:Entity)
         RETURN elementId(e) AS id,
                coalesce(e.name, e.label, 'entity') AS name,
                coalesce(e.entity_type, 'Unknown') AS type,
                coalesce(e.mention_count, 0) AS mentions
         ORDER BY e.mention_count DESC
         LIMIT $limit",
    )
    .param("limit", entity_limit);

    match graph.execute(entities_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let id = row.get::<String>("id").unwrap_or_default();
                let name = row.get::<String>("name").unwrap_or_default();
                let entity_type = row.get::<String>("type").unwrap_or_default();
                let mentions = row.get::<i64>("mentions").unwrap_or(0);

                if !id.is_empty() {
                    let mut properties = HashMap::new();
                    properties.insert("entity_type".to_string(), entity_type);
                    properties.insert("mentions".to_string(), mentions.to_string());

                    all_node_ids.push(id.clone());
                    data.nodes.push(GraphNode {
                        id,
                        label: name,
                        node_type: "Entity".to_string(),
                        properties,
                    });
                }
            }
        }
        Err(e) => warn!(error = %e, "Failed to fetch Entity nodes"),
    }

    // ── 4. Fetch edges between sampled nodes ─────────────────────
    if !all_node_ids.is_empty() {
        let edges_query = neo4rs::query(
            "MATCH (a)-[r]->(b)
             WHERE elementId(a) IN $ids AND elementId(b) IN $ids
             RETURN elementId(a) AS from_id,
                    elementId(b) AS to_id,
                    type(r) AS rel_type
             LIMIT 500",
        )
        .param("ids", all_node_ids);

        match graph.execute(edges_query).await {
            Ok(mut result) => {
                while let Ok(Some(row)) = result.next().await {
                    let from = row.get::<String>("from_id").unwrap_or_default();
                    let to = row.get::<String>("to_id").unwrap_or_default();
                    let label = row.get::<String>("rel_type").unwrap_or_default();

                    if !from.is_empty() && !to.is_empty() {
                        data.edges.push(GraphEdge {
                            from,
                            to,
                            label,
                            properties: HashMap::new(),
                        });
                    }
                }
            }
            Err(e) => warn!(error = %e, "Failed to fetch edges"),
        }
    }

    debug!(
        nodes = data.nodes.len(),
        edges = data.edges.len(),
        "Returning graph sample"
    );
    HttpResponse::Ok().json(data)
}

#[cfg(not(feature = "neo4j"))]
pub async fn get_graph_sample(_query: web::Query<SampleQuery>) -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "Neo4j feature not enabled"
    }))
}

/// GET /graph/search - Search for entities
/// v1.1.0: Fixed - uses elementId() for reliable IDs
#[cfg(feature = "neo4j")]
pub async fn search_entities(query: web::Query<SearchQuery>) -> HttpResponse {
    let search_term = &query.q;
    let _limit = query.limit.unwrap_or(20).min(100);

    let Some(client) = get_neo4j_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Neo4j not connected"
        }));
    };

    let graph = client.graph();
    let mut results = Vec::new();

    let search_query = neo4rs::query(
        "MATCH (e:Entity)
         WHERE toLower(e.name) CONTAINS toLower($term)
         RETURN elementId(e) AS id, e.name as name, e.entity_type as type, e.mention_count as mentions
         ORDER BY e.mention_count DESC
         LIMIT $limit",
    )
    .param("term", search_term.clone())
    .param("limit", _limit as i64);

    match graph.execute(search_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let id = row.get::<String>("id").unwrap_or_default();
                let name = row.get::<String>("name").unwrap_or_default();
                let entity_type = row.get::<String>("type").unwrap_or_default();
                let mentions = row.get::<i64>("mentions").unwrap_or(0);

                let mut properties = HashMap::new();
                properties.insert("entity_type".to_string(), entity_type);
                properties.insert("mentions".to_string(), mentions.to_string());

                results.push(GraphNode {
                    id,
                    label: name,
                    node_type: "Entity".to_string(),
                    properties,
                });
            }
        }
        Err(e) => {
            warn!(error = %e, "Entity search failed");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Search failed: {}", e)
            }));
        }
    }

    HttpResponse::Ok().json(results)
}

#[cfg(not(feature = "neo4j"))]
pub async fn search_entities(_query: web::Query<SearchQuery>) -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "Neo4j feature not enabled"
    }))
}

/// GET /graph/search/enhanced - Graph-enhanced search
#[cfg(feature = "neo4j")]
pub async fn graph_enhanced_search(
    query: web::Query<SearchQuery>,
    retriever: web::Data<std::sync::Arc<std::sync::Mutex<crate::retriever::Retriever>>>,
) -> HttpResponse {
    let search_term = &query.q;
    let _limit = query.limit.unwrap_or(10).min(50);

    let vector_results = match retriever.lock() {
        Ok(mut r) => match r.search(search_term) {
            Ok(results) => results,
            Err(e) => {
                warn!(error = %e, "Vector search failed");
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Vector search failed: {}", e)
                }));
            }
        },
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to lock retriever"
            }));
        }
    };

    let results: Vec<GraphSearchResult> = vector_results
        .into_iter()
        .enumerate()
        .map(|(i, content)| GraphSearchResult {
            chunk_id: format!("chunk_{}", i),
            content,
            score: 1.0 - (i as f32 * 0.1),
            entities: vec![],
            related_chunks: vec![],
        })
        .collect();

    HttpResponse::Ok().json(GraphSearchResponse {
        total_results: results.len(),
        results,
        graph_enhanced: false,
    })
}

#[cfg(not(feature = "neo4j"))]
pub async fn graph_enhanced_search(
    _query: web::Query<SearchQuery>,
    _retriever: web::Data<std::sync::Arc<std::sync::Mutex<crate::retriever::Retriever>>>,
) -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "Neo4j feature not enabled"
    }))
}

// ============================================================================
// Graph Rebuild - Shared Helper + HTTP Handler
// ============================================================================

/// Rebuild knowledge graph from Tantivy index.
/// Shared helper called by both POST /graph/rebuild and reindex job.
/// Uses the existing index_to_knowledge_graph pipeline (EntityExtractorTool + KnowledgeBuilder).
///
/// v1.3.0: Extracted from rebuild_knowledge_graph handler for reuse.
#[cfg(feature = "neo4j")]
pub async fn rebuild_graph_from_index() -> GraphBuildResult {
    use crate::api::{get_knowledge_builder, get_retriever_handle};

    // Check KnowledgeBuilder
    if get_knowledge_builder().is_none() {
        return GraphBuildResult {
            status: "error".to_string(),
            documents_processed: 0,
            chunks_processed: 0,
            entities_extracted: 0,
            errors: vec!["KnowledgeBuilder not initialized. Is Neo4j connected?".to_string()],
        };
    }

    // Read all doc_ids + content from Tantivy
    let Some(retriever_handle) = get_retriever_handle() else {
        return GraphBuildResult {
            status: "error".to_string(),
            documents_processed: 0,
            chunks_processed: 0,
            entities_extracted: 0,
            errors: vec!["Retriever not initialized".to_string()],
        };
    };

    let chunks_data: Vec<(String, String)> = match retriever_handle.lock() {
        Ok(ret) => {
            let reader = match ret.index.reader() {
                Ok(r) => r,
                Err(e) => {
                    return GraphBuildResult {
                        status: "error".to_string(),
                        documents_processed: 0,
                        chunks_processed: 0,
                        entities_extracted: 0,
                        errors: vec![format!("Failed to get index reader: {}", e)],
                    };
                }
            };
            let searcher = reader.searcher();
            let mut results = Vec::new();

            for segment_reader in searcher.segment_readers() {
                let store_reader = match segment_reader.get_store_reader(1) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                for doc_num in 0..segment_reader.max_doc() {
                    if segment_reader.is_deleted(doc_num) {
                        continue;
                    }
                    if let Ok(doc) = store_reader.get::<tantivy::TantivyDocument>(doc_num) {
                        let doc_id = doc
                            .get_first(ret.doc_id_field)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let content = doc
                            .get_first(ret.content_field)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !doc_id.is_empty() && !content.is_empty() {
                            results.push((doc_id, content));
                        }
                    }
                }
            }
            results
        }
        Err(_) => {
            return GraphBuildResult {
                status: "error".to_string(),
                documents_processed: 0,
                chunks_processed: 0,
                entities_extracted: 0,
                errors: vec!["Failed to lock retriever".to_string()],
            };
        }
    };
    // Retriever lock is dropped here

    if chunks_data.is_empty() {
        return GraphBuildResult {
            status: "completed".to_string(),
            documents_processed: 0,
            chunks_processed: 0,
            entities_extracted: 0,
            errors: vec![],
        };
    }

    // Group by document (filename before #)
    let mut docs_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (chunk_id, content) in &chunks_data {
        let filename = chunk_id.split('#').next().unwrap_or(chunk_id).to_string();
        docs_map
            .entry(filename)
            .or_default()
            .push((chunk_id.clone(), content.clone()));
    }

    info!(
        documents = docs_map.len(),
        total_chunks = chunks_data.len(),
        "Starting knowledge graph rebuild from index"
    );

    // Use the existing index_to_knowledge_graph pipeline
    // which handles KnowledgeBuilder + EntityExtractorTool + entity linking
    let mut docs_processed = 0usize;
    for (filename, chunks) in &docs_map {
        crate::api::index_to_knowledge_graph(filename, filename, filename, chunks).await;
        docs_processed += 1;
    }

    info!(
        docs = docs_processed,
        "Knowledge graph rebuild completed"
    );

    GraphBuildResult {
        status: "completed".to_string(),
        documents_processed: docs_processed,
        chunks_processed: chunks_data.len(),
        entities_extracted: 0, // exact count tracked inside index_to_knowledge_graph
        errors: vec![],
    }
}

#[cfg(not(feature = "neo4j"))]
pub async fn rebuild_graph_from_index() -> GraphBuildResult {
    GraphBuildResult {
        status: "error".to_string(),
        documents_processed: 0,
        chunks_processed: 0,
        entities_extracted: 0,
        errors: vec!["Neo4j feature not enabled".to_string()],
    }
}

/// POST /graph/rebuild - Rebuild knowledge graph from indexed documents
/// v1.3.0: Now delegates to rebuild_graph_from_index() helper
#[cfg(feature = "neo4j")]
pub async fn rebuild_knowledge_graph() -> HttpResponse {
    let result = rebuild_graph_from_index().await;
    HttpResponse::Ok().json(result)
}

#[cfg(not(feature = "neo4j"))]
pub async fn rebuild_knowledge_graph() -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "Neo4j feature not enabled"
    }))
}

// ============================================================================
// Helpers
// ============================================================================

/// Truncate a label string to max_len characters, appending "..." if truncated
fn truncate_label(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

// ============================================================================
// Route Configuration
// ============================================================================

pub fn configure_graph_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/graph")
            // Neo4j endpoints (require Neo4j running)
            .route("/stats", web::get().to(get_graph_stats))
            .route("/sample", web::get().to(get_graph_sample))
            .route("/search", web::get().to(search_entities))
            .route("/search/enhanced", web::get().to(graph_enhanced_search))
            .route("/rebuild", web::post().to(rebuild_knowledge_graph))
            .route("/export", web::post().to(export_graph_to_json))
            // Petgraph runtime endpoints (NO Neo4j required)
            .route("/rt/stats", web::get().to(get_petgraph_stats))
            .route("/rt/node/{id}", web::get().to(get_petgraph_node))
            .route("/rt/traverse", web::post().to(petgraph_traverse))
            .route("/rt/neighbors/{id}", web::get().to(get_petgraph_neighbors)),
    );
}