//! Phase 1 relational PDF extraction integration tests.
//!
//! Two kinds of coverage:
//!
//! 1. **Synthetic WordSpan tests** exercise the line_grouper → column_detect
//!    → ir_builder → chunk_ir pipeline with hand-built `WordSpan` inputs.
//!    Fast, deterministic, no fixture dependency.
//! 2. **Real-PDF test** feeds the bundled `two_column_invoice.pdf` fixture
//!    through `word_extractor::extract_words` (lopdf) so the full Stage 0–2
//!    chain is covered end-to-end on a real PDF byte stream — including
//!    the parts the synthetic tests can't reach (PostScript content-stream
//!    parsing, bbox normalisation, word ordering).

#![cfg(feature = "layout_ml")]

use ag::doc_ir::ColumnPosition;
use ag::memory::chunker::ChunkerConfig;
use ag::memory::chunker_factory::{chunk_ir, FixedChunker};
use ag::pdf::column_detect::assign_columns;
use ag::pdf::ir_builder::build_ir;
use ag::pdf::layout_model::RegionTag;
use ag::pdf::line_grouper::group_words_into_lines;
use ag::pdf::table_model::TableModel;
use ag::pdf::word_extractor::{extract_words, WordSpan};

/// Build a synthetic two-column invoice: left column at x0=50, right column
/// at x0=550, 6 lines per column on a single page.
fn synth_invoice_words() -> Vec<WordSpan> {
    let mut words = Vec::new();
    // Left column: descriptive labels
    let left_labels = [
        "Renewal fee",
        "Late payment fee",
        "Cancellation fee",
        "Reinstatement fee",
        "Document copy fee",
        "Account closure fee",
    ];
    for (i, label) in left_labels.iter().enumerate() {
        let y = 100 + (i as i64) * 30;
        let mut x = 50i64;
        for word in label.split_whitespace() {
            let w = (word.len() as i64) * 8;
            words.push(WordSpan {
                text: word.to_string(),
                page: 1,
                bbox: Some([x, y, x + w, y + 18]),
            });
            x += w + 4;
        }
    }
    // Right column: amounts (aligned to a different y so line_grouper
    // separates them — we want each amount to be its own line in the
    // right column, paired with the corresponding label by y-position).
    let right_amounts = [
        "EUR 200", "EUR 75", "EUR 150", "EUR 50", "EUR 10", "EUR 100",
    ];
    for (i, amount) in right_amounts.iter().enumerate() {
        let y = 100 + (i as i64) * 30;
        let mut x = 550i64;
        for word in amount.split_whitespace() {
            let w = (word.len() as i64) * 8;
            words.push(WordSpan {
                text: word.to_string(),
                page: 1,
                bbox: Some([x, y, x + w, y + 18]),
            });
            x += w + 4;
        }
    }
    words
}

#[test]
fn two_column_words_produce_column_pure_chunks() {
    let words = synth_invoice_words();
    let lines = group_words_into_lines(&words);

    // Each label / amount should land in its own line.
    assert!(
        lines.len() >= 12,
        "expected >=12 lines from 12 label+amount groups, got {}",
        lines.len()
    );

    let columns = assign_columns(&lines);

    // The k=2 split should be confident.
    let s = columns.pages[0]
        .silhouette
        .expect("silhouette should be computed for >= MIN_LINES_FOR_KMEANS lines");
    assert!(s > 0.30, "expected silhouette > 0.30, got {}", s);

    // Build the per-word column map exactly like NativePdfExtractor does.
    let mut word_columns = vec![ColumnPosition::Multi; words.len()];
    for (line, col) in lines.iter().zip(columns.positions.iter()) {
        for wi in line.word_range.clone() {
            word_columns[wi] = *col;
        }
    }

    // All words classified as Text (heuristic baseline) so build_ir doesn't
    // split them into headers / tables.
    let tags = vec![RegionTag::Text; words.len()];
    let table_model = TableModel::load_or_text();
    let ir = build_ir(
        "invoice.pdf",
        &words,
        &tags,
        table_model,
        Some(&word_columns),
    );

    // DocIR should carry column_position metadata on at least one block of
    // each column. Adaptive-k labels left-to-right as Col(0), Col(1), ….
    let any_col0 = ir
        .blocks
        .iter()
        .any(|b| b.metadata.get("column_position").map(|s| s.as_str()) == Some("col0"));
    let any_col1 = ir
        .blocks
        .iter()
        .any(|b| b.metadata.get("column_position").map(|s| s.as_str()) == Some("col1"));
    assert!(any_col0, "expected at least one Col(0) block in IR");
    assert!(any_col1, "expected at least one Col(1) block in IR");

    // Run the column-aware chunker. Each chunk must be column-pure (no
    // mixing of different non-Multi columns).
    let chunker = FixedChunker::new(ChunkerConfig::default());
    let chunks = chunk_ir(&ir, &chunker);

    for (text, meta) in &chunks {
        let has_col0 = meta.column_position_set.contains(&ColumnPosition::Col(0));
        let has_col1 = meta.column_position_set.contains(&ColumnPosition::Col(1));
        assert!(
            !(has_col0 && has_col1),
            "chunk '{}' mixes Col(0) and Col(1) content (column_position_set={:?})",
            text,
            meta.column_position_set
        );
    }

    // And at least one chunk should carry only Col(1) content — that's the
    // chunk a renewal-fee BM25 query would land on.
    let any_pure_col1 = chunks.iter().any(|(_, m)| {
        m.column_position_set.contains(&ColumnPosition::Col(1))
            && !m.column_position_set.contains(&ColumnPosition::Col(0))
    });
    assert!(
        any_pure_col1,
        "expected at least one Col(1)-only chunk so a 'renewal fee' query \
         can land on right-column-only content"
    );
}

#[test]
fn missing_bboxes_degrade_gracefully_to_multi() {
    // extractous fallback path: every WordSpan has bbox=None.
    let words: Vec<WordSpan> = ["Hello", "world", "of", "PDFs"]
        .iter()
        .map(|w| WordSpan {
            text: (*w).to_string(),
            page: 1,
            bbox: None,
        })
        .collect();

    let lines = group_words_into_lines(&words);
    let columns = assign_columns(&lines);

    // No bbox → no clustering → every line tagged Single (because
    // line_count < MIN_LINES_FOR_KMEANS) or Multi (when bbox missing). Both
    // are acceptable: the requirement is that we never crash and never
    // pretend to know columns we can't measure.
    for col in &columns.positions {
        assert!(
            matches!(col, ColumnPosition::Single | ColumnPosition::Multi),
            "expected Single or Multi for bbox-less input, got {:?}",
            col
        );
    }
}

/// End-to-end on the bundled fixture PDF (the same one shipped via
/// `db::pdf_rows::DEMO_PDF_BYTES` and ingested at boot). This exercises
/// `word_extractor::extract_words` (lopdf content-stream parsing) which the
/// synthetic tests above can't reach.
///
/// The fixture is a hand-written PostScript document: 1 header line,
/// 1 sub-header, 1 "Schedule of charges" line, 6 left-column labels at
/// x≈60pt, 6 right-column amounts at x≈400pt, 1 footer at x≈60pt.
#[test]
fn real_fixture_pdf_splits_body_into_left_and_right_columns() {
    const FIXTURE: &[u8] = include_bytes!("fixtures/pdf/two_column_invoice.pdf");

    let words = extract_words(FIXTURE).expect("lopdf must parse the bundled fixture");
    assert!(
        !words.is_empty(),
        "lopdf returned 0 words on the fixture — either the PDF is malformed \
         or lopdf is misconfigured"
    );
    let words_with_bbox = words.iter().filter(|w| w.bbox.is_some()).count();
    assert!(
        words_with_bbox > 0,
        "all {} words have bbox=None — lopdf fell through to extractous, \
         which means the column detector cannot run on this fixture",
        words.len()
    );

    let lines = group_words_into_lines(&words);
    let columns = assign_columns(&lines);

    // Adaptive k should land on 2 here — the body has 6+6 lines neatly
    // split at x=60 vs x=400, swamping the 3 header lines and 1 footer
    // line that are also at x=60.
    assert_eq!(columns.pages.len(), 1, "fixture is single-page");
    assert_eq!(
        columns.pages[0].k_used, 2,
        "two-column body should drive adaptive-k to k=2"
    );
    let s = columns.pages[0]
        .silhouette
        .expect("silhouette should be computed for a 14+-line page");
    assert!(s > 0.30, "silhouette {} must clear the 0.30 threshold", s);

    // Confirm both columns are actually represented in line classifications.
    let cols_seen: std::collections::BTreeSet<u8> = columns
        .positions
        .iter()
        .filter_map(|c| match c {
            ColumnPosition::Col(n) => Some(*n),
            _ => None,
        })
        .collect();
    assert!(
        cols_seen.contains(&0) && cols_seen.contains(&1),
        "expected both Col(0) and Col(1) in line classifications, got {:?}",
        cols_seen
    );

    // The amounts ("EUR 200", "EUR 75", etc.) are in the right column. At
    // least one line containing "EUR" should be tagged Col(1).
    let any_eur_in_col1 = lines
        .iter()
        .zip(columns.positions.iter())
        .any(|(l, c)| matches!(c, ColumnPosition::Col(1)) && l.text.contains("EUR"));
    assert!(
        any_eur_in_col1,
        "at least one EUR-amount line must land in the right column (Col(1))"
    );

    // And conversely: at least one label-only line (no "EUR") should land
    // in Col(0). "Renewal fee", "Late payment fee", etc.
    let any_label_in_col0 = lines.iter().zip(columns.positions.iter()).any(|(l, c)| {
        matches!(c, ColumnPosition::Col(0)) && l.text.contains("fee") && !l.text.contains("EUR")
    });
    assert!(
        any_label_in_col0,
        "at least one 'fee' label line must land in the left column (Col(0))"
    );

    // Run the column-aware chunker end-to-end. Every chunk must be
    // column-pure (no chunk contains both Col(0) and Col(1) content).
    let mut word_columns = vec![ColumnPosition::Multi; words.len()];
    for (line, col) in lines.iter().zip(columns.positions.iter()) {
        for wi in line.word_range.clone() {
            word_columns[wi] = *col;
        }
    }
    let tags = vec![RegionTag::Text; words.len()];
    let ir = build_ir(
        "two_column_invoice.pdf",
        &words,
        &tags,
        TableModel::load_or_text(),
        Some(&word_columns),
    );
    let chunker = FixedChunker::new(ChunkerConfig::default());
    let chunks = chunk_ir(&ir, &chunker);

    for (text, meta) in &chunks {
        let has_col0 = meta.column_position_set.contains(&ColumnPosition::Col(0));
        let has_col1 = meta.column_position_set.contains(&ColumnPosition::Col(1));
        assert!(
            !(has_col0 && has_col1),
            "chunk mixes Col(0) and Col(1) content — column-aware chunker \
             failed to flush at the boundary. chunk='{}' set={:?}",
            text,
            meta.column_position_set
        );
    }
}
