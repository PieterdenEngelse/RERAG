// src/doc_ir.rs — Shared Intermediate Representation for all document extractors.
//
// Every extractor (built-in or external) converts its native format into DocIR.
// The chunker consumes DocIR instead of flat text, so it can respect structural
// boundaries (headers flush chunks, tables and code blocks are never split).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

/// Which column of a multi-column page a line / block / chunk belongs to.
///
/// `Single` — the page is a single-column layout (no split).
/// `Col(i)` — 0-based index in left-to-right order, output by adaptive-k
///            column detection (k ∈ 2..=6 chosen by silhouette).
/// `Multi`  — either no k clears the silhouette threshold, the bbox was
///            missing, or the block spans column boundaries (i.e. "ambiguous,
///            don't trust this for column-pure chunking").
///
/// Wire format (SQLite `pdf_lines.column_position`, DocBlock metadata,
/// JSON): `"single"`, `"col0"`, `"col1"`, …, `"multi"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColumnPosition {
    Single,
    Col(u8),
    Multi,
}

impl ColumnPosition {
    pub fn as_str(self) -> String {
        match self {
            ColumnPosition::Single => "single".to_string(),
            ColumnPosition::Col(i) => format!("col{}", i),
            ColumnPosition::Multi => "multi".to_string(),
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "single" => Some(ColumnPosition::Single),
            "multi" => Some(ColumnPosition::Multi),
            other => other
                .strip_prefix("col")
                .and_then(|n| n.parse::<u8>().ok())
                .map(ColumnPosition::Col),
        }
    }
}

impl Serialize for ColumnPosition {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for ColumnPosition {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <String as Deserialize>::deserialize(d)?;
        ColumnPosition::from_str_opt(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid ColumnPosition: {}", s)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BlockType {
    Text,
    Header { level: u8 },
    Table { rows: usize, cols: usize },
    Code { language: Option<String> },
    List { ordered: bool },
    Image { alt: Option<String> },
    Formula,
    Caption,
    Footnote,
    PageBreak,
}

impl BlockType {
    pub fn name(&self) -> &'static str {
        match self {
            BlockType::Text => "Text",
            BlockType::Header { .. } => "Header",
            BlockType::Table { .. } => "Table",
            BlockType::Code { .. } => "Code",
            BlockType::List { .. } => "List",
            BlockType::Image { .. } => "Image",
            BlockType::Formula => "Formula",
            BlockType::Caption => "Caption",
            BlockType::Footnote => "Footnote",
            BlockType::PageBreak => "PageBreak",
        }
    }

    /// Atomic blocks are emitted as a single chunk and never split.
    pub fn is_atomic(&self) -> bool {
        matches!(
            self,
            BlockType::Table { .. }
                | BlockType::Code { .. }
                | BlockType::Formula
                | BlockType::Image { .. }
        )
    }

    /// Strong-boundary blocks always flush pending text and start a new accumulation.
    pub fn is_strong_boundary(&self) -> bool {
        matches!(self, BlockType::Header { .. } | BlockType::PageBreak)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub page: u32,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocBlock {
    pub id: String,
    pub block_type: BlockType,
    /// Canonical plain text — always populated.
    pub text: String,
    /// Optional richer representation (Markdown for headers / code / tables).
    pub markdown: Option<String>,
    /// Spatial position; populated by external extractors (Docling, etc.).
    pub bbox: Option<BoundingBox>,
    /// Page number shortcut.
    pub page: Option<u32>,
    pub metadata: HashMap<String, String>,
}

impl DocBlock {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            block_type: BlockType::Text,
            text: content.into(),
            markdown: None,
            bbox: None,
            page: None,
            metadata: HashMap::new(),
        }
    }

    pub fn header(level: u8, content: impl Into<String>) -> Self {
        let t: String = content.into();
        let hashes = "#".repeat(level as usize);
        let md = format!("{} {}", hashes, t);
        Self {
            id: Uuid::new_v4().to_string(),
            block_type: BlockType::Header { level },
            text: t,
            markdown: Some(md),
            bbox: None,
            page: None,
            metadata: HashMap::new(),
        }
    }

    pub fn code(language: Option<String>, content: impl Into<String>) -> Self {
        let t: String = content.into();
        let lang = language.as_deref().unwrap_or("").to_string();
        let md = format!("```{}\n{}\n```", lang, t);
        Self {
            id: Uuid::new_v4().to_string(),
            block_type: BlockType::Code { language },
            text: t,
            markdown: Some(md),
            bbox: None,
            page: None,
            metadata: HashMap::new(),
        }
    }

    /// Marker block emitted between PDF pages so the chunker creates a
    /// fresh section_id per page (PageBreak is a strong boundary).
    /// Carries the upcoming page number for downstream consumers; text
    /// is empty because the boundary itself has no content.
    pub fn page_break(page: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            block_type: BlockType::PageBreak,
            text: String::new(),
            markdown: None,
            bbox: None,
            page: Some(page),
            metadata: HashMap::new(),
        }
    }

    pub fn table(rows: usize, cols: usize, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            block_type: BlockType::Table { rows, cols },
            text: content.into(),
            markdown: None,
            bbox: None,
            page: None,
            metadata: HashMap::new(),
        }
    }

    /// Text used for embedding: markdown when available (richer for models), else plain text.
    pub fn embed_text(&self) -> &str {
        self.markdown.as_deref().unwrap_or(&self.text)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocIR {
    pub source: String,
    pub content_type: String,
    pub blocks: Vec<DocBlock>,
    pub page_count: Option<u32>,
    pub metadata: HashMap<String, String>,
}

impl DocIR {
    pub fn new(source: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            content_type: content_type.into(),
            blocks: Vec::new(),
            page_count: None,
            metadata: HashMap::new(),
        }
    }

    /// Push a block, silently dropping empty non-atomic blocks.
    /// Strong-boundary blocks (Header, PageBreak) are also kept even when
    /// their text is empty — PageBreak in particular is empty by design
    /// and exists purely as a chunker boundary marker.
    pub fn push(&mut self, block: DocBlock) {
        if !block.text.trim().is_empty()
            || block.block_type.is_atomic()
            || block.block_type.is_strong_boundary()
        {
            self.blocks.push(block);
        }
    }

    /// Stamp every block with an extractor label (e.g. "builtin/pdf", "docling").
    /// Called after IR construction so `chunk_ir` picks up the provenance.
    /// Also records the label at the doc level so callers can recover it
    /// without iterating blocks.
    pub fn tag_extractor(&mut self, label: &str) {
        self.metadata
            .insert("extractor".to_string(), label.to_string());
        for block in &mut self.blocks {
            block
                .metadata
                .insert("extractor".to_string(), label.to_string());
        }
    }

    /// Returns the extractor label set by `tag_extractor`, falling back to the
    /// first block's metadata for older IRs, then to "external".
    pub fn extractor_tag(&self) -> &str {
        if let Some(t) = self.metadata.get("extractor") {
            return t;
        }
        if let Some(t) = self
            .blocks
            .first()
            .and_then(|b| b.metadata.get("extractor"))
        {
            return t;
        }
        "external"
    }

    /// Flatten all blocks to plain text (for Store normalization / metrics).
    pub fn to_plain_text(&self) -> String {
        self.blocks
            .iter()
            .filter(|b| !b.text.is_empty())
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Metadata attached to each chunk produced by `chunk_ir()`.
/// Survives the chunking boundary so search results can carry provenance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChunkMeta {
    /// Dominant block type of the source content ("Text", "Header", "Table", …).
    pub block_type: String,
    /// Page number of the first block in the chunk's source accumulation.
    pub page: Option<u32>,
    /// Extractor that produced the source DocIR ("builtin", "docling", "unstructured").
    pub extractor: String,
    /// Ancestor heading chain (root → leaf), e.g. ["AMD", "Financial Statements", "Cash Flows"].
    /// Empty for content before the first header.
    #[serde(default)]
    pub heading_path: Vec<String>,
    /// UUID shared by all chunks within the same heading-bounded section.
    /// Empty for content carved off before any boundary (rare; covers pre-header text).
    #[serde(default)]
    pub section_id: String,
    /// Column positions covered by the source blocks. Populated by the
    /// relational PDF extractor (see `pdf::column_detect`). Empty for
    /// non-PDF content or when the extractor didn't run. After the
    /// column-aware `chunk_ir` boundary logic this set is normally a
    /// singleton; `Multi` means low confidence on a page where columns
    /// weren't separable.
    #[serde(default)]
    pub column_position_set: BTreeSet<ColumnPosition>,
}

#[cfg(test)]
mod docir_push_tests {
    //! Regression: `DocIR::push` used to drop empty non-atomic blocks
    //! unconditionally, which silently filtered every `PageBreak` (empty
    //! text by design). That broke `pdf_paged_ir`'s per-page sectioning —
    //! the chunker never saw the boundary blocks so every PDF chunk
    //! landed in one section_id.
    use super::{BlockType, DocBlock, DocIR};

    #[test]
    fn page_break_is_kept_despite_empty_text() {
        let mut ir = DocIR::new("t.pdf", "pdf");
        ir.push(DocBlock::page_break(2));
        assert_eq!(ir.blocks.len(), 1);
        assert!(matches!(ir.blocks[0].block_type, BlockType::PageBreak));
    }

    #[test]
    fn empty_text_block_still_dropped() {
        // We didn't accidentally weaken the filter — empty non-atomic
        // non-boundary blocks (plain empty Text) still get filtered.
        let mut ir = DocIR::new("t.pdf", "pdf");
        ir.push(DocBlock::text(""));
        assert_eq!(ir.blocks.len(), 0);
    }
}
