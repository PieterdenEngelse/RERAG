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

/// Helper: export Neo4j graph to JSON and reload petgraph runtime.
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

/// POST /graph/reconnect - Reconnect to Neo4j (useful after container restart)
pub async fn reconnect_neo4j() -> HttpResponse {
    let config = crate::graph::config::GraphConfig::from_env();
    if !config.enabled {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Neo4j not enabled"
        }));
    }
    match crate::graph::Neo4jClient::new(config.clone()).await {
        Ok(client) => {
            if let Err(e) = client.init_schema().await {
                tracing::warn!(error = %e, "Schema init failed during reconnect");
            }
            let kb_config = crate::graph::config::GraphConfig::from_env();
            let knowledge_builder = crate::graph::KnowledgeBuilder::new(client.graph(), kb_config);
            crate::api::set_knowledge_builder(std::sync::Arc::new(knowledge_builder));
            crate::api::set_neo4j_client(client);
            tracing::info!("Neo4j reconnected successfully");
            HttpResponse::Ok().json(serde_json::json!({"status": "connected"}))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Neo4j reconnect failed");
            HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": format!("Reconnect failed: {}", e)
            }))
        }
    }
}

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
         OPTIONAL MATCH (n)-[:MENTIONS]->(e:Entity)
         RETURN n.id AS id, n.content AS content,
                coalesce(collect(e.normalized_name), []) AS entities,
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

    // Fetch chunk-to-chunk relationships via shared entity mentions
    let rels_query = neo4rs::query(
        "MATCH (n:Chunk)-[:MENTIONS]->(e:Entity)<-[:MENTIONS]-(m:Chunk)
         WHERE n.id <> m.id
         RETURN n.id AS from_id, m.id AS to_id,
                'RELATED_VIA_ENTITY' AS rel_type,
                0.8 AS confidence,
                e.normalized_name AS entity_name",
    );

    match graph.execute(rels_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let from_id: String = row.get("from_id").unwrap_or_default();
                let to_id: String = row.get("to_id").unwrap_or_default();
                let rel_type: String = row.get("rel_type").unwrap_or_default();
                let confidence: f32 = row.get("confidence").unwrap_or(0.8);
                let entity_name: String = row.get("entity_name").unwrap_or_default();
                let meta = serde_json::json!({"entity": entity_name});

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

/// GET /graph/search/enhanced - Graph-enhanced search with RRF fusion
/// A1-v2: Uses global RETRIEVER static (no app_data needed)
#[cfg(feature = "neo4j")]
pub async fn graph_enhanced_search(query: web::Query<SearchQuery>) -> HttpResponse {
    let search_term = &query.q;
    let limit = query.limit.unwrap_or(10).min(50) as usize;
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
    let graph_results: Vec<(String, f32, Vec<String>)> = match get_neo4j_client() {
        Some(client) => {
            let graph = client.graph();
            // Find entities matching query, traverse to source chunks, return chunk content
            let cypher = neo4rs::query(
                "MATCH (c:Chunk)-[:MENTIONS]->(e:Entity)
                 WHERE toLower(e.name) CONTAINS toLower($term)
                 RETURN c.content AS content, c.id AS chunk_id,
                        collect(DISTINCT e.name) AS entities,
                        sum(e.mention_count) AS relevance
                 ORDER BY relevance DESC
                 LIMIT $limit",
            )
            .param("term", search_term.clone())
            .param("limit", (limit * 2) as i64);

            match graph.execute(cypher).await {
                Ok(mut result) => {
                    let mut items = Vec::new();
                    let mut rank = 0usize;
                    while let Ok(Some(row)) = result.next().await {
                        let content = row.get::<String>("content").unwrap_or_default();
                        let entities: Vec<String> =
                            row.get::<Vec<String>>("entities").unwrap_or_default();
                        if !content.is_empty() {
                            let score = 1.0 / (rrf_k + rank as f32 + 1.0);
                            items.push((content, score, entities));
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
            info!("Neo4j not connected, using BM25 only");
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

#[cfg(not(feature = "neo4j"))]
pub async fn graph_enhanced_search(_query: web::Query<SearchQuery>) -> HttpResponse {
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
    let throttle_ms: u64 = std::env::var("GRAPH_REBUILD_THROTTLE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    for (filename, chunks) in &docs_map {
        #[cfg(feature = "neo4j")]
        if let Some(kb) = crate::api::get_knowledge_builder() {
            crate::graph::index_to_knowledge_graph(&kb, filename, filename, filename, chunks).await;
        }
        docs_processed += 1;
        // B1-v1: Yield to prevent CPU starvation + configurable throttle
        tokio::task::yield_now().await;
        if throttle_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(throttle_ms)).await;
        }
        if docs_processed % 5 == 0 {
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
            .route("/reconnect", web::post().to(reconnect_neo4j))
            .route("/export", web::post().to(export_graph_to_json))
            // Petgraph runtime endpoints (NO Neo4j required)
            .route("/rt/stats", web::get().to(get_petgraph_stats))
            .route("/rt/node/{id}", web::get().to(get_petgraph_node))
            .route("/rt/traverse", web::post().to(petgraph_traverse))
            .route("/rt/neighbors/{id}", web::get().to(get_petgraph_neighbors)),
    );
}
