// File: src/monitoring/trace_alerting.rs
// Purpose: Trace-based alerting by querying Tempo for anomalies
// Version: 1.0.0
//
// Queries Tempo every 30 seconds for trace anomalies:
// - High latency spans (> threshold)
// - Error status codes
// - Unusual patterns
//
// Sends alerts via webhook when anomalies are detected.

use crate::monitoring::metrics::REGISTRY;
use once_cell::sync::Lazy;
use prometheus::{IntCounterVec, Opts};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Configuration for trace-based alerting
#[derive(Debug, Clone)]
pub struct TraceAlertingConfig {
    /// Enable trace alerting
    pub enabled: bool,
    /// Tempo API endpoint (e.g., http://localhost:3200 or https://localhost:3200)
    pub tempo_url: String,
    /// Whether to skip TLS verification for Tempo (useful for self-signed certs in dev)
    pub insecure_tls: bool,
    /// Alert check interval in seconds (default: 30)
    pub interval_secs: u64,
    /// Latency threshold in milliseconds (default: 1000)
    pub latency_threshold_ms: u64,
    /// Error rate threshold (0.0-1.0, default: 0.05 = 5%)
    pub error_rate_threshold: f64,
    /// Webhook URL for alerts (optional)
    pub webhook_url: Option<String>,
    /// Lookback window in seconds (default: 60)
    pub lookback_window_secs: u64,
}

// Prometheus metrics for trace alerting
pub static TRACE_ANOMALIES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let opts = Opts::new(
        "trace_anomalies_total",
        "Total number of trace anomalies detected by type",
    );
    let cv = IntCounterVec::new(opts, &["type"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

pub static TRACE_ALERT_CHECKS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let opts = Opts::new(
        "trace_alert_checks_total",
        "Total number of trace alert checks by status",
    );
    let cv = IntCounterVec::new(opts, &["status"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

impl TraceAlertingConfig {
    /// Load configuration from environment variables
    ///
    /// Environment variables:
    /// - `TEMPO_ENABLED`: Enable trace alerting (default: false)
    /// - `TEMPO_URL`: Tempo API endpoint (default: http://127.0.0.1:3200)
    /// - `TEMPO_ALERT_INSECURE_TLS`: Skip TLS verification for Tempo HTTPS endpoint (default: false)
    /// - `TEMPO_ALERT_INTERVAL_SECS`: Check interval (default: 30)
    /// - `TEMPO_LATENCY_THRESHOLD_MS`: Latency threshold (default: 1000)
    /// - `TEMPO_ERROR_RATE_THRESHOLD`: Error rate threshold (default: 0.05)
    /// - `TEMPO_ALERT_WEBHOOK_URL`: Webhook URL for alerts
    /// - `TEMPO_LOOKBACK_WINDOW_SECS`: Lookback window (default: 60)
    pub fn from_env() -> Self {
        let enabled = env::var("TEMPO_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let tempo_url =
            env::var("TEMPO_URL").unwrap_or_else(|_| "http://127.0.0.1:3200".to_string());

        let insecure_tls = env::var("TEMPO_ALERT_INSECURE_TLS")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        let interval_secs = env::var("TEMPO_ALERT_INTERVAL_SECS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u64>()
            .unwrap_or(30);

        let latency_threshold_ms = env::var("TEMPO_LATENCY_THRESHOLD_MS")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u64>()
            .unwrap_or(1000);

        let error_rate_threshold = env::var("TEMPO_ERROR_RATE_THRESHOLD")
            .unwrap_or_else(|_| "0.05".to_string())
            .parse::<f64>()
            .unwrap_or(0.05);

        let webhook_url = env::var("TEMPO_ALERT_WEBHOOK_URL").ok();

        let lookback_window_secs = env::var("TEMPO_LOOKBACK_WINDOW_SECS")
            .unwrap_or_else(|_| "60".to_string())
            .parse::<u64>()
            .unwrap_or(60);

        if enabled {
            info!(
                tempo_url = %tempo_url,
                insecure_tls = insecure_tls,
                interval_secs = interval_secs,
                latency_threshold_ms = latency_threshold_ms,
                error_rate_threshold = error_rate_threshold,
                lookback_window_secs = lookback_window_secs,
                webhook_url = ?webhook_url,
                "Trace-based alerting enabled"
            );
        } else {
            debug!("Trace-based alerting disabled (set TEMPO_ENABLED=true to enable)");
        }

        Self {
            enabled,
            tempo_url,
            insecure_tls,
            interval_secs,
            latency_threshold_ms,
            error_rate_threshold,
            webhook_url,
            lookback_window_secs,
        }
    }

    /// Check if alerting is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Trace anomaly event for alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceAnomalyEvent {
    /// Type of anomaly: "high_latency", "error_status", "high_error_rate"
    pub anomaly_type: String,
    /// Trace ID (if applicable)
    pub trace_id: Option<String>,
    /// Span name (if applicable)
    pub span_name: Option<String>,
    /// Duration in milliseconds (if applicable)
    pub duration_ms: Option<u64>,
    /// Error message (if applicable)
    pub error_message: Option<String>,
    /// Number of affected traces
    pub affected_traces: u64,
    /// Total traces analyzed
    pub total_traces: u64,
    /// Unix timestamp
    pub timestamp: u64,
}

impl TraceAnomalyEvent {
    /// Create a high latency anomaly event
    pub fn high_latency(trace_id: String, span_name: String, duration_ms: u64) -> Self {
        Self {
            anomaly_type: "high_latency".to_string(),
            trace_id: Some(trace_id),
            span_name: Some(span_name),
            duration_ms: Some(duration_ms),
            error_message: None,
            affected_traces: 1,
            total_traces: 0,
            timestamp: current_timestamp(),
        }
    }

    /// Create an error status anomaly event
    pub fn error_status(trace_id: String, span_name: String, error_message: String) -> Self {
        Self {
            anomaly_type: "error_status".to_string(),
            trace_id: Some(trace_id),
            span_name: Some(span_name),
            duration_ms: None,
            error_message: Some(error_message),
            affected_traces: 1,
            total_traces: 0,
            timestamp: current_timestamp(),
        }
    }

    /// Create a high error rate anomaly event
    pub fn high_error_rate(affected_traces: u64, total_traces: u64) -> Self {
        Self {
            anomaly_type: "high_error_rate".to_string(),
            trace_id: None,
            span_name: None,
            duration_ms: None,
            error_message: None,
            affected_traces,
            total_traces,
            timestamp: current_timestamp(),
        }
    }

    /// Convert to JSON payload for webhook
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "anomaly_type": self.anomaly_type,
            "trace_id": self.trace_id,
            "span_name": self.span_name,
            "duration_ms": self.duration_ms,
            "error_message": self.error_message,
            "affected_traces": self.affected_traces,
            "total_traces": self.total_traces,
            "timestamp": self.timestamp,
        })
    }
}

/// Tempo API response for search
#[derive(Debug, Deserialize)]
struct TempoSearchResponse {
    traces: Vec<TempoTrace>,
}

/// Tempo trace metadata
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TempoTrace {
    #[serde(rename = "traceID")]
    trace_id: String,
    #[serde(rename = "rootServiceName")]
    root_service_name: Option<String>,
    #[serde(rename = "rootTraceName")]
    root_trace_name: Option<String>,
    #[serde(rename = "startTimeUnixNano")]
    start_time_unix_nano: Option<String>,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
}

/// Start the trace alerting background task
///
/// Spawns a tokio task that runs every `interval_secs` seconds,
/// queries Tempo for anomalies, and sends alerts via webhook.
///
/// # Arguments
/// * `config` - Trace alerting configuration
///
/// # Returns
/// Handle to the spawned task (can be used to cancel)
pub fn start_trace_alerting(config: TraceAlertingConfig) -> tokio::task::JoinHandle<()> {
    info!("Starting trace alerting background task...");

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(config.interval_secs));
        let mut tempo_available = true;

        loop {
            interval.tick().await;

            if !config.is_enabled() {
                debug!("Trace alerting disabled, skipping check");
                continue;
            }

            if let Err(e) = ensure_tempo_reachable(&config).await {
                if tempo_available {
                    warn!(
                        error = ?e,
                        tempo_url = %config.tempo_url,
                        "Tempo endpoint unreachable, suspending trace alerting until it comes back"
                    );
                    tempo_available = false;
                } else {
                    debug!("Tempo endpoint still unreachable, skipping trace alerting check");
                }
                continue;
            } else if !tempo_available {
                info!(tempo_url = %config.tempo_url, "Tempo reachable again, resuming trace alerting checks");
                tempo_available = true;
            }

            debug!("Running trace anomaly check...");

            match check_for_anomalies(&config).await {
                Ok(anomalies) => {
                    TRACE_ALERT_CHECKS_TOTAL.with_label_values(&["ok"]).inc();
                    if anomalies.is_empty() {
                        debug!("No anomalies detected");
                    } else {
                        info!(count = anomalies.len(), "Detected trace anomalies");

                        for anomaly in anomalies {
                            send_anomaly_alert(&config, anomaly).await;
                        }
                    }
                }
                Err(e) => {
                    TRACE_ALERT_CHECKS_TOTAL.with_label_values(&["error"]).inc();
                    warn!(error = ?e, "Failed to check for trace anomalies");
                }
            }
        }
    })
}

/// Check Tempo for trace anomalies
///
/// Queries Tempo API for recent traces and analyzes them for:
/// - High latency spans
/// - Error status codes
/// - High error rate
///
/// # Arguments
/// * `config` - Trace alerting configuration
///
/// # Returns
/// List of detected anomalies
async fn check_for_anomalies(
    config: &TraceAlertingConfig,
) -> Result<Vec<TraceAnomalyEvent>, Box<dyn std::error::Error + Send + Sync>> {
    let mut anomalies = Vec::new();

    // Calculate time range for query
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let start_time = now - config.lookback_window_secs;

    // Query Tempo for recent traces
    let client = build_tempo_client(config)?;
    let search_url = format!("{}/api/search", config.tempo_url);

    debug!(
        url = %search_url,
        start_time = start_time,
        end_time = now,
        "Querying Tempo for traces"
    );

    // Query for all traces in the time window
    let response = client
        .get(&search_url)
        .query(&[
            ("start", start_time.to_string()),
            ("end", now.to_string()),
            ("limit", "100".to_string()),
        ])
        .timeout(Duration::from_millis(100)) // ~100ms per alert check
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Tempo API error: {}", response.status()).into());
    }

    let search_result: TempoSearchResponse = response.json().await?;
    let total_traces = search_result.traces.len() as u64;

    debug!(total_traces = total_traces, "Retrieved traces from Tempo");

    if total_traces == 0 {
        return Ok(anomalies);
    }

    // Analyze traces for anomalies

    for trace in search_result.traces {
        // Check for high latency
        if let Some(duration_ms) = trace.duration_ms {
            if duration_ms > config.latency_threshold_ms {
                let span_name = trace
                    .root_trace_name
                    .unwrap_or_else(|| "unknown".to_string());

                anomalies.push(TraceAnomalyEvent::high_latency(
                    trace.trace_id.clone(),
                    span_name,
                    duration_ms,
                ));
                TRACE_ANOMALIES_TOTAL
                    .with_label_values(&["high_latency"])
                    .inc();
            }
        }

        // Check for errors (we'd need to fetch full trace details for this)
        // For now, we'll use a simplified approach based on trace metadata
        // In production, you'd query /api/traces/{traceID} for full details

        // Note: Tempo's /api/search doesn't include error status in metadata
        // To detect errors, we need to either:
        // 1. Use TraceQL: /api/v2/search with query like {status=error}
        // 2. Fetch full trace details for each trace
        // For performance, we'll use TraceQL in a separate query
    }

    // Query for error traces using TraceQL (if supported)
    let error_traces = query_error_traces(config, start_time, now).await?;
    let error_count = error_traces.len() as u64;

    for trace in error_traces {
        let span_name = trace
            .root_trace_name
            .unwrap_or_else(|| "unknown".to_string());

        anomalies.push(TraceAnomalyEvent::error_status(
            trace.trace_id,
            span_name,
            "Trace contains error status".to_string(),
        ));
        TRACE_ANOMALIES_TOTAL
            .with_label_values(&["error_status"])
            .inc();
    }

    // Check for high error rate
    if total_traces > 0 {
        let error_rate = error_count as f64 / total_traces as f64;

        if error_rate > config.error_rate_threshold {
            anomalies.push(TraceAnomalyEvent::high_error_rate(
                error_count,
                total_traces,
            ));
            TRACE_ANOMALIES_TOTAL
                .with_label_values(&["high_error_rate"])
                .inc();
        }
    }

    Ok(anomalies)
}

/// Query Tempo for traces with error status using TraceQL
///
/// Uses Tempo's TraceQL API to find traces with error status.
/// Falls back to empty list if TraceQL is not supported.
async fn query_error_traces(
    config: &TraceAlertingConfig,
    start_time: u64,
    end_time: u64,
) -> Result<Vec<TempoTrace>, Box<dyn std::error::Error + Send + Sync>> {
    let client = build_tempo_client(config)?;
    let search_url = format!("{}/api/search", config.tempo_url);

    // Try to query for error traces
    // Note: TraceQL syntax varies by Tempo version
    // This is a simplified approach - adjust based on your Tempo version
    let response = client
        .get(&search_url)
        .query(&[
            ("start", start_time.to_string()),
            ("end", end_time.to_string()),
            ("q", "{status=error}".to_string()), // TraceQL query
            ("limit", "100".to_string()),
        ])
        .timeout(Duration::from_millis(100))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let search_result: TempoSearchResponse = resp.json().await?;
            Ok(search_result.traces)
        }
        Ok(resp) => {
            debug!(
                status = %resp.status(),
                "TraceQL query failed, skipping error detection"
            );
            Ok(Vec::new())
        }
        Err(e) => {
            debug!(error = ?e, "TraceQL query failed, skipping error detection");
            Ok(Vec::new())
        }
    }
}

/// Send anomaly alert via webhook
///
/// Spawns a non-blocking task to send the alert.
/// Failures are logged as warnings.
async fn send_anomaly_alert(config: &TraceAlertingConfig, anomaly: TraceAnomalyEvent) {
    let webhook_url = match &config.webhook_url {
        Some(url) => url.clone(),
        None => {
            debug!("No webhook URL configured, skipping alert");
            return;
        }
    };

    let payload = anomaly.to_json();
    let anomaly_type = anomaly.anomaly_type.clone();

    // Spawn non-blocking task
    tokio::spawn(async move {
        match send_webhook(&webhook_url, &payload).await {
            Ok(_) => {
                info!(
                    webhook_url = %webhook_url,
                    anomaly_type = %anomaly_type,
                    "Trace anomaly alert sent successfully"
                );
            }
            Err(e) => {
                warn!(
                    webhook_url = %webhook_url,
                    anomaly_type = %anomaly_type,
                    error = ?e,
                    "Failed to send trace anomaly alert (non-fatal)"
                );
            }
        }
    });
}

async fn ensure_tempo_reachable(
    config: &TraceAlertingConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = build_tempo_client(config)?;
    let health_url = format!("{}/ready", config.tempo_url.trim_end_matches('/'));

    let response = client
        .get(&health_url)
        .timeout(Duration::from_millis(250))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("Tempo health check failed: {}", response.status()).into())
    }
}

fn build_tempo_client(
    config: &TraceAlertingConfig,
) -> Result<reqwest::Client, Box<dyn std::error::Error + Send + Sync>> {
    let mut builder = reqwest::Client::builder();

    if config.tempo_url.starts_with("https://") && config.insecure_tls {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder
        .build()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
}

/// Send webhook request
async fn send_webhook(
    url: &str,
    payload: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "HTTP {}: {}",
            response.status(),
            response.text().await.unwrap_or_default()
        )
        .into())
    }
}

/// Get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_disabled_by_default() {
        std::env::remove_var("TEMPO_ENABLED");
        let config = TraceAlertingConfig::from_env();
        assert!(!config.is_enabled());
    }

    #[test]
    fn test_config_enabled() {
        std::env::set_var("TEMPO_ENABLED", "true");
        let config = TraceAlertingConfig::from_env();
        assert!(config.is_enabled());
        std::env::remove_var("TEMPO_ENABLED");
    }

    #[test]
    fn test_high_latency_event() {
        let event = TraceAnomalyEvent::high_latency(
            "trace123".to_string(),
            "GET /api/search".to_string(),
            1500,
        );
        assert_eq!(event.anomaly_type, "high_latency");
        assert_eq!(event.trace_id, Some("trace123".to_string()));
        assert_eq!(event.duration_ms, Some(1500));
    }

    #[test]
    fn test_error_status_event() {
        let event = TraceAnomalyEvent::error_status(
            "trace456".to_string(),
            "POST /upload".to_string(),
            "Internal server error".to_string(),
        );
        assert_eq!(event.anomaly_type, "error_status");
        assert_eq!(
            event.error_message,
            Some("Internal server error".to_string())
        );
    }

    #[test]
    fn test_high_error_rate_event() {
        let event = TraceAnomalyEvent::high_error_rate(10, 100);
        assert_eq!(event.anomaly_type, "high_error_rate");
        assert_eq!(event.affected_traces, 10);
        assert_eq!(event.total_traces, 100);
    }

    #[test]
    fn test_event_to_json() {
        let event = TraceAnomalyEvent::high_latency(
            "trace123".to_string(),
            "GET /api/search".to_string(),
            1500,
        );
        let json = event.to_json();
        assert_eq!(json["anomaly_type"], "high_latency");
        assert_eq!(json["trace_id"], "trace123");
        assert_eq!(json["duration_ms"], 1500);
    }
}
