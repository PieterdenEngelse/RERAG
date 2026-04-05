//! Rig tool wrappers for AG's existing services.
//! Each tool wraps an existing AG capability and exposes it
//! via Rig's Tool trait for LLM-driven tool calling.

pub mod tantivy_search;
pub mod memory_recall;
pub mod memory_store;
pub mod graph_search;

pub use tantivy_search::TantivySearchTool;
pub use memory_recall::MemoryRecallTool;
pub use memory_store::MemoryStoreTool;
pub use graph_search::GraphSearchTool;
