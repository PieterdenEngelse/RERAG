// src/pdf/column_detect.rs — Adaptive-k column classifier.
//
// For each page, runs 1-D k-means on the left-edge x-coordinate of every
// bbox-bearing line for k ∈ 2..=MAX_K. Among k's whose silhouette clears
// SILHOUETTE_THRESHOLD, picks the *smallest* k whose score is within
// PARSIMONY_EPSILON of the best — parsimony tiebreak prevents
// over-segmentation when higher k's tie on silhouette because their extra
// clusters end up empty (those empty clusters get skipped by the silhouette
// formula, so a 5-cluster split of bimodal data scores the same as k=2).
// If no k clears the threshold, the page collapses to `Multi` (we don't
// pretend to know columns we can't measure). Pages with too few
// bbox-bearing lines skip k-means and tag every line `Single`.
//
// The chosen k is persisted in `pdf_pages.column_k_used`, the mean
// silhouette in `pdf_pages.column_silhouette`. Column labels are
// 0-based left-to-right (`Col(0)` is leftmost). Downstream consumers
// (ir_builder, chunker_factory) treat any `Col(a) → Col(b), a != b`
// same-page transition as a strong boundary.

use super::line_grouper::LineSpan;
use crate::doc_ir::ColumnPosition;
use std::collections::HashMap;

/// Pages with silhouette below this score collapse to `Multi` — the
/// k-cluster split was likely spurious.
pub const SILHOUETTE_THRESHOLD: f32 = 0.30;

/// Pages with fewer than this many bbox-bearing lines skip k-means entirely
/// and tag every line as `Single`.
pub const MIN_LINES_FOR_KMEANS: usize = 4;

/// Upper bound on the number of columns the adaptive k-means considers.
/// Real PDFs almost never exceed 4 columns; 6 leaves headroom without any
/// meaningful cost (silhouette is O(n²) per page; running ~5 extra passes
/// on a few hundred lines is microseconds).
pub const MAX_K: u8 = 6;

/// Parsimony tolerance for the smallest-k tiebreak. Among k's that clear
/// `SILHOUETTE_THRESHOLD`, the smallest k whose silhouette is within this
/// many points of the best wins. Set high enough to absorb the "empty
/// cluster" tie (extra clusters in over-segmentation cost ~0 silhouette),
/// low enough that a genuine 3-column split still beats a forced 2.
pub const PARSIMONY_EPSILON: f32 = 0.05;

/// Per-page outcome.
#[derive(Debug, Clone)]
pub struct PageColumns {
    pub page: u32,
    pub line_count: u32,
    /// The chosen k. `1` for `Single` (no k-means ran); `2..=MAX_K` for
    /// adaptive picks; whatever k produced the best (failed) silhouette
    /// when the page collapses to `Multi`.
    pub k_used: u8,
    /// Mean silhouette of the chosen k. None when k-means didn't run
    /// (single-line page, or zero bbox-bearing lines).
    pub silhouette: Option<f32>,
}

/// One ColumnPosition per input line, plus per-page diagnostics.
pub struct ColumnAssignment {
    pub positions: Vec<ColumnPosition>,
    pub pages: Vec<PageColumns>,
}

/// Assign a `ColumnPosition` to each line in `lines`, grouped per page.
/// Input order is preserved in `positions`.
pub fn assign_columns(lines: &[LineSpan]) -> ColumnAssignment {
    let mut positions = vec![ColumnPosition::Multi; lines.len()];
    let mut pages: Vec<PageColumns> = Vec::new();

    if lines.is_empty() {
        return ColumnAssignment { positions, pages };
    }

    // Group line indices by page, preserving discovery order.
    let mut by_page: Vec<(u32, Vec<usize>)> = Vec::new();
    let mut page_seen: HashMap<u32, usize> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        match page_seen.get(&line.page) {
            Some(&slot) => by_page[slot].1.push(i),
            None => {
                page_seen.insert(line.page, by_page.len());
                by_page.push((line.page, vec![i]));
            }
        }
    }

    for (page, idxs) in by_page {
        let line_count = idxs.len() as u32;

        let bbox_lines: Vec<(usize, i64)> = idxs
            .iter()
            .filter_map(|&i| lines[i].x0().map(|x| (i, x)))
            .collect();

        if bbox_lines.len() < MIN_LINES_FOR_KMEANS {
            for &i in &idxs {
                positions[i] = ColumnPosition::Single;
            }
            pages.push(PageColumns {
                page,
                line_count,
                k_used: 1,
                silhouette: None,
            });
            continue;
        }

        let xs: Vec<f32> = bbox_lines.iter().map(|(_, x)| *x as f32).collect();

        // Adaptive k: try every k in 2..=MAX_K (capped at point count),
        // then apply the parsimony tiebreak — among k's that cleared the
        // silhouette threshold, pick the smallest k within PARSIMONY_EPSILON
        // of the best. `candidates` is populated in ascending k order, so
        // `find()` returns the smallest qualifying k.
        let k_max = (MAX_K as usize).min(bbox_lines.len()) as u8;
        let mut candidates: Vec<(u8, Vec<u8>, Vec<f32>, f32)> = Vec::new();
        let mut best_failed: Option<(u8, f32)> = None;

        for k in 2..=k_max {
            let (labels, centroids) = kmeans_1d(&xs, k);
            let s = silhouette_score(&xs, &labels, k);
            if s >= SILHOUETTE_THRESHOLD {
                candidates.push((k, labels, centroids, s));
            } else {
                let better = best_failed.map(|b| s > b.1).unwrap_or(true);
                if better {
                    best_failed = Some((k, s));
                }
            }
        }

        let best = if candidates.is_empty() {
            None
        } else {
            let s_max = candidates
                .iter()
                .map(|c| c.3)
                .fold(f32::NEG_INFINITY, f32::max);
            let cutoff = s_max - PARSIMONY_EPSILON;
            candidates.into_iter().find(|c| c.3 >= cutoff)
        };

        if let Some((k, labels, centroids, s)) = best {
            // Centroid order → 0-based left-to-right column index.
            let mut order: Vec<usize> = (0..k as usize).collect();
            order.sort_by(|&a, &b| centroids[a].partial_cmp(&centroids[b]).unwrap());
            let mut label_to_col: Vec<u8> = vec![0; k as usize];
            for (rank, &cluster) in order.iter().enumerate() {
                label_to_col[cluster] = rank as u8;
            }

            for ((i, _), &label) in bbox_lines.iter().zip(labels.iter()) {
                positions[*i] = ColumnPosition::Col(label_to_col[label as usize]);
            }
            // Bbox-less lines on this page → Multi (we can't place them).
            for &i in &idxs {
                if lines[i].x0().is_none() {
                    positions[i] = ColumnPosition::Multi;
                }
            }
            pages.push(PageColumns {
                page,
                line_count,
                k_used: k,
                silhouette: Some(s),
            });
        } else {
            // No k cleared threshold — the page isn't separable into columns.
            for &i in &idxs {
                positions[i] = ColumnPosition::Multi;
            }
            let (k, s) = best_failed.unwrap_or((2, 0.0));
            pages.push(PageColumns {
                page,
                line_count,
                k_used: k,
                silhouette: Some(s),
            });
        }
    }

    ColumnAssignment { positions, pages }
}

/// 1-D k-means with `k` clusters. Returns (per-point labels, centroids).
/// Initial centroids are linearly spaced between `min(xs)` and `max(xs)` —
/// deterministic, no RNG.
fn kmeans_1d(xs: &[f32], k: u8) -> (Vec<u8>, Vec<f32>) {
    debug_assert!(k >= 1);
    debug_assert!(xs.len() >= k as usize);

    let k = k as usize;
    let xmin = xs.iter().cloned().fold(f32::INFINITY, f32::min);
    let xmax = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut centroids: Vec<f32> = if k == 1 {
        vec![(xmin + xmax) * 0.5]
    } else {
        (0..k)
            .map(|i| xmin + (xmax - xmin) * (i as f32) / ((k - 1) as f32))
            .collect()
    };

    let mut labels = vec![0u8; xs.len()];
    if (xmax - xmin).abs() < f32::EPSILON {
        // Every point identical — degenerate.
        return (labels, centroids);
    }

    for _ in 0..32 {
        let mut changed = false;
        for (i, &x) in xs.iter().enumerate() {
            let mut best_c = 0u8;
            let mut best_d = f32::INFINITY;
            for (ci, &c) in centroids.iter().enumerate() {
                let d = (x - c).abs();
                if d < best_d {
                    best_d = d;
                    best_c = ci as u8;
                }
            }
            if best_c != labels[i] {
                labels[i] = best_c;
                changed = true;
            }
        }

        let mut sums = vec![0.0f32; k];
        let mut counts = vec![0u32; k];
        for (&x, &l) in xs.iter().zip(labels.iter()) {
            sums[l as usize] += x;
            counts[l as usize] += 1;
        }
        for ci in 0..k {
            if counts[ci] > 0 {
                centroids[ci] = sums[ci] / counts[ci] as f32;
            }
        }
        if !changed {
            break;
        }
    }
    (labels, centroids)
}

/// Mean silhouette score over `xs` given cluster `labels` (0..k). Returns 0.0
/// when fewer than two clusters are non-empty.
fn silhouette_score(xs: &[f32], labels: &[u8], k: u8) -> f32 {
    let n = xs.len();
    if n < 2 || k < 2 {
        return 0.0;
    }
    let k = k as usize;
    let mut cluster_counts = vec![0u32; k];
    for &l in labels {
        cluster_counts[l as usize] += 1;
    }
    let non_empty = cluster_counts.iter().filter(|&&c| c > 0).count();
    if non_empty < 2 {
        return 0.0;
    }

    let mut total = 0.0f32;
    let mut counted = 0u32;
    for (i, &x_i) in xs.iter().enumerate() {
        let l_i = labels[i] as usize;

        // Per-cluster sums of |x_i - x_j| over j != i.
        let mut sums = vec![0.0f32; k];
        let mut counts = vec![0u32; k];
        for (j, &x_j) in xs.iter().enumerate() {
            if i == j {
                continue;
            }
            let l_j = labels[j] as usize;
            sums[l_j] += (x_i - x_j).abs();
            counts[l_j] += 1;
        }

        if counts[l_i] == 0 {
            // i is alone in its cluster — silhouette is undefined; skip.
            continue;
        }
        let a = sums[l_i] / counts[l_i] as f32;

        // b = mean distance to the closest OTHER cluster.
        let mut b = f32::INFINITY;
        for ci in 0..k {
            if ci == l_i || counts[ci] == 0 {
                continue;
            }
            let mean = sums[ci] / counts[ci] as f32;
            if mean < b {
                b = mean;
            }
        }
        if b.is_infinite() {
            continue;
        }

        let denom = a.max(b);
        if denom > 0.0 {
            total += (b - a) / denom;
            counted += 1;
        }
    }
    if counted == 0 {
        0.0
    } else {
        total / counted as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::line_grouper::LineSpan;

    fn line(page: u32, line_idx: u32, x0: i64, text: &str) -> LineSpan {
        LineSpan {
            page,
            line_idx,
            text: text.into(),
            bbox: Some([
                x0,
                line_idx as i64 * 20,
                x0 + 100,
                line_idx as i64 * 20 + 18,
            ]),
            word_range: 0..1,
        }
    }

    #[test]
    fn empty_input() {
        let out = assign_columns(&[]);
        assert!(out.positions.is_empty());
        assert!(out.pages.is_empty());
    }

    #[test]
    fn two_column_invoice_separates_into_col0_and_col1() {
        let lines: Vec<LineSpan> = (0..6)
            .map(|i| line(1, i, 50, "left"))
            .chain((0..6).map(|i| line(1, i + 6, 550, "right")))
            .collect();
        let out = assign_columns(&lines);
        assert_eq!(out.positions.len(), 12);
        for p in &out.positions[..6] {
            assert_eq!(*p, ColumnPosition::Col(0), "expected Col(0) for low x0");
        }
        for p in &out.positions[6..] {
            assert_eq!(*p, ColumnPosition::Col(1), "expected Col(1) for high x0");
        }
        assert_eq!(out.pages.len(), 1);
        assert_eq!(out.pages[0].k_used, 2);
        let s = out.pages[0].silhouette.unwrap();
        assert!(s > SILHOUETTE_THRESHOLD, "silhouette {} should pass", s);
    }

    #[test]
    fn three_column_page_is_recognised_as_k3() {
        // 5 lines at each of three x positions → k=3 should dominate.
        let lines: Vec<LineSpan> = (0..5)
            .map(|i| line(1, i, 50, "left"))
            .chain((0..5).map(|i| line(1, i + 5, 350, "middle")))
            .chain((0..5).map(|i| line(1, i + 10, 700, "right")))
            .collect();
        let out = assign_columns(&lines);
        assert_eq!(out.pages.len(), 1);
        assert_eq!(out.pages[0].k_used, 3, "expected adaptive k=3");
        // Position-order matches input grouping.
        for p in &out.positions[..5] {
            assert_eq!(*p, ColumnPosition::Col(0));
        }
        for p in &out.positions[5..10] {
            assert_eq!(*p, ColumnPosition::Col(1));
        }
        for p in &out.positions[10..] {
            assert_eq!(*p, ColumnPosition::Col(2));
        }
    }

    #[test]
    fn single_column_page_collapses_to_multi_via_failed_silhouette() {
        let lines: Vec<LineSpan> = (0..8).map(|i| line(1, i, 100, "body")).collect();
        let out = assign_columns(&lines);
        for p in &out.positions {
            assert_eq!(*p, ColumnPosition::Multi);
        }
    }

    #[test]
    fn too_few_lines_yields_single() {
        let lines = vec![line(1, 0, 50, "a"), line(1, 1, 550, "b")];
        let out = assign_columns(&lines);
        for p in &out.positions {
            assert_eq!(*p, ColumnPosition::Single);
        }
        assert_eq!(out.pages[0].k_used, 1);
        assert!(out.pages[0].silhouette.is_none());
    }

    #[test]
    fn parsimony_picks_smallest_k_when_higher_k_ties() {
        // Bimodal data: 8 lines at x=60, 6 at x=400. k=2 splits cleanly.
        // Higher k's snap most points to the same two centroids and score
        // identically (empty clusters get skipped). Parsimony must pick k=2.
        let lines: Vec<LineSpan> = (0..8)
            .map(|i| line(1, i, 60, "left"))
            .chain((0..6).map(|i| line(1, i + 8, 400, "right")))
            .collect();
        let out = assign_columns(&lines);
        assert_eq!(out.pages.len(), 1);
        assert_eq!(
            out.pages[0].k_used, 2,
            "parsimony tiebreak must prefer the smallest k when higher k's \
             tie on silhouette (e.g. when extra clusters end up empty)"
        );
    }

    #[test]
    fn pages_are_classified_independently() {
        let mut lines: Vec<LineSpan> = (0..6)
            .map(|i| line(1, i, 50, "left"))
            .chain((0..6).map(|i| line(1, i + 6, 550, "right")))
            .collect();
        // Page 2: single column.
        lines.extend((0..6).map(|i| line(2, i, 100, "single")));

        let out = assign_columns(&lines);
        assert_eq!(out.pages.len(), 2);
        assert_eq!(out.pages[0].page, 1);
        assert_eq!(out.pages[1].page, 2);
        assert_eq!(out.positions[0], ColumnPosition::Col(0));
        assert_eq!(out.positions[6], ColumnPosition::Col(1));
        assert_eq!(out.positions[12], ColumnPosition::Multi);
    }
}
