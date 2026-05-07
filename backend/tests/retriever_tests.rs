use ag::retriever::{Retriever, RetrieverError};
use std::path::Path;
use tempfile::tempdir;

/// Helper to create a retriever with a temporary index directory and vector file
fn make_retriever_with_vector_file(temp_dir: &Path, vector_file: &Path) -> Retriever {
    Retriever::new_with_vector_file(temp_dir.to_str().unwrap(), vector_file.to_str().unwrap())
        .expect("Failed to create retriever")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_indexing_and_search() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever.begin_batch().expect("Failed to start batch mode");

        let docs = vec![
            (
                "doc1".to_string(),
                "Doc 1".to_string(),
                "The quick brown fox jumps".to_string(),
            ),
            (
                "doc2".to_string(),
                "Doc 2".to_string(),
                "Rust programming language".to_string(),
            ),
            (
                "doc3".to_string(),
                "Doc 3".to_string(),
                "Retriever test content".to_string(),
            ),
        ];

        let added = retriever
            .add_documents_batch(docs)
            .expect("Failed to add documents in batch");
        assert_eq!(added, 3, "Should have added exactly 3 documents");

        retriever.end_batch().expect("Failed to end batch mode");

        let results = retriever.search("rust").expect("Search failed");
        assert!(
            results
                .iter()
                .any(|r| r.contains("Rust programming language")),
            "Expected to find 'Rust programming language' in search results, got: {:?}",
            results
        );
    }

    #[test]
    fn test_single_document_indexing_and_search() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever
            .add_document(
                "test_doc",
                "TestTitle",
                "A unique document about testing retrievers",
            )
            .expect("Failed to add document");

        let results = retriever.search("retrievers").expect("Search failed");
        assert!(
            results.iter().any(|r| r.contains("testing retrievers")),
            "Expected to find 'testing retrievers' in search results, got: {:?}",
            results
        );
    }

    #[test]
    fn test_vector_save_and_load_roundtrip() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");

        // Create first retriever and add vectors
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);
        retriever.add_vector_with_id("doc1".to_string(), vec![1.0, 0.0, 0.0]);
        retriever.add_vector_with_id("doc2".to_string(), vec![0.0, 1.0, 0.0]);
        retriever.force_save().expect("Failed to save vectors");

        // Create second retriever with the SAME vector file
        let retriever2 = make_retriever_with_vector_file(dir.path(), &vector_file);

        assert_eq!(
            retriever2.vectors.len(),
            2,
            "Should have loaded 2 vectors from file"
        );
        assert!(
            retriever2.doc_id_to_vector_idx.contains_key("doc1"),
            "Should have loaded mapping for doc1"
        );
        assert!(
            retriever2.doc_id_to_vector_idx.contains_key("doc2"),
            "Should have loaded mapping for doc2"
        );
    }

    #[test]
    fn test_auto_save_threshold_triggers() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever.set_auto_save_threshold(2);

        retriever.add_vector_with_id("v1".to_string(), vec![0.1, 0.2]);
        retriever.add_vector_with_id("v2".to_string(), vec![0.3, 0.4]);

        assert!(
            vector_file.exists(),
            "Auto-save should have created vector file after reaching threshold of 2"
        );
    }

    #[test]
    fn test_vector_similarity_via_search() {
        // Test cosine similarity indirectly through vector_search
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        // Add identical and orthogonal vectors
        retriever.add_vector(vec![1.0, 0.0, 0.0]); // index 0
        retriever.add_vector(vec![1.0, 0.0, 0.0]); // index 1 - identical to query
        retriever.add_vector(vec![0.0, 1.0, 0.0]); // index 2 - orthogonal to query

        let query = vec![1.0, 0.0, 0.0];
        let results = retriever.vector_search(&query, 3);

        assert_eq!(results.len(), 3, "Should return all 3 vectors");

        // Identical vectors should have highest similarity (~1.0)
        assert!(
            results[0].1 > 0.99,
            "Identical vectors should have similarity ~1.0, got {}",
            results[0].1
        );

        // Orthogonal vector should have lowest similarity (~0.0)
        assert!(
            results[2].1.abs() < 0.1,
            "Orthogonal vectors should have similarity ~0.0, got {}",
            results[2].1
        );
    }

    #[test]
    fn test_add_and_search_document() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever
            .add_document(
                "test_doc_1",
                "Test Title",
                "This is test content about Rust",
            )
            .expect("Failed to add document");

        let results = retriever.search("Rust").expect("Search failed");
        assert!(
            !results.is_empty(),
            "Search should return results for 'Rust'"
        );
        assert_eq!(
            results[0], "This is test content about Rust",
            "First result should match the added document content"
        );

        let metrics = retriever.get_metrics();
        assert_eq!(
            metrics.total_documents_indexed, 1,
            "Should have indexed exactly 1 document"
        );
        assert_eq!(metrics.total_searches, 1, "Should have performed 1 search");
        assert_eq!(
            metrics.cache_misses, 1,
            "First search should be a cache miss"
        );
        assert!(
            metrics.avg_search_latency_us > 0.0,
            "Search latency should be positive"
        );
    }

    #[test]
    fn test_batch_indexing() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        let docs = vec![
            (
                "doc1".to_string(),
                "Doc1".to_string(),
                "First document".to_string(),
            ),
            (
                "doc2".to_string(),
                "Doc2".to_string(),
                "Second document".to_string(),
            ),
            (
                "doc3".to_string(),
                "Doc3".to_string(),
                "Third document".to_string(),
            ),
        ];

        let count = retriever
            .add_documents_batch(docs)
            .expect("Failed to add documents in batch");
        assert_eq!(count, 3, "Should have added 3 documents");

        let results = retriever.search("document").expect("Search failed");
        assert_eq!(
            results.len(),
            3,
            "Should find all 3 documents containing 'document'"
        );

        let metrics = retriever.get_metrics();
        assert_eq!(
            metrics.total_documents_indexed, 3,
            "Metrics should show 3 documents indexed"
        );
    }

    #[test]
    fn test_vector_operations() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever.add_vector(vec![1.0, 0.0, 0.0]);
        retriever.add_vector(vec![0.9, 0.1, 0.0]);
        retriever.add_vector(vec![0.0, 1.0, 0.0]);

        let query = vec![1.0, 0.0, 0.0];
        let results = retriever.vector_search(&query, 2);

        assert_eq!(results.len(), 2, "Should return top 2 results");
        assert_eq!(
            results[0].0, 0,
            "Most similar vector should be at index 0 (exact match)"
        );
        assert!(
            results[0].1 > results[1].1,
            "First result should have higher similarity than second: {} > {}",
            results[0].1,
            results[1].1
        );

        let metrics = retriever.get_metrics();
        assert_eq!(metrics.total_vectors, 3, "Should have 3 vectors in storage");
    }

    #[test]
    fn test_save_and_load_vectors() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");

        // Create first retriever and save vectors
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);
        retriever.add_vector_with_id("doc1".to_string(), vec![1.0, 2.0, 3.0]);
        retriever.add_vector_with_id("doc2".to_string(), vec![4.0, 5.0, 6.0]);
        retriever
            .save_vectors(vector_file.to_str().unwrap())
            .expect("Failed to save vectors");

        // Create second retriever with different index dir but same vector file
        let dir2 = tempdir().expect("Failed to create second temp directory");
        let retriever2 = make_retriever_with_vector_file(dir2.path(), &vector_file);

        assert_eq!(retriever2.vectors.len(), 2, "Should have loaded 2 vectors");
        assert_eq!(
            retriever2.doc_id_to_vector_idx.len(),
            2,
            "Should have loaded 2 ID mappings"
        );
        assert_eq!(
            retriever2.metrics.total_vectors, 2,
            "Metrics should show 2 vectors"
        );
    }

    #[test]
    fn test_rerank_by_vector_similarity() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever.add_vector(vec![1.0, 0.0, 0.0]);
        retriever.add_vector(vec![0.0, 1.0, 0.0]);
        retriever.add_vector(vec![0.9, 0.1, 0.0]);

        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![0, 1, 2];
        let results = retriever
            .rerank_by_vector_similarity(&query, &candidates)
            .expect("Reranking failed");

        assert_eq!(results.len(), 3, "Should return all 3 candidates reranked");
        assert_eq!(
            results[0].0, 0,
            "Index 0 should be most similar (exact match)"
        );
        assert_eq!(results[1].0, 2, "Index 2 should be second most similar");
        assert_eq!(
            results[2].0, 1,
            "Index 1 should be least similar (orthogonal)"
        );
    }

    #[test]
    fn test_batch_mode_errors() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        // Test begin_batch can be called
        retriever.begin_batch().expect("Failed to start batch mode");

        // Should error when calling begin_batch twice
        let result = retriever.begin_batch();
        assert!(
            result.is_err(),
            "Should error when calling begin_batch while already in batch mode"
        );

        // Add documents in batch mode
        let docs = vec![(
            "test_doc".to_string(),
            "title".to_string(),
            "content".to_string(),
        )];
        retriever
            .add_documents_batch(docs)
            .expect("Failed to add documents to batch");

        retriever.end_batch().expect("Failed to end batch mode");

        // Should error when calling end_batch without active batch
        let result = retriever.end_batch();
        assert!(
            result.is_err(),
            "Should error when calling end_batch without active batch"
        );
    }

    #[test]
    fn test_hybrid_search_without_vectors() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever
            .add_document("test_doc_1", "Test", "Rust programming")
            .expect("Failed to add document");

        let results = retriever
            .hybrid_search("Rust", None)
            .expect("Hybrid search failed");
        assert!(
            !results.is_empty(),
            "Hybrid search without vectors should still return text search results"
        );
    }

    #[test]
    fn test_index_chunk() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        let vector = vec![1.0, 2.0, 3.0];
        retriever
            .index_chunk("chunk1", "This is a chunk", vector.clone(), None)
            .expect("Failed to index chunk");

        assert_eq!(
            retriever.vectors.len(),
            1,
            "Should have 1 vector after indexing chunk"
        );

        let results = retriever.search("chunk").expect("Search failed");
        assert!(
            !results.is_empty(),
            "Should find the indexed chunk in search results"
        );
    }

    #[test]
    fn test_search_caching() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever
            .add_document("test_doc_1", "Test", "Rust programming")
            .expect("Failed to add document");

        let results1 = retriever.search("Rust").expect("First search failed");
        let results2 = retriever.search("Rust").expect("Second search failed");

        assert_eq!(
            results1, results2,
            "Cached search should return identical results"
        );

        let (cache_size, _) = retriever.cache_stats();
        assert_eq!(cache_size, 1, "Cache should contain 1 entry");

        let metrics = retriever.get_metrics();
        assert_eq!(
            metrics.total_searches, 2,
            "Should have performed 2 searches"
        );
        assert_eq!(metrics.cache_hits, 1, "Should have 1 cache hit");
        assert_eq!(metrics.cache_misses, 1, "Should have 1 cache miss");
        assert!(
            (metrics.cache_hit_rate() - 0.5).abs() < 0.001,
            "Cache hit rate should be 0.5 (1 hit out of 2 searches), got {}",
            metrics.cache_hit_rate()
        );
    }

    #[test]
    fn test_parallel_vector_search() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        // Add 1500 vectors in a circle pattern to test parallel search performance
        const NUM_VECTORS: usize = 1500;
        for i in 0..NUM_VECTORS {
            let angle = (i as f32) * 0.01;
            retriever.add_vector(vec![angle.cos(), angle.sin(), 0.0]);
        }

        let query = vec![1.0, 0.0, 0.0];
        let results = retriever.vector_search(&query, 10);

        assert_eq!(results.len(), 10, "Should return top 10 results");
        assert_eq!(
            results[0].0, 0,
            "Vector at index 0 should be most similar to query [1.0, 0.0, 0.0]"
        );
    }

    #[test]
    fn test_cache_clear() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever
            .add_document("test_doc_1", "Test", "Rust programming")
            .expect("Failed to add document");

        let _ = retriever.search("Rust").expect("Search failed");

        let (cache_size, _) = retriever.cache_stats();
        assert_eq!(cache_size, 1, "Cache should have 1 entry after search");

        retriever.clear_cache();

        let (cache_size, _) = retriever.cache_stats();
        assert_eq!(cache_size, 0, "Cache should be empty after clear");
    }

    #[test]
    fn test_metrics_index_size() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever
            .add_document("test_doc_1", "Test", "Rust programming")
            .expect("Failed to add document");

        let metrics = retriever.get_metrics();
        let index_size = metrics
            .get_index_size_bytes()
            .expect("Failed to get index size");
        assert!(index_size > 0, "Index size should be positive");

        let human_size = metrics
            .get_index_size_human()
            .expect("Failed to get human-readable index size");
        assert!(
            !human_size.is_empty(),
            "Human-readable size should not be empty"
        );
    }

    #[test]
    fn test_metrics_reset() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        retriever
            .add_document("test_doc_1", "Test", "Rust programming")
            .expect("Failed to add document");
        let _ = retriever.search("Rust").expect("Search failed");

        let original_metrics = retriever.get_metrics();
        assert!(
            original_metrics.total_searches > 0,
            "Should have search activity before reset"
        );

        retriever.reset_metrics();

        let reset_metrics = retriever.get_metrics();
        assert_eq!(
            reset_metrics.total_searches, 0,
            "Total searches should be 0 after reset"
        );
        assert_eq!(
            reset_metrics.cache_hits, 0,
            "Cache hits should be 0 after reset"
        );
        assert_eq!(
            reset_metrics.cache_misses, 0,
            "Cache misses should be 0 after reset"
        );
        assert_eq!(
            reset_metrics.total_documents_indexed, 0,
            "Total documents indexed should be 0 after reset"
        );
    }

    #[test]
    fn test_health_check() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        // Health check on empty retriever should pass
        retriever
            .health_check()
            .expect("Health check should pass on empty retriever");

        // Add vectors with proper dimensionality (384)
        retriever.add_vector_with_id("doc1".to_string(), vec![1.0; 384]);
        retriever.add_vector_with_id("doc2".to_string(), vec![2.0; 384]);
        retriever
            .health_check()
            .expect("Health check should pass after adding vectors");

        // Introduce inconsistency: extra mapping without vector
        retriever
            .doc_id_to_vector_idx
            .insert("extra_doc".to_string(), 0);
        let result = retriever.health_check();
        assert!(
            result.is_err(),
            "Health check should fail with vector storage inconsistency"
        );
        if let Err(RetrieverError::VectorError(msg)) = result {
            assert!(
                msg.contains("Vector storage inconsistency"),
                "Error message should mention inconsistency, got: {}",
                msg
            );
        } else {
            panic!("Expected VectorError for inconsistency, got: {:?}", result);
        }

        // Fix inconsistency
        retriever.doc_id_to_vector_idx.remove("extra_doc");
        retriever
            .health_check()
            .expect("Health check should pass after fixing inconsistency");

        // Introduce invalid index
        retriever
            .doc_id_to_vector_idx
            .insert("doc1".to_string(), 999);
        let result = retriever.health_check();
        assert!(
            result.is_err(),
            "Health check should fail with invalid vector index"
        );
        if let Err(RetrieverError::VectorError(msg)) = result {
            assert!(
                msg.contains("Invalid vector index"),
                "Error message should mention invalid index, got: {}",
                msg
            );
        } else {
            panic!("Expected VectorError for invalid index, got: {:?}", result);
        }

        // Fix invalid index
        retriever.doc_id_to_vector_idx.insert("doc1".to_string(), 0);
        retriever
            .health_check()
            .expect("Health check should pass after fixing invalid index");
    }

    #[test]
    fn test_health_check_with_corrupted_index_dir() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        // Remove the index directory while retriever still holds reference
        std::fs::remove_dir_all(dir.path()).expect("Failed to remove index directory for test");

        let result = retriever.health_check();
        assert!(
            result.is_err(),
            "Health check should fail when index directory is missing"
        );
        if let Err(RetrieverError::DirectoryError(msg)) = result {
            assert!(
                msg.contains("does not exist"),
                "Error message should mention missing directory, got: {}",
                msg
            );
        } else {
            panic!(
                "Expected DirectoryError for missing directory, got: {:?}",
                result
            );
        }
    }

    #[test]
    fn test_repair_vector_mappings() {
        let dir = tempdir().expect("Failed to create temp directory");
        let vector_file = dir.path().join("vectors.json");
        let mut retriever = make_retriever_with_vector_file(dir.path(), &vector_file);

        // Add vectors with and without IDs
        retriever.add_vector_with_id("doc1".to_string(), vec![1.0, 2.0, 3.0]);
        retriever.add_vector(vec![4.0, 5.0, 6.0]); // No ID
        retriever.add_vector(vec![7.0, 8.0, 9.0]); // No ID

        assert_eq!(
            retriever.vectors.len(),
            3,
            "Should have 3 vectors in storage"
        );
        assert_eq!(
            retriever.doc_id_to_vector_idx.len(),
            1,
            "Should have only 1 mapping (for doc1)"
        );

        // Repair should add mappings for the 2 unmapped vectors
        let repaired = retriever.repair_vector_mappings();
        assert_eq!(repaired, 2, "Should have repaired 2 unmapped vectors");

        assert_eq!(
            retriever.doc_id_to_vector_idx.len(),
            3,
            "Should now have 3 mappings after repair"
        );

        // Health check should pass after repair
        retriever
            .health_check()
            .expect("Health check should pass after repairing mappings");
    }
}
