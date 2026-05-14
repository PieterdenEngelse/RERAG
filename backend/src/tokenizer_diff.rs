//! Tokenizer diff engine: evaluate a *candidate* GGUF tokenizer against the
//! golden-corpus baseline captured in [`crate::db::golden_sample`].
//!
//! The baseline is a snapshot of real chunks under the *currently active*
//! tokenizer. The diff engine loads the candidate, re-tokenizes each baseline
//! chunk, and reports per-entry differences plus aggregate stats.
//!
//! Read-only: this never swaps the live tokenizer. Step 4 adds the
//! "accept swap" UI on top of the report.
//!
//! Scope: entries captured under heuristic mode (no `baseline_token_ids`) are
//! skipped — heuristic counts aren't a real baseline to evaluate against.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::db::golden_sample;
use crate::gguf_tokenizer::{
    resolve_llama_server_gguf_path, resolve_ollama_gguf_path, GgufTokenCounter, TokenCounter,
};

#[derive(Debug, Clone, Deserialize)]
pub struct DiffRequest {
    /// Absolute path to a candidate GGUF file.
    pub candidate_path: Option<String>,
    /// Ollama model tag (e.g. `phi:latest`) — resolved to its blob path.
    pub candidate_ollama_model: Option<String>,
    /// Use the active llama.cpp model (resolved via ~/.config/ag/llama-server.env).
    pub candidate_llama_cpp: Option<bool>,
    /// Max number of per-entry diffs to include in the response. Aggregate
    /// stats are always computed over all entries with baseline IDs.
    /// Defaults to 50.
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffEntry {
    pub id: i64,
    pub chunk_text: String,
    pub position_in_corpus: u64,
    pub baseline_count: usize,
    pub candidate_count: usize,
    /// candidate - baseline (signed).
    pub count_delta: i32,
    /// True iff the full token-id sequences are equal.
    pub ids_match: bool,
    /// Length of the longest matching prefix between baseline and candidate
    /// id sequences.
    pub common_prefix_len: usize,
    /// Length of the longest matching suffix, capped so prefix + suffix never
    /// exceed `min(len_baseline, len_candidate)`.
    pub common_suffix_len: usize,
    pub baseline_token_ids: Vec<u32>,
    pub candidate_token_ids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DiffSummary {
    /// Entries with baseline IDs (i.e. those the engine could actually diff).
    pub entries_total: usize,
    /// Entries skipped because they were captured under heuristic mode.
    pub entries_skipped: usize,
    /// Entries where baseline ids == candidate ids.
    pub entries_identical: usize,
    /// Entries where token *count* changed (subset that overlaps with
    /// `entries_ids_changed`; a count change implies an id change).
    pub entries_count_changed: usize,
    /// Entries where token id sequence differs (count may or may not change).
    pub entries_ids_changed: usize,
    pub total_baseline_tokens: u64,
    pub total_candidate_tokens: u64,
    /// (candidate_total - baseline_total) / baseline_total * 100, in percent.
    /// `None` when baseline_total is 0.
    pub total_delta_pct: Option<f64>,
    /// Mean of signed per-entry count deltas.
    pub mean_count_delta: f64,
    /// Mean of |per-entry count delta|.
    pub mean_count_delta_abs: f64,
    pub max_count_delta_abs: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    pub candidate_path: String,
    pub candidate_model_name: String,
    pub candidate_vocab_size: usize,
    /// The tokenizer model name recorded on the baseline entries (taken from
    /// the first diffable entry — the sample is captured under one tokenizer
    /// at a time so this is unambiguous).
    pub baseline_tokenizer_model: Option<String>,
    pub generated_at: String,
    pub summary: DiffSummary,
    pub entries: Vec<DiffEntry>,
}

fn common_prefix_len(a: &[u32], b: &[u32]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

fn common_suffix_len(a: &[u32], b: &[u32], cap: usize) -> usize {
    let raw = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    raw.min(cap)
}

/// Resolve a [`DiffRequest`] to an absolute candidate GGUF path.
pub fn resolve_candidate_path(req: &DiffRequest) -> Result<PathBuf> {
    let llama_cpp = req.candidate_llama_cpp.unwrap_or(false);
    let sources = [
        req.candidate_path.is_some(),
        req.candidate_ollama_model.is_some(),
        llama_cpp,
    ]
    .iter()
    .filter(|&&v| v)
    .count();
    if sources > 1 {
        return Err(anyhow!(
            "Specify exactly one of candidate_path, candidate_ollama_model, or candidate_llama_cpp"
        ));
    }
    if let Some(p) = &req.candidate_path {
        let pb = PathBuf::from(p);
        if !pb.exists() {
            return Err(anyhow!("candidate_path does not exist: {}", p));
        }
        return Ok(pb);
    }
    if let Some(m) = &req.candidate_ollama_model {
        return resolve_ollama_gguf_path(m)
            .with_context(|| format!("Failed to resolve Ollama model {:?}", m));
    }
    if llama_cpp {
        return resolve_llama_server_gguf_path().context("Failed to resolve llama.cpp model path");
    }
    Err(anyhow!(
        "Must specify candidate_path, candidate_ollama_model, or candidate_llama_cpp"
    ))
}

/// Run the diff. Loads the candidate tokenizer fresh — does not touch the
/// live `TokenCounterHandle`.
pub fn compute_diff(req: &DiffRequest) -> Result<DiffReport> {
    let candidate_path = resolve_candidate_path(req)?;
    let candidate = GgufTokenCounter::from_gguf_file(&candidate_path)
        .with_context(|| format!("Failed to load candidate {:?}", candidate_path))?;

    // Fetch the entire sample. The cap inside `golden_sample::list` clamps to
    // 1000, which is well above DEFAULT_CAPACITY (100) and safely covers the
    // documented max (5000) for typical use. If a user sets GOLDEN_SAMPLE_SIZE
    // above 1000 we still only diff the first 1000 entries.
    let all_entries = golden_sample::list(1000);

    let mut baseline_tokenizer_model: Option<String> = None;
    let mut diffs: Vec<DiffEntry> = Vec::new();
    let mut entries_skipped: usize = 0;
    let mut total_baseline_tokens: u64 = 0;
    let mut total_candidate_tokens: u64 = 0;
    let mut entries_identical: usize = 0;
    let mut entries_count_changed: usize = 0;
    let mut entries_ids_changed: usize = 0;
    let mut sum_signed_delta: i64 = 0;
    let mut sum_abs_delta: u64 = 0;
    let mut max_abs_delta: u32 = 0;

    for entry in all_entries {
        let Some(baseline_ids) = entry.baseline_token_ids.clone() else {
            entries_skipped += 1;
            continue;
        };
        if baseline_tokenizer_model.is_none() {
            baseline_tokenizer_model = Some(entry.tokenizer_model.clone());
        }
        let candidate_ids = candidate.encode_ids(&entry.chunk_text).unwrap_or_default();

        let baseline_count = baseline_ids.len();
        let candidate_count = candidate_ids.len();
        let count_delta = candidate_count as i32 - baseline_count as i32;
        let ids_match = baseline_ids == candidate_ids;

        let prefix = common_prefix_len(&baseline_ids, &candidate_ids);
        let suffix_cap = baseline_count.min(candidate_count).saturating_sub(prefix);
        let suffix = common_suffix_len(&baseline_ids, &candidate_ids, suffix_cap);

        total_baseline_tokens += baseline_count as u64;
        total_candidate_tokens += candidate_count as u64;
        if ids_match {
            entries_identical += 1;
        } else {
            entries_ids_changed += 1;
        }
        if count_delta != 0 {
            entries_count_changed += 1;
        }
        sum_signed_delta += count_delta as i64;
        let abs_delta = count_delta.unsigned_abs();
        sum_abs_delta += abs_delta as u64;
        if abs_delta > max_abs_delta {
            max_abs_delta = abs_delta;
        }

        diffs.push(DiffEntry {
            id: entry.id,
            chunk_text: entry.chunk_text,
            position_in_corpus: entry.position_in_corpus,
            baseline_count,
            candidate_count,
            count_delta,
            ids_match,
            common_prefix_len: prefix,
            common_suffix_len: suffix,
            baseline_token_ids: baseline_ids,
            candidate_token_ids: candidate_ids,
        });
    }

    let entries_total = diffs.len();
    let mean_count_delta = if entries_total > 0 {
        sum_signed_delta as f64 / entries_total as f64
    } else {
        0.0
    };
    let mean_count_delta_abs = if entries_total > 0 {
        sum_abs_delta as f64 / entries_total as f64
    } else {
        0.0
    };
    let total_delta_pct = if total_baseline_tokens > 0 {
        Some(
            (total_candidate_tokens as f64 - total_baseline_tokens as f64)
                / total_baseline_tokens as f64
                * 100.0,
        )
    } else {
        None
    };

    // Sort by absolute delta descending, then by ids_match (false first), so
    // the most interesting entries appear at the top of the response.
    diffs.sort_by(|a, b| {
        let ad = a.count_delta.unsigned_abs();
        let bd = b.count_delta.unsigned_abs();
        bd.cmp(&ad)
            .then_with(|| a.ids_match.cmp(&b.ids_match))
            .then_with(|| a.position_in_corpus.cmp(&b.position_in_corpus))
    });

    let limit = req.limit.unwrap_or(50).clamp(1, 1000);
    let entries: Vec<DiffEntry> = diffs.into_iter().take(limit).collect();

    Ok(DiffReport {
        candidate_path: candidate_path.display().to_string(),
        candidate_model_name: candidate.model_name().to_string(),
        candidate_vocab_size: candidate.vocab_size(),
        baseline_tokenizer_model,
        generated_at: Utc::now().to_rfc3339(),
        summary: DiffSummary {
            entries_total,
            entries_skipped,
            entries_identical,
            entries_count_changed,
            entries_ids_changed,
            total_baseline_tokens,
            total_candidate_tokens,
            total_delta_pct,
            mean_count_delta,
            mean_count_delta_abs,
            max_count_delta_abs: max_abs_delta,
        },
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_and_suffix_identical() {
        let a: Vec<u32> = vec![1, 2, 3, 4];
        let b: Vec<u32> = vec![1, 2, 3, 4];
        assert_eq!(common_prefix_len(&a, &b), 4);
        let cap = a.len().min(b.len()).saturating_sub(4);
        assert_eq!(common_suffix_len(&a, &b, cap), 0);
    }

    #[test]
    fn prefix_and_suffix_diverge_in_middle() {
        let a: Vec<u32> = vec![1, 2, 9, 9, 5, 6];
        let b: Vec<u32> = vec![1, 2, 7, 7, 7, 5, 6];
        let p = common_prefix_len(&a, &b);
        assert_eq!(p, 2);
        let cap = a.len().min(b.len()).saturating_sub(p);
        let s = common_suffix_len(&a, &b, cap);
        assert_eq!(s, 2);
    }

    #[test]
    fn prefix_and_suffix_no_overlap_after_cap() {
        // a fully contained in prefix-extended b — naive suffix would
        // double-count.
        let a: Vec<u32> = vec![1, 2, 3];
        let b: Vec<u32> = vec![1, 2, 3, 1, 2, 3];
        let p = common_prefix_len(&a, &b);
        assert_eq!(p, 3);
        let cap = a.len().min(b.len()).saturating_sub(p);
        let s = common_suffix_len(&a, &b, cap);
        assert_eq!(s, 0);
    }

    #[test]
    fn resolve_candidate_path_requires_one_field() {
        let req = DiffRequest {
            candidate_path: None,
            candidate_ollama_model: None,
            candidate_llama_cpp: None,
            limit: None,
        };
        assert!(resolve_candidate_path(&req).is_err());
        let req = DiffRequest {
            candidate_path: Some("/x".into()),
            candidate_ollama_model: Some("y".into()),
            candidate_llama_cpp: None,
            limit: None,
        };
        assert!(resolve_candidate_path(&req).is_err());
    }
}
