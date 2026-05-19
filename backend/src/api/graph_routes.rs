//! Graph API routes for knowledge graph visualization and search
//! v1.3.0 - Extracted rebuild_graph_from_index() helper for reuse
//!
//! Provides endpoints for:
//! - Graph statistics (FalkorDB and Petgraph)
//! - Graph data sampling for visualization
//! - Entity search
//! - Graph-enhanced search (vector + graph)
//! - Graph rebuild from indexed documents
//! - Petgraph runtime endpoints (work without FalkorDB)
//!
//! INSTALLER IMPACT (v1.3.0): None - same routes, same response types

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tantivy::schema::Value;
use tracing::{debug, info, warn};

#[cfg(feature = "graph")]
use crate::api::get_graph_client;
#[cfg(feature = "graph")]
use crate::graph::client::{falkor_value_to_json, lit, row_f64, row_i64, row_str, row_str_vec};

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
// Petgraph Runtime Endpoints (Option C: No FalkorDB required at runtime)
// ============================================================================

/// GET /graph/rt/stats - Get petgraph runtime stats (no FalkorDB needed)
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
// Export for Petgraph (Option C: FalkorDB design-time → Petgraph runtime)
// ============================================================================

/// Helper: export FalkorDB graph to JSON and reload petgraph runtime.
/// Called automatically after async reindex completes.
pub async fn export_and_reload_graph() {
    // Trigger the existing export endpoint logic by calling it as a function
    // We simulate by calling the HTTP handler and discarding the response
    let response = export_graph_to_json().await;
    if response.status().is_success() {
        tracing::info!("Auto-export after reindex: success");
    } else {
        tracing::warn!("Auto-export after reindex: failed");
    }
}

/// POST /graph/reconnect - Reconnect to FalkorDB (useful after container restart)
pub async fn reconnect_graph() -> HttpResponse {
    let config = crate::graph::config::GraphConfig::from_env();
    if !config.enabled {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "FalkorDB not enabled"
        }));
    }
    match crate::graph::GraphClient::new(config.clone()).await {
        Ok(client) => {
            if let Err(e) = client.init_schema().await {
                tracing::warn!(error = %e, "Schema init failed during reconnect");
            }
            let kb_config = crate::graph::config::GraphConfig::from_env();
            let knowledge_builder = crate::graph::KnowledgeBuilder::new(client.graph(), kb_config);
            crate::api::set_knowledge_builder(std::sync::Arc::new(knowledge_builder));
            crate::api::set_graph_client(client);
            tracing::info!("FalkorDB reconnected successfully");
            HttpResponse::Ok().json(serde_json::json!({"status": "connected"}))
        }
        Err(e) => {
            tracing::warn!(error = %e, "FalkorDB reconnect failed");
            HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": format!("Reconnect failed: {}", e)
            }))
        }
    }
}

/// POST /graph/export - Export FalkorDB graph to JSON for petgraph runtime
#[cfg(feature = "graph")]
pub async fn export_graph_to_json() -> HttpResponse {
    let Some(client) = get_graph_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "FalkorDB not connected. Start FalkorDB to export."
        }));
    };

    let handle = client.graph();

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
    match handle
        .query(
            "MATCH (n:Chunk)
             OPTIONAL MATCH (n)-[:MENTIONS]->(e:Entity)
             RETURN n.id AS id, n.content AS content,
                    coalesce(collect(e.normalized_name), []) AS entities,
                    coalesce(n.source, 'unknown') AS source",
            &HashMap::new(),
        )
        .await
    {
        Ok(rows) => {
            for row in rows {
                export.nodes.push(ChunkNode {
                    id: row_str(&row, 0),
                    content: row_str(&row, 1),
                    entities: row_str_vec(&row, 2),
                    source: row_str(&row, 3),
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

    // Fetch chunk-to-chunk relationships via shared entity mentions
    match handle
        .query(
            "MATCH (n:Chunk)-[:MENTIONS]->(e:Entity)<-[:MENTIONS]-(m:Chunk)
             WHERE n.id <> m.id
             RETURN n.id AS from_id, m.id AS to_id,
                    'RELATED_VIA_ENTITY' AS rel_type,
                    0.8 AS confidence,
                    e.normalized_name AS entity_name",
            &HashMap::new(),
        )
        .await
    {
        Ok(rows) => {
            for row in rows {
                let entity_name = row_str(&row, 4);
                export.relationships.push(ExportRelationship {
                    from_id: row_str(&row, 0),
                    to_id: row_str(&row, 1),
                    rel_type: row_str(&row, 2),
                    confidence: row_f64(&row, 3, 0.8) as f32,
                    meta: serde_json::json!({ "entity": entity_name }),
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
            // Reload petgraph runtime with fresh data
            let json_path_clone = json_path.clone();
            tokio::spawn(async move {
                crate::graph::petgraph_runtime::reload_from_json_path(&json_path_clone).await;
            });

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

#[cfg(not(feature = "graph"))]
pub async fn export_graph_to_json() -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "FalkorDB feature not enabled. Build with --features graph to export."
    }))
}

// ============================================================================
// FalkorDB Handlers
// ============================================================================

/// GET /graph/stats - Get graph statistics
#[cfg(feature = "graph")]
pub async fn get_graph_stats() -> HttpResponse {
    let Some(client) = get_graph_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "FalkorDB not connected"
        }));
    };

    let handle = client.graph();
    let mut stats = GraphStats::default();

    match handle
        .query(
            "MATCH (d:Document) WITH count(d) AS docs
             MATCH (c:Chunk) WITH docs, count(c) AS chunks
             MATCH (e:Entity) WITH docs, chunks, count(e) AS entities
             MATCH ()-[r]->() WITH docs, chunks, entities, count(r) AS rels
             RETURN docs, chunks, entities, rels",
            &HashMap::new(),
        )
        .await
    {
        Ok(rows) => {
            if let Some(row) = rows.first() {
                stats.document_count = row_i64(row, 0) as usize;
                stats.chunk_count = row_i64(row, 1) as usize;
                stats.entity_count = row_i64(row, 2) as usize;
                stats.relationship_count = row_i64(row, 3) as usize;
            }
        }
        Err(e) => warn!(error = %e, "Failed to fetch graph stats"),
    }

    match handle
        .query(
            "MATCH (e:Entity)
             RETURN e.entity_type AS type, count(*) AS count
             ORDER BY count DESC",
            &HashMap::new(),
        )
        .await
    {
        Ok(rows) => {
            for row in rows {
                let entity_type = row_str(&row, 0);
                if !entity_type.is_empty() {
                    stats.entity_types.push(EntityTypeCount {
                        entity_type,
                        count: row_i64(&row, 1) as usize,
                    });
                }
            }
        }
        Err(e) => warn!(error = %e, "Failed to fetch entity types"),
    }

    HttpResponse::Ok().json(stats)
}

#[cfg(not(feature = "graph"))]
pub async fn get_graph_stats() -> HttpResponse {
    // Fallback to petgraph stats when FalkorDB not available
    get_petgraph_stats().await
}

/// Request body for `POST /graph/query`.
#[derive(Debug, Deserialize)]
pub struct GraphQueryRequest {
    /// The Cypher query to run (read-only).
    pub cypher: String,
}

/// POST /graph/query - Run an ad-hoc read-only Cypher query against FalkorDB.
///
/// Uses `GRAPH.RO_QUERY`, so any write (CREATE/MERGE/DELETE/SET) is rejected by
/// FalkorDB server-side. Intended for inspection/debugging on a localhost box.
#[cfg(feature = "graph")]
pub async fn query_graph(body: web::Json<GraphQueryRequest>) -> HttpResponse {
    let cypher = body.cypher.trim();
    if cypher.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "cypher is required"
        }));
    }

    let Some(client) = get_graph_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "FalkorDB not connected"
        }));
    };

    match client.execute_query_ro(cypher).await {
        Ok(rows) => {
            let json_rows: Vec<Vec<serde_json::Value>> = rows
                .iter()
                .map(|row| row.iter().map(falkor_value_to_json).collect())
                .collect();
            HttpResponse::Ok().json(serde_json::json!({
                "row_count": json_rows.len(),
                "rows": json_rows,
            }))
        }
        Err(e) => {
            warn!(error = %e, "Ad-hoc Cypher query failed");
            HttpResponse::BadRequest().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

#[cfg(not(feature = "graph"))]
pub async fn query_graph(_body: web::Json<GraphQueryRequest>) -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "graph feature not enabled"
    }))
}

/// GET /graph/sample - Get sample graph data for visualization
/// v1.1.0: Fixed - fetches all node types + edges + uses elementId() for reliable IDs
#[cfg(feature = "graph")]
pub async fn get_graph_sample(query: web::Query<SampleQuery>) -> HttpResponse {
    let limit = query.limit.unwrap_or(50).min(200);

    let Some(client) = get_graph_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "FalkorDB not connected"
        }));
    };

    let handle = client.graph();
    let mut data = GraphData::default();
    // FalkorDB has no `elementId()`; `ID(n)` returns an integer node handle.
    let mut all_node_ids: Vec<i64> = Vec::new();

    // Proportional limits per node type
    let doc_limit = ((limit as f64) * 0.2).ceil() as i64;
    let chunk_limit = ((limit as f64) * 0.3).ceil() as i64;
    let entity_limit = ((limit as f64) * 0.5).ceil() as i64;

    // ── 1. Fetch Document nodes ──────────────────────────────────
    match handle
        .query(
            "MATCH (d:Document)
             RETURN ID(d) AS id,
                    coalesce(d.title, d.name, 'Untitled') AS label
             LIMIT $limit",
            &crate::params! { "limit" => lit::int(doc_limit) },
        )
        .await
    {
        Ok(rows) => {
            for row in rows {
                let id = row_i64(&row, 0);
                all_node_ids.push(id);
                data.nodes.push(GraphNode {
                    id: id.to_string(),
                    label: truncate_label(&row_str(&row, 1), 30),
                    node_type: "Document".to_string(),
                    properties: HashMap::new(),
                });
            }
        }
        Err(e) => warn!(error = %e, "Failed to fetch Document nodes"),
    }

    // ── 2. Fetch Chunk nodes ─────────────────────────────────────
    match handle
        .query(
            "MATCH (c:Chunk)
             RETURN ID(c) AS id,
                    coalesce(c.chunk_id, left(c.content, 40), 'chunk') AS label
             LIMIT $limit",
            &crate::params! { "limit" => lit::int(chunk_limit) },
        )
        .await
    {
        Ok(rows) => {
            for row in rows {
                let id = row_i64(&row, 0);
                all_node_ids.push(id);
                data.nodes.push(GraphNode {
                    id: id.to_string(),
                    label: truncate_label(&row_str(&row, 1), 40),
                    node_type: "Chunk".to_string(),
                    properties: HashMap::new(),
                });
            }
        }
        Err(e) => warn!(error = %e, "Failed to fetch Chunk nodes"),
    }

    // ── 3. Fetch Entity nodes ────────────────────────────────────
    match handle
        .query(
            "MATCH (e:Entity)
             RETURN ID(e) AS id,
                    coalesce(e.name, e.label, 'entity') AS name,
                    coalesce(e.entity_type, 'Unknown') AS type,
                    coalesce(e.mention_count, 0) AS mentions
             ORDER BY e.mention_count DESC
             LIMIT $limit",
            &crate::params! { "limit" => lit::int(entity_limit) },
        )
        .await
    {
        Ok(rows) => {
            for row in rows {
                let id = row_i64(&row, 0);
                let mut properties = HashMap::new();
                properties.insert("entity_type".to_string(), row_str(&row, 2));
                properties.insert("mentions".to_string(), row_i64(&row, 3).to_string());
                all_node_ids.push(id);
                data.nodes.push(GraphNode {
                    id: id.to_string(),
                    label: row_str(&row, 1),
                    node_type: "Entity".to_string(),
                    properties,
                });
            }
        }
        Err(e) => warn!(error = %e, "Failed to fetch Entity nodes"),
    }

    // ── 4. Fetch edges between sampled nodes ─────────────────────
    if !all_node_ids.is_empty() {
        match handle
            .query(
                "MATCH (a)-[r]->(b)
                 WHERE ID(a) IN $ids AND ID(b) IN $ids
                 RETURN ID(a) AS from_id, ID(b) AS to_id, type(r) AS rel_type
                 LIMIT 500",
                &crate::params! { "ids" => lit::int_list(&all_node_ids) },
            )
            .await
        {
            Ok(rows) => {
                for row in rows {
                    data.edges.push(GraphEdge {
                        from: row_i64(&row, 0).to_string(),
                        to: row_i64(&row, 1).to_string(),
                        label: row_str(&row, 2),
                        properties: HashMap::new(),
                    });
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

#[cfg(not(feature = "graph"))]
pub async fn get_graph_sample(_query: web::Query<SampleQuery>) -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "FalkorDB feature not enabled"
    }))
}

/// GET /graph/search - Search for entities
/// v1.1.0: Fixed - uses elementId() for reliable IDs
#[cfg(feature = "graph")]
pub async fn search_entities(query: web::Query<SearchQuery>) -> HttpResponse {
    let search_term = &query.q;
    let _limit = query.limit.unwrap_or(20).min(100);

    let Some(client) = get_graph_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "FalkorDB not connected"
        }));
    };

    let handle = client.graph();
    let mut results = Vec::new();

    match handle
        .query(
            "MATCH (e:Entity)
             WHERE toLower(e.name) CONTAINS toLower($term)
             RETURN ID(e) AS id, e.name AS name, e.entity_type AS type,
                    e.mention_count AS mentions
             ORDER BY e.mention_count DESC
             LIMIT $limit",
            &crate::params! {
                "term" => lit::str(search_term),
                "limit" => lit::int(_limit as i64),
            },
        )
        .await
    {
        Ok(rows) => {
            for row in rows {
                let mut properties = HashMap::new();
                properties.insert("entity_type".to_string(), row_str(&row, 2));
                properties.insert("mentions".to_string(), row_i64(&row, 3).to_string());
                results.push(GraphNode {
                    id: row_i64(&row, 0).to_string(),
                    label: row_str(&row, 1),
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

#[cfg(not(feature = "graph"))]
pub async fn search_entities(_query: web::Query<SearchQuery>) -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "FalkorDB feature not enabled"
    }))
}

/// GET /graph/search/enhanced - Graph-enhanced search with RRF fusion
/// A1-v2: Uses global RETRIEVER static (no app_data needed)
#[cfg(feature = "graph")]
pub async fn graph_enhanced_search(query: web::Query<SearchQuery>) -> HttpResponse {
    let search_term = &query.q;
    let limit = query.limit.unwrap_or(10).min(50);
    let rrf_k: f32 = 60.0;

    // ── 1. BM25 search → (content, score) ──
    let bm25_results: Vec<(String, f32)> = match crate::api::get_retriever_handle() {
        Some(handle) => match handle.lock() {
            Ok(mut r) => match r.search(search_term) {
                Ok(results) => results
                    .into_iter()
                    .enumerate()
                    .map(|(rank, content)| {
                        let score = 1.0 / (rrf_k + rank as f32 + 1.0);
                        (content, score)
                    })
                    .collect(),
                Err(e) => {
                    warn!(error = %e, "BM25 search failed in enhanced search");
                    vec![]
                }
            },
            Err(_) => {
                warn!("Failed to lock retriever in enhanced search");
                vec![]
            }
        },
        None => {
            warn!("Retriever not initialized");
            vec![]
        }
    };

    // ── 2. Graph search → entity→chunk content resolution ──
    let graph_results: Vec<(String, f32, Vec<String>)> = match get_graph_client() {
        Some(client) => {
            let handle = client.graph();
            // Find entities matching query, traverse to source chunks, return chunk content
            let params = crate::params! {
                "term" => lit::str(search_term),
                "limit" => lit::int((limit * 2) as i64),
            };
            match handle
                .query(
                    "MATCH (c:Chunk)-[:MENTIONS]->(e:Entity)
                     WHERE toLower(e.name) CONTAINS toLower($term)
                     RETURN c.content AS content, c.id AS chunk_id,
                            collect(DISTINCT e.name) AS entities,
                            sum(e.mention_count) AS relevance
                     ORDER BY relevance DESC
                     LIMIT $limit",
                    &params,
                )
                .await
            {
                Ok(rows) => {
                    let mut items = Vec::new();
                    let mut rank = 0usize;
                    for row in rows {
                        let content = row_str(&row, 0);
                        if !content.is_empty() {
                            let score = 1.0 / (rrf_k + rank as f32 + 1.0);
                            items.push((content, score, row_str_vec(&row, 2)));
                            rank += 1;
                        }
                    }
                    items
                }
                Err(e) => {
                    warn!(error = %e, "Graph search failed, continuing with BM25 only");
                    vec![]
                }
            }
        }
        None => {
            info!("FalkorDB not connected, using BM25 only");
            vec![]
        }
    };

    // ── 3. RRF fusion by content string ──
    let mut fusion_map: std::collections::HashMap<String, GraphSearchResult> =
        std::collections::HashMap::new();

    // Add BM25 results
    for (content, score) in &bm25_results {
        fusion_map
            .entry(content.clone())
            .and_modify(|r| r.score += score)
            .or_insert(GraphSearchResult {
                chunk_id: String::new(),
                content: content.clone(),
                score: *score,
                entities: vec![],
                related_chunks: vec![],
            });
    }

    // Add graph results (entities included)
    for (content, score, entities) in &graph_results {
        fusion_map
            .entry(content.clone())
            .and_modify(|r| {
                r.score += score;
                // Merge entities, dedup
                for e in entities {
                    if !r.entities.contains(e) {
                        r.entities.push(e.clone());
                    }
                }
            })
            .or_insert(GraphSearchResult {
                chunk_id: String::new(),
                content: content.clone(),
                score: *score,
                entities: entities.clone(),
                related_chunks: vec![],
            });
    }

    // ── 4. Sort by fused score, truncate ──
    let mut results: Vec<GraphSearchResult> = fusion_map.into_values().collect();
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);

    let graph_enhanced = !graph_results.is_empty();

    HttpResponse::Ok().json(GraphSearchResponse {
        total_results: results.len(),
        results,
        graph_enhanced,
    })
}

#[cfg(not(feature = "graph"))]
pub async fn graph_enhanced_search(_query: web::Query<SearchQuery>) -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "FalkorDB feature not enabled"
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
#[cfg(feature = "graph")]
pub async fn rebuild_graph_from_index() -> GraphBuildResult {
    use crate::api::{get_knowledge_builder, get_retriever_handle};

    // Check KnowledgeBuilder
    if get_knowledge_builder().is_none() {
        return GraphBuildResult {
            status: "error".to_string(),
            documents_processed: 0,
            chunks_processed: 0,
            entities_extracted: 0,
            errors: vec!["KnowledgeBuilder not initialized. Is FalkorDB connected?".to_string()],
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
    let throttle_ms: u64 = std::env::var("GRAPH_REBUILD_THROTTLE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    for (filename, chunks) in &docs_map {
        #[cfg(feature = "graph")]
        if let Some(kb) = crate::api::get_knowledge_builder() {
            crate::graph::index_to_knowledge_graph(&kb, filename, filename, filename, chunks).await;
        }
        docs_processed += 1;
        // B1-v1: Yield to prevent CPU starvation + configurable throttle
        tokio::task::yield_now().await;
        if throttle_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(throttle_ms)).await;
        }
        if docs_processed.is_multiple_of(5) {
            info!(
                progress = docs_processed,
                total = docs_map.len(),
                "Graph rebuild progress"
            );
        }
    }

    info!(docs = docs_processed, "Knowledge graph rebuild completed");

    GraphBuildResult {
        status: "completed".to_string(),
        documents_processed: docs_processed,
        chunks_processed: chunks_data.len(),
        entities_extracted: 0, // exact count tracked inside index_to_knowledge_graph
        errors: vec![],
    }
}

#[cfg(not(feature = "graph"))]
pub async fn rebuild_graph_from_index() -> GraphBuildResult {
    GraphBuildResult {
        status: "error".to_string(),
        documents_processed: 0,
        chunks_processed: 0,
        entities_extracted: 0,
        errors: vec!["FalkorDB feature not enabled".to_string()],
    }
}

/// POST /graph/rebuild - Rebuild knowledge graph from indexed documents
/// v1.3.0: Now delegates to rebuild_graph_from_index() helper
#[cfg(feature = "graph")]
pub async fn rebuild_knowledge_graph() -> HttpResponse {
    let result = rebuild_graph_from_index().await;
    HttpResponse::Ok().json(result)
}

#[cfg(not(feature = "graph"))]
pub async fn rebuild_knowledge_graph() -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "FalkorDB feature not enabled"
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
            // FalkorDB endpoints (require FalkorDB running)
            .route("/stats", web::get().to(get_graph_stats))
            .route("/sample", web::get().to(get_graph_sample))
            .route("/search", web::get().to(search_entities))
            .route("/search/enhanced", web::get().to(graph_enhanced_search))
            .route("/rebuild", web::post().to(rebuild_knowledge_graph))
            .route("/reconnect", web::post().to(reconnect_graph))
            .route("/export", web::post().to(export_graph_to_json))
            .route("/query", web::post().to(query_graph))
            // Petgraph runtime endpoints (NO FalkorDB required)
            .route("/rt/stats", web::get().to(get_petgraph_stats))
            .route("/rt/node/{id}", web::get().to(get_petgraph_node))
            .route("/rt/traverse", web::post().to(petgraph_traverse))
            .route("/rt/neighbors/{id}", web::get().to(get_petgraph_neighbors)),
    );
}
