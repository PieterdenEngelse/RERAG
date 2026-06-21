// src/pdf/page_type.rs — Phase 2 page-type heuristic.
//
// Classifies each page of a PDF as Cover / Toc / Body / Appendix based purely
// on the lines collected by `line_grouper`. No ML, no font analysis: the four
// signals it uses (page number, line count, header text, TOC-entry pattern)
// are cheap to compute and survive scanned / native / OCR'd PDFs alike.
//
// Why these four classes: they're the ones a reader would actually want to
// filter by ("show me only body pages of this report"), and they're the
// minimal set that lets the dashboard pick distinct badge colours. Finer
// distinctions (front-matter, index, references, bibliography) all collapse
// into one of these four for retrieval purposes — front-matter is cover-like,
// references / bibliography look TOC-like to this classifier and that's
// fine.

use super::line_grouper::LineSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageType {
    Cover,
    Toc,
    Body,
    Appendix,
}

impl PageType {
    /// Stable wire string used in SQLite + JSON + the frontend palette.
    pub fn as_str(self) -> &'static str {
        match self {
            PageType::Cover => "cover",
            PageType::Toc => "toc",
            PageType::Body => "body",
            PageType::Appendix => "appendix",
        }
    }
}

/// Pages with fewer than this many lines tend to be title / cover pages —
/// a real body page of any length has at least ~15 wrapped lines.
const COVER_MAX_LINES: usize = 15;

/// Among the first N lines of a page we look for TOC / Appendix headings.
/// Limiting the search window prevents body pages that happen to mention
/// "appendix" in passing from being mis-classified.
const HEADING_SCAN_LINES: usize = 4;

/// A page qualifies as TOC if at least this fraction of its lines look like
/// TOC entries (i.e. text followed by a page-number digit, optionally with
/// a dot leader).
const TOC_ENTRY_RATIO: f32 = 0.5;

/// And at least this many TOC entries on the page in absolute terms — keeps
/// short pages with two coincidentally-numbered lines from triggering.
const TOC_MIN_ENTRIES: usize = 3;

/// Classify a page given its grouped lines (in reading order). `page_num`
/// is 1-based, matching `LineSpan.page`.
pub fn classify_page(page_num: u32, lines: &[LineSpan]) -> PageType {
    // Appendix takes precedence — a page that opens with "Appendix A: …"
    // should be tagged appendix even if it also has cover-like brevity.
    if has_appendix_heading(lines) {
        return PageType::Appendix;
    }

    if looks_like_toc(lines) {
        return PageType::Toc;
    }

    // Cover: by definition page 1, and short. We require *both* conditions
    // — a page-1 with 200 lines of dense body is clearly a body-first
    // document (the article PDF case), not a cover.
    if page_num == 1 && lines.len() <= COVER_MAX_LINES {
        return PageType::Cover;
    }

    PageType::Body
}

fn has_appendix_heading(lines: &[LineSpan]) -> bool {
    lines
        .iter()
        .take(HEADING_SCAN_LINES)
        .any(|l| is_appendix_text(&l.text))
}

fn is_appendix_text(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    // "Appendix" / "Appendix A" / "Annex 1" / "Anhang" (German). We require
    // the appendix word to be at the *start* of the line so a body paragraph
    // mentioning "see appendix B" doesn't trip the check.
    t.starts_with("appendix") || t.starts_with("annex ") || t.starts_with("anhang")
}

fn looks_like_toc(lines: &[LineSpan]) -> bool {
    // Cheap header sniff — covers English, German, French, Spanish. Any one
    // of these as the page's lead line is enough to call it TOC even if the
    // body entries aren't dot-leader-heavy.
    let header_hit = lines
        .iter()
        .take(HEADING_SCAN_LINES)
        .any(|l| is_toc_header(&l.text));
    if header_hit {
        return true;
    }

    // Otherwise require a meaningful density of TOC-shaped entries.
    if lines.len() < TOC_MIN_ENTRIES {
        return false;
    }
    let entry_count = lines.iter().filter(|l| looks_like_toc_entry(&l.text)).count();
    if entry_count < TOC_MIN_ENTRIES {
        return false;
    }
    (entry_count as f32) / (lines.len() as f32) >= TOC_ENTRY_RATIO
}

fn is_toc_header(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    t == "contents"
        || t == "table of contents"
        || t == "index"
        || t == "inhaltsverzeichnis"
        || t == "table des matières"
        || t == "índice"
}

fn looks_like_toc_entry(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 4 {
        return false;
    }
    // Must end with a 1–4 digit page number.
    let bytes = t.as_bytes();
    let mut i = bytes.len();
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    let digit_run = bytes.len() - i;
    if !(1..=4).contains(&digit_run) {
        return false;
    }
    // And the prefix before the page-number digits has to look like text —
    // not just punctuation, not empty. Allowing dot-leader / whitespace
    // between the prefix and the digits handles `Introduction ........ 12`
    // as well as `Introduction 12`.
    let prefix = t[..i].trim_end_matches(|c: char| c.is_whitespace() || c == '.' || c == '·');
    let alpha = prefix.chars().filter(|c| c.is_alphabetic()).count();
    alpha >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(idx: u32, text: &str) -> LineSpan {
        LineSpan {
            page: 1,
            line_idx: idx,
            text: text.to_string(),
            bbox: None,
            word_range: 0..1,
        }
    }

    #[test]
    fn empty_page_is_body() {
        assert_eq!(classify_page(5, &[]), PageType::Body);
    }

    #[test]
    fn short_page_one_is_cover() {
        let lines = vec![
            line(0, "Quarterly Report 2026"),
            line(1, "Prepared for the Board"),
            line(2, "Acme Corp"),
        ];
        assert_eq!(classify_page(1, &lines), PageType::Cover);
    }

    #[test]
    fn short_page_two_is_body_not_cover() {
        // Cover only applies on page 1.
        let lines = vec![line(0, "Heading"), line(1, "A short paragraph.")];
        assert_eq!(classify_page(2, &lines), PageType::Body);
    }

    #[test]
    fn explicit_toc_header_classifies_as_toc() {
        let lines = vec![
            line(0, "Table of Contents"),
            line(1, "Introduction"),
            line(2, "Methodology"),
        ];
        assert_eq!(classify_page(2, &lines), PageType::Toc);
    }

    #[test]
    fn dot_leader_entries_classify_as_toc() {
        let lines = vec![
            line(0, "Introduction ........ 1"),
            line(1, "Method .............. 5"),
            line(2, "Results ............ 12"),
            line(3, "Discussion ......... 18"),
            line(4, "References ......... 24"),
        ];
        assert_eq!(classify_page(2, &lines), PageType::Toc);
    }

    #[test]
    fn body_text_does_not_match_toc() {
        // A page that happens to have one line ending in a digit isn't a TOC.
        let lines = vec![
            line(0, "In 2024 we observed a sharp increase in usage."),
            line(1, "Specifically, the API call rate doubled."),
            line(2, "The chart on page 12"),
            line(3, "shows the trend clearly."),
        ];
        assert_eq!(classify_page(7, &lines), PageType::Body);
    }

    #[test]
    fn appendix_heading_classifies_as_appendix() {
        let lines = vec![
            line(0, "Appendix A: Raw data tables"),
            line(1, "The following tables list the per-region figures."),
        ];
        assert_eq!(classify_page(40, &lines), PageType::Appendix);
    }

    #[test]
    fn appendix_takes_precedence_over_cover_shape() {
        // Short page 1 starting with "Appendix" — appendix wins.
        let lines = vec![line(0, "Appendix"), line(1, "Glossary")];
        assert_eq!(classify_page(1, &lines), PageType::Appendix);
    }

    #[test]
    fn body_mention_of_appendix_does_not_misclassify() {
        // "see appendix B" mid-paragraph must NOT trigger appendix.
        let lines = vec![
            line(0, "Results are summarised below."),
            line(1, "For raw data, see appendix B."),
            line(2, "The trend matches expectations."),
        ];
        assert_eq!(classify_page(10, &lines), PageType::Body);
    }

    #[test]
    fn long_page_one_is_body_not_cover() {
        // Page 1 with many lines (e.g. an article without a separate cover).
        let lines: Vec<LineSpan> = (0..30).map(|i| line(i, "Body paragraph text.")).collect();
        assert_eq!(classify_page(1, &lines), PageType::Body);
    }
}
