// src/pdf/line_grouper.rs — Stage 1.5: group word spans into line spans.
//
// Sits between word_extractor (Stage 1) and column_detect (then ir_builder).
// Words on the same page whose y-bands overlap AND whose x-positions are
// close enough collapse into one LineSpan; the line carries the joined text,
// the union bbox, and a back-pointer (word range) so downstream stages can
// map words → line → column_position.
//
// Why the x-gap check matters: a typical two-column PDF emits words in
// reading order (left-line-1, right-line-1, left-line-2, right-line-2, …).
// Without the x-aware split, the y-band check alone merges a left-column
// word and a right-column word at the same y into one LineSpan whose bbox
// spans both columns — and column_detect then places that line in the page
// centroid and tags it Multi. The x-gap threshold separates "natural
// inter-word space" (≤ ~30 units in the 0..1000 normalised frame) from
// "column gutter" (≥ ~40 units).
//
// Words missing bboxes (extractous fallback) flow through unchanged: each
// becomes its own LineSpan with bbox=None and contributes nothing to the
// column classification.

use super::word_extractor::WordSpan;

/// Minimum horizontal gap between a running line's bbox and the next word's
/// bbox that we treat as "different column" rather than "wide inter-word
/// space". Tuned for the lopdf word extractor's 0..1000 normalised frame.
pub const X_GAP_LINE_BREAK: i64 = 40;

/// One line on one page.
#[derive(Debug, Clone)]
pub struct LineSpan {
    pub page: u32,
    pub line_idx: u32,
    pub text: String,
    /// Union bbox over the words in `word_range`. None when all source words
    /// had no bbox (extractous fallback path).
    pub bbox: Option<[i64; 4]>,
    /// Half-open `[start, end)` indices into the original `WordSpan` slice.
    pub word_range: std::ops::Range<usize>,
}

impl LineSpan {
    /// Horizontal centroid (x0) for column clustering. None when bbox is None.
    pub fn x0(&self) -> Option<i64> {
        self.bbox.map(|b| b[0])
    }
}

/// Group `words` into per-page lines.
///
/// Algorithm: walk words in document order; start a new line whenever the
/// page changes or the current word's vertical band fails to overlap the
/// running line's band. The y-tolerance is derived from the line's existing
/// height so multi-size lines (a heading next to body text) still cluster
/// correctly.
pub fn group_words_into_lines(words: &[WordSpan]) -> Vec<LineSpan> {
    if words.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<LineSpan> = Vec::new();
    let mut cur_start = 0usize;
    let mut cur_page = words[0].page;
    let mut cur_bbox: Option<[i64; 4]> = words[0].bbox;
    let mut cur_text = String::new();
    let mut page_line_counts: std::collections::HashMap<u32, u32> =
        std::collections::HashMap::new();

    let push_line = |lines: &mut Vec<LineSpan>,
                     page: u32,
                     bbox: Option<[i64; 4]>,
                     range: std::ops::Range<usize>,
                     text: String,
                     counts: &mut std::collections::HashMap<u32, u32>| {
        let idx = counts.entry(page).or_insert(0);
        let line_idx = *idx;
        *idx += 1;
        lines.push(LineSpan {
            page,
            line_idx,
            text,
            bbox,
            word_range: range,
        });
    };

    for (i, w) in words.iter().enumerate() {
        let starts_new_line = if w.page != cur_page {
            true
        } else {
            match (cur_bbox, w.bbox) {
                (Some(c), Some(b)) => !y_bands_overlap(c, b) || x_gap_too_large(c, b),
                // No bbox info on at least one side — keep accumulating
                // by document order (extractous fallback path).
                _ => false,
            }
        };

        if starts_new_line && i > cur_start {
            push_line(
                &mut lines,
                cur_page,
                cur_bbox,
                cur_start..i,
                std::mem::take(&mut cur_text),
                &mut page_line_counts,
            );
            cur_start = i;
            cur_page = w.page;
            cur_bbox = w.bbox;
        } else {
            cur_bbox = union_bbox(cur_bbox, w.bbox);
        }

        if !cur_text.is_empty() {
            cur_text.push(' ');
        }
        cur_text.push_str(&w.text);
    }

    if cur_start < words.len() {
        push_line(
            &mut lines,
            cur_page,
            cur_bbox,
            cur_start..words.len(),
            cur_text,
            &mut page_line_counts,
        );
    }

    lines
}

/// True when the candidate word's bbox is horizontally separated from the
/// running line's bbox by more than `X_GAP_LINE_BREAK`. Catches both
/// directions:
///   * word starts well to the right of the line's current right edge —
///     this is the common "reading order across columns" case;
///   * word ends well to the left of the line's current left edge — this
///     covers a left-column word arriving after a right-column word on a
///     later line that y-overlapped the running line.
fn x_gap_too_large(line_bbox: [i64; 4], word_bbox: [i64; 4]) -> bool {
    let (line_x0, line_x1) = (line_bbox[0], line_bbox[2]);
    let (word_x0, word_x1) = (word_bbox[0], word_bbox[2]);
    word_x0 > line_x1 + X_GAP_LINE_BREAK || word_x1 + X_GAP_LINE_BREAK < line_x0
}

fn y_bands_overlap(a: [i64; 4], b: [i64; 4]) -> bool {
    let (a_y0, a_y1) = (a[1].min(a[3]), a[1].max(a[3]));
    let (b_y0, b_y1) = (b[1].min(b[3]), b[1].max(b[3]));
    let overlap_lo = a_y0.max(b_y0);
    let overlap_hi = a_y1.min(b_y1);
    if overlap_hi <= overlap_lo {
        return false;
    }
    let overlap = (overlap_hi - overlap_lo) as f64;
    let h_a = (a_y1 - a_y0) as f64;
    let h_b = (b_y1 - b_y0) as f64;
    let min_h = h_a.min(h_b).max(1.0);
    overlap / min_h >= 0.5
}

fn union_bbox(a: Option<[i64; 4]>, b: Option<[i64; 4]>) -> Option<[i64; 4]> {
    match (a, b) {
        (Some(a), Some(b)) => Some([
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, page: u32, bbox: Option<[i64; 4]>) -> WordSpan {
        WordSpan {
            text: text.into(),
            page,
            bbox,
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(group_words_into_lines(&[]).is_empty());
    }

    #[test]
    fn same_y_band_collapses_to_one_line() {
        let words = vec![
            w("Hello", 1, Some([10, 100, 60, 120])),
            w("world", 1, Some([70, 100, 130, 120])),
        ];
        let lines = group_words_into_lines(&words);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello world");
        assert_eq!(lines[0].word_range, 0..2);
        assert_eq!(lines[0].bbox, Some([10, 100, 130, 120]));
    }

    #[test]
    fn distinct_y_bands_split_into_two_lines() {
        let words = vec![
            w("Top", 1, Some([10, 100, 60, 120])),
            w("Bottom", 1, Some([10, 200, 60, 220])),
        ];
        let lines = group_words_into_lines(&words);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_idx, 0);
        assert_eq!(lines[1].line_idx, 1);
    }

    #[test]
    fn page_break_starts_new_line() {
        let words = vec![
            w("A", 1, Some([10, 100, 30, 120])),
            w("B", 2, Some([10, 100, 30, 120])),
        ];
        let lines = group_words_into_lines(&words);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].page, 1);
        assert_eq!(lines[1].page, 2);
        // Per-page line_idx resets.
        assert_eq!(lines[1].line_idx, 0);
    }

    #[test]
    fn reading_order_across_two_columns_splits_per_column() {
        // Same-y words from left column and right column come in reading
        // order: L1 word A, R1 word B, then L2 word C, R2 word D one line
        // down. Without the x-aware split, line_grouper would have merged
        // (A,B) and (C,D) into one wide line each. With the split, each
        // column gets its own line.
        let words = vec![
            w("A", 1, Some([50, 100, 110, 120])),  // left col, line 1
            w("B", 1, Some([550, 100, 610, 120])), // right col, line 1
            w("C", 1, Some([50, 130, 110, 150])),  // left col, line 2
            w("D", 1, Some([550, 130, 610, 150])), // right col, line 2
        ];
        let lines = group_words_into_lines(&words);
        assert_eq!(
            lines.len(),
            4,
            "expected 4 column-pure lines, got {}",
            lines.len()
        );
        assert_eq!(lines[0].text, "A");
        assert_eq!(lines[1].text, "B");
        assert_eq!(lines[2].text, "C");
        assert_eq!(lines[3].text, "D");
    }

    #[test]
    fn small_inter_word_gap_does_not_split() {
        // Words within the same column with normal inter-word spacing
        // (< X_GAP_LINE_BREAK) must stay on one line.
        let words = vec![
            w("Renewal", 1, Some([50, 100, 110, 120])),
            w("fee", 1, Some([120, 100, 145, 120])), // gap = 10
        ];
        let lines = group_words_into_lines(&words);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Renewal fee");
    }

    #[test]
    fn missing_bboxes_flow_through_in_order() {
        let words = vec![w("A", 1, None), w("B", 1, None), w("C", 2, None)];
        let lines = group_words_into_lines(&words);
        // Same page + no bbox info → one line. Page change starts a new line.
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "A B");
        assert_eq!(lines[1].text, "C");
        assert!(lines[0].bbox.is_none());
    }
}
