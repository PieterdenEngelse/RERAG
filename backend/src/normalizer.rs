//! Text normalization for the RAG pipeline.
//!
//! Three targets, three levels:
//!
//! | Target | Unicode | Whitespace | Punct canon |
//! |--------|---------|------------|-------------|
//! | Store  | NFC     | ✓          | —           |
//! | Embed  | NFKC    | ✓          | —           |
//! | Index  | NFKC    | ✓          | ✓           |
//!
//! Apply at ingestion:
//!   extract_text → normalize(Store) → chunker → normalize(Index) for Tantivy
//!                                              → normalize(Embed) for embedder + NER
//!
//! Apply at query time:
//!   BM25 query  → normalize(Index)
//!   Embed query → normalize(Embed)

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizeTarget {
    /// Tantivy BM25 field — NFKC + whitespace + punct canonicalization.
    Index,
    /// Embedding model and NER input — NFKC + whitespace, no punct stripping.
    Embed,
    /// User-visible stored content — NFC + whitespace only.
    Store,
}

pub fn normalize(text: &str, target: NormalizeTarget) -> String {
    let s: String = match target {
        NormalizeTarget::Store => text.nfc().collect(),
        NormalizeTarget::Index | NormalizeTarget::Embed => text.nfkc().collect(),
    };
    let s = normalize_whitespace(&s);
    match target {
        NormalizeTarget::Index => canonicalize_punct(&s),
        _ => s,
    }
}

/// Upgrade already-Embed-normalized text to Index by adding punct canonicalization.
/// Avoids re-running NFKC and whitespace passes on text that's already clean.
pub fn to_index(embed_normalized: &str) -> String {
    canonicalize_punct(embed_normalized)
}

/// Normalize whitespace in five ordered steps:
///
/// 1. Strip zero-width and invisible chars (U+00AD soft hyphen, U+200B–U+200D,
///    U+2060 word joiner, U+FEFF BOM).
/// 2. Map \r\n and lone \r → \n.
/// 3. Map form feed (U+000C) and vertical tab (U+000B) → \n (PDF page boundaries).
/// 4. Map all Unicode space variants → U+0020.
/// 5. Collapse runs of spaces → single space. Newlines are preserved so the
///    chunker can detect paragraph boundaries via \n\n.
fn normalize_whitespace(text: &str) -> String {
    // Phase 1: per-character replacements
    let mut phase1 = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            // Zero-width / invisible — strip entirely
            '\u{00AD}' // soft hyphen
            | '\u{200B}' // zero-width space
            | '\u{200C}' // zero-width non-joiner
            | '\u{200D}' // zero-width joiner
            | '\u{2060}' // word joiner
            | '\u{FEFF}' // BOM / zero-width no-break space
            => {}

            // CR+LF or lone CR → LF
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                phase1.push('\n');
            }

            // Form feed + vertical tab → LF (PDF page/section boundary)
            '\u{000C}' | '\u{000B}' => phase1.push('\n'),

            // Unicode space variants → ASCII space
            '\u{00A0}'             // non-breaking space
            | '\u{2000}'..='\u{200A}' // en quad … hair space
            | '\u{202F}'           // narrow no-break space
            | '\u{205F}'           // medium mathematical space
            | '\u{3000}'           // ideographic space
            => phase1.push(' '),

            _ => phase1.push(ch),
        }
    }

    // Phase 2: collapse runs of spaces (newlines untouched — chunker needs \n\n)
    let mut out = String::with_capacity(phase1.len());
    let mut prev_space = false;
    for ch in phase1.chars() {
        if ch == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

/// Canonicalize punctuation for BM25 phrase matching.
/// Applied to the Tantivy index field only — not to embeddings, NER, or stored content.
///
/// - Smart/typographic quotes → ASCII equivalents
/// - Typographic hyphens and minus → ASCII hyphen
/// - En-dash and em-dash → spaced hyphen (they are clause separators, not compounds)
/// - Ellipsis → three dots
fn canonicalize_punct(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            // Double quotes: left, right, low-9
            '\u{201C}' | '\u{201D}' | '\u{201E}' => out.push('"'),
            // Single quotes: left, right, low-9
            '\u{2018}' | '\u{2019}' | '\u{201A}' => out.push('\''),
            // Hyphen (U+2010), non-breaking hyphen (U+2011), minus sign (U+2212)
            '\u{2010}' | '\u{2011}' | '\u{2212}' => out.push('-'),
            // En-dash → spaced hyphen (clause separator, not a compound marker)
            '\u{2013}' => out.push_str(" - "),
            // Em-dash → spaced hyphen
            '\u{2014}' => out.push_str(" - "),
            // Horizontal ellipsis → three ASCII dots
            '\u{2026}' => out.push_str("..."),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfkc_fixes_ligatures() {
        // ﬁ U+FB01, ﬂ U+FB02 — common in PDF font encodings
        let out = normalize("\u{FB01}nancial \u{FB02}ight", NormalizeTarget::Index);
        assert!(out.contains("financial"), "ﬁ ligature should decompose");
        assert!(out.contains("flight"), "ﬂ ligature should decompose");
    }

    #[test]
    fn nbsp_normalized_to_space() {
        let out = normalize("word1\u{00A0}word2", NormalizeTarget::Index);
        assert_eq!(out, "word1 word2");
    }

    #[test]
    fn zero_width_space_stripped() {
        let out = normalize("tech\u{200B}nology", NormalizeTarget::Index);
        assert_eq!(out, "technology");
    }

    #[test]
    fn soft_hyphen_stripped() {
        let out = normalize("busi\u{00AD}ness", NormalizeTarget::Index);
        assert_eq!(out, "business");
    }

    #[test]
    fn smart_quotes_only_for_index() {
        let input = "\u{201C}hello\u{201D}";
        assert_eq!(normalize(input, NormalizeTarget::Index), "\"hello\"");
        // Embed preserves smart quotes for the embedding model's own tokenizer
        assert!(normalize(input, NormalizeTarget::Embed).contains('\u{201C}'));
    }

    #[test]
    fn paragraph_boundary_preserved() {
        let out = normalize("para one\n\npara two", NormalizeTarget::Index);
        assert!(out.contains("\n\n"), "double newline must survive for chunker");
    }

    #[test]
    fn crlf_normalized() {
        let out = normalize("line1\r\nline2\rline3", NormalizeTarget::Store);
        assert_eq!(out, "line1\nline2\nline3");
    }

    #[test]
    fn form_feed_to_newline() {
        // PDF page boundary
        let out = normalize("page1\u{000C}page2", NormalizeTarget::Embed);
        assert_eq!(out, "page1\npage2");
    }

    #[test]
    fn emdash_gets_spaces() {
        let out = normalize("word\u{2014}word", NormalizeTarget::Index);
        assert_eq!(out, "word - word");
    }

    #[test]
    fn space_run_collapsed() {
        let out = normalize("a   b", NormalizeTarget::Store);
        assert_eq!(out, "a b");
    }

    #[test]
    fn to_index_upgrades_embed() {
        let embed = normalize("Hello\u{2014}World", NormalizeTarget::Embed);
        // em-dash still present in Embed output
        assert!(embed.contains('\u{2014}'));
        // to_index adds punct canon on top
        let index = to_index(&embed);
        assert_eq!(index, "Hello - World");
    }
}
