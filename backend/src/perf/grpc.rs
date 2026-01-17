//! gRPC API Support
//! 
//! Provides gRPC service definitions for high-performance RPC.
//! gRPC offers:
//! - Binary protocol (protobuf) - smaller payloads
//! - HTTP/2 - multiplexing, streaming
//! - Bidirectional streaming
//! - Strong typing with code generation
//! 
//! # Usage
//! This module provides service trait definitions. To use:
//! 1. Add tonic to Cargo.toml
//! 2. Create .proto files
//! 3. Implement the service traits
//! 4. Run with tonic server

use std::pin::Pin;
use std::future::Future;

/// Search request for gRPC
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub top_k: u32,
    pub filters: Vec<Filter>,
}

/// Search filter
#[derive(Debug, Clone)]
pub struct Filter {
    pub field: String,
    pub value: String,
    pub operator: FilterOperator,
}

/// Filter operator
#[derive(Debug, Clone, Copy)]
pub enum FilterOperator {
    Equals,
    Contains,
    GreaterThan,
    LessThan,
}

/// Search response
#[derive(Debug, Clone)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total_count: u64,
    pub took_ms: u64,
}

/// Individual search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub doc_id: String,
    pub score: f32,
    pub content: String,
    pub metadata: Vec<(String, String)>,
}

/// Embedding request
#[derive(Debug, Clone)]
pub struct EmbedRequest {
    pub texts: Vec<String>,
}

/// Embedding response
#[derive(Debug, Clone)]
pub struct EmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
}

/// Index request
#[derive(Debug, Clone)]
pub struct IndexRequest {
    pub doc_id: String,
    pub content: String,
    pub metadata: Vec<(String, String)>,
}

/// Index response
#[derive(Debug, Clone)]
pub struct IndexResponse {
    pub success: bool,
    pub message: String,
}

/// Health check request
#[derive(Debug, Clone)]
pub struct HealthRequest {}

/// Health check response
#[derive(Debug, Clone)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_secs: u64,
}

/// Health status
#[derive(Debug, Clone, Copy)]
pub enum HealthStatus {
    Serving,
    NotServing,
    Unknown,
}

/// RAG service trait
/// 
/// Implement this trait to provide gRPC RAG functionality.
/// Use with tonic to generate the actual gRPC server.
pub trait RagService: Send + Sync + 'static {
    /// Search for documents
    fn search(
        &self,
        request: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, ServiceError>> + Send>>;

    /// Generate embeddings
    fn embed(
        &self,
        request: EmbedRequest,
    ) -> Pin<Box<dyn Future<Output = Result<EmbedResponse, ServiceError>> + Send>>;

    /// Index a document
    fn index(
        &self,
        request: IndexRequest,
    ) -> Pin<Box<dyn Future<Output = Result<IndexResponse, ServiceError>> + Send>>;

    /// Health check
    fn health(
        &self,
        request: HealthRequest,
    ) -> Pin<Box<dyn Future<Output = Result<HealthResponse, ServiceError>> + Send>>;
}

/// Streaming search service trait
pub trait StreamingRagService: RagService {
    /// Stream search results
    fn search_stream(
        &self,
        request: SearchRequest,
    ) -> Pin<Box<dyn futures_util::Stream<Item = Result<SearchResult, ServiceError>> + Send>>;

    /// Bidirectional streaming for chat
    fn chat_stream(
        &self,
        requests: Pin<Box<dyn futures_util::Stream<Item = ChatMessage> + Send>>,
    ) -> Pin<Box<dyn futures_util::Stream<Item = Result<ChatMessage, ServiceError>> + Send>>;
}

/// Chat message for streaming
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Service error
#[derive(Debug)]
pub enum ServiceError {
    NotFound(String),
    InvalidRequest(String),
    Internal(String),
    Unavailable(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
            Self::Unavailable(msg) => write!(f, "Service unavailable: {}", msg),
        }
    }
}

impl std::error::Error for ServiceError {}

/// gRPC server configuration
#[derive(Debug, Clone)]
pub struct GrpcConfig {
    pub address: String,
    pub port: u16,
    pub max_message_size: usize,
    pub concurrency_limit: usize,
    pub timeout_secs: u64,
    pub tls_enabled: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".to_string(),
            port: 50051,
            max_message_size: 4 * 1024 * 1024, // 4MB
            concurrency_limit: 256,
            timeout_secs: 30,
            tls_enabled: false,
            cert_path: None,
            key_path: None,
        }
    }
}

impl GrpcConfig {
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

/// Proto file content for code generation
/// 
/// Save this to `proto/rag.proto` and use with tonic-build
pub const PROTO_DEFINITION: &str = r#"
syntax = "proto3";

package rag;

service RagService {
    rpc Search(SearchRequest) returns (SearchResponse);
    rpc Embed(EmbedRequest) returns (EmbedResponse);
    rpc Index(IndexRequest) returns (IndexResponse);
    rpc Health(HealthRequest) returns (HealthResponse);
    rpc SearchStream(SearchRequest) returns (stream SearchResult);
    rpc Chat(stream ChatMessage) returns (stream ChatMessage);
}

message SearchRequest {
    string query = 1;
    uint32 top_k = 2;
    repeated Filter filters = 3;
}

message Filter {
    string field = 1;
    string value = 2;
    string operator = 3;
}

message SearchResponse {
    repeated SearchResult results = 1;
    uint64 total_count = 2;
    uint64 took_ms = 3;
}

message SearchResult {
    string doc_id = 1;
    float score = 2;
    string content = 3;
    map<string, string> metadata = 4;
}

message EmbedRequest {
    repeated string texts = 1;
}

message EmbedResponse {
    repeated Embedding embeddings = 1;
}

message Embedding {
    repeated float values = 1;
}

message IndexRequest {
    string doc_id = 1;
    string content = 2;
    map<string, string> metadata = 3;
}

message IndexResponse {
    bool success = 1;
    string message = 2;
}

message HealthRequest {}

message HealthResponse {
    string status = 1;
    string version = 2;
    uint64 uptime_secs = 3;
}

message ChatMessage {
    string role = 1;
    string content = 2;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_config() {
        let config = GrpcConfig::default();
        assert_eq!(config.bind_address(), "127.0.0.1:50051");
    }

    #[test]
    fn test_search_request() {
        let req = SearchRequest {
            query: "test query".to_string(),
            top_k: 10,
            filters: vec![],
        };
        assert_eq!(req.query, "test query");
    }

    #[test]
    fn test_proto_definition() {
        assert!(PROTO_DEFINITION.contains("service RagService"));
        assert!(PROTO_DEFINITION.contains("rpc Search"));
    }
}
