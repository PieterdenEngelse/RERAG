//! ONNX-based NER extractor using dslim/bert-base-NER. v1.0.0

use ort::value::Tensor;
use std::sync::{Mutex, OnceLock};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct NerEntity {
    pub text: String,
    pub label: String,
    pub score: f32,
}

fn id_to_label(id: usize) -> &'static str {
    match id {
        0 => "O",
        1 => "B-MISC",
        2 => "I-MISC",
        3 => "B-PER",
        4 => "I-PER",
        5 => "B-ORG",
        6 => "I-ORG",
        7 => "B-LOC",
        8 => "I-LOC",
        _ => "O",
    }
}

fn bio_to_type(label: &str) -> Option<&'static str> {
    if label.ends_with("PER") {
        Some("PERSON")
    } else if label.ends_with("ORG") {
        Some("ORG")
    } else if label.ends_with("LOC") {
        Some("LOC")
    } else if label.ends_with("MISC") {
        Some("MISC")
    } else {
        None
    }
}

struct NerRuntime {
    session: ort::session::Session,
    tokenizer: tokenizers::Tokenizer,
}

static NER_RUNTIME: OnceLock<Mutex<Option<NerRuntime>>> = OnceLock::new();

fn get_or_init_runtime() -> &'static Mutex<Option<NerRuntime>> {
    NER_RUNTIME.get_or_init(|| {
        let model_dir =
            std::env::var("NER_MODEL_PATH").unwrap_or_else(|_| "models/ner".to_string());
        let model_path = format!("{}/model.onnx", model_dir);
        let tokenizer_path = format!("{}/tokenizer.json", model_dir);

        if !std::path::Path::new(&model_path).exists() {
            warn!(path = %model_path, "NER model not found, NER disabled");
            return Mutex::new(None);
        }

        let _ = ort::init().with_name("ner").commit();

        let session = match ort::session::Session::builder()
            .and_then(|mut b| b.commit_from_file(&model_path))
        {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "Failed to load NER model");
                return Mutex::new(None);
            }
        };

        let tokenizer = match tokenizers::Tokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => {
                warn!(error = %e, "Failed to load NER tokenizer");
                return Mutex::new(None);
            }
        };

        debug!("NER runtime initialized");
        Mutex::new(Some(NerRuntime { session, tokenizer }))
    })
}

/// Decode one item's worth of logits from the flat output tensor.
/// `data` is the full `[batch, seq, labels]` output; `item` is the batch index.
fn decode_item(
    data: &[f32],
    item: usize,
    seq_len: usize,
    num_labels: usize,
    tokens: &[impl AsRef<str>],
    real_seq_len: usize,
) -> Vec<NerEntity> {
    let item_offset = item * seq_len * num_labels;
    let mut entities: Vec<NerEntity> = Vec::new();
    let mut current: Option<(String, String, f32)> = None;

    // Skip [CLS] (0) and [SEP] (real_seq_len-1); ignore padding beyond real_seq_len.
    for i in 1..real_seq_len.saturating_sub(1) {
        let offset = item_offset + i * num_labels;
        let logits = &data[offset..offset + num_labels];

        let (best_id, &best_logit) =
            logits
                .iter()
                .enumerate()
                .fold((0usize, &f32::NEG_INFINITY), |(bi, bs), (j, v)| {
                    if v > bs { (j, v) } else { (bi, bs) }
                });

        let exp_sum: f32 = logits.iter().map(|&v| v.exp()).sum();
        let score = best_logit.exp() / exp_sum;
        let label = id_to_label(best_id);
        let token = tokens.get(i).map(|s| s.as_ref()).unwrap_or("");
        let word = token.strip_prefix("##").unwrap_or(token);

        if label == "O" {
            if let Some((text, lbl, sc)) = current.take() {
                if let Some(t) = bio_to_type(&lbl) {
                    entities.push(NerEntity { text, label: t.to_string(), score: sc });
                }
            }
        } else if label.starts_with("B-") {
            if let Some((text, lbl, sc)) = current.take() {
                if let Some(t) = bio_to_type(&lbl) {
                    entities.push(NerEntity { text, label: t.to_string(), score: sc });
                }
            }
            current = Some((word.to_string(), label.to_string(), score));
        } else if label.starts_with("I-") {
            if let Some((ref mut text, _, ref mut sc)) = current {
                if token.starts_with("##") { text.push_str(word); }
                else { text.push(' '); text.push_str(word); }
                *sc = (*sc + score) / 2.0;
            }
        }
    }
    if let Some((text, lbl, sc)) = current {
        if let Some(t) = bio_to_type(&lbl) {
            entities.push(NerEntity { text, label: t.to_string(), score: sc });
        }
    }
    entities.retain(|e| e.score >= 0.7 && e.text.len() >= 2);
    entities.dedup_by(|a, b| a.text.eq_ignore_ascii_case(&b.text));
    entities
}

/// Run NER on a batch of texts in a single ONNX call.
/// Sequences are padded to the longest in the batch with [PAD]=0 tokens
/// and an attention mask of 0 so the model ignores them.
/// Returns one `Vec<NerEntity>` per input text, in the same order.
/// Falls back to empty vecs if the runtime is not loaded.
pub fn extract_entities_batch(texts: &[&str]) -> Vec<Vec<NerEntity>> {
    if texts.is_empty() {
        return vec![];
    }

    // Single-text fast path — avoids padding overhead.
    if texts.len() == 1 {
        return vec![extract_entities(texts[0])];
    }

    let lock = get_or_init_runtime();
    let mut guard = match lock.lock() {
        Ok(g) => g,
        Err(_) => return vec![vec![]; texts.len()],
    };
    let runtime = match guard.as_mut() {
        Some(r) => r,
        None => return vec![vec![]; texts.len()],
    };

    let truncated: Vec<&str> = texts
        .iter()
        .map(|t| if t.len() > 2000 { &t[..2000] } else { t })
        .collect();

    // Tokenize all texts and record their real sequence lengths.
    let mut encodings = Vec::with_capacity(truncated.len());
    for text in &truncated {
        match runtime.tokenizer.encode(*text, true) {
            Ok(e) => encodings.push(e),
            Err(err) => {
                warn!(error = %err, "NER tokenization failed in batch");
                return vec![vec![]; texts.len()];
            }
        }
    }

    let real_seq_lens: Vec<usize> = encodings.iter().map(|e| e.get_ids().len()).collect();
    let max_seq_len = *real_seq_lens.iter().max().unwrap_or(&0);
    if max_seq_len == 0 {
        return vec![vec![]; texts.len()];
    }

    let batch = encodings.len();
    let mut all_ids = vec![0i64; batch * max_seq_len];
    let mut all_att = vec![0i64; batch * max_seq_len];
    let mut all_type = vec![0i64; batch * max_seq_len];

    for (i, enc) in encodings.iter().enumerate() {
        let ids: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
        let att: Vec<i64> = enc.get_attention_mask().iter().map(|&x| x as i64).collect();
        let types: Vec<i64> = enc.get_type_ids().iter().map(|&x| x as i64).collect();
        let len = ids.len();
        let base = i * max_seq_len;
        all_ids[base..base + len].copy_from_slice(&ids);
        all_att[base..base + len].copy_from_slice(&att);
        all_type[base..base + len].copy_from_slice(&types);
        // Padding positions stay 0 (already initialised).
    }

    let shape = vec![batch as i64, max_seq_len as i64];
    let ids_t = match Tensor::from_array((shape.clone(), all_ids)) {
        Ok(t) => t, Err(e) => { warn!(error=%e); return vec![vec![]; texts.len()]; }
    };
    let att_t = match Tensor::from_array((shape.clone(), all_att)) {
        Ok(t) => t, Err(e) => { warn!(error=%e); return vec![vec![]; texts.len()]; }
    };
    let type_t = match Tensor::from_array((shape, all_type)) {
        Ok(t) => t, Err(e) => { warn!(error=%e); return vec![vec![]; texts.len()]; }
    };

    let outputs = match runtime.session.run(ort::inputs![
        "input_ids" => ids_t,
        "attention_mask" => att_t,
        "token_type_ids" => type_t
    ]) {
        Ok(o) => o,
        Err(e) => { warn!(error = %e, "NER batch inference failed"); return vec![vec![]; texts.len()]; }
    };

    let (shape, data) = match outputs[0].try_extract_tensor::<f32>() {
        Ok(t) => t,
        Err(e) => { warn!(error = %e, "NER batch output extraction failed"); return vec![vec![]; texts.len()]; }
    };
    let num_labels = shape[2] as usize;

    encodings
        .iter()
        .enumerate()
        .map(|(i, enc)| {
            let result = decode_item(data, i, max_seq_len, num_labels, enc.get_tokens(), real_seq_lens[i]);
            debug!(item = i, count = result.len(), "NER batch item decoded");
            result
        })
        .collect()
}

/// Single-text convenience wrapper. Use `extract_entities_batch` when processing many chunks.
pub fn extract_entities(text: &str) -> Vec<NerEntity> {
    let text = if text.len() > 2000 { &text[..2000] } else { text };

    let lock = get_or_init_runtime();
    let mut guard = match lock.lock() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    let runtime = match guard.as_mut() {
        Some(r) => r,
        None => return vec![],
    };

    let encoding = match runtime.tokenizer.encode(text, true) {
        Ok(e) => e,
        Err(e) => { warn!(error = %e, "NER tokenization failed"); return vec![]; }
    };

    let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let attention: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
    let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();

    if ids.is_empty() { return vec![]; }
    let seq_len = ids.len();

    let ids_tensor = match Tensor::from_array((vec![1i64, seq_len as i64], ids)) {
        Ok(t) => t, Err(e) => { warn!(error=%e); return vec![]; }
    };
    let att_tensor = match Tensor::from_array((vec![1i64, seq_len as i64], attention)) {
        Ok(t) => t, Err(e) => { warn!(error=%e); return vec![]; }
    };
    let type_tensor = match Tensor::from_array((vec![1i64, seq_len as i64], type_ids)) {
        Ok(t) => t, Err(e) => { warn!(error=%e); return vec![]; }
    };

    let outputs = match runtime.session.run(ort::inputs![
        "input_ids" => ids_tensor,
        "attention_mask" => att_tensor,
        "token_type_ids" => type_tensor
    ]) {
        Ok(o) => o,
        Err(e) => { warn!(error = %e, "NER inference failed"); return vec![]; }
    };

    let (shape, data) = match outputs[0].try_extract_tensor::<f32>() {
        Ok(t) => t,
        Err(e) => { warn!(error = %e, "NER output extraction failed"); return vec![]; }
    };

    let num_labels = shape[2] as usize;
    decode_item(&data, 0, seq_len, num_labels, encoding.get_tokens(), seq_len)
}
