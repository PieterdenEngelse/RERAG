//! Rig agentic-mode statistics — counters and token budget tracking.
//!
//! All writes are atomic / lock-free counters except for the token history,
//! which uses a small Mutex-guarded ring buffer (≤ 100 entries).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

// ── Atomic session counters ───────────────────────────────────────────────────

static AGENTIC_CALLS: AtomicUsize = AtomicUsize::new(0);
static AGENTIC_FALLBACKS: AtomicUsize = AtomicUsize::new(0);
static RIG_TOOL_CALLS: AtomicUsize = AtomicUsize::new(0);
/// Total milliseconds across all agentic sessions (for avg latency)
static AGENTIC_TOTAL_MS: AtomicU64 = AtomicU64::new(0);

/// Called when a full agentic session completes (success or fallback)
pub fn record_agentic_call(duration_ms: u64) {
    AGENTIC_CALLS.fetch_add(1, Ordering::Relaxed);
    AGENTIC_TOTAL_MS.fetch_add(duration_ms, Ordering::Relaxed);
}

/// Called when Rig fails and the handler falls back to Classic/Hybrid
pub fn record_agentic_fallback() {
    AGENTIC_FALLBACKS.fetch_add(1, Ordering::Relaxed);
}

/// Called once per individual Rig tool call (each tool instruments itself)
pub fn record_rig_tool_call() {
    RIG_TOOL_CALLS.fetch_add(1, Ordering::Relaxed);
}

// ── Token ring buffer ─────────────────────────────────────────────────────────

const TOKEN_HISTORY_SIZE: usize = 100;

struct TokenEntry {
    tokens_in: usize,
    /// 0 when context_limit is unknown
    context_limit: usize,
    exact: bool,
}

static TOKEN_HISTORY: Mutex<Option<VecDeque<TokenEntry>>> = Mutex::new(None);

fn token_history() -> std::sync::MutexGuard<'static, Option<VecDeque<TokenEntry>>> {
    TOKEN_HISTORY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Record token counts for one agentic session.
/// `tokens_in`     — tokens in the prompt sent to the Rig agent
/// `context_limit` — num_ctx from hardware config (0 = unknown)
/// `exact`         — whether a GGUF tokenizer was used
pub fn record_token_usage(tokens_in: usize, context_limit: usize, exact: bool) {
    let mut guard = token_history();
    let history = guard.get_or_insert_with(|| VecDeque::with_capacity(TOKEN_HISTORY_SIZE));
    if history.len() >= TOKEN_HISTORY_SIZE {
        history.pop_front();
    }
    history.push_back(TokenEntry {
        tokens_in,
        context_limit,
        exact,
    });
}

// ── Snapshot read ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct RigStatsSnapshot {
    pub agentic_calls_total: usize,
    pub agentic_fallbacks_total: usize,
    pub rig_tool_calls_total: usize,
    pub avg_session_ms: f64,
    pub fallback_rate_pct: f64,

    // Token stats across last ≤100 sessions
    pub avg_tokens_in: f64,
    pub max_tokens_in: usize,
    pub avg_ctx_utilization_pct: f64,
    /// "exact" | "heuristic" | "unknown"
    pub counter_type: String,
    pub token_sample_count: usize,
}

pub fn snapshot() -> RigStatsSnapshot {
    let calls = AGENTIC_CALLS.load(Ordering::Relaxed);
    let fallbacks = AGENTIC_FALLBACKS.load(Ordering::Relaxed);
    let tool_calls = RIG_TOOL_CALLS.load(Ordering::Relaxed);
    let total_ms = AGENTIC_TOTAL_MS.load(Ordering::Relaxed);

    let avg_session_ms = if calls > 0 {
        total_ms as f64 / calls as f64
    } else {
        0.0
    };
    let fallback_rate_pct = if calls > 0 {
        (fallbacks as f64 / calls as f64) * 100.0
    } else {
        0.0
    };

    // Token stats
    let guard = token_history();
    let (avg_tokens_in, max_tokens_in, avg_ctx_pct, counter_type, sample_count) =
        match guard.as_ref() {
            None => (0.0, 0, 0.0, "unknown".to_string(), 0),
            Some(h) if h.is_empty() => (0.0, 0, 0.0, "unknown".to_string(), 0),
            Some(h) => {
                let n = h.len();
                let sum_tokens: usize = h.iter().map(|e| e.tokens_in).sum();
                let max_tokens = h.iter().map(|e| e.tokens_in).max().unwrap_or(0);
                let avg_tok = sum_tokens as f64 / n as f64;

                let util_entries: Vec<f64> = h
                    .iter()
                    .filter(|e| e.context_limit > 0)
                    .map(|e| e.tokens_in as f64 / e.context_limit as f64 * 100.0)
                    .collect();
                let avg_util = if util_entries.is_empty() {
                    0.0
                } else {
                    util_entries.iter().sum::<f64>() / util_entries.len() as f64
                };

                let exact_count = h.iter().filter(|e| e.exact).count();
                let ctype = if exact_count == 0 {
                    "heuristic".to_string()
                } else if exact_count == n {
                    "exact".to_string()
                } else {
                    format!("mixed ({}/{} exact)", exact_count, n)
                };

                (avg_tok, max_tokens, avg_util, ctype, n)
            }
        };

    RigStatsSnapshot {
        agentic_calls_total: calls,
        agentic_fallbacks_total: fallbacks,
        rig_tool_calls_total: tool_calls,
        avg_session_ms,
        fallback_rate_pct,
        avg_tokens_in,
        max_tokens_in,
        avg_ctx_utilization_pct: avg_ctx_pct,
        counter_type,
        token_sample_count: sample_count,
    }
}
