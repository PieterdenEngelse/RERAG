// src/monitoring/tool_stats.rs
// Track tool execution statistics for monitoring

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// Maximum number of executions to keep in history
const MAX_HISTORY_SIZE: usize = 100;

/// Record of a single tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecution {
    pub tool_type: String,
    pub query: String,
    pub success: bool,
    pub result_preview: String,
    pub execution_time_ms: u64,
    pub confidence: f32,
    pub timestamp: String,
    pub source: Option<String>,
}

/// Aggregated statistics for a tool
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolAggregateStats {
    pub tool_type: String,
    pub total_calls: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub total_latency_ms: u64,
    pub avg_latency_ms: f64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub avg_confidence: f32,
    pub last_used: Option<String>,
}

/// LLM-specific latency stats from last N calls
#[derive(Clone, Debug, Serialize, Default)]
pub struct LlmLatencyStats {
    pub call_count: usize,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub last_ms: Option<u64>,
    pub calls_last_hour: usize,
    pub last_backend: Option<String>,
}

const LLM_LATENCY_HISTORY_SIZE: usize = 20;

struct LlmLatencyEntry {
    latency_ms: u64,
    timestamp: chrono::DateTime<chrono::Utc>,
    backend: String,
}

static LLM_LATENCIES: once_cell::sync::Lazy<std::sync::Mutex<VecDeque<LlmLatencyEntry>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(VecDeque::with_capacity(LLM_LATENCY_HISTORY_SIZE)));

/// Global tool execution history
struct ToolExecutionHistory {
    executions: VecDeque<ToolExecution>,
    stats: std::collections::HashMap<String, ToolAggregateStats>,
}

impl Default for ToolExecutionHistory {
    fn default() -> Self {
        Self {
            executions: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            stats: std::collections::HashMap::new(),
        }
    }
}

static TOOL_HISTORY: OnceLock<Mutex<ToolExecutionHistory>> = OnceLock::new();

fn get_history() -> &'static Mutex<ToolExecutionHistory> {
    TOOL_HISTORY.get_or_init(|| Mutex::new(ToolExecutionHistory::default()))
}

/// Record a tool execution
pub fn record_tool_execution(
    tool_type: &str,
    query: &str,
    success: bool,
    result: &str,
    execution_time_ms: u64,
    confidence: f32,
    source: Option<&str>,
) {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Create preview (truncate result)
    let result_preview = if result.len() > 100 {
        format!("{}...", &result[..100])
    } else {
        result.to_string()
    };

    let execution = ToolExecution {
        tool_type: tool_type.to_string(),
        query: if query.len() > 200 {
            format!("{}...", &query[..200])
        } else {
            query.to_string()
        },
        success,
        result_preview,
        execution_time_ms,
        confidence,
        timestamp: timestamp.clone(),
        source: source.map(|s| s.to_string()),
    };

    // Track LLM-specific latencies
    if tool_type == "LLMGenerate" {
        if let Ok(mut llm_hist) = LLM_LATENCIES.lock() {
            llm_hist.push_back(LlmLatencyEntry {
                latency_ms: execution_time_ms,
                timestamp: chrono::Utc::now(),
                backend: source.unwrap_or("unknown").to_string(),
            });
            while llm_hist.len() > LLM_LATENCY_HISTORY_SIZE {
                llm_hist.pop_front();
            }
        }
    }

    if let Ok(mut history) = get_history().lock() {
        // Add to execution history
        if history.executions.len() >= MAX_HISTORY_SIZE {
            history.executions.pop_back();
        }
        history.executions.push_front(execution);

        // Update aggregate stats
        let stats = history
            .stats
            .entry(tool_type.to_string())
            .or_insert_with(|| ToolAggregateStats {
                tool_type: tool_type.to_string(),
                min_latency_ms: u64::MAX,
                ..Default::default()
            });

        stats.total_calls += 1;
        if success {
            stats.success_count += 1;
        } else {
            stats.failure_count += 1;
        }
        stats.total_latency_ms += execution_time_ms;
        stats.avg_latency_ms = stats.total_latency_ms as f64 / stats.total_calls as f64;
        stats.min_latency_ms = stats.min_latency_ms.min(execution_time_ms);
        stats.max_latency_ms = stats.max_latency_ms.max(execution_time_ms);

        // Running average for confidence
        let prev_avg = stats.avg_confidence;
        let n = stats.total_calls as f32;
        stats.avg_confidence = prev_avg + (confidence - prev_avg) / n;

        stats.last_used = Some(timestamp);
    }
}

/// Get LLM latency statistics from last 20 calls
pub fn get_llm_latency_stats() -> LlmLatencyStats {
    let llm_hist = match LLM_LATENCIES.lock() {
        Ok(h) => h,
        Err(_) => return LlmLatencyStats::default(),
    };

    if llm_hist.is_empty() {
        return LlmLatencyStats::default();
    }

    let now = chrono::Utc::now();
    let hour_ago = now - chrono::Duration::hours(1);

    let mut latencies: Vec<u64> = llm_hist.iter().map(|e| e.latency_ms).collect();
    let calls_last_hour = llm_hist.iter().filter(|e| e.timestamp >= hour_ago).count();

    let call_count = latencies.len();
    let sum: u64 = latencies.iter().sum();
    let avg_ms = sum as f64 / call_count as f64;

    latencies.sort();
    let min_ms = latencies[0];
    let max_ms = latencies[call_count - 1];
    let last_ms = llm_hist.back().map(|e| e.latency_ms);

    // p95: for 20 items, index 18 (0.95 * 20 - 1 = 18)
    let p95_idx = ((call_count as f64) * 0.95).ceil() as usize - 1;
    let p95_idx = p95_idx.clamp(0, call_count - 1);
    let p95_ms = latencies[p95_idx] as f64;

    let last_backend = llm_hist.back().map(|e| e.backend.clone());

    LlmLatencyStats {
        call_count,
        avg_ms,
        p95_ms,
        min_ms,
        max_ms,
        last_ms,
        calls_last_hour,
        last_backend,
    }
}

/// Get recent tool executions
pub fn get_recent_executions(limit: usize) -> Vec<ToolExecution> {
    if let Ok(history) = get_history().lock() {
        history.executions.iter().take(limit).cloned().collect()
    } else {
        vec![]
    }
}

/// Get aggregate stats for all tools
pub fn get_tool_stats() -> Vec<ToolAggregateStats> {
    if let Ok(history) = get_history().lock() {
        history.stats.values().cloned().collect()
    } else {
        vec![]
    }
}

/// Get stats for a specific tool
pub fn get_tool_stat(tool_type: &str) -> Option<ToolAggregateStats> {
    if let Ok(history) = get_history().lock() {
        history.stats.get(tool_type).cloned()
    } else {
        None
    }
}

/// Clear all execution history
pub fn clear_history() {
    if let Ok(mut history) = get_history().lock() {
        history.executions.clear();
        history.stats.clear();
    }
}

/// Response structure for the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResponse {
    pub status: String,
    pub request_id: String,
    pub executions: Vec<ToolExecution>,
    pub count: usize,
}

/// Response structure for tool stats API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatsResponse {
    pub status: String,
    pub request_id: String,
    pub stats: Vec<ToolAggregateStats>,
    pub total_tools: usize,
    pub total_executions: usize,
}
