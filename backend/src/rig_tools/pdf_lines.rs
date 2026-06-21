//! Relational PDF lookup tool — reads from the `pdf_lines` sidecar table
//! populated by the column-aware native PDF extractor. Lets the agent ask
//! structural questions ("right-column lines on page 3") instead of
//! regex-hunting the chunk text.

use crate::db::pdf_rows::{get_lines, LineRow};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct GetLinesArgs {
    /// Document filename (matches the same identifier chunk_ids use as a
    /// prefix — e.g. "invoice.pdf").
    pub document_id: String,
    /// Page number (1-based). Optional — omit to query across all pages.
    #[serde(default)]
    pub page: Option<u32>,
    /// Column filter: "single", "left", "right", or "multi". Optional.
    #[serde(default)]
    pub column: Option<String>,
}

#[derive(Serialize)]
pub struct GetLinesResult {
    pub count: usize,
    pub lines: Vec<LineRow>,
}

#[derive(Debug, thiserror::Error)]
#[error("PDF lines lookup error: {0}")]
pub struct GetLinesError(pub String);

pub struct GetLinesInColumnTool;

impl Tool for GetLinesInColumnTool {
    const NAME: &'static str = "get_lines_in_column";
    type Error = GetLinesError;
    type Args = GetLinesArgs;
    type Output = GetLinesResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "get_lines_in_column".to_string(),
            description: "Fetch raw lines from a PDF, optionally filtered by page and column \
                          ('single', 'left', 'right', 'multi'). Use this when a question asks \
                          about content in a specific column of a multi-column document — \
                          e.g. 'what's the renewal fee?' on an invoice with a left-column \
                          label / right-column value layout. The document_id is the file's \
                          name (e.g. 'invoice.pdf'). Returns an empty list when the document \
                          isn't a PDF or wasn't processed with relational extraction enabled."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "document_id": {
                        "type": "string",
                        "description": "Document filename, same as the chunk_id prefix."
                    },
                    "page": {
                        "type": "integer",
                        "description": "1-based page number. Omit to search across all pages."
                    },
                    "column": {
                        "type": "string",
                        "enum": ["single", "left", "right", "multi"],
                        "description": "Column filter."
                    }
                },
                "required": ["document_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let document_id = args.document_id.clone();
        let page = args.page;
        let column = args.column.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<Vec<LineRow>, String> {
            let db_path = crate::db::chunk_settings::get_db_path()
                .ok_or_else(|| "SQLite path not configured".to_string())?;
            let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
            get_lines(&conn, &document_id, page, column.as_deref()).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| GetLinesError(format!("Task join error: {}", e)))?;

        match result {
            Ok(lines) => Ok(GetLinesResult {
                count: lines.len(),
                lines,
            }),
            Err(e) => Err(GetLinesError(e)),
        }
    }
}
