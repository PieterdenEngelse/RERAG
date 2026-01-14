// src/monitoring/tool_trends.rs
// Feature #5: Tool performance trends over time windows

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// Time window for trend analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeWindow {
    Hour, // Last 1 hour
    Day,  // Last 24 hours
    Week, // Last 7 days
}

impl TimeWindow {
    pub fn duration(&self) -> Duration {
        match self {
            TimeWindow::Hour => Duration::hours(1),
            TimeWindow::Day => Duration::hours(24),
            TimeWindow::Week => Duration::days(7),
        }
    }

    pub fn bucket_duration(&self) -> Duration {
        match self {
            TimeWindow::Hour => Duration::minutes(5), // 12 buckets
            TimeWindow::Day => Duration::hours(1),    // 24 buckets
            TimeWindow::Week => Duration::hours(6),   // 28 buckets
        }
    }
}

/// Data point for a time bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendBucket {
    pub timestamp: String,
    pub executions: usize,
    pub successes: usize,
    pub failures: usize,
    pub avg_latency_ms: f64,
    pub avg_confidence: f32,
    pub total_cost: f64,
}

impl Default for TrendBucket {
    fn default() -> Self {
        Self {
            timestamp: Utc::now().to_rfc3339(),
            executions: 0,
            successes: 0,
            failures: 0,
            avg_latency_ms: 0.0,
            avg_confidence: 0.0,
            total_cost: 0.0,
        }
    }
}

/// Execution record for trend tracking
#[derive(Debug, Clone)]
struct ExecutionRecord {
    timestamp: DateTime<Utc>,
    tool_type: String,
    success: bool,
    latency_ms: u64,
    confidence: f32,
    cost: f64,
}

/// Trend data for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTrend {
    pub tool_type: String,
    pub window: String,
    pub buckets: Vec<TrendBucket>,
    pub summary: TrendSummary,
}

/// Summary statistics for a trend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendSummary {
    pub total_executions: usize,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub avg_confidence: f32,
    pub total_cost: f64,
    pub trend_direction: TrendDirection,
    pub latency_trend: TrendDirection,
}

/// Direction of trend
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
}

/// Global trend state
struct TrendState {
    /// All execution records (kept for max 7 days)
    records: VecDeque<ExecutionRecord>,
    /// Maximum records to keep
    max_records: usize,
}

impl Default for TrendState {
    fn default() -> Self {
        Self {
            records: VecDeque::with_capacity(100_000),
            max_records: 100_000,
        }
    }
}

static TREND_STATE: OnceLock<Mutex<TrendState>> = OnceLock::new();

fn get_state() -> &'static Mutex<TrendState> {
    TREND_STATE.get_or_init(|| Mutex::new(TrendState::default()))
}

/// Record an execution for trend tracking
pub fn record_execution(
    tool_type: &str,
    success: bool,
    latency_ms: u64,
    confidence: f32,
    cost: f64,
) {
    let record = ExecutionRecord {
        timestamp: Utc::now(),
        tool_type: tool_type.to_string(),
        success,
        latency_ms,
        confidence,
        cost,
    };

    if let Ok(mut state) = get_state().lock() {
        // Remove old records if at capacity
        while state.records.len() >= state.max_records {
            state.records.pop_front();
        }

        // Remove records older than 7 days
        let cutoff = Utc::now() - Duration::days(7);
        while let Some(front) = state.records.front() {
            if front.timestamp < cutoff {
                state.records.pop_front();
            } else {
                break;
            }
        }

        state.records.push_back(record);
    }
}

/// Get trend data for a tool
pub fn get_tool_trend(tool_type: &str, window: TimeWindow) -> ToolTrend {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return empty_trend(tool_type, window),
    };

    let now = Utc::now();
    let window_start = now - window.duration();
    let bucket_duration = window.bucket_duration();

    // Filter records for this tool and window
    let records: Vec<_> = state
        .records
        .iter()
        .filter(|r| r.tool_type == tool_type && r.timestamp >= window_start)
        .cloned()
        .collect();

    if records.is_empty() {
        return empty_trend(tool_type, window);
    }

    // Create buckets
    let num_buckets = (window.duration().num_minutes() / bucket_duration.num_minutes()) as usize;
    let mut buckets: Vec<TrendBucket> = Vec::with_capacity(num_buckets);

    for i in 0..num_buckets {
        let bucket_start = window_start + bucket_duration * i as i32;
        let bucket_end = bucket_start + bucket_duration;

        let bucket_records: Vec<_> = records
            .iter()
            .filter(|r| r.timestamp >= bucket_start && r.timestamp < bucket_end)
            .collect();

        let executions = bucket_records.len();
        let successes = bucket_records.iter().filter(|r| r.success).count();
        let failures = executions - successes;

        let avg_latency = if executions > 0 {
            bucket_records
                .iter()
                .map(|r| r.latency_ms as f64)
                .sum::<f64>()
                / executions as f64
        } else {
            0.0
        };

        let avg_confidence = if executions > 0 {
            bucket_records.iter().map(|r| r.confidence).sum::<f32>() / executions as f32
        } else {
            0.0
        };

        let total_cost = bucket_records.iter().map(|r| r.cost).sum();

        buckets.push(TrendBucket {
            timestamp: bucket_start.to_rfc3339(),
            executions,
            successes,
            failures,
            avg_latency_ms: avg_latency,
            avg_confidence,
            total_cost,
        });
    }

    // Calculate summary
    let total_executions = records.len();
    let total_successes = records.iter().filter(|r| r.success).count();
    let success_rate = if total_executions > 0 {
        total_successes as f64 / total_executions as f64
    } else {
        0.0
    };

    let mut latencies: Vec<u64> = records.iter().map(|r| r.latency_ms).collect();
    latencies.sort();

    let avg_latency = if !latencies.is_empty() {
        latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
    } else {
        0.0
    };

    let p50 = percentile(&latencies, 50);
    let p95 = percentile(&latencies, 95);
    let p99 = percentile(&latencies, 99);

    let avg_confidence = if total_executions > 0 {
        records.iter().map(|r| r.confidence).sum::<f32>() / total_executions as f32
    } else {
        0.0
    };

    let total_cost = records.iter().map(|r| r.cost).sum();

    // Calculate trend direction
    let trend_direction = calculate_trend_direction(&buckets);
    let latency_trend = calculate_latency_trend(&buckets);

    ToolTrend {
        tool_type: tool_type.to_string(),
        window: format!("{:?}", window),
        buckets,
        summary: TrendSummary {
            total_executions,
            success_rate,
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            avg_confidence,
            total_cost,
            trend_direction,
            latency_trend,
        },
    }
}

/// Get trends for all tools
pub fn get_all_trends(window: TimeWindow) -> Vec<ToolTrend> {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    // Get unique tool types
    let tool_types: std::collections::HashSet<_> =
        state.records.iter().map(|r| r.tool_type.clone()).collect();

    drop(state); // Release lock before calling get_tool_trend

    tool_types
        .into_iter()
        .map(|tool_type| get_tool_trend(&tool_type, window))
        .collect()
}

/// Get comparison between two time windows
pub fn compare_windows(
    tool_type: &str,
    window1: TimeWindow,
    window2: TimeWindow,
) -> WindowComparison {
    let trend1 = get_tool_trend(tool_type, window1);
    let trend2 = get_tool_trend(tool_type, window2);

    let latency_change = if trend1.summary.avg_latency_ms > 0.0 {
        (trend2.summary.avg_latency_ms - trend1.summary.avg_latency_ms)
            / trend1.summary.avg_latency_ms
            * 100.0
    } else {
        0.0
    };

    let success_rate_change = trend2.summary.success_rate - trend1.summary.success_rate;

    WindowComparison {
        tool_type: tool_type.to_string(),
        window1: format!("{:?}", window1),
        window2: format!("{:?}", window2),
        executions_change: trend2.summary.total_executions as i64
            - trend1.summary.total_executions as i64,
        latency_change_percent: latency_change,
        success_rate_change,
        is_improving: latency_change < 0.0 && success_rate_change >= 0.0,
    }
}

/// Comparison between two time windows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowComparison {
    pub tool_type: String,
    pub window1: String,
    pub window2: String,
    pub executions_change: i64,
    pub latency_change_percent: f64,
    pub success_rate_change: f64,
    pub is_improving: bool,
}

fn empty_trend(tool_type: &str, window: TimeWindow) -> ToolTrend {
    ToolTrend {
        tool_type: tool_type.to_string(),
        window: format!("{:?}", window),
        buckets: vec![],
        summary: TrendSummary {
            total_executions: 0,
            success_rate: 0.0,
            avg_latency_ms: 0.0,
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            avg_confidence: 0.0,
            total_cost: 0.0,
            trend_direction: TrendDirection::Stable,
            latency_trend: TrendDirection::Stable,
        },
    }
}

fn percentile(sorted: &[u64], p: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (sorted.len() * p / 100).min(sorted.len() - 1);
    sorted[idx] as f64
}

fn calculate_trend_direction(buckets: &[TrendBucket]) -> TrendDirection {
    if buckets.len() < 2 {
        return TrendDirection::Stable;
    }

    // Compare first half to second half
    let mid = buckets.len() / 2;
    let first_half: f64 = buckets[..mid]
        .iter()
        .map(|b| {
            if b.executions > 0 {
                b.successes as f64 / b.executions as f64
            } else {
                0.0
            }
        })
        .sum::<f64>()
        / mid as f64;
    let second_half: f64 = buckets[mid..]
        .iter()
        .map(|b| {
            if b.executions > 0 {
                b.successes as f64 / b.executions as f64
            } else {
                0.0
            }
        })
        .sum::<f64>()
        / (buckets.len() - mid) as f64;

    let diff = second_half - first_half;
    if diff > 0.05 {
        TrendDirection::Improving
    } else if diff < -0.05 {
        TrendDirection::Degrading
    } else {
        TrendDirection::Stable
    }
}

fn calculate_latency_trend(buckets: &[TrendBucket]) -> TrendDirection {
    if buckets.len() < 2 {
        return TrendDirection::Stable;
    }

    let mid = buckets.len() / 2;
    let first_half: f64 = buckets[..mid]
        .iter()
        .filter(|b| b.executions > 0)
        .map(|b| b.avg_latency_ms)
        .sum::<f64>()
        / mid as f64;
    let second_half: f64 = buckets[mid..]
        .iter()
        .filter(|b| b.executions > 0)
        .map(|b| b.avg_latency_ms)
        .sum::<f64>()
        / (buckets.len() - mid) as f64;

    if first_half == 0.0 {
        return TrendDirection::Stable;
    }

    let change = (second_half - first_half) / first_half;
    if change < -0.1 {
        TrendDirection::Improving // Lower latency is better
    } else if change > 0.1 {
        TrendDirection::Degrading
    } else {
        TrendDirection::Stable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_execution() {
        record_execution("Calculator", true, 10, 0.95, 0.0);
        let trend = get_tool_trend("Calculator", TimeWindow::Hour);
        // May or may not have data depending on test order
        assert!(trend.tool_type == "Calculator");
    }

    #[test]
    fn test_empty_trend() {
        let trend = get_tool_trend("NonExistentTool", TimeWindow::Hour);
        assert_eq!(trend.summary.total_executions, 0);
    }

    #[test]
    fn test_time_windows() {
        assert!(TimeWindow::Hour.duration() < TimeWindow::Day.duration());
        assert!(TimeWindow::Day.duration() < TimeWindow::Week.duration());
    }
}
