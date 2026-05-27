// src/pdf/native_extractor.rs — DocExtractor impl: full native PDF pipeline.
//
// Priority in registry (set by main.rs):
//   Docling sidecar (if up) > NativePdfExtractor > built-in pdftotext
//
// Graceful degradation at each stage:
//   lopdf bbox fail    → text-only words (bboxes = None)
//   candle model fail  → heuristic region tagger
//   ORT model fail     → text-mode table clustering

use super::ir_builder::build_ir;
use super::layout_model::LayoutModel;
use super::table_model::TableModel;
use super::word_extractor::extract_words;
use crate::doc_ir::DocIR;
use crate::extractor::DocExtractor;
use crate::mime_detect::ContentType;
use tracing::{debug, warn};

pub struct NativePdfExtractor;

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

        // Stage 4: assemble DocIR
        let mut ir = build_ir(filename, &words, &tags, table_model);
        ir.tag_extractor("native_pdf");

        let char_count: usize = ir.blocks.iter().map(|b| b.text.len()).sum();
        if char_count == 0 {
            anyhow::bail!("native_pdf: extracted zero chars from '{}'", filename);
        }

        Ok(ir)
    }
}
