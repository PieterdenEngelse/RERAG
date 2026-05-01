// src/pdf/table_model.rs — Stage 3b: table structure recognition.
//
// Two-path design:
//   1. ORT TableFormer: microsoft/table-transformer-structure-recognition ONNX.
//      Loaded lazily; requires ONNX model file.
//   2. Text-mode clustering: pure-Rust y-coordinate clustering → rows/cols.
//      Used when TableFormer model is unavailable.

use super::word_extractor::WordSpan;
use std::sync::OnceLock;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct TableStructure {
    pub rows: u32,
    pub cols: u32,
    /// Markdown representation: "| A | B |\n|---|---|\n| 1 | 2 |"
    pub markdown: String,
}

pub struct TableModel {
    _inner: TableModelInner,
}

enum TableModelInner {
    #[allow(dead_code)]
    Ort(OrtTableFormer),
    TextMode,
}

static MODEL: OnceLock<TableModel> = OnceLock::new();

impl TableModel {
    pub fn load_or_text() -> &'static TableModel {
        MODEL.get_or_init(|| match load_ort_model() {
            Ok(m) => {
                info!("TableFormer ORT model loaded");
                TableModel {
                    _inner: TableModelInner::Ort(m),
                }
            }
            Err(e) => {
                warn!(error = %e, "TableFormer ORT model unavailable, using text-mode clustering");
                TableModel {
                    _inner: TableModelInner::TextMode,
                }
            }
        })
    }

    pub fn is_ort_loaded(&self) -> bool {
        matches!(self._inner, TableModelInner::Ort(_))
    }

    pub fn structure(&self, table_words: &[WordSpan]) -> TableStructure {
        match &self._inner {
            TableModelInner::Ort(m) => m
                .structure(table_words)
                .unwrap_or_else(|e| {
                    debug!(error = %e, "ORT TableFormer failed, falling back to text-mode");
                    text_mode_structure(table_words)
                }),
            TableModelInner::TextMode => text_mode_structure(table_words),
        }
    }
}

// ── ORT TableFormer ────────────────────────────────────────────────────────────

struct OrtTableFormer {
    #[allow(dead_code)]
    session: ort::session::Session,
}

fn load_ort_model() -> anyhow::Result<OrtTableFormer> {
    use ort::session::builder::GraphOptimizationLevel;

    let model_path = std::env::var("TABLE_FORMER_MODEL_PATH").map_err(|_| {
        anyhow::anyhow!(
            "TABLE_FORMER_MODEL_PATH not set. Download from: \
             huggingface.co/microsoft/table-transformer-structure-recognition"
        )
    })?;

    if !std::path::Path::new(&model_path).exists() {
        anyhow::bail!("TableFormer model not found at {}", model_path);
    }

    let session = ort::session::Session::builder()
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .commit_from_file(&model_path)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(OrtTableFormer { session })
}

impl OrtTableFormer {
    fn structure(&self, words: &[WordSpan]) -> anyhow::Result<TableStructure> {
        // TableFormer requires an image patch of the table region.
        // Until PDF rendering is available, fall back to text-mode.
        debug!("ORT TableFormer: image rendering not yet available, using text-mode");
        Ok(text_mode_structure(words))
    }
}

// ── Text-mode clustering ────────────────────────────────────────────────────

/// Cluster table words into rows by y-coordinate proximity, then columns by
/// x-coordinate, and render as a Markdown grid.
pub fn text_mode_structure(words: &[WordSpan]) -> TableStructure {
    if words.is_empty() {
        return TableStructure {
            rows: 0,
            cols: 0,
            markdown: String::new(),
        };
    }

    // --- Row detection: cluster by y0 with tolerance 30 units ---
    let mut rows: Vec<Vec<&WordSpan>> = Vec::new();

    for word in words {
        let y = word.bbox.map(|b| b[1]).unwrap_or(0);
        if let Some(row) = rows.last_mut() {
            let row_y = row[0].bbox.map(|b| b[1]).unwrap_or(0);
            if (y - row_y).abs() <= 30 {
                row.push(word);
                continue;
            }
        }
        rows.push(vec![word]);
    }

    // Sort each row by x0
    for row in &mut rows {
        row.sort_by_key(|w| w.bbox.map(|b| b[0]).unwrap_or(0));
    }

    if rows.is_empty() {
        return TableStructure {
            rows: 0,
            cols: 0,
            markdown: String::new(),
        };
    }

    // --- Column detection: how many cells per row? use max ---
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let num_rows = rows.len() as u32;

    // --- Render Markdown grid ---
    let mut md = String::new();
    for (ri, row) in rows.iter().enumerate() {
        // Pad row to num_cols
        let mut cells: Vec<String> = row.iter().map(|w| w.text.clone()).collect();
        while cells.len() < num_cols {
            cells.push(String::new());
        }
        let line: String = cells.iter().map(|c| format!("| {} ", c)).collect();
        md.push_str(&line);
        md.push_str("|\n");
        // Header separator after first row
        if ri == 0 {
            let sep: String = (0..num_cols).map(|_| "|---").collect();
            md.push_str(&sep);
            md.push_str("|\n");
        }
    }

    TableStructure {
        rows: num_rows,
        cols: num_cols as u32,
        markdown: md,
    }
}
