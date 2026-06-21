// src/pdf/native_extractor.rs — DocExtractor impl: full native PDF pipeline.
//
// Priority in registry (set by main.rs):
//   Docling sidecar (if up) > NativePdfExtractor > built-in pdftotext
//
// Graceful degradation at each stage:
//   lopdf bbox fail    → text-only words (bboxes = None)
//   candle model fail  → heuristic region tagger
//   ORT model fail     → text-mode table clustering
//
// Phase 1 relational extension: after word extraction, lines and column
// assignments are computed once. The per-word column tags ride into
// build_ir so each DocBlock gets a `column_position` metadata stamp. The
// raw line / page rows are stashed in DocIR.metadata as JSON so the upload
// pipeline (which knows the document_id) can persist them to SQLite —
// see `relational::lines_meta_key` / `pages_meta_key` for the keys.

use super::column_detect;
use super::ir_builder::build_ir;
use super::layout_model::LayoutModel;
use super::line_grouper::group_words_into_lines;
use super::table_model::TableModel;
use super::word_extractor::extract_words;
use crate::doc_ir::{ColumnPosition, DocIR};
use crate::extractor::DocExtractor;
use crate::mime_detect::ContentType;
use serde::Serialize;
use tracing::{debug, warn};

pub struct NativePdfExtractor;

/// Metadata keys used to ferry relational rows from the extractor into the
/// upload pipeline. Centralised here so the persistence stage doesn't have
/// to hardcode strings.
pub mod relational {
    pub const LINES_KEY: &str = "pdf_relational_lines_json";
    pub const PAGES_KEY: &str = "pdf_relational_pages_json";
}

/// One row destined for the `pdf_lines` table. Serialized into DocIR
/// metadata; the upload pipeline parses and inserts.
#[derive(Debug, Clone, Serialize)]
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

/// One row destined for the `pdf_pages` table.
#[derive(Debug, Clone, Serialize)]
pub struct PageRow {
    pub page: u32,
    pub line_count: u32,
    pub column_k_used: u8,
    pub column_silhouette: Option<f32>,
    pub is_scanned: bool,
}

impl DocExtractor for NativePdfExtractor {
    fn name(&self) -> &str {
        "native_pdf"
    }

    fn can_handle(&self, ct: &ContentType) -> bool {
        matches!(ct, ContentType::Pdf)
    }

    fn extract(&self, bytes: Vec<u8>, filename: &str, _ct: &ContentType) -> anyhow::Result<DocIR> {
        // Stage 1: word extraction
        let words = extract_words(&bytes).map_err(|e| {
            warn!(filename, error = %e, "native_pdf: word extraction failed");
            e
        })?;

        if words.is_empty() {
            anyhow::bail!("native_pdf: no text extracted from '{}'", filename);
        }

        debug!(filename, words = words.len(), "native_pdf: words extracted");

        // Stage 1.5: line grouping + column detection (Phase 1 relational).
        // Always runs when LAYOUT_ML_ENABLED — cost is small and the
        // per-corpus `relational_pdf_enabled` flag gates persistence only,
        // not extraction. Doing it here means the column-aware DocBlock
        // tags are available even when the corpus hasn't opted into the
        // SQLite sidecar tables.
        let lines = group_words_into_lines(&words);
        let columns = column_detect::assign_columns(&lines);

        let mut word_columns: Vec<ColumnPosition> = vec![ColumnPosition::Multi; words.len()];
        for (line, &col) in lines.iter().zip(columns.positions.iter()) {
            for wi in line.word_range.clone() {
                word_columns[wi] = col;
            }
        }

        let line_rows: Vec<LineRow> = lines
            .iter()
            .zip(columns.positions.iter())
            .map(|(l, col)| LineRow {
                page: l.page,
                line_idx: l.line_idx,
                text: l.text.clone(),
                x0: l.bbox.map(|b| b[0]),
                y0: l.bbox.map(|b| b[1]),
                x1: l.bbox.map(|b| b[2]),
                y1: l.bbox.map(|b| b[3]),
                column_position: col.as_str().to_string(),
            })
            .collect();

        // is_scanned heuristic: a page with bbox-bearing lines = native PDF
        // text; a page with zero bbox-bearing lines means we fell back to
        // extractous (text-only) for that page, which most often signals a
        // scanned PDF. Phase 1 uses this as the single signal.
        let mut has_bbox_lines_per_page: std::collections::HashMap<u32, bool> =
            std::collections::HashMap::new();
        for l in &lines {
            let entry = has_bbox_lines_per_page.entry(l.page).or_insert(false);
            *entry = *entry || l.bbox.is_some();
        }

        let page_rows: Vec<PageRow> = columns
            .pages
            .iter()
            .map(|p| PageRow {
                page: p.page,
                line_count: p.line_count,
                column_k_used: p.k_used,
                column_silhouette: p.silhouette,
                is_scanned: !*has_bbox_lines_per_page.get(&p.page).unwrap_or(&true),
            })
            .collect();

        // Stage 2: region classification (DETR, word-ORT, or heuristic)
        let layout = LayoutModel::load_or_heuristic();
        let tags = layout.classify(&words, &bytes);

        debug!(
            filename,
            candle = layout.is_candle_loaded(),
            "native_pdf: regions classified"
        );

        // Stage 3: table structure (ORT or text-mode)
        let table_model = TableModel::load_or_text();

        // Stage 4: assemble DocIR (with per-word column tags so blocks get
        // their `column_position` metadata).
        let mut ir = build_ir(filename, &words, &tags, table_model, Some(&word_columns));
        ir.tag_extractor("native_pdf");

        // Stash relational rows in DocIR metadata so the upload pipeline
        // can persist them after document_id is allocated.
        if let Ok(json) = serde_json::to_string(&line_rows) {
            ir.metadata.insert(relational::LINES_KEY.to_string(), json);
        }
        if let Ok(json) = serde_json::to_string(&page_rows) {
            ir.metadata.insert(relational::PAGES_KEY.to_string(), json);
        }

        let char_count: usize = ir.blocks.iter().map(|b| b.text.len()).sum();
        if char_count == 0 {
            anyhow::bail!("native_pdf: extracted zero chars from '{}'", filename);
        }

        Ok(ir)
    }
}
