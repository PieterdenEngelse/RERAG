// backend/src/graph/client.rs
// FalkorDB client with a cheap, cloneable graph handle.
//
// FalkorDB is a Redis module: it speaks RESP, not Bolt. Queries go through
// `GRAPH.QUERY <key> "<cypher>"`. The `falkordb` crate wraps this; its
// `AsyncGraph::query` needs `&mut self`, so each operation selects its own
// graph from the (cheaply cloneable) client.
//
// Requires: cargo build --features graph
//
// NOTE: type names (`GraphClient`, `GraphClientError`) are kept for now — the
// FalkorDB rename is a separate, final commit. See docs/falkordb-migration.md.

use crate::graph::config::GraphConfig;
use falkordb::{
    AsyncGraph, FalkorAsyncClient, FalkorClientBuilder, FalkorConnectionInfo, FalkorDBError,
    FalkorValue,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// One result row: column values in `RETURN` order.
pub type Row = Vec<FalkorValue>;

/// Error type for graph operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphClientError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Feature disabled")]
    Disabled,
}

impl From<FalkorDBError> for GraphClientError {
    fn from(err: FalkorDBError) -> Self {
        GraphClientError::Query(err.to_string())
    }
}

/// Cypher literal encoders for FalkorDB query parameters.
///
/// FalkorDB's parameter mechanism (`CYPHER key=value query`) parses the value
/// as a Cypher expression, so every parameter value must be a valid Cypher
/// literal. These helpers produce the right-hand side of `key=value`.
pub mod lit {
    /// Encode a string as a single-quoted Cypher string literal.
    /// Escapes quotes, backslashes, and control characters — safe for arbitrary
    /// document/chunk content.
    pub fn str(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('\'');
        for c in s.chars() {
            match c {
                '\'' => out.push_str("\\'"),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    out.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => out.push(c),
            }
        }
        out.push('\'');
        out
    }

    /// Encode a signed integer literal.
    pub fn int(n: i64) -> String {
        n.to_string()
    }

    /// Encode a float literal. Non-finite values fall back to `0.0`
    /// (Cypher has no NaN/Inf literal).
    pub fn float(n: f64) -> String {
        if n.is_finite() {
            // Always include a decimal point so it parses as a float.
            if n.fract() == 0.0 {
                format!("{n:.1}")
            } else {
                n.to_string()
            }
        } else {
            "0.0".to_string()
        }
    }

    /// Encode a boolean literal.
    pub fn bool(b: bool) -> String {
        b.to_string()
    }

    /// Encode a list of strings as a Cypher list literal: `['a','b']`.
    pub fn str_list(items: &[String]) -> String {
        let mut out = String::from("[");
        for (i, s) in items.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&str(s));
        }
        out.push(']');
        out
    }

    /// Encode a list of integers as a Cypher list literal: `[1,2,3]`.
    pub fn int_list(items: &[i64]) -> String {
        let mut out = String::from("[");
        for (i, n) in items.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&n.to_string());
        }
        out.push(']');
        out
    }

    /// Encode a `vecf32(...)` literal for FalkorDB vector params.  Vectors
    /// flow through literal substitution (the bound-params path doesn't
    /// support the `vecf32` constructor today).
    pub fn vecf32(items: &[f32]) -> String {
        let mut out = String::from("vecf32([");
        for (i, x) in items.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            if x.is_finite() {
                if x.fract() == 0.0 {
                    out.push_str(&format!("{x:.1}"));
                } else {
                    out.push_str(&x.to_string());
                }
            } else {
                out.push_str("0.0");
            }
        }
        out.push_str("])");
        out
    }
}

// ── Row value extraction ──────────────────────────────────────────────
// Result rows come back positionally in `RETURN`-clause order. These keep
// read sites terse and tolerant of absent / wrongly-typed columns.

/// Extract an owned `String` from row column `i` (empty if absent).
pub fn row_str(row: &Row, i: usize) -> String {
    row.get(i)
        .and_then(|v| v.as_string())
        .cloned()
        .unwrap_or_default()
}

/// Extract an `i64` from row column `i` (0 if absent).
pub fn row_i64(row: &Row, i: usize) -> i64 {
    row.get(i).and_then(|v| v.to_i64()).unwrap_or(0)
}

/// Extract an `f64` from row column `i` (`default` if absent).
pub fn row_f64(row: &Row, i: usize, default: f64) -> f64 {
    row.get(i).and_then(|v| v.to_f64()).unwrap_or(default)
}

/// Extract a `bool` from row column `i` (false if absent).
pub fn row_bool(row: &Row, i: usize) -> bool {
    row.get(i).and_then(|v| v.to_bool()).unwrap_or(false)
}

/// Extract a `Vec<String>` from a list-typed row column `i`.
pub fn row_str_vec(row: &Row, i: usize) -> Vec<String> {
    row.get(i)
        .and_then(|v| v.as_vec())
        .map(|items| {
            items
                .iter()
                .filter_map(|x| x.as_string().cloned())
                .collect()
        })
        .unwrap_or_default()
}

/// Convert a `FalkorValue` into a `serde_json::Value` for API responses.
pub fn falkor_value_to_json(value: &FalkorValue) -> serde_json::Value {
    use serde_json::{json, Value};
    match value {
        FalkorValue::String(s) => Value::String(s.clone()),
        FalkorValue::Bool(b) => Value::Bool(*b),
        FalkorValue::I64(n) => Value::from(*n),
        FalkorValue::F64(f) => Value::from(*f),
        FalkorValue::None => Value::Null,
        FalkorValue::Array(items) => Value::Array(items.iter().map(falkor_value_to_json).collect()),
        FalkorValue::Map(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), falkor_value_to_json(v)))
                .collect(),
        ),
        FalkorValue::Node(node) => json!({
            "_type": "node",
            "id": node.entity_id,
            "labels": node.labels,
            "properties": node
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), falkor_value_to_json(v)))
                .collect::<serde_json::Map<String, Value>>(),
        }),
        FalkorValue::Edge(edge) => json!({
            "_type": "edge",
            "id": edge.entity_id,
            "relationship": edge.relationship_type,
            "src": edge.src_node_id,
            "dst": edge.dst_node_id,
            "properties": edge
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), falkor_value_to_json(v)))
                .collect::<serde_json::Map<String, Value>>(),
        }),
        other => Value::String(format!("{other:?}")),
    }
}

/// Current Unix time in milliseconds — the app-side replacement for a
/// server-side `datetime()`. Timestamps are stored as plain `i64` epoch-millis.
pub fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Convenience: build a `HashMap<String, String>` parameter map.
/// Usage: `params!{ "id" => lit::str(&id), "n" => lit::int(count) }`.
#[macro_export]
macro_rules! params {
    ( $( $k:expr => $v:expr ),* $(,)? ) => {{
        let mut m: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        $( m.insert($k.to_string(), $v); )*
        m
    }};
}

/// A cheap, cloneable handle to one FalkorDB graph.
///
/// Replaces the `Arc<neo4rs::Graph>` that the builders/retriever used to share.
/// Every call selects a fresh `AsyncGraph` because `query()` needs `&mut`.
#[derive(Clone)]
pub struct GraphHandle {
    client: Arc<FalkorAsyncClient>,
    graph_name: String,
    /// Per-query command timeout in ms (0 = none). Applied to row-returning
    /// queries; `run` (schema init / ingestion writes) is left untimed.
    command_timeout_ms: u64,
}

impl GraphHandle {
    /// Select a fresh `AsyncGraph` for one operation.
    fn select(&self) -> AsyncGraph {
        self.client.select_graph(self.graph_name.as_str())
    }

    /// The graph key name (the first argument to `GRAPH.QUERY`).
    pub fn name(&self) -> &str {
        &self.graph_name
    }

    /// Run a query for its side effects, discarding the result set.
    pub async fn run(
        &self,
        cypher: &str,
        params: &HashMap<String, String>,
    ) -> Result<(), GraphClientError> {
        let mut g = self.select();
        let qb = g.query(cypher);
        let qb = if params.is_empty() {
            qb
        } else {
            qb.with_params(params)
        };
        qb.execute().await?;
        Ok(())
    }

    /// Run a query and collect every row into owned values.
    ///
    /// Rows are returned in `RETURN`-clause column order — index positionally.
    pub async fn query(
        &self,
        cypher: &str,
        params: &HashMap<String, String>,
    ) -> Result<Vec<Row>, GraphClientError> {
        let mut g = self.select();
        let qb = g.query(cypher);
        let qb = if params.is_empty() {
            qb
        } else {
            qb.with_params(params)
        };
        let qb = if self.command_timeout_ms > 0 {
            qb.with_timeout(self.command_timeout_ms as i64)
        } else {
            qb
        };
        let result = qb.execute().await?;
        Ok(result.data.collect())
    }

    /// Run a read-only query (`GRAPH.RO_QUERY`); FalkorDB rejects any writes.
    ///
    /// Rows are returned in `RETURN`-clause column order — index positionally.
    pub async fn query_ro(
        &self,
        cypher: &str,
        params: &HashMap<String, String>,
    ) -> Result<Vec<Row>, GraphClientError> {
        let mut g = self.select();
        let qb = g.ro_query(cypher);
        let qb = if params.is_empty() {
            qb
        } else {
            qb.with_params(params)
        };
        let qb = if self.command_timeout_ms > 0 {
            qb.with_timeout(self.command_timeout_ms as i64)
        } else {
            qb
        };
        let result = qb.execute().await?;
        Ok(result.data.collect())
    }
}

/// FalkorDB client wrapper with connection management.
#[derive(Clone)]
pub struct GraphClient {
    handle: GraphHandle,
    #[allow(dead_code)]
    config: GraphConfig,
    connected: bool,
}

impl GraphClient {
    /// Create a new client and establish a connection.
    pub async fn new(config: GraphConfig) -> Result<Self, GraphClientError> {
        if !config.enabled {
            return Err(GraphClientError::Disabled);
        }

        let url = build_connection_url(&config.uri, &config.password);
        info!(uri = %config.uri, graph = %config.database, "Connecting to FalkorDB");

        let conn_info: FalkorConnectionInfo = url
            .as_str()
            .try_into()
            .map_err(|e: FalkorDBError| GraphClientError::Connection(e.to_string()))?;

        let client = FalkorClientBuilder::new_async()
            .with_connection_info(conn_info)
            .build()
            .await
            .map_err(|e| GraphClientError::Connection(e.to_string()))?;

        let handle = GraphHandle {
            client: Arc::new(client),
            graph_name: config.database.clone(),
            command_timeout_ms: config.command_timeout_ms,
        };

        // Verify the connection is actually live.
        handle
            .run("RETURN 1", &HashMap::new())
            .await
            .map_err(|e| GraphClientError::Connection(e.to_string()))?;

        info!("Successfully connected to FalkorDB");

        Ok(Self {
            handle,
            config,
            connected: true,
        })
    }

    /// Whether the client is connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get the cloneable graph handle (used by the builders/retriever).
    pub fn graph(&self) -> GraphHandle {
        self.handle.clone()
    }

    /// Initialize graph indexes.
    ///
    /// Uniqueness is enforced by `MERGE` on the keys below, so explicit unique
    /// constraints are omitted — these range indexes are what back `MERGE`
    /// lookups and ordered scans. Full-text indexes back `/graph` search.
    pub async fn init_schema(&self) -> Result<(), GraphClientError> {
        info!("Initializing FalkorDB schema");
        let no_params = HashMap::new();

        // Range indexes — merge keys and ordered scans. `MERGE` already
        // enforces uniqueness, so no explicit unique constraints are created.
        let range_indexes: &[&str] = &[
            "CREATE INDEX FOR (d:Document) ON (d.id)",
            "CREATE INDEX FOR (c:Chunk) ON (c.id)",
            "CREATE INDEX FOR (c:Chunk) ON (c.embedding_id)",
            "CREATE INDEX FOR (e:Entity) ON (e.id)",
            "CREATE INDEX FOR (e:Entity) ON (e.normalized_name)",
            "CREATE INDEX FOR (e:Entity) ON (e.entity_type)",
            "CREATE INDEX FOR (c:Concept) ON (c.id)",
            "CREATE INDEX FOR (c:Concept) ON (c.name)",
            "CREATE INDEX FOR (c:Concept) ON (c.domain)",
            "CREATE INDEX FOR (a:Agent) ON (a.id)",
            "CREATE INDEX FOR (g:Goal) ON (g.id)",
            "CREATE INDEX FOR (g:Goal) ON (g.status)",
            "CREATE INDEX FOR (e:Episode) ON (e.id)",
            "CREATE INDEX FOR (e:Episode) ON (e.created_at)",
        ];
        for stmt in range_indexes {
            if let Err(e) = self.handle.run(stmt, &no_params).await {
                swallow_already_exists("index", stmt, &e);
            }
        }

        // Full-text indexes — back the /graph entity/concept/chunk search.
        let fulltext: &[&str] = &[
            "CALL db.idx.fulltext.createNodeIndex('Entity', 'name', 'description')",
            "CALL db.idx.fulltext.createNodeIndex('Concept', 'name', 'description')",
            "CALL db.idx.fulltext.createNodeIndex('Chunk', 'content')",
        ];
        for stmt in fulltext {
            if let Err(e) = self.handle.run(stmt, &no_params).await {
                swallow_already_exists("fulltext index", stmt, &e);
            }
        }

        // Vector index on Entity.embedding — backs the proxy-pointer entity
        // reconciler.  Dimension must match the live embedder; we read it once
        // at schema init and bake the literal into the DDL (FalkorDB does not
        // accept a bound parameter here).
        let embed_dim = crate::embedder::embedding_dim();
        let vector_stmt = format!(
            "CREATE VECTOR INDEX FOR (e:Entity) ON (e.embedding) OPTIONS {{ dimension: {}, similarityFunction: 'cosine' }}",
            embed_dim
        );
        if let Err(e) = self.handle.run(&vector_stmt, &no_params).await {
            swallow_already_exists("vector index", &vector_stmt, &e);
        }

        info!("FalkorDB schema initialization complete");
        Ok(())
    }

    /// Get node and relationship counts grouped by label / type.
    pub async fn get_stats(&self) -> Result<GraphStats, GraphClientError> {
        let no_params = HashMap::new();

        let node_rows = self
            .handle
            .query(
                "MATCH (n)
                 WITH labels(n) AS labels, count(*) AS count
                 UNWIND labels AS label
                 RETURN label, sum(count) AS node_count
                 ORDER BY node_count DESC",
                &no_params,
            )
            .await?;
        let mut node_counts = HashMap::new();
        for row in node_rows {
            let label = row_str(&row, 0);
            let count = row_i64(&row, 1);
            if !label.is_empty() {
                node_counts.insert(label, count as usize);
            }
        }

        let rel_rows = self
            .handle
            .query(
                "MATCH ()-[r]->()
                 RETURN type(r) AS rel_type, count(*) AS count
                 ORDER BY count DESC",
                &no_params,
            )
            .await?;
        let mut rel_counts = HashMap::new();
        for row in rel_rows {
            let rel_type = row_str(&row, 0);
            let count = row_i64(&row, 1);
            if !rel_type.is_empty() {
                rel_counts.insert(rel_type, count as usize);
            }
        }

        let total_nodes = node_counts.values().sum();
        let total_relationships = rel_counts.values().sum();

        Ok(GraphStats {
            node_counts,
            relationship_counts: rel_counts,
            total_nodes,
            total_relationships,
        })
    }

    /// Health check — verify the connection is alive.
    pub async fn health_check(&self) -> Result<bool, GraphClientError> {
        match self.handle.run("RETURN 1", &HashMap::new()).await {
            Ok(_) => Ok(true),
            Err(e) => {
                error!("FalkorDB health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Execute a raw Cypher query, returning owned rows.
    pub async fn execute_query(&self, cypher: &str) -> Result<Vec<Row>, GraphClientError> {
        self.handle.query(cypher, &HashMap::new()).await
    }

    /// Execute a read-only Cypher query, returning owned rows.
    pub async fn execute_query_ro(&self, cypher: &str) -> Result<Vec<Row>, GraphClientError> {
        self.handle.query_ro(cypher, &HashMap::new()).await
    }

    /// Run a raw Cypher query without returning results.
    pub async fn run_query(&self, cypher: &str) -> Result<(), GraphClientError> {
        self.handle.run(cypher, &HashMap::new()).await
    }
}

/// Inject a password into a `redis://host:port` URL as `redis://:pw@host:port`.
fn build_connection_url(uri: &str, password: &str) -> String {
    if password.is_empty() {
        return uri.to_string();
    }
    match uri.find("://") {
        // No existing userinfo expected in the configured URL.
        Some(idx) if !uri[idx + 3..].contains('@') => {
            let (scheme, rest) = uri.split_at(idx + 3);
            format!("{scheme}:{password}@{rest}")
        }
        _ => uri.to_string(),
    }
}

/// Treat "already exists" errors from index creation as success.
fn swallow_already_exists(kind: &str, name: &str, err: &GraphClientError) {
    let msg = err.to_string().to_lowercase();
    if msg.contains("already") || msg.contains("exist") || msg.contains("indexed") {
        debug!("{} `{}` already present", kind, name);
    } else {
        warn!("Failed to create {} `{}`: {}", kind, name, err);
    }
}

/// Graph statistics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphStats {
    pub node_counts: HashMap<String, usize>,
    pub relationship_counts: HashMap<String, usize>,
    pub total_nodes: usize,
    pub total_relationships: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disabled_client() {
        let config = GraphConfig::default(); // disabled by default
        let result = GraphClient::new(config).await;
        assert!(matches!(result, Err(GraphClientError::Disabled)));
    }

    #[test]
    fn test_lit_str_escapes() {
        assert_eq!(lit::str("hi"), "'hi'");
        assert_eq!(lit::str("it's"), "'it\\'s'");
        assert_eq!(lit::str("a\nb"), "'a\\nb'");
        assert_eq!(lit::str("c:\\x"), "'c:\\\\x'");
    }

    #[test]
    fn test_lit_str_list() {
        assert_eq!(
            lit::str_list(&["a".to_string(), "b".to_string()]),
            "['a','b']"
        );
        assert_eq!(lit::str_list(&[]), "[]");
    }

    #[test]
    fn test_build_connection_url() {
        assert_eq!(
            build_connection_url("redis://localhost:6380", "pw"),
            "redis://:pw@localhost:6380"
        );
        assert_eq!(
            build_connection_url("redis://localhost:6380", ""),
            "redis://localhost:6380"
        );
    }
}
