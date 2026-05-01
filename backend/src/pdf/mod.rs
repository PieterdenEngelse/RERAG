// src/pdf/mod.rs — Native in-process PDF extraction pipeline.
//
// Feature-gated behind `layout_ml`.  When the feature is compiled in and
// LAYOUT_ML_ENABLED=true is set at runtime, `NativePdfExtractor` registers
// in the DocExtractor registry at priority below Docling but above the
// built-in pdftotext path.
//
// Pipeline: Extractous (text) → lopdf (bboxes) → LayoutXLM/heuristic (region
// classification) → TableFormer (table structure) → DocIR builder.

#[cfg(feature = "layout_ml")]
pub mod word_extractor;
#[cfg(feature = "layout_ml")]
pub mod layout_model;
#[cfg(feature = "layout_ml")]
pub mod table_model;
#[cfg(feature = "layout_ml")]
pub mod ir_builder;
#[cfg(feature = "layout_ml")]
pub mod native_extractor;
