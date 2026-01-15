// src/tools/database_query.rs
// Database Query Tool - Execute safe read-only SQL queries

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use rusqlite::Connection;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct DatabaseQueryTool {
    db_path: Option<PathBuf>,
    success_count: usize,
    total_count: usize,
}

impl DatabaseQueryTool {
    pub fn new() -> Self {
        Self {
            db_path: None,
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self {
            db_path: Some(db_path),
            success_count: 0,
            total_count: 0,
        }
    }

    /// Get the database path, falling back to agent memory database
    fn get_db_path(&self) -> Option<PathBuf> {
        if self.db_path.is_some() {
            return self.db_path.clone();
        }
        // Try to get from global chunk settings first
        if let Some(path) = crate::db::chunk_settings::get_db_path() {
            return Some(path);
        }
        // Fall back to agent memory database
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ag");
        let agent_db = data_dir.join("agent.db");
        if agent_db.exists() {
            return Some(agent_db);
        }
        None
    }

    /// Check if a query is safe (read-only)
    fn is_safe_query(query: &str) -> bool {
        let query_upper = query.to_uppercase();
        let query_trimmed = query_upper.trim();

        // Only allow SELECT statements
        if !query_trimmed.starts_with("SELECT") {
            return false;
        }

        // Block dangerous keywords
        let dangerous = [
            "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "TRUNCATE", "REPLACE",
            "ATTACH", "DETACH", "PRAGMA", "VACUUM", "REINDEX", "ANALYZE",
        ];

        for keyword in dangerous {
            if query_upper.contains(keyword) {
                return false;
            }
        }

        true
    }

    /// Execute a query and return results as formatted text
    fn execute_query(&self, query: &str) -> Result<String, String> {
        let db_path = self
            .get_db_path()
            .ok_or_else(|| "Database path not configured".to_string())?;

        if !db_path.exists() {
            return Err(format!("Database not found at {:?}", db_path));
        }

        let conn =
            Connection::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

        let mut stmt = conn
            .prepare(query)
            .map_err(|e| format!("Invalid SQL: {}", e))?;

        let column_count = stmt.column_count();
        let column_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

        let mut rows_data: Vec<Vec<String>> = Vec::new();

        let rows = stmt
            .query_map([], |row| {
                let mut row_data = Vec::new();
                for i in 0..column_count {
                    let value: String = match row.get_ref(i) {
                        Ok(val) => match val {
                            rusqlite::types::ValueRef::Null => "NULL".to_string(),
                            rusqlite::types::ValueRef::Integer(i) => i.to_string(),
                            rusqlite::types::ValueRef::Real(f) => format!("{:.4}", f),
                            rusqlite::types::ValueRef::Text(t) => {
                                let s = String::from_utf8_lossy(t).to_string();
                                if s.len() > 100 {
                                    format!("{}...", &s[..100])
                                } else {
                                    s
                                }
                            }
                            rusqlite::types::ValueRef::Blob(b) => {
                                format!("[BLOB {} bytes]", b.len())
                            }
                        },
                        Err(_) => "ERROR".to_string(),
                    };
                    row_data.push(value);
                }
                Ok(row_data)
            })
            .map_err(|e| format!("Query execution failed: {}", e))?;

        for row in rows {
            if let Ok(data) = row {
                rows_data.push(data);
            }
        }

        // Format output
        if rows_data.is_empty() {
            return Ok("Query returned no results.".to_string());
        }

        let mut output = format!("Query returned {} row(s):\n\n", rows_data.len());

        // Header
        output.push_str("| ");
        for name in &column_names {
            output.push_str(&format!("{} | ", name));
        }
        output.push('\n');

        // Separator
        output.push_str("|");
        for _ in &column_names {
            output.push_str("---|");
        }
        output.push('\n');

        // Data rows (limit to 50)
        for (_i, row) in rows_data.iter().take(50).enumerate() {
            output.push_str("| ");
            for val in row {
                output.push_str(&format!("{} | ", val));
            }
            output.push('\n');
        }

        if rows_data.len() > 50 {
            output.push_str(&format!("\n... and {} more rows", rows_data.len() - 50));
        }

        Ok(output)
    }

    /// List available tables
    fn list_tables(&self) -> Result<String, String> {
        let db_path = self
            .get_db_path()
            .ok_or_else(|| "Database path not configured".to_string())?;

        let conn =
            Connection::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .map_err(|e| format!("Failed to list tables: {}", e))?;

        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| format!("Query failed: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        if tables.is_empty() {
            Ok("No tables found in database.".to_string())
        } else {
            Ok(format!("Available tables: {}", tables.join(", ")))
        }
    }
}

#[async_trait]
impl Tool for DatabaseQueryTool {
    fn tool_type(&self) -> ToolType {
        ToolType::DatabaseQuery
    }

    fn description(&self) -> String {
        "Execute read-only SQL queries against the application database".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.80
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();
        debug!("DatabaseQueryTool: executing '{}'", query);

        let query_trimmed = query.trim();

        // Handle special commands
        if query_trimmed.to_lowercase() == "show tables"
            || query_trimmed.to_lowercase() == "list tables"
        {
            match self.list_tables() {
                Ok(result) => {
                    return Ok(ToolResult {
                        tool: ToolType::DatabaseQuery,
                        success: true,
                        result,
                        metadata: ToolMetadata {
                            execution_time_ms: start.elapsed().as_millis() as u64,
                            confidence: 1.0,
                            source: Some("DatabaseQuery".to_string()),
                            cost: Some(0.0),
                        },
                    });
                }
                Err(e) => {
                    return Ok(ToolResult {
                        tool: ToolType::DatabaseQuery,
                        success: false,
                        result: e,
                        metadata: ToolMetadata {
                            execution_time_ms: start.elapsed().as_millis() as u64,
                            confidence: 0.0,
                            source: Some("DatabaseQuery".to_string()),
                            cost: Some(0.0),
                        },
                    });
                }
            }
        }

        // Safety check
        if !Self::is_safe_query(query_trimmed) {
            warn!(
                "DatabaseQueryTool: blocked unsafe query '{}'",
                query_trimmed
            );
            return Ok(ToolResult {
                tool: ToolType::DatabaseQuery,
                success: false,
                result:
                    "Only SELECT queries are allowed. Use 'show tables' to see available tables."
                        .to_string(),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("DatabaseQuery".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        // Execute the query
        match self.execute_query(query_trimmed) {
            Ok(result) => Ok(ToolResult {
                tool: ToolType::DatabaseQuery,
                success: true,
                result,
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.95,
                    source: Some("DatabaseQuery".to_string()),
                    cost: Some(0.0),
                },
            }),
            Err(e) => Ok(ToolResult {
                tool: ToolType::DatabaseQuery,
                success: false,
                result: e,
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("DatabaseQuery".to_string()),
                    cost: Some(0.0),
                },
            }),
        }
    }

    fn update_success(&mut self, success: bool) {
        self.total_count += 1;
        if success {
            self.success_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_query_detection() {
        assert!(DatabaseQueryTool::is_safe_query("SELECT * FROM users"));
        assert!(DatabaseQueryTool::is_safe_query(
            "SELECT id, name FROM users WHERE id = 1"
        ));
        assert!(!DatabaseQueryTool::is_safe_query(
            "INSERT INTO users VALUES (1, 'test')"
        ));
        assert!(!DatabaseQueryTool::is_safe_query("DELETE FROM users"));
        assert!(!DatabaseQueryTool::is_safe_query("DROP TABLE users"));
        assert!(!DatabaseQueryTool::is_safe_query(
            "UPDATE users SET name = 'test'"
        ));
        assert!(!DatabaseQueryTool::is_safe_query(
            "SELECT * FROM users; DROP TABLE users"
        ));
    }

    #[tokio::test]
    async fn test_database_query_no_db() {
        let tool = DatabaseQueryTool::new();
        let result = tool.execute("SELECT 1").await;
        assert!(result.is_ok());
    }
}
