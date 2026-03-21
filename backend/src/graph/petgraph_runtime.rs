// src/graph/petgraph_runtime.rs
// Version: 2.0.1 - Standalone mode with optional Neo4j

use petgraph::graph::{Graph, NodeIndex};
use petgraph::Directed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::OnceCell;
use tracing::{debug, info, warn};

#[cfg(feature = "neo4j")]
use super::client::Neo4jClient;

// ─────────────────────────────────────────────────────────────
// Data Structures
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkNode {
    pub id: String,
    pub content: String,
    pub entities: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    #[serde(rename = "type")]
    pub r#type: String,
    pub confidence: f32,
    #[serde(default)]
    pub meta: serde_json::Value,
}

pub type KnowledgeGraph = Graph<ChunkNode, Relationship, Directed>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeGraph {
    pub graph: KnowledgeGraph,
    #[serde(skip)]
    pub node_index_by_id: HashMap<String, NodeIndex>,
}

impl RuntimeGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            node_index_by_id: HashMap::new(),
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }

    /// Rebuild the node index (required after deserialization)
    pub fn rebuild_index(&mut self) {
        self.node_index_by_id.clear();
        for idx in self.graph.node_indices() {
            if let Some(node) = self.graph.node_weight(idx) {
                self.node_index_by_id.insert(node.id.clone(), idx);
            }
        }
    }
}

impl Default for RuntimeGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────
// Global Runtime Graph (Thread-Safe Singleton)
// ─────────────────────────────────────────────────────────────

pub static RUNTIME_GRAPH: std::sync::RwLock<Option<Arc<RuntimeGraph>>> = std::sync::RwLock::new(None);

pub fn get_runtime_graph() -> Arc<RuntimeGraph> {
    RUNTIME_GRAPH
        .read()
        .expect("Runtime graph lock poisoned")
        .clone()
        .unwrap_or_else(|| Arc::new(RuntimeGraph::new()))
}

pub fn set_runtime_graph(graph: Arc<RuntimeGraph>) {
    let mut lock = RUNTIME_GRAPH.write().expect("Runtime graph lock poisoned");
    *lock = Some(graph);
}

pub async fn reload_from_json_path(path: &str) {
    let dir = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let bin_cache = dir.join("runtime_graph.bin");
    if bin_cache.exists() {
        let _ = std::fs::remove_file(&bin_cache);
    }
    let compiler = GraphCompiler::new_standalone_from_path(path);
    let runtime_graph = Arc::new(compiler.compile().await);
    set_runtime_graph(runtime_graph);
    tracing::info!(path = %path, "Petgraph runtime reloaded from JSON");
}

// ─────────────────────────────────────────────────────────────
// Graph Compiler
// ─────────────────────────────────────────────────────────────

pub struct GraphCompiler {
    #[cfg(feature = "neo4j")]
    neo4j_client: Option<Neo4jClient>,
    #[allow(dead_code)]
    neo4j_enabled: bool,
    data_dir: String,
}

impl GraphCompiler {
    /// Create compiler WITHOUT Neo4j (file-only mode)
    pub fn new_standalone_from_path(json_path: &str) -> Self {
        let dir = std::path::Path::new(json_path)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_string_lossy()
            .to_string();
        Self {
            #[cfg(feature = "neo4j")]
            neo4j_client: None,
            neo4j_enabled: false,
            data_dir: dir,
        }
    }

    pub fn new_standalone(data_dir: &str) -> Self {
        Self {
            #[cfg(feature = "neo4j")]
            neo4j_client: None,
            neo4j_enabled: false,
            data_dir: data_dir.into(),
        }
    }

    /// Create compiler WITH Neo4j client
    #[cfg(feature = "neo4j")]
    pub fn new_with_neo4j(neo4j_client: Neo4jClient, data_dir: &str) -> Self {
        Self {
            neo4j_client: Some(neo4j_client),
            neo4j_enabled: true,
            data_dir: data_dir.into(),
        }
    }

    /// Legacy constructor for backward compatibility
    #[cfg(feature = "neo4j")]
    pub fn new(neo4j_client: Neo4jClient, data_dir: &str) -> Self {
        Self::new_with_neo4j(neo4j_client, data_dir)
    }

    pub async fn compile(&self) -> RuntimeGraph {
        let disk_path = format!("{}/runtime_graph.bin", self.data_dir);
        let json_path = format!("{}/knowledge_graph.json", self.data_dir);
        let start = Instant::now();

        // 1. Try binary cache first (fastest)
        if Path::new(&disk_path).exists() {
            match Self::load_from_disk(&disk_path) {
                Ok(graph) => {
                    info!(
                        "ParallelGroup: Loaded {} nodes, {} edges from binary cache in {:?}",
                        graph.node_count(),
                        graph.edge_count(),
                        start.elapsed()
                    );
                    return graph;
                }
                Err(e) => {
                    debug!("ParallelGroup: Binary cache invalid, will rebuild: {}", e);
                }
            }
        }

        // 2. Try JSON file (human-readable, no Neo4j needed)
        if Path::new(&json_path).exists() {
            match Self::load_from_json(&json_path) {
                Ok(graph) => {
                    info!(
                        "ParallelGroup: Loaded {} nodes, {} edges from JSON in {:?}",
                        graph.node_count(),
                        graph.edge_count(),
                        start.elapsed()
                    );
                    // Warm binary cache for next startup
                    if let Err(e) = Self::save_to_disk(&graph, &disk_path) {
                        debug!("ParallelGroup: Failed to cache binary: {}", e);
                    }
                    return graph;
                }
                Err(e) => {
                    warn!("ParallelGroup: JSON load failed: {}", e);
                }
            }
        }

        // 3. Try Neo4j if enabled
        #[cfg(feature = "neo4j")]
        if self.neo4j_enabled {
            if let Some(ref client) = self.neo4j_client {
                info!("ParallelGroup: Compiling from Neo4j (cold start)...");
                match self.compile_from_neo4j(client).await {
                    Ok(graph) => {
                        if let Err(e) = Self::save_to_disk(&graph, &disk_path) {
                            warn!("ParallelGroup: Failed to save binary cache: {}", e);
                        }
                        info!(
                            "ParallelGroup: Compiled {} nodes, {} edges from Neo4j in {:?}",
                            graph.node_count(),
                            graph.edge_count(),
                            start.elapsed()
                        );
                        return graph;
                    }
                    Err(e) => {
                        warn!("ParallelGroup: Neo4j compilation failed: {}", e);
                    }
                }
            }
        }

        // 4. Return empty graph
        info!("ParallelGroup: No graph data found, starting with empty graph");
        RuntimeGraph::new()
    }

    #[cfg(feature = "neo4j")]
    async fn compile_from_neo4j(
        &self,
        client: &Neo4jClient,
    ) -> Result<RuntimeGraph, Box<dyn std::error::Error>> {
        let query = neo4rs::Query::new(
            r#"
            MATCH (n:Chunk)-[r]->(m:Chunk)
            RETURN 
                n.id AS from_id, n.content AS from_content,
                coalesce(n.entities, []) AS from_entities,
                coalesce(n.source, 'unknown') AS from_source,
                type(r) AS rel_type,
                coalesce(r.confidence, 0.8) AS confidence,
                coalesce(r.metadata, {}) AS metadata,
                m.id AS to_id, m.content AS to_content,
                coalesce(m.entities, []) AS to_entities,
                coalesce(m.source, 'unknown') AS to_source
            "#
            .to_string(),
        );

        let graph_client = client.graph();
        let mut stream = graph_client.execute(query).await?;

        let mut runtime = RuntimeGraph::new();
        let mut node_cache: HashMap<String, NodeIndex> = HashMap::new();

        while let Some(row) = stream.next().await? {
            let from_id: String = row.get("from_id")?;
            let from_content: String = row.get("from_content")?;
            let from_entities: Vec<String> = row.get("from_entities").unwrap_or_default();
            let from_source: String = row.get("from_source").unwrap_or_default();
            let to_id: String = row.get("to_id")?;
            let to_content: String = row.get("to_content")?;
            let to_entities: Vec<String> = row.get("to_entities").unwrap_or_default();
            let to_source: String = row.get("to_source").unwrap_or_default();
            let rel_type: String = row.get("rel_type")?;
            let confidence: f32 = row.get("confidence").unwrap_or(0.8);
            let meta: serde_json::Value = row.get("metadata").unwrap_or(serde_json::json!({}));

            let from_idx = *node_cache.entry(from_id.clone()).or_insert_with(|| {
                let idx = runtime.graph.add_node(ChunkNode {
                    id: from_id.clone(),
                    content: from_content.clone(),
                    entities: from_entities.clone(),
                    source: from_source.clone(),
                });
                runtime.node_index_by_id.insert(from_id.clone(), idx);
                idx
            });

            let to_idx = *node_cache.entry(to_id.clone()).or_insert_with(|| {
                let idx = runtime.graph.add_node(ChunkNode {
                    id: to_id.clone(),
                    content: to_content.clone(),
                    entities: to_entities.clone(),
                    source: to_source.clone(),
                });
                runtime.node_index_by_id.insert(to_id.clone(), idx);
                idx
            });

            runtime.graph.add_edge(
                from_idx,
                to_idx,
                Relationship {
                    r#type: rel_type,
                    confidence,
                    meta,
                },
            );
        }

        Ok(runtime)
    }

    pub(crate) fn save_to_disk(
        graph: &RuntimeGraph,
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = bincode::serialize(graph)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub(crate) fn load_from_disk(path: &str) -> Result<RuntimeGraph, Box<dyn std::error::Error>> {
        let data = std::fs::read(path)?;
        let mut graph: RuntimeGraph = bincode::deserialize(&data)?;
        graph.rebuild_index();
        Ok(graph)
    }

    pub(crate) fn load_from_json(path: &str) -> Result<RuntimeGraph, Box<dyn std::error::Error>> {
        let data = std::fs::read_to_string(path)?;

        // Try direct deserialization
        if let Ok(mut graph) = serde_json::from_str::<RuntimeGraph>(&data) {
            graph.rebuild_index();
            return Ok(graph);
        }

        // Fallback: Convert from export format
        Self::convert_from_export_format(&data)
    }

    fn convert_from_export_format(data: &str) -> Result<RuntimeGraph, Box<dyn std::error::Error>> {
        #[derive(Deserialize)]
        struct ExportFormat {
            nodes: Vec<ChunkNode>,
            relationships: Vec<ExportRelationship>,
        }

        #[derive(Deserialize)]
        struct ExportRelationship {
            from_id: String,
            to_id: String,
            #[serde(rename = "type")]
            rel_type: String,
            #[serde(default = "default_confidence")]
            confidence: f32,
            #[serde(default)]
            meta: serde_json::Value,
        }

        fn default_confidence() -> f32 {
            0.8
        }

        let export: ExportFormat = serde_json::from_str(data)?;
        let mut runtime = RuntimeGraph::new();

        for node in export.nodes {
            let idx = runtime.graph.add_node(node.clone());
            runtime.node_index_by_id.insert(node.id, idx);
        }

        for rel in export.relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                runtime.node_index_by_id.get(&rel.from_id),
                runtime.node_index_by_id.get(&rel.to_id),
            ) {
                runtime.graph.add_edge(
                    from_idx,
                    to_idx,
                    Relationship {
                        r#type: rel.rel_type,
                        confidence: rel.confidence,
                        meta: rel.meta,
                    },
                );
            }
        }

        Ok(runtime)
    }
}

// ─────────────────────────────────────────────────────────────
// Graph Query Engine
// ─────────────────────────────────────────────────────────────

pub struct GraphQuery<'a> {
    runtime: &'a RuntimeGraph,
}

impl<'a> GraphQuery<'a> {
    pub fn new(runtime: &'a RuntimeGraph) -> Self {
        Self { runtime }
    }

    pub fn has_data(&self) -> bool {
        !self.runtime.is_empty()
    }

    pub fn constrained_bfs(
        &self,
        seed_id: &str,
        max_hops: usize,
        rel_filter: &str,
    ) -> Vec<&ChunkNode> {
        let Some(&start) = self.runtime.node_index_by_id.get(seed_id) else {
            return vec![];
        };

        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((start, 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth > max_hops || visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            if depth > 0 {
                results.push(&self.runtime.graph[current]);
            }

            if depth < max_hops {
                for neighbor in self.runtime.graph.neighbors(current) {
                    if let Some(edge_id) = self.runtime.graph.find_edge(current, neighbor) {
                        if let Some(edge_weight) = self.runtime.graph.edge_weight(edge_id) {
                            if (rel_filter.is_empty() || edge_weight.r#type.contains(rel_filter))
                                && edge_weight.confidence > 0.6
                            {
                                queue.push_back((neighbor, depth + 1));
                            }
                        }
                    }
                }
            }
        }

        results
    }

    pub fn get_neighbors(&self, node_id: &str) -> Vec<(&ChunkNode, &Relationship)> {
        let Some(&idx) = self.runtime.node_index_by_id.get(node_id) else {
            return vec![];
        };

        let mut results = Vec::new();
        for neighbor in self.runtime.graph.neighbors(idx) {
            if let Some(edge_id) = self.runtime.graph.find_edge(idx, neighbor) {
                if let (Some(node), Some(rel)) = (
                    self.runtime.graph.node_weight(neighbor),
                    self.runtime.graph.edge_weight(edge_id),
                ) {
                    results.push((node, rel));
                }
            }
        }
        results
    }

    pub fn get_node(&self, node_id: &str) -> Option<&ChunkNode> {
        self.runtime
            .node_index_by_id
            .get(node_id)
            .and_then(|&idx| self.runtime.graph.node_weight(idx))
    }
}

// ─────────────────────────────────────────────────────────────
// Initialization Functions
// ─────────────────────────────────────────────────────────────

#[cfg(feature = "neo4j")]
pub async fn initialize_from_neo4j(neo4j_client: Neo4jClient) {
    let compiler = GraphCompiler::new_with_neo4j(neo4j_client, "data");
    let runtime_graph = Arc::new(compiler.compile().await);
    set_runtime_graph(runtime_graph);
}

pub async fn initialize_standalone(data_dir: &str) {
    let compiler = GraphCompiler::new_standalone(data_dir);
    let runtime_graph = Arc::new(compiler.compile().await);
    set_runtime_graph(runtime_graph);
}

/// Force-reload the petgraph runtime from a specific JSON file path.


pub fn export_to_json(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = get_runtime_graph();
    let json = serde_json::to_string_pretty(&*runtime)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_basic_graph_operations() {
        let mut graph = RuntimeGraph::new();

        // Add nodes
        let idx1 = graph.graph.add_node(ChunkNode {
            id: "n1".into(),
            content: "Hello World".into(),
            entities: vec!["greeting".into()],
            source: "test.pdf".into(),
        });
        let idx2 = graph.graph.add_node(ChunkNode {
            id: "n2".into(),
            content: "Goodbye World".into(),
            entities: vec!["farewell".into()],
            source: "test.pdf".into(),
        });

        graph.node_index_by_id.insert("n1".into(), idx1);
        graph.node_index_by_id.insert("n2".into(), idx2);

        // Add edge
        graph.graph.add_edge(
            idx1,
            idx2,
            Relationship {
                r#type: "FOLLOWS".into(),
                confidence: 0.95,
                meta: serde_json::json!({}),
            },
        );

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert!(!graph.is_empty());

        println!("✅ Basic graph operations: PASSED");
    }

    #[test]
    fn test_graph_query() {
        let mut graph = RuntimeGraph::new();

        // Create: n1 -> n2 -> n3
        let idx1 = graph.graph.add_node(ChunkNode {
            id: "n1".into(),
            content: "First".into(),
            entities: vec![],
            source: "test".into(),
        });
        let idx2 = graph.graph.add_node(ChunkNode {
            id: "n2".into(),
            content: "Second".into(),
            entities: vec![],
            source: "test".into(),
        });
        let idx3 = graph.graph.add_node(ChunkNode {
            id: "n3".into(),
            content: "Third".into(),
            entities: vec![],
            source: "test".into(),
        });

        graph.node_index_by_id.insert("n1".into(), idx1);
        graph.node_index_by_id.insert("n2".into(), idx2);
        graph.node_index_by_id.insert("n3".into(), idx3);

        graph.graph.add_edge(
            idx1,
            idx2,
            Relationship {
                r#type: "NEXT".into(),
                confidence: 0.9,
                meta: serde_json::json!({}),
            },
        );
        graph.graph.add_edge(
            idx2,
            idx3,
            Relationship {
                r#type: "NEXT".into(),
                confidence: 0.8,
                meta: serde_json::json!({}),
            },
        );

        let query = GraphQuery::new(&graph);

        // Test get_node
        let node = query.get_node("n1");
        assert!(node.is_some());
        assert_eq!(node.unwrap().content, "First");
        println!("✅ get_node: PASSED");

        // Test get_neighbors
        let neighbors = query.get_neighbors("n1");
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].0.id, "n2");
        println!("✅ get_neighbors: PASSED");

        // Test BFS 1 hop
        let results = query.constrained_bfs("n1", 1, "");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "n2");
        println!("✅ BFS 1-hop: PASSED");

        // Test BFS 2 hops
        let results = query.constrained_bfs("n1", 2, "");
        assert_eq!(results.len(), 2);
        println!("✅ BFS 2-hop: PASSED");

        // Test BFS with filter
        let results = query.constrained_bfs("n1", 2, "NEXT");
        assert_eq!(results.len(), 2);
        println!("✅ BFS with filter: PASSED");

        // Test non-existent node
        let results = query.constrained_bfs("nonexistent", 1, "");
        assert!(results.is_empty());
        println!("✅ Non-existent node: PASSED");
    }

    #[test]
    fn test_json_loading() {
        // Create temp directory
        let temp_dir = std::env::temp_dir().join("petgraph_test");
        fs::create_dir_all(&temp_dir).unwrap();

        let json_path = temp_dir.join("knowledge_graph.json");

        // Write test JSON
        let json_data = r#"{
            "nodes": [
                {"id": "a1", "content": "Content A", "entities": ["X"], "source": "doc.pdf"},
                {"id": "a2", "content": "Content B", "entities": ["Y"], "source": "doc.pdf"}
            ],
            "relationships": [
                {"from_id": "a1", "to_id": "a2", "type": "LINKS_TO", "confidence": 0.85}
            ]
        }"#;

        fs::write(&json_path, json_data).unwrap();

        // Load it
        let result = GraphCompiler::load_from_json(json_path.to_str().unwrap());
        assert!(result.is_ok(), "Failed to load JSON: {:?}", result.err());

        let graph = result.unwrap();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert!(graph.node_index_by_id.contains_key("a1"));
        assert!(graph.node_index_by_id.contains_key("a2"));

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();

        println!("✅ JSON loading: PASSED");
    }

    #[test]
    fn test_binary_save_load() {
        let temp_dir = std::env::temp_dir().join("petgraph_bin_test");
        fs::create_dir_all(&temp_dir).unwrap();
        let bin_path = temp_dir.join("test.bin");

        // Create graph
        let mut graph = RuntimeGraph::new();
        let idx = graph.graph.add_node(ChunkNode {
            id: "bin_test".into(),
            content: "Binary test".into(),
            entities: vec![],
            source: "test".into(),
        });
        graph.node_index_by_id.insert("bin_test".into(), idx);

        // Save
        let save_result = GraphCompiler::save_to_disk(&graph, bin_path.to_str().unwrap());
        assert!(save_result.is_ok());
        println!("✅ Binary save: PASSED");

        // Load
        let load_result = GraphCompiler::load_from_disk(bin_path.to_str().unwrap());
        assert!(load_result.is_ok());

        let loaded = load_result.unwrap();
        assert_eq!(loaded.node_count(), 1);
        assert!(loaded.node_index_by_id.contains_key("bin_test"));
        println!("✅ Binary load: PASSED");

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_compile_standalone() {
        let temp_dir = std::env::temp_dir().join("petgraph_compile_test");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create JSON file
        let json_data = r#"{
            "nodes": [
                {"id": "c1", "content": "Compile test", "entities": [], "source": "test"}
            ],
            "relationships": []
        }"#;
        fs::write(temp_dir.join("knowledge_graph.json"), json_data).unwrap();

        // Compile
        let compiler = GraphCompiler::new_standalone(temp_dir.to_str().unwrap());
        let graph = compiler.compile().await;

        assert_eq!(graph.node_count(), 1);
        println!("✅ Compile standalone: PASSED");

        // Check binary cache was created
        assert!(temp_dir.join("runtime_graph.bin").exists());
        println!("✅ Binary cache created: PASSED");

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }
}
