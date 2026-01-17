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

// Phase 1 modules
pub mod simd;
pub mod compression;
pub mod quantization;
pub mod hnsw;
pub mod bloom;
pub mod arena;
pub mod batch;
pub mod streaming;

// Phase 2 modules
pub mod hybrid_search;
pub mod product_quantization;
pub mod semantic_cache;
pub mod reranking;
pub mod sqlite_opt;
pub mod connection_pool;
pub mod request_coalescing;
pub mod mixed_precision;
pub mod io_uring;
pub mod http2;
pub mod grpc;
pub mod tiered_storage;
pub mod sharding;
pub mod integration;
pub mod onnx_embedder;

// Re-export commonly used items from Phase 1
pub use simd::{cosine_similarity_simd, dot_product_simd, normalize_simd};
pub use compression::{compress_vectors, decompress_vectors};
pub use quantization::{QuantizedVector, quantize, dequantize};
pub use hnsw::HnswIndex;
pub use bloom::VectorBloomFilter;
pub use arena::SearchArena;

// Re-export commonly used items from Phase 2
pub use hybrid_search::{HybridSearcher, HybridSearchConfig, reciprocal_rank_fusion};
pub use product_quantization::{PQIndex, PQCodebook};
pub use semantic_cache::{SemanticCache, SemanticCacheConfig};
pub use reranking::{Reranker, RerankConfig};
pub use sqlite_opt::{SqliteConfig, optimize_connection};
pub use connection_pool::{ConnectionPool, PoolConfig};
pub use request_coalescing::{RequestCoalescer, Singleflight};
pub use mixed_precision::{F16Vector, F16VectorStore};
pub use http2::Http2Config;
pub use grpc::GrpcConfig;
pub use tiered_storage::{TieredStorage, TieringPolicy, StorageTier};
pub use sharding::{ShardRouter, ShardConfig, ShardingStrategy};
