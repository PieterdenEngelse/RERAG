//! Performance Optimization Module
//!
//! This module provides high-performance utilities for the AG RAG system.
//!
//! ## Phase 1 Optimizations (1-12)
//! 1. SIMD-accelerated vector operations (4-8x faster)
//! 2. LZ4 compression for vectors (2x smaller files)
//! 3. Vector quantization (4x memory reduction)
//! 4. HNSW index for approximate nearest neighbor (O(log n) search)
//! 5. Bloom filters for fast negative lookups
//! 6. Arena allocator for temporary allocations
//! 7. Batch processing utilities
//! 8. Streaming response utilities
//! 9. Memory-mapped files (in retriever.rs)
//! 10. Cache persistence (in retriever.rs, embedder.rs)
//! 11. Incremental updates (in retriever.rs)
//! 12. rkyv serialization (throughout)
//!
//! ## Phase 2 Optimizations (13-32)
//! 13. Hybrid search (BM25 + vector)
//! 14. Product quantization (16x memory reduction)
//! 15. io_uring (Linux async I/O) - requires tokio-uring
//! 16. SQLite WAL mode and optimizations
//! 17. Semantic query cache
//! 18. Re-ranking with diversity
//! 19. gRPC API - requires tonic setup
//! 20. HTTP/2 - enabled via Actix TLS
//! 21. Connection pooling
//! 22. Request coalescing
//! 23. Write-ahead log - in sqlite_opt
//! 24. GPU embeddings - requires CUDA/Metal
//! 25. Mixed precision (FP16)
//! 26. ONNX runtime - requires ort crate
//! 27. Model distillation - training required
//! 28. Batched GPU inference - requires GPU
//! 29. Sharding - requires distributed setup
//! 30. Read replicas - requires distributed setup
//! 31. Edge caching - requires CDN
//! 32. Tiered storage - in progress

// Core utilities
pub mod cache_aligned;

// Phase 1 modules
pub mod arena;
pub mod batch;
pub mod bloom;
pub mod compression;
pub mod hnsw;
pub mod quantization;
pub mod simd;
pub mod streaming;

// Phase 2 modules
pub mod connection_pool;
pub mod grpc;
pub mod http2;
pub mod hybrid_search;
pub mod integration;
pub mod io_uring;
pub mod mixed_precision;
pub mod onnx_embedder;
pub mod product_quantization;
pub mod request_coalescing;
pub mod reranking;
pub mod semantic_cache;
pub mod sharding;
pub mod sqlite_opt;
pub mod tiered_storage;

// Re-export commonly used items from Phase 1
pub use arena::SearchArena;
pub use bloom::VectorBloomFilter;
pub use compression::{compress_vectors, decompress_vectors};
pub use hnsw::HnswIndex;
pub use quantization::{dequantize, quantize, QuantizedVector};
pub use simd::{cosine_similarity_simd, dot_product_simd, normalize_simd};

// Re-export commonly used items from Phase 2
pub use cache_aligned::CacheAligned;
pub use connection_pool::{ConnectionPool, PoolConfig};
pub use grpc::GrpcConfig;
pub use http2::Http2Config;
pub use hybrid_search::{reciprocal_rank_fusion, HybridSearchConfig, HybridSearcher};
pub use mixed_precision::{F16Vector, F16VectorStore};
pub use product_quantization::{PQCodebook, PQIndex};
pub use request_coalescing::{RequestCoalescer, Singleflight};
pub use reranking::{RerankConfig, Reranker};
pub use semantic_cache::{SemanticCache, SemanticCacheConfig};
pub use sharding::{ShardConfig, ShardRouter, ShardingStrategy};
pub use sqlite_opt::{optimize_connection, SqliteConfig};
pub use tiered_storage::{StorageTier, TieredStorage, TieringPolicy};
