// src/pdf/word_extractor.rs — Stage 1: extract word-level text + bboxes from PDF.
//
// Primary path: lopdf content-stream parser → word positions normalised 0-1000.
// Fallback: extractous text extraction → words with bbox=None.
//
// Downstream stages handle None bboxes gracefully (text-only classification).

use std::io::Read;
use tracing::debug;

/// One word token with its page number and optional normalised bounding box.
#[derive(Debug, Clone)]
pub struct WordSpan {
    pub text: String,
    pub page: u32,
    /// [x0, y0, x1, y1] all in [0, 1000].  None when position is unavailable.
    pub bbox: Option<[i64; 4]>,
}

/// Extract word spans from raw PDF bytes.
/// Returns bboxes when lopdf can parse the content stream; falls back to
/// extractous (text-only) on any error.
pub fn extract_words(bytes: &[u8]) -> anyhow::Result<Vec<WordSpan>> {
    match extract_via_lopdf(bytes) {
        Ok(words) if !words.is_empty() => {
            debug!(count = words.len(), "lopdf word extraction succeeded");
            return Ok(words);
        }
        Err(e) => debug!(error = %e, "lopdf extraction failed, falling back to extractous"),
        Ok(_) => debug!("lopdf returned 0 words, falling back to extractous"),
    }
    extract_via_extractous(bytes)
}

// ── extractous fallback ───────────────────────────────────────────────────────

fn extract_via_extractous(bytes: &[u8]) -> anyhow::Result<Vec<WordSpan>> {
    use extractous::Extractor;

    let extractor = Extractor::new();
    let (mut reader, _) = extractor.extract_bytes(bytes)?;
    let mut text = String::new();
    reader.read_to_string(&mut text)?;

    let mut words = Vec::new();
    let mut page = 1u32;

    for line in text.lines() {
        // Form-feed byte signals a page break in many text extractors.
        if line.contains('\x0C') {
            page += 1;
            continue;
        }
        for word in line.split_whitespace() {
            words.push(WordSpan {
                text: word.to_string(),
                page,
                bbox: None,
            });
        }
    }
    Ok(words)
}

// ── lopdf path ────────────────────────────────────────────────────────────────

fn extract_via_lopdf(bytes: &[u8]) -> anyhow::Result<Vec<WordSpan>> {
    use std::io::Cursor;
    let doc = lopdf::Document::load_from(Cursor::new(bytes))?;
    let mut all_words = Vec::new();

    for (page_idx, page_id) in doc.page_iter().enumerate() {
        let page_num = (page_idx + 1) as u32;
        let (pw, ph) = page_dims(&doc, page_id).unwrap_or((595.0, 842.0));

        let content_bytes = match doc.get_page_content(page_id) {
            Ok(b) => b,
            Err(e) => {
                debug!(page = page_num, error = %e, "lopdf: skipping page (no content)");
                continue;
            }
        };

        let content = match lopdf::content::Content::decode(&content_bytes) {
            Ok(c) => c,
            Err(e) => {
                debug!(page = page_num, error = %e, "lopdf: failed to decode content stream");
                continue;
            }
        };

        let words = parse_page_words(&content.operations, pw, ph, page_num);
        all_words.extend(words);
    }
    Ok(all_words)
}

fn page_dims(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Option<(f64, f64)> {
    let page = doc.get_object(page_id).ok()?.as_dict().ok()?;
    let media = page.get(b"MediaBox").ok()?.as_array().ok()?;
    if media.len() < 4 {
        return None;
    }
    let x0 = obj_as_f64(&media[0])?;
    let y0 = obj_as_f64(&media[1])?;
    let x1 = obj_as_f64(&media[2])?;
    let y1 = obj_as_f64(&media[3])?;
    Some((x1 - x0, y1 - y0))
}

fn obj_as_f64(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Real(v) => Some(*v as f64),
        lopdf::Object::Integer(v) => Some(*v as f64),
        _ => None,
    }
}

fn obj_as_str(obj: &lopdf::Object) -> Option<String> {
    match obj {
        lopdf::Object::String(bytes, _) => Some(pdf_bytes_to_utf8(bytes)),
        _ => None,
    }
}

fn pdf_bytes_to_utf8(bytes: &[u8]) -> String {
    // UTF-16 BE (BOM 0xFE 0xFF)
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let words: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&words);
    }
    // Assume Latin-1
    bytes.iter().map(|&b| b as char).collect()
}

/// Walk content-stream operations, emitting `WordSpan`s with approximate bboxes.
fn parse_page_words(
    ops: &[lopdf::content::Operation],
    page_w: f64,
    page_h: f64,
    page: u32,
) -> Vec<WordSpan> {
    let mut words = Vec::new();

    // Text state
    let mut in_text = false;
    let mut tx = 0.0f64; // text cursor x in PDF user-space
    let mut ty = 0.0f64; // text cursor y in PDF user-space
    let mut font_size = 10.0f64;
    let mut leading = 0.0f64;

    let norm_x = |x: f64| (x / page_w * 1000.0).round().clamp(0.0, 1000.0) as i64;
    let norm_y = |y: f64| ((page_h - y) / page_h * 1000.0).round().clamp(0.0, 1000.0) as i64;

    let emit = |words: &mut Vec<WordSpan>, text: &str, tx: f64, ty: f64, fs: f64| {
        if text.trim().is_empty() {
            return;
        }
        // Approximate x1 using character width estimate (0.6 × font size per char)
        let char_w = fs * 0.6;
        let x0 = norm_x(tx);
        let y0 = norm_y(ty + fs);
        let x1 = norm_x(tx + text.len() as f64 * char_w).min(1000);
        let y1 = norm_y(ty);
        let bbox = Some([x0, y0, x1.max(x0), y1.max(y0)]);
        for word in text.split_whitespace() {
            words.push(WordSpan {
                text: word.to_string(),
                page,
                bbox,
            });
        }
    };

    for op in ops {
        match op.operator.as_str() {
            "BT" => {
                in_text = true;
                tx = 0.0;
                ty = 0.0;
            }
            "ET" => {
                in_text = false;
            }
            "Tf" if in_text => {
                // [font_name font_size] Tf
                if let Some(sz) = op.operands.get(1).and_then(obj_as_f64) {
                    font_size = sz.abs().max(1.0);
                }
            }
            "TL" if in_text => {
                // [leading] TL
                if let Some(l) = op.operands.first().and_then(obj_as_f64) {
                    leading = l;
                }
            }
            "Tm" if in_text && op.operands.len() >= 6 => {
                // [a b c d e f] Tm — sets text matrix; e=tx, f=ty
                tx = op.operands[4].as_float().unwrap_or(0.0) as f64;
                ty = op.operands[5].as_float().unwrap_or(0.0) as f64;
            }
            "Td" | "TD" if in_text && op.operands.len() >= 2 => {
                tx += op.operands[0].as_float().unwrap_or(0.0) as f64;
                ty += op.operands[1].as_float().unwrap_or(0.0) as f64;
                if op.operator == "TD" {
                    leading = -op.operands[1].as_float().unwrap_or(0.0) as f64;
                }
            }
            "T*" if in_text => {
                ty -= leading;
            }
            "Tj" if in_text => {
                if let Some(text) = op.operands.first().and_then(obj_as_str) {
                    emit(&mut words, &text, tx, ty, font_size);
                    tx += text.len() as f64 * font_size * 0.6;
                }
            }
            "'" if in_text => {
                ty -= leading;
                if let Some(text) = op.operands.first().and_then(obj_as_str) {
                    emit(&mut words, &text, tx, ty, font_size);
                }
            }
            "\"" if in_text && op.operands.len() >= 3 => {
                if let Some(l) = op.operands.get(1).and_then(obj_as_f64) {
                    leading = l;
                }
                ty -= leading;
                if let Some(text) = op.operands.get(2).and_then(obj_as_str) {
                    emit(&mut words, &text, tx, ty, font_size);
                }
            }
            "TJ" if in_text => {
                // [(str) kern (str) ...] TJ
                if let Some(lopdf::Object::Array(arr)) = op.operands.first() {
                    let mut full = String::new();
                    for item in arr {
                        match item {
                            lopdf::Object::String(b, _) => full.push_str(&pdf_bytes_to_utf8(b)),
                            lopdf::Object::Integer(k) => {
                                // Negative kern = advance; positive = retract
                                if *k < -100 {
                                    full.push(' ');
                                }
                            }
                            lopdf::Object::Real(k) => {
                                if *k < -100.0 {
                                    full.push(' ');
                                }
                            }
                            _ => {}
                        }
                    }
                    emit(&mut words, &full, tx, ty, font_size);
                    tx += full.len() as f64 * font_size * 0.6;
                }
            }
            _ => {}
        }
    }
    words
}
