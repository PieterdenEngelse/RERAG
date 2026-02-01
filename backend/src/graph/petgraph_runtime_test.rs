// backend/src/graph/petgraph_runtime_test.rs

#[cfg(test)]
mod tests {
    use crate::graph::petgraph_runtime::{
        ChunkNode, GraphCompiler, GraphQuery, RuntimeGraph,
    };
    use std::fs;
    use tempfile::tempdir;

    fn create_test_json(dir: &str) -> String {
        let json = r#"{
            "nodes": [
                {"id": "n1", "content": "Node 1 content", "entities": ["A"], "source": "test.pdf"},
                {"id": "n2", "content": "Node 2 content", "entities": ["B"], "source": "test.pdf"},
                {"id": "n3", "content": "Node 3 content", "entities": ["C"], "source": "test.pdf"}
            ],
            "relationships": [
                {"from_id": "n1", "to_id": "n2", "type": "RELATED_TO", "confidence": 0.9},
                {"from_id": "n2", "to_id": "n3", "type": "MENTIONS", "confidence": 0.7}
            ]
        }"#;

        let path = format!("{}/knowledge_graph.json", dir);
        fs::create_dir_all(dir).unwrap();
        fs::write(&path, json).unwrap();
        path
    }

    #[test]
    fn test_runtime_graph_new() {
        let graph = RuntimeGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_load_from_json() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();
        create_test_json(dir_path);

        let json_path = format!("{}/knowledge_graph.json", dir_path);
        let result = GraphCompiler::load_from_json(&json_path);

        assert!(result.is_ok(), "Failed to load JSON: {:?}", result.err());

        let graph = result.unwrap();
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);

        // Verify index was rebuilt
        assert!(graph.node_index_by_id.contains_key("n1"));
        assert!(graph.node_index_by_id.contains_key("n2"));
        assert!(graph.node_index_by_id.contains_key("n3"));
    }

    #[test]
    fn test_save_and_load_binary() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();

        // Create a graph
        let mut graph = RuntimeGraph::new();
        let idx1 = graph.graph.add_node(ChunkNode {
            id: "test1".to_string(),
            content: "Test content".to_string(),
            entities: vec!["entity1".to_string()],
            source: "test.pdf".to_string(),
        });
        graph.node_index_by_id.insert("test1".to_string(), idx1);

        // Save to disk
        let bin_path = format!("{}/test_graph.bin", dir_path);
        let save_result = GraphCompiler::save_to_disk(&graph, &bin_path);
        assert!(save_result.is_ok());

        // Load from disk
        let load_result = GraphCompiler::load_from_disk(&bin_path);
        assert!(load_result.is_ok());

        let loaded = load_result.unwrap();
        assert_eq!(loaded.node_count(), 1);
        assert!(loaded.node_index_by_id.contains_key("test1"));
    }

    #[test]
    fn test_graph_query_bfs() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();
        create_test_json(dir_path);

        let json_path = format!("{}/knowledge_graph.json", dir_path);
        let graph = GraphCompiler::load_from_json(&json_path).unwrap();

        let query = GraphQuery::new(&graph);

        // Test BFS from n1, max 2 hops
        let results = query.constrained_bfs("n1", 2, "");
        assert!(!results.is_empty());

        // Should find n2 (1 hop) and n3 (2 hops)
        let ids: Vec<&str> = results.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"n2"));
    }

    #[test]
    fn test_graph_query_get_neighbors() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();
        create_test_json(dir_path);

        let json_path = format!("{}/knowledge_graph.json", dir_path);
        let graph = GraphCompiler::load_from_json(&json_path).unwrap();

        let query = GraphQuery::new(&graph);

        let neighbors = query.get_neighbors("n1");
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].0.id, "n2");
        assert_eq!(neighbors[0].1.r#type, "RELATED_TO");
    }

    #[test]
    fn test_graph_query_get_node() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();
        create_test_json(dir_path);

        let json_path = format!("{}/knowledge_graph.json", dir_path);
        let graph = GraphCompiler::load_from_json(&json_path).unwrap();

        let query = GraphQuery::new(&graph);

        let node = query.get_node("n1");
        assert!(node.is_some());
        assert_eq!(node.unwrap().content, "Node 1 content");

        let missing = query.get_node("nonexistent");
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_compile_standalone() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();
        create_test_json(dir_path);

        let compiler = GraphCompiler::new_standalone(dir_path);
        let graph = compiler.compile().await;

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }

    #[tokio::test]
    async fn test_compile_empty_when_no_files() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();

        // No files created - should return empty graph
        let compiler = GraphCompiler::new_standalone(dir_path);
        let graph = compiler.compile().await;

        assert!(graph.is_empty());
    }
}