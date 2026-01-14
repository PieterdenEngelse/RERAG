// src/monitoring/tool_alerts.rs
// Feature #8: Alerting on tool failures via webhooks

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Alert configuration for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAlertConfig {
    /// Whether alerting is enabled for this tool
    pub enabled: bool,
    /// Failure rate threshold (0.0 - 1.0) to trigger alert
    pub failure_rate_threshold: f64,
    /// Minimum executions before alerting
    pub min_executions: usize,
    /// Time window in seconds for calculating failure rate
    pub window_secs: u64,
    /// Cooldown between alerts in seconds
    pub cooldown_secs: u64,
    /// Latency threshold in ms to trigger alert
    pub latency_threshold_ms: Option<u64>,
}

impl Default for ToolAlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            failure_rate_threshold: 0.3, // 30% failure rate
            min_executions: 10,
            window_secs: 300,                 // 5 minutes
            cooldown_secs: 600,               // 10 minutes between alerts
            latency_threshold_ms: Some(5000), // 5 second latency
        }
    }
}

/// Alert severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Warning,
    Error,
    Critical,
}

/// Alert type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    HighFailureRate { rate: f64, threshold: f64 },
    HighLatency { latency_ms: u64, threshold_ms: u64 },
    ToolDisabled { reason: String },
    ConsecutiveFailures { count: usize },
}

/// Alert record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAlert {
    pub id: String,
    pub tool_type: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: String,
    pub acknowledged: bool,
    pub webhook_sent: bool,
}

/// Execution record for alert tracking
struct AlertExecutionRecord {
    timestamp: Instant,
    success: bool,
    latency_ms: u64,
}

/// Per-tool alert state
struct ToolAlertState {
    config: ToolAlertConfig,
    recent_executions: Vec<AlertExecutionRecord>,
    last_alert_time: Option<Instant>,
    consecutive_failures: usize,
}

impl ToolAlertState {
    fn new(config: ToolAlertConfig) -> Self {
        Self {
            config,
            recent_executions: Vec::new(),
            last_alert_time: None,
            consecutive_failures: 0,
        }
    }

    fn cleanup_old(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(self.config.window_secs);
        self.recent_executions.retain(|r| r.timestamp > cutoff);
    }

    fn failure_rate(&self) -> f64 {
        if self.recent_executions.is_empty() {
            return 0.0;
        }
        let failures = self.recent_executions.iter().filter(|r| !r.success).count();
        failures as f64 / self.recent_executions.len() as f64
    }

    fn avg_latency(&self) -> f64 {
        if self.recent_executions.is_empty() {
            return 0.0;
        }
        let total: u64 = self.recent_executions.iter().map(|r| r.latency_ms).sum();
        total as f64 / self.recent_executions.len() as f64
    }

    fn can_alert(&self) -> bool {
        if let Some(last) = self.last_alert_time {
            last.elapsed() > Duration::from_secs(self.config.cooldown_secs)
        } else {
            true
        }
    }
}

/// Global alert state
struct AlertState {
    tool_states: HashMap<String, ToolAlertState>,
    alerts: Vec<ToolAlert>,
    webhook_url: Option<String>,
    max_alerts: usize,
}

impl Default for AlertState {
    fn default() -> Self {
        let webhook_url = std::env::var("TOOL_ALERT_WEBHOOK_URL").ok();

        Self {
            tool_states: HashMap::new(),
            alerts: Vec::new(),
            webhook_url,
            max_alerts: 1000,
        }
    }
}

static ALERT_STATE: OnceLock<Mutex<AlertState>> = OnceLock::new();

fn get_state() -> &'static Mutex<AlertState> {
    ALERT_STATE.get_or_init(|| Mutex::new(AlertState::default()))
}

/// Record an execution and check for alerts
pub fn record_and_check(tool_type: &str, success: bool, latency_ms: u64) -> Option<ToolAlert> {
    let mut state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return None,
    };

    // Get or create tool state
    let tool_state = state
        .tool_states
        .entry(tool_type.to_string())
        .or_insert_with(|| ToolAlertState::new(ToolAlertConfig::default()));

    // Record execution
    tool_state.recent_executions.push(AlertExecutionRecord {
        timestamp: Instant::now(),
        success,
        latency_ms,
    });

    // Update consecutive failures
    if success {
        tool_state.consecutive_failures = 0;
    } else {
        tool_state.consecutive_failures += 1;
    }

    // Cleanup old records
    tool_state.cleanup_old();

    // Check if alerting is enabled and we have enough data
    if !tool_state.config.enabled {
        return None;
    }
    if tool_state.recent_executions.len() < tool_state.config.min_executions {
        return None;
    }
    if !tool_state.can_alert() {
        return None;
    }

    // Check for alert conditions
    let mut alert: Option<ToolAlert> = None;

    // Check failure rate
    let failure_rate = tool_state.failure_rate();
    if failure_rate >= tool_state.config.failure_rate_threshold {
        let severity = if failure_rate >= 0.8 {
            AlertSeverity::Critical
        } else if failure_rate >= 0.5 {
            AlertSeverity::Error
        } else {
            AlertSeverity::Warning
        };

        alert = Some(create_alert(
            tool_type,
            AlertType::HighFailureRate {
                rate: failure_rate,
                threshold: tool_state.config.failure_rate_threshold,
            },
            severity,
            format!(
                "Tool {} has {:.1}% failure rate (threshold: {:.1}%)",
                tool_type,
                failure_rate * 100.0,
                tool_state.config.failure_rate_threshold * 100.0
            ),
        ));
    }

    // Check latency
    if let Some(threshold) = tool_state.config.latency_threshold_ms {
        let avg_latency = tool_state.avg_latency();
        if avg_latency > threshold as f64 {
            let latency_alert = create_alert(
                tool_type,
                AlertType::HighLatency {
                    latency_ms: avg_latency as u64,
                    threshold_ms: threshold,
                },
                AlertSeverity::Warning,
                format!(
                    "Tool {} has high latency: {:.0}ms (threshold: {}ms)",
                    tool_type, avg_latency, threshold
                ),
            );
            // Only use latency alert if no failure alert
            if alert.is_none() {
                alert = Some(latency_alert);
            }
        }
    }

    // Check consecutive failures
    if tool_state.consecutive_failures >= 5 {
        let consec_alert = create_alert(
            tool_type,
            AlertType::ConsecutiveFailures {
                count: tool_state.consecutive_failures,
            },
            AlertSeverity::Error,
            format!(
                "Tool {} has {} consecutive failures",
                tool_type, tool_state.consecutive_failures
            ),
        );
        if alert.is_none() || tool_state.consecutive_failures >= 10 {
            alert = Some(consec_alert);
        }
    }

    // If we have an alert, record it and send webhook
    if let Some(ref mut a) = alert {
        tool_state.last_alert_time = Some(Instant::now());

        // Send webhook
        if let Some(ref url) = state.webhook_url {
            a.webhook_sent = send_webhook_sync(url, a);
        }

        // Store alert
        if state.alerts.len() >= state.max_alerts {
            state.alerts.remove(0);
        }
        state.alerts.push(a.clone());

        info!(
            tool = tool_type,
            severity = ?a.severity,
            "Tool alert triggered: {}",
            a.message
        );
    }

    alert
}

fn create_alert(
    tool_type: &str,
    alert_type: AlertType,
    severity: AlertSeverity,
    message: String,
) -> ToolAlert {
    ToolAlert {
        id: uuid::Uuid::new_v4().to_string(),
        tool_type: tool_type.to_string(),
        alert_type,
        severity,
        message,
        timestamp: Utc::now().to_rfc3339(),
        acknowledged: false,
        webhook_sent: false,
    }
}

fn send_webhook_sync(url: &str, alert: &ToolAlert) -> bool {
    // Use blocking HTTP client for simplicity
    // In production, this should be async
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create HTTP client: {}", e);
            return false;
        }
    };

    let payload = serde_json::json!({
        "alert_id": alert.id,
        "tool_type": alert.tool_type,
        "severity": format!("{:?}", alert.severity),
        "message": alert.message,
        "timestamp": alert.timestamp,
        "source": "ag-tools-monitor"
    });

    match client.post(url).json(&payload).send() {
        Ok(resp) => {
            if resp.status().is_success() {
                info!(url = url, "Alert webhook sent successfully");
                true
            } else {
                warn!(url = url, status = %resp.status(), "Alert webhook failed");
                false
            }
        }
        Err(e) => {
            error!(url = url, error = %e, "Alert webhook request failed");
            false
        }
    }
}

/// Get recent alerts
pub fn get_alerts(limit: usize) -> Vec<ToolAlert> {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    state.alerts.iter().rev().take(limit).cloned().collect()
}

/// Get alerts for a specific tool
pub fn get_tool_alerts(tool_type: &str, limit: usize) -> Vec<ToolAlert> {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    state
        .alerts
        .iter()
        .rev()
        .filter(|a| a.tool_type == tool_type)
        .take(limit)
        .cloned()
        .collect()
}

/// Acknowledge an alert
pub fn acknowledge_alert(alert_id: &str) -> bool {
    let mut state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return false,
    };

    if let Some(alert) = state.alerts.iter_mut().find(|a| a.id == alert_id) {
        alert.acknowledged = true;
        true
    } else {
        false
    }
}

/// Set alert configuration for a tool
pub fn set_alert_config(tool_type: &str, config: ToolAlertConfig) {
    if let Ok(mut state) = get_state().lock() {
        let entry = state
            .tool_states
            .entry(tool_type.to_string())
            .or_insert_with(|| ToolAlertState::new(config.clone()));
        entry.config = config;
    }
}

/// Get alert configuration for a tool
pub fn get_alert_config(tool_type: &str) -> ToolAlertConfig {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return ToolAlertConfig::default(),
    };

    state
        .tool_states
        .get(tool_type)
        .map(|s| s.config.clone())
        .unwrap_or_default()
}

/// Set webhook URL
pub fn set_webhook_url(url: Option<String>) {
    if let Ok(mut state) = get_state().lock() {
        state.webhook_url = url;
    }
}

/// Get current alert status for all tools
pub fn get_alert_status() -> Vec<ToolAlertStatus> {
    let state = match get_state().lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    state
        .tool_states
        .iter()
        .map(|(tool_type, tool_state)| ToolAlertStatus {
            tool_type: tool_type.clone(),
            enabled: tool_state.config.enabled,
            failure_rate: tool_state.failure_rate(),
            avg_latency_ms: tool_state.avg_latency(),
            consecutive_failures: tool_state.consecutive_failures,
            recent_executions: tool_state.recent_executions.len(),
            in_cooldown: !tool_state.can_alert(),
        })
        .collect()
}

/// Alert status for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAlertStatus {
    pub tool_type: String,
    pub enabled: bool,
    pub failure_rate: f64,
    pub avg_latency_ms: f64,
    pub consecutive_failures: usize,
    pub recent_executions: usize,
    pub in_cooldown: bool,
}

/// Manually trigger an alert (for testing or manual intervention)
pub fn trigger_manual_alert(tool_type: &str, message: &str, severity: AlertSeverity) -> ToolAlert {
    let alert = create_alert(
        tool_type,
        AlertType::ToolDisabled {
            reason: message.to_string(),
        },
        severity,
        message.to_string(),
    );

    if let Ok(mut state) = get_state().lock() {
        if state.alerts.len() >= state.max_alerts {
            state.alerts.remove(0);
        }
        state.alerts.push(alert.clone());
    }

    alert
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_success() {
        let alert = record_and_check("TestTool", true, 100);
        // Should not alert on success
        assert!(alert.is_none());
    }

    #[test]
    fn test_get_alerts() {
        let alerts = get_alerts(10);
        // Should return empty or existing alerts
        assert!(alerts.len() <= 10);
    }

    #[test]
    fn test_alert_config() {
        let config = ToolAlertConfig {
            enabled: true,
            failure_rate_threshold: 0.5,
            ..Default::default()
        };
        set_alert_config("ConfigTestTool", config.clone());
        let retrieved = get_alert_config("ConfigTestTool");
        assert_eq!(retrieved.failure_rate_threshold, 0.5);
    }
}
