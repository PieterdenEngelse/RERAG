// src/memory/mod.rs

pub mod agent;
pub mod chunker;
pub mod chunker_factory;
pub mod decision_engine;
pub mod llm_provider;
pub mod persistence;
pub mod prompt_cache;
pub mod query;
pub mod vector_store;
// pub mod multi_agent;  // TODO: Fix after core is stable

pub use agent::{
    Agent, AgentContext, AgentMemoryLayer, Episode, Goal, GoalStatus, Reflection, ReflectionType,
    Task, TaskStatus,
};
pub use chunker::{Chunk, ChunkMetadata, ChunkerConfig, SemanticChunker, SourceType};
pub use decision_engine::{
    Decision, DecisionEngine, ExecutionPlan, ExecutionResult, PlanStep, Tool,
};
pub use llm_provider::{create_llm_provider, LLMConfig, LLMError, LLMProvider};
pub use prompt_cache::{CacheOptimizedPrompt, CacheStats, CacheableSegment, SegmentType};
pub use persistence::{backup_vector_store, load_vector_store, save_vector_store};
pub use query::{
    ContextChunk, RagConfig, RagError, RagQueryPipeline, RagQueryRequest, RagQueryResponse,
};
pub use vector_store::{
    SearchResult, StoreStats, VectorRecord, VectorStore, VectorStoreConfig, VectorStoreError,
};
