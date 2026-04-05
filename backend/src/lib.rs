pub mod path_manager;
pub mod db {
    pub mod api_keys;
    pub mod chunk_settings;
    pub mod llm_settings;
    pub mod param_hardware;
    pub mod param_store;
    pub mod path_resolver;
    pub mod schema_init;
}
pub mod agent;
pub mod api;
pub mod rig_tools;
pub mod config;
pub mod embedder;
pub mod gguf_tokenizer;
pub mod index;
pub mod inference_gateway;
pub mod mime_detect;
pub mod parser;
pub mod retriever;
pub mod rules;
pub use retriever::Retriever;
pub mod agent_memory;
pub mod cache;
pub mod installer;
pub mod memory; // The folder
pub mod monitoring;
pub mod tools;
pub use monitoring::performance_analysis;
pub use monitoring::trace_middleware;
pub mod file_watcher;
pub mod graph;
pub mod perf;
pub mod security;
pub mod training; // Performance optimizations (SIMD, compression, HNSW, etc.) // Neo4j Knowledge Graph for GraphRAG (Phase 27)
