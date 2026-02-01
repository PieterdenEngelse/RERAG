//! Graph API routes for knowledge graph visualization and search
//!
//! Provides endpoints for:
//! - Graph statistics (Neo4j and Petgraph)
//! - Graph data sampling for visualization
//! - Entity search
//! - Graph-enhanced search (vector + graph)
//! - Petgraph runtime endpoints (work without Neo4j)

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

#[cfg(feature = "neo4j")]
use crate::api::get_neo4j_client;

// Import petgraph runtime (always available)
use crate::graph::petgraph_runtime::{get_runtime_graph, GraphQuery, ChunkNode, Relationship};

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
                coalesce(n.source, 'unknown') AS source"
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
                coalesce(r.metadata, {}) AS meta"
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
// Neo4j Handlers (existing - kept for backward compatibility)
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

    let entities_query = neo4rs::query(
        "MATCH (e:Entity)
         RETURN e.id as id, e.name as name, e.entity_type as type, e.mention_count as mentions
         ORDER BY e.mention_count DESC
         LIMIT $limit",
    )
    .param("limit", limit as i64);

    match graph.execute(entities_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let id = row.get::<String>("id").unwrap_or_default();
                let name = row.get::<String>("name").unwrap_or_default();
                let entity_type = row.get::<String>("type").unwrap_or_default();
                let mentions = row.get::<i64>("mentions").unwrap_or(0);

                let mut properties = HashMap::new();
                properties.insert("entity_type".to_string(), entity_type.clone());
                properties.insert("mentions".to_string(), mentions.to_string());

                data.nodes.push(GraphNode {
                    id: id.clone(),
                    label: name,
                    node_type: "Entity".to_string(),
                    properties,
                });
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch entities");
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
         RETURN e.id as id, e.name as name, e.entity_type as type, e.mention_count as mentions
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

/// POST /graph/rebuild - Rebuild knowledge graph
#[cfg(feature = "neo4j")]
pub async fn rebuild_knowledge_graph() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "not_implemented",
        "message": "Use /graph/export to export Neo4j to petgraph format"
    }))
}

#[cfg(not(feature = "neo4j"))]
pub async fn rebuild_knowledge_graph() -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "Neo4j feature not enabled"
    }))
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
