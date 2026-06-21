// src/db/pdf_rows.rs — Persistence for the Phase 1 relational PDF sidecar
// tables (`pdf_lines`, `pdf_pages`). See `pdf::native_extractor::relational`
// for the metadata keys that ferry these rows out of the extractor.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRow {
    pub page: u32,
    pub line_idx: u32,
    pub text: String,
    pub x0: Option<i64>,
    pub y0: Option<i64>,
    pub x1: Option<i64>,
    pub y1: Option<i64>,
    pub column_position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRow {
    pub page: u32,
    pub line_count: u32,
    pub column_k_used: u8,
    pub column_silhouette: Option<f32>,
    pub is_scanned: bool,
}

/// Replace all `pdf_lines` rows for `document_id` with `lines`. Returns the
/// number of rows inserted. Uses a single transaction; partial failure
/// rolls back.
pub fn replace_lines(
    conn: &mut Connection,
    document_id: &str,
    lines: &[LineRow],
) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM pdf_lines WHERE document_id = ?1",
        params![document_id],
    )?;
    let mut inserted = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO pdf_lines
                (document_id, page, line_idx, text, x0, y0, x1, y1, column_position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for l in lines {
            stmt.execute(params![
                document_id,
                l.page,
                l.line_idx,
                l.text,
                l.x0,
                l.y0,
                l.x1,
                l.y1,
                l.column_position
            ])?;
            inserted += 1;
        }
    }
    tx.commit()?;
    Ok(inserted)
}

/// Replace all `pdf_pages` rows for `document_id` with `pages`.
pub fn replace_pages(
    conn: &mut Connection,
    document_id: &str,
    pages: &[PageRow],
) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM pdf_pages WHERE document_id = ?1",
        params![document_id],
    )?;
    let mut inserted = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO pdf_pages
                (document_id, page, line_count, column_k_used, column_silhouette, is_scanned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for p in pages {
            stmt.execute(params![
                document_id,
                p.page,
                p.line_count,
                p.column_k_used,
                p.column_silhouette,
                if p.is_scanned { 1 } else { 0 }
            ])?;
            inserted += 1;
        }
    }
    tx.commit()?;
    Ok(inserted)
}

/// Look up lines for a document, optionally filtered by page and/or
/// `column_position`. Result ordered by `(page, line_idx)`.
pub fn get_lines(
    conn: &Connection,
    document_id: &str,
    page: Option<u32>,
    column: Option<&str>,
) -> rusqlite::Result<Vec<LineRow>> {
    let mut sql = String::from(
        "SELECT page, line_idx, text, x0, y0, x1, y1, column_position
         FROM pdf_lines WHERE document_id = ?1",
    );
    if page.is_some() {
        sql.push_str(" AND page = ?2");
    }
    if column.is_some() {
        if page.is_some() {
            sql.push_str(" AND column_position = ?3");
        } else {
            sql.push_str(" AND column_position = ?2");
        }
    }
    sql.push_str(" ORDER BY page, line_idx");

    let mut stmt = conn.prepare(&sql)?;
    let map_row = |row: &rusqlite::Row<'_>| {
        Ok(LineRow {
            page: row.get(0)?,
            line_idx: row.get(1)?,
            text: row.get(2)?,
            x0: row.get(3)?,
            y0: row.get(4)?,
            x1: row.get(5)?,
            y1: row.get(6)?,
            column_position: row.get(7)?,
        })
    };

    let rows: Vec<LineRow> = match (page, column) {
        (Some(p), Some(c)) => stmt
            .query_map(params![document_id, p, c], map_row)?
            .filter_map(|r| r.ok())
            .collect(),
        (Some(p), None) => stmt
            .query_map(params![document_id, p], map_row)?
            .filter_map(|r| r.ok())
            .collect(),
        (None, Some(c)) => stmt
            .query_map(params![document_id, c], map_row)?
            .filter_map(|r| r.ok())
            .collect(),
        (None, None) => stmt
            .query_map(params![document_id], map_row)?
            .filter_map(|r| r.ok())
            .collect(),
    };
    Ok(rows)
}

pub fn get_pages(conn: &Connection, document_id: &str) -> rusqlite::Result<Vec<PageRow>> {
    let mut stmt = conn.prepare(
        "SELECT page, line_count, column_k_used, column_silhouette, is_scanned
         FROM pdf_pages WHERE document_id = ?1 ORDER BY page",
    )?;
    let rows: Vec<PageRow> = stmt
        .query_map(params![document_id], |row| {
            Ok(PageRow {
                page: row.get(0)?,
                line_count: row.get(1)?,
                column_k_used: row.get(2)?,
                column_silhouette: row.get(3)?,
                is_scanned: row.get::<_, i64>(4)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Document id of the canned demo invoice the PDF Extraction page defaults
/// to. Kept in one place so the frontend default and the backend seeder stay
/// in sync.
pub const DEMO_INVOICE_DOC_ID: &str = "two_column_invoice.pdf";

/// Bytes of the bundled fixture PDF. Embedded at compile time so the
/// installer doesn't need to ship a separate file. See
/// `backend/tests/fixtures/pdf/two_column_invoice.ps` for the PostScript
/// source — regenerate with `ps2pdf two_column_invoice.ps
/// two_column_invoice.pdf`.
pub const DEMO_PDF_BYTES: &[u8] = include_bytes!("../../tests/fixtures/pdf/two_column_invoice.pdf");

/// Insert a canned two-column invoice into `pdf_lines` / `pdf_pages` so the
/// PDF Extraction page renders something useful on first boot. No-ops if the
/// document already has rows — so a user uploading a real PDF with the same
/// filename always wins over the demo.
///
/// Returns `true` when seeding happened, `false` when an existing row set was
/// detected and left alone.
pub fn seed_demo_if_missing(conn: &mut Connection) -> rusqlite::Result<bool> {
    let lines_present: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pdf_lines WHERE document_id = ?1",
        params![DEMO_INVOICE_DOC_ID],
        |row| row.get(0),
    )?;
    if lines_present > 0 {
        return Ok(false);
    }
    // pdf_lines / pdf_pages only — `extraction_records`, `preprocess`,
    // `canon`, and `chunking` stats are populated by the real-ingest path
    // in `ingest_demo_pdf` (called from main.rs after the retriever is
    // up). Seeding extraction_records here would double-count the
    // by_format counter against the real ingest.
    replace_lines(conn, DEMO_INVOICE_DOC_ID, &demo_invoice_lines())?;
    replace_pages(conn, DEMO_INVOICE_DOC_ID, &demo_invoice_pages())?;
    Ok(true)
}

fn demo_invoice_lines() -> Vec<LineRow> {
    let pairs = [
        ("Renewal fee", 60i64, "EUR 200", 50i64),
        ("Late payment fee", 100, "EUR 75", 45),
        ("Cancellation fee", 100, "EUR 150", 50),
        ("Reinstatement fee", 110, "EUR 50", 45),
        ("Document copy fee", 110, "EUR 10", 45),
        ("Account closure fee", 130, "EUR 100", 50),
    ];
    let mut out = Vec::with_capacity(pairs.len() * 2);
    let mut idx: u32 = 0;
    for (i, (label, lw, amount, aw)) in pairs.iter().enumerate() {
        let y0 = 100 + (i as i64) * 50;
        let y1 = y0 + 22;
        out.push(LineRow {
            page: 1,
            line_idx: idx,
            text: (*label).to_string(),
            x0: Some(80),
            y0: Some(y0),
            x1: Some(80 + lw),
            y1: Some(y1),
            column_position: "col0".to_string(),
        });
        idx += 1;
        out.push(LineRow {
            page: 1,
            line_idx: idx,
            text: (*amount).to_string(),
            x0: Some(620),
            y0: Some(y0),
            x1: Some(620 + aw),
            y1: Some(y1),
            column_position: "col1".to_string(),
        });
        idx += 1;
    }
    out
}

fn demo_invoice_pages() -> Vec<PageRow> {
    vec![PageRow {
        page: 1,
        line_count: 12,
        column_k_used: 2,
        column_silhouette: Some(0.72),
        is_scanned: false,
    }]
}

/// Run the bundled demo PDF through the real ingestion path so every TIP
/// board (parser, preprocess, canon, chunking) lights up with measured
/// counters — not the synthetic ones written by [`seed_demo_if_missing`].
///
/// Idempotent via the `extraction_records` table: skips when a real
/// (non-seed) row already exists for the demo filename. Self-heals if
/// the row gets pruned (7-day retention) or the user deletes the demo
/// via the API — next boot will re-ingest.
///
/// Returns the number of indexed chunks. Logs and returns Err on any
/// failure — boot should never panic because of the demo.
pub async fn ingest_demo_pdf(
    data_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    retriever: std::sync::Arc<std::sync::Mutex<crate::Retriever>>,
    chunker_mode: crate::config::ChunkerMode,
) -> Result<usize, String> {
    use std::fs;

    // Gate: skip if a REAL extraction_records row already exists.
    // Discriminate against the legacy "(seeded demo)" row left by the
    // pre-real-ingest seeder — so an upgrade boot from that binary
    // still triggers a fresh real ingest.
    {
        let conn = rusqlite::Connection::open(&db_path).map_err(|e| format!("open db: {e}"))?;
        let has_real: bool = conn
            .query_row(
                "SELECT 1 FROM extraction_records
                 WHERE filename = ?1 AND path != '(seeded demo)' LIMIT 1",
                params![DEMO_INVOICE_DOC_ID],
                |_| Ok(()),
            )
            .is_ok();
        if has_real {
            return Ok(0);
        }
    }

    let demo_dir = data_dir.join("demo");
    fs::create_dir_all(&demo_dir).map_err(|e| format!("create demo dir: {e}"))?;
    let path = demo_dir.join(DEMO_INVOICE_DOC_ID);
    if !path.exists() {
        fs::write(&path, DEMO_PDF_BYTES).map_err(|e| format!("write demo PDF: {e}"))?;
    }

    // Drop any legacy `(seeded demo)` row in extraction_records (both
    // SQLite and the in-memory STATS recent_files vec). The real ingest
    // below will append its own real entry.
    crate::monitoring::extraction_stats::forget_file(DEMO_INVOICE_DOC_ID);

    let ir = match crate::index::extract_ir_async(&path, "default").await {
        Some(ir) => ir,
        None => return Err("extract_ir_async returned None".to_string()),
    };

    let path_for_blocking = path.clone();
    let prepared = tokio::task::spawn_blocking(move || {
        let cfg = crate::db::chunk_settings::global_config();
        let chunker = crate::index::default_chunker(chunker_mode);
        crate::index::prepare_doc(
            &path_for_blocking,
            &ir,
            chunker_mode,
            chunker.as_ref(),
            "default",
            cfg.context_prefix_enabled,
        )
    })
    .await
    .map_err(|e| format!("prepare_doc join: {e}"))?;

    let chunks = {
        let mut retr = retriever
            .lock()
            .map_err(|e| format!("retriever lock: {e}"))?;
        retr.begin_batch()
            .map_err(|e| format!("begin_batch: {e}"))?;
        let (n, _graph) = crate::index::index_prepared_doc(&mut retr, prepared)
            .map_err(|e| format!("index_prepared_doc: {e}"))?;
        retr.commit().map_err(|e| format!("commit: {e}"))?;
        n
    };
    Ok(chunks)
}
