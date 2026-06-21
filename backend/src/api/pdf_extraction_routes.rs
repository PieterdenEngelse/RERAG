//! HTTP routes serving the relational PDF sidecar (Phase 1).
//!
//! These read from `pdf_lines` / `pdf_pages` and back the extraction view
//! that's the educational payoff of relational PDF parsing — users see what
//! columns were detected, how confident the silhouette score is, and how
//! lines were grouped.

use actix_web::{web, HttpResponse, Result};

/// GET /pdf/extraction/{document_id}
///
/// Returns line + page rows for `document_id` (the filename used as
/// chunk_id prefix). Empty arrays when the document hasn't been processed
/// with relational extraction.
pub async fn get_extraction(path: web::Path<String>) -> Result<HttpResponse> {
    let document_id = path.into_inner();
    let db_path = match crate::db::chunk_settings::get_db_path() {
        Some(p) => p,
        None => {
            return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "SQLite path not configured",
            })));
        }
    };
    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string(),
            })));
        }
    };

    let lines = crate::db::pdf_rows::get_lines(&conn, &document_id, None, None).unwrap_or_default();
    let pages = crate::db::pdf_rows::get_pages(&conn, &document_id).unwrap_or_default();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "document_id": document_id,
        "page_count": pages.len(),
        "line_count": lines.len(),
        "pages": pages,
        "lines": lines,
    })))
}
