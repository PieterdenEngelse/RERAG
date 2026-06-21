// src/pdf/ir_builder.rs — Stage 4: assemble DocIR from classified word spans.
//
// Groups consecutive same-tagged word spans into candidate blocks.
// Tables get routed through TableModel for row/col structure.
// Footers and "Other" tags are dropped (noise suppression analogous to dedupe_pdf_noise).

use super::layout_model::RegionTag;
use super::table_model::TableModel;
use super::word_extractor::WordSpan;
use crate::doc_ir::{BlockType, BoundingBox, ColumnPosition, DocBlock, DocIR};

pub fn build_ir(
    source: &str,
    words: &[WordSpan],
    tags: &[RegionTag],
    table_model: &TableModel,
    word_columns: Option<&[ColumnPosition]>,
) -> DocIR {
    assert_eq!(words.len(), tags.len(), "words/tags length mismatch");
    if let Some(cols) = word_columns {
        assert_eq!(words.len(), cols.len(), "words/columns length mismatch");
    }

    let mut ir = DocIR::new(source, "pdf");

    if words.is_empty() {
        return ir;
    }

    // --- Group into runs of equal RegionTag ---
    let mut run_start = 0usize;
    while run_start < words.len() {
        let current_tag = tags[run_start];

        // Skip noise tags
        if current_tag == RegionTag::Other || current_tag == RegionTag::Footer {
            run_start += 1;
            continue;
        }

        // Advance run_end while tag is the same. When column info is
        // available, also stop on same-page cross-column transitions —
        // otherwise the heuristic baseline (every word tagged Text) emits
        // one giant block per page that mixes left and right column words,
        // collapses to `Multi` in `aggregate_column`, and gives the
        // column-aware chunker nothing to split on.
        let mut run_end = run_start + 1;
        while run_end < words.len() && tags[run_end] == current_tag {
            if let Some(cols) = word_columns {
                let prev_col = cols[run_end - 1];
                let next_col = cols[run_end];
                let same_page = words[run_end - 1].page == words[run_end].page;
                let crossing = matches!(
                    (prev_col, next_col),
                    (ColumnPosition::Col(a), ColumnPosition::Col(b)) if a != b
                );
                if same_page && crossing {
                    break;
                }
            }
            run_end += 1;
        }

        let run_words = &words[run_start..run_end];
        let run_text = run_words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let page = run_words
            .first()
            .and_then(|w| if w.page > 0 { Some(w.page) } else { None });
        let bbox = run_bbox(run_words);

        let mut block: DocBlock = match current_tag {
            RegionTag::Title => DocBlock::header(1, run_text),
            RegionTag::SectionHeader => DocBlock::header(2, run_text),
            RegionTag::Table => {
                let structure = table_model.structure(run_words);
                let markdown = if structure.markdown.is_empty() {
                    run_text.clone()
                } else {
                    structure.markdown.clone()
                };
                let mut b = DocBlock::text(run_text);
                b.block_type = BlockType::Table {
                    rows: structure.rows as usize,
                    cols: structure.cols as usize,
                };
                b.markdown = Some(markdown);
                b
            }
            RegionTag::Figure => {
                let mut b = DocBlock::text(String::new());
                b.block_type = BlockType::Image {
                    alt: Some(run_text),
                };
                b
            }
            RegionTag::Caption => {
                let mut b = DocBlock::text(run_text);
                b.block_type = BlockType::Caption;
                b
            }
            RegionTag::List => {
                let mut b = DocBlock::text(run_text);
                b.block_type = BlockType::List { ordered: false };
                b
            }
            RegionTag::Header => DocBlock::header(3, run_text),
            // Text and anything else → plain Text block
            _ => DocBlock::text(run_text),
        };

        block.page = page;
        block.bbox = bbox;

        if let Some(cols) = word_columns {
            let col = aggregate_column(&cols[run_start..run_end]);
            block
                .metadata
                .insert("column_position".to_string(), col.as_str().to_string());
        }

        ir.push(block);

        run_start = run_end;
    }

    ir
}

/// Collapse the column tags of a run of words into a single value. If every
/// word agrees on one non-Multi column, the run gets that column. Mixed runs
/// (different left/right within one DocBlock) collapse to `Multi` —
/// downstream the column-aware chunker treats `Multi` as "don't trust this
/// for column-pure chunking."
fn aggregate_column(cols: &[ColumnPosition]) -> ColumnPosition {
    let mut seen: Option<ColumnPosition> = None;
    for &c in cols {
        match (seen, c) {
            (None, c) => seen = Some(c),
            (Some(prev), c) if prev == c => {}
            _ => return ColumnPosition::Multi,
        }
    }
    seen.unwrap_or(ColumnPosition::Multi)
}

/// Compute the bounding box that spans all words in a run.
fn run_bbox(words: &[WordSpan]) -> Option<BoundingBox> {
    let with_bbox: Vec<[i64; 4]> = words.iter().filter_map(|w| w.bbox).collect();
    if with_bbox.is_empty() {
        return None;
    }
    let page = words.first()?.page;
    let x0 = with_bbox.iter().map(|b| b[0]).min()? as f32;
    let y0 = with_bbox.iter().map(|b| b[1]).min()? as f32;
    let x1 = with_bbox.iter().map(|b| b[2]).max()? as f32;
    let y1 = with_bbox.iter().map(|b| b[3]).max()? as f32;
    Some(BoundingBox {
        page,
        x0,
        y0,
        x1,
        y1,
    })
}
