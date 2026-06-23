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
use super::page_type::classify_page;
use super::table_model::TableModel;
use super::word_extractor::extract_words;
use crate::doc_ir::{ColumnPosition, DocIR};
use crate::extractor::DocExtractor;
use crate::mime_detect::ContentType;
use serde::Serialize;
use std::collections::BTreeMap;
use tracing::{debug, warn};

pub struct NativePdfExtractor;

/// Metadata keys used to ferry relational rows from the extractor into the
/// upload pipeline. Centralised here so the persistence stage doesn't have
/// to hardcode strings.
pub mod relational {
    pub const LINES_KEY: &str = "pdf_relational_lines_json";
    pub const PAGES_KEY: &str = "pdf_relational_pages_json";
    /// Phase 2: per-document summary aggregated from pages + lines.
    pub const SUMMARY_KEY: &str = "pdf_relational_summary_json";
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
    pub page_type: String,
}

/// Document-level summary row destined for `pdf_parsing_summary`.
#[derive(Debug, Clone, Serialize)]
pub struct SummaryRow {
    pub page_count: u32,
    pub scanned_page_count: u32,
    pub total_lines: u32,
    pub bbox_coverage_pct: Option<f32>,
    pub page_types: BTreeMap<String, u32>,
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

        // Phase 2: classify each page (cover/toc/body/appendix) from the
        // line text alone. Cheap and runs once per page.
        let mut lines_by_page: std::collections::HashMap<u32, Vec<&super::line_grouper::LineSpan>> =
            std::collections::HashMap::new();
        for l in &lines {
            lines_by_page.entry(l.page).or_default().push(l);
        }
        let mut page_type_for: std::collections::HashMap<u32, String> =
            std::collections::HashMap::new();
        for (page_num, page_lines) in &lines_by_page {
            // classify_page expects &[LineSpan], so dereference the &LineSpans.
            let owned: Vec<super::line_grouper::LineSpan> =
                page_lines.iter().map(|l| (*l).clone()).collect();
            let pt = classify_page(*page_num, &owned);
            page_type_for.insert(*page_num, pt.as_str().to_string());
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
                page_type: page_type_for
                    .get(&p.page)
                    .cloned()
                    .unwrap_or_else(|| "body".to_string()),
            })
            .collect();

        // Phase 2: document-level summary.
        let summary_row = build_summary(&page_rows, &words);

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
        if let Ok(json) = serde_json::to_string(&summary_row) {
            ir.metadata
                .insert(relational::SUMMARY_KEY.to_string(), json);
        }

        let char_count: usize = ir.blocks.iter().map(|b| b.text.len()).sum();
        if char_count == 0 {
            anyhow::bail!("native_pdf: extracted zero chars from '{}'", filename);
        }

        Ok(ir)
    }
}

/// Build the document-level summary from per-page rows + raw words. Pure
/// over its inputs so it's trivial to unit-test once a fixture lands.
fn build_summary(page_rows: &[PageRow], words: &[super::word_extractor::WordSpan]) -> SummaryRow {
    let page_count = page_rows.len() as u32;
    let scanned_page_count = page_rows.iter().filter(|p| p.is_scanned).count() as u32;
    let total_lines: u32 = page_rows.iter().map(|p| p.line_count).sum();

    let bbox_coverage_pct = if words.is_empty() {
        None
    } else {
        let with_bbox = words.iter().filter(|w| w.bbox.is_some()).count() as f32;
        Some(with_bbox / words.len() as f32 * 100.0)
    };

    let mut page_types: BTreeMap<String, u32> = BTreeMap::new();
    for p in page_rows {
        *page_types.entry(p.page_type.clone()).or_insert(0) += 1;
    }
    for key in ["cover", "toc", "body", "appendix"] {
        page_types.entry(key.to_string()).or_insert(0);
    }

    SummaryRow {
        page_count,
        scanned_page_count,
        total_lines,
        bbox_coverage_pct,
        page_types,
    }
}

#[cfg(test)]
mod build_summary_tests {
    use super::*;
    use crate::pdf::word_extractor::WordSpan;

    fn page(page_num: u32, line_count: u32, is_scanned: bool, page_type: &str) -> PageRow {
        PageRow {
            page: page_num,
            line_count,
            column_k_used: 1,
            column_silhouette: None,
            is_scanned,
            page_type: page_type.to_string(),
        }
    }

    fn word_with_bbox() -> WordSpan {
        WordSpan {
            text: "hi".to_string(),
            page: 1,
            bbox: Some([0, 0, 10, 10]),
        }
    }

    fn word_without_bbox() -> WordSpan {
        WordSpan {
            text: "hi".to_string(),
            page: 1,
            bbox: None,
        }
    }

    #[test]
    fn empty_inputs_give_zero_counts_and_no_coverage() {
        let s = build_summary(&[], &[]);
        assert_eq!(s.page_count, 0);
        assert_eq!(s.scanned_page_count, 0);
        assert_eq!(s.total_lines, 0);
        assert_eq!(s.bbox_coverage_pct, None);
        // All four canonical keys present with zero counts so the UI palette
        // can render every badge even on an empty document.
        for key in ["cover", "toc", "body", "appendix"] {
            assert_eq!(s.page_types.get(key).copied(), Some(0), "key={key}");
        }
    }

    #[test]
    fn scanned_count_and_total_lines_aggregate_across_pages() {
        let pages = vec![
            page(1, 12, false, "body"),
            page(2, 20, true, "body"),
            page(3, 8, true, "appendix"),
        ];
        let s = build_summary(&pages, &[]);
        assert_eq!(s.page_count, 3);
        assert_eq!(s.scanned_page_count, 2);
        assert_eq!(s.total_lines, 40);
    }

    #[test]
    fn page_type_aggregation_fills_missing_canonical_keys() {
        // Only body pages present — cover/toc/appendix should still appear
        // as zero entries so the frontend doesn't have to special-case.
        let pages = vec![page(1, 5, false, "body"), page(2, 5, false, "body")];
        let s = build_summary(&pages, &[]);
        assert_eq!(s.page_types.get("body").copied(), Some(2));
        assert_eq!(s.page_types.get("cover").copied(), Some(0));
        assert_eq!(s.page_types.get("toc").copied(), Some(0));
        assert_eq!(s.page_types.get("appendix").copied(), Some(0));
    }

    #[test]
    fn unknown_page_type_string_is_preserved_alongside_canonical_keys() {
        // A future page-type value should pass through, not get dropped.
        let pages = vec![page(1, 3, false, "frontmatter")];
        let s = build_summary(&pages, &[]);
        assert_eq!(s.page_types.get("frontmatter").copied(), Some(1));
        // Canonical keys still get filled in too.
        assert_eq!(s.page_types.get("body").copied(), Some(0));
    }

    #[test]
    fn bbox_coverage_is_full_when_every_word_has_bbox() {
        let pages = vec![page(1, 1, false, "body")];
        let words = vec![word_with_bbox(), word_with_bbox(), word_with_bbox()];
        let s = build_summary(&pages, &words);
        assert_eq!(s.bbox_coverage_pct, Some(100.0));
    }

    #[test]
    fn bbox_coverage_is_partial_when_some_words_lack_bbox() {
        let pages = vec![page(1, 1, false, "body")];
        // 1 of 4 words has a bbox → 25%.
        let words = vec![
            word_with_bbox(),
            word_without_bbox(),
            word_without_bbox(),
            word_without_bbox(),
        ];
        let s = build_summary(&pages, &words);
        let pct = s.bbox_coverage_pct.expect("Some when words present");
        assert!((pct - 25.0).abs() < 0.01, "expected ~25.0, got {pct}");
    }

    #[test]
    fn bbox_coverage_is_zero_when_no_word_has_bbox() {
        // Distinguishes "full text-only fallback" (0%) from "empty document"
        // (None) — the UI shows them differently.
        let pages = vec![page(1, 1, true, "body")];
        let words = vec![word_without_bbox(), word_without_bbox()];
        let s = build_summary(&pages, &words);
        assert_eq!(s.bbox_coverage_pct, Some(0.0));
    }
}
