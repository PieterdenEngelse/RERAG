// src/pdf/mod.rs — Native in-process PDF extraction pipeline.
//
// Feature-gated behind `layout_ml`.  When the feature is compiled in,
// `NativePdfExtractor` is always registered in the DocExtractor registry
// at priority below Docling but above the built-in pdftotext path.
// Whether it actually runs for a given PDF is decided per-corpus at
// extract time — see `db::corpora::effective_native_pdf_enabled`.
// LAYOUT_ML_ENABLED is the corpus default and also gates model pre-warm.
//
// Pipeline: Extractous (text) → lopdf (bboxes) → LayoutXLM/heuristic (region
// classification) → TableFormer (table structure) → DocIR builder.

#[cfg(feature = "layout_ml")]
pub mod column_detect;
#[cfg(feature = "layout_ml")]
pub mod ir_builder;
#[cfg(feature = "layout_ml")]
pub mod layout_model;
#[cfg(feature = "layout_ml")]
pub mod line_grouper;
#[cfg(feature = "layout_ml")]
pub mod native_extractor;
#[cfg(feature = "layout_ml")]
pub mod page_type;
#[cfg(feature = "layout_ml")]
pub mod table_model;
#[cfg(feature = "layout_ml")]
pub mod word_extractor;
