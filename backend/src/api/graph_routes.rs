//! Graph API routes for knowledge graph visualization and search
//!
//! Provides endpoints for:
//! - Graph statistics
//! - Graph data sampling for visualization
//! - Entity search
//! - Graph-enhanced search (vector + graph)

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

#[cfg(feature = "neo4j")]
use crate::api::get_neo4j_client;

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
// Handlers
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

    // Query for counts
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

    // Query for entity types
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
    HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "Neo4j feature not enabled"
    }))
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

    // Fetch entities (most interesting nodes)
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

    // Fetch some documents
    let docs_query = neo4rs::query(
        "MATCH (d:Document)
         RETURN d.id as id, d.title as title, d.chunk_count as chunks
         LIMIT 10",
    );

    match graph.execute(docs_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let id = row.get::<String>("id").unwrap_or_default();
                let title = row.get::<String>("title").unwrap_or_default();
                let chunks = row.get::<i64>("chunks").unwrap_or(0);

                let mut properties = HashMap::new();
                properties.insert("chunks".to_string(), chunks.to_string());

                data.nodes.push(GraphNode {
                    id: id.clone(),
                    label: title,
                    node_type: "Document".to_string(),
                    properties,
                });
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch documents");
        }
    }

    // Fetch some chunks (to show document-chunk-entity relationships)
    let chunks_query = neo4rs::query(
        "MATCH (c:Chunk)
         RETURN c.id as id, c.position as position
         LIMIT 20",
    );

    match graph.execute(chunks_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let id = row.get::<String>("id").unwrap_or_default();
                let position = row.get::<i64>("position").unwrap_or(0);

                let mut properties = HashMap::new();
                properties.insert("position".to_string(), position.to_string());

                // Use a shorter label for chunks
                let label = if id.len() > 15 {
                    format!("Chunk {}", position)
                } else {
                    id.clone()
                };

                data.nodes.push(GraphNode {
                    id: id.clone(),
                    label,
                    node_type: "Chunk".to_string(),
                    properties,
                });
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch chunks");
        }
    }

    // Fetch relationships - all types (MENTIONS, HAS_CHUNK, RELATED_TO, co_occurs_with)
    let rels_query = neo4rs::query(
        "MATCH (n1)-[r]->(n2)
         WHERE (n1:Entity OR n1:Document OR n1:Chunk) AND (n2:Entity OR n2:Document OR n2:Chunk)
         RETURN 
           COALESCE(n1.id, n1.name, toString(id(n1))) as from_id,
           COALESCE(n2.id, n2.name, toString(id(n2))) as to_id,
           type(r) as rel_type
         LIMIT $limit",
    )
    .param("limit", (limit * 3) as i64);

    match graph.execute(rels_query).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                let from = row.get::<String>("from_id").unwrap_or_default();
                let to = row.get::<String>("to_id").unwrap_or_default();
                let rel_type = row.get::<String>("rel_type").unwrap_or_default();

                let mut properties = HashMap::new();
                properties.insert("type".to_string(), rel_type.clone());

                data.edges.push(GraphEdge {
                    from,
                    to,
                    label: rel_type,
                    properties,
                });
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch relationships");
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
    let limit = query.limit.unwrap_or(20).min(100);

    let Some(client) = get_neo4j_client() else {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Neo4j not connected"
        }));
    };

    let graph = client.graph();
    let mut results = Vec::new();

    // Search entities by name (case-insensitive contains)
    let search_query = neo4rs::query(
        "MATCH (e:Entity)
         WHERE toLower(e.name) CONTAINS toLower($term)
         RETURN e.id as id, e.name as name, e.entity_type as type, e.mention_count as mentions
         ORDER BY e.mention_count DESC
         LIMIT $limit",
    )
    .param("term", search_term.clone())
    .param("limit", limit as i64);

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
/// Combines vector similarity with graph relationships
#[cfg(feature = "neo4j")]
pub async fn graph_enhanced_search(
    query: web::Query<SearchQuery>,
    retriever: web::Data<std::sync::Arc<std::sync::Mutex<crate::retriever::Retriever>>>,
) -> HttpResponse {
    let search_term = &query.q;
    let limit = query.limit.unwrap_or(10).min(50);

    // First, do vector search
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

    // If Neo4j is available, enhance with graph data
    let Some(client) = get_neo4j_client() else {
        // Return vector-only results
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

        return HttpResponse::Ok().json(GraphSearchResponse {
            total_results: results.len(),
            results,
            graph_enhanced: false,
        });
    };

    let graph = client.graph();
    let mut enhanced_results = Vec::new();

    for (i, content) in vector_results.into_iter().enumerate() {
        let chunk_id = format!("chunk_{}", i);
        let score = 1.0 - (i as f32 * 0.1);

        // Find entities mentioned in this chunk (by content match)
        let mut entities = Vec::new();
        let mut related_chunks = Vec::new();

        // Search for entities that might be in this content
        let entity_query = neo4rs::query(
            "MATCH (c:Chunk)-[:MENTIONS]->(e:Entity)
             WHERE c.content CONTAINS $content_sample
             RETURN DISTINCT e.name as name
             LIMIT 5",
        )
        .param(
            "content_sample",
            content.chars().take(100).collect::<String>(),
        );

        if let Ok(mut result) = graph.execute(entity_query).await {
            while let Ok(Some(row)) = result.next().await {
                if let Ok(name) = row.get::<String>("name") {
                    entities.push(name);
                }
            }
        }

        // Find related chunks through shared entities
        if !entities.is_empty() {
            let related_query = neo4rs::query(
                "MATCH (c1:Chunk)-[:MENTIONS]->(e:Entity)<-[:MENTIONS]-(c2:Chunk)
                 WHERE e.name IN $entities AND c1 <> c2
                 RETURN DISTINCT c2.id as chunk_id
                 LIMIT 3",
            )
            .param("entities", entities.clone());

            if let Ok(mut result) = graph.execute(related_query).await {
                while let Ok(Some(row)) = result.next().await {
                    if let Ok(id) = row.get::<String>("chunk_id") {
                        related_chunks.push(id);
                    }
                }
            }
        }

        enhanced_results.push(GraphSearchResult {
            chunk_id,
            content,
            score,
            entities,
            related_chunks,
        });
    }

    HttpResponse::Ok().json(GraphSearchResponse {
        total_results: enhanced_results.len(),
        results: enhanced_results,
        graph_enhanced: true,
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
// Route Configuration
// ============================================================================

pub fn configure_graph_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/graph")
            .route("/stats", web::get().to(get_graph_stats))
            .route("/sample", web::get().to(get_graph_sample))
            .route("/search", web::get().to(search_entities))
            .route("/search/enhanced", web::get().to(graph_enhanced_search)),
    );
}
