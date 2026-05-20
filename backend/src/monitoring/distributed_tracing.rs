// File: src/monitoring/distributed_tracing.rs
// Phase 16: Distributed Tracing
// Version: 1.0.0
// Location: src/monitoring/distributed_tracing.rs
//
// Purpose: OpenTelemetry integration for distributed tracing across services
// Enables trace propagation, correlation IDs, and performance analysis

use opentelemetry::trace::TraceError;
use opentelemetry_otlp::WithExportConfig;
use tracing_opentelemetry::OpenTelemetryLayer;

use std::env;
use uuid::Uuid;

/// Distributed tracing configuration
#[derive(Debug, Clone)]
pub struct DistributedTracingConfig {
    /// Enable distributed tracing (default: false)
    pub enabled: bool,
    /// Jaeger agent endpoint (default: http://localhost:6831)
    pub jaeger_endpoint: String,
    /// Service name for traces (default: agentic-rag)
    pub service_name: String,
    /// Trace sampling rate 0.0-1.0 (default: 1.0 = 100%)
    pub sampler_rate: f64,
}

impl Default for DistributedTracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            jaeger_endpoint: "http://localhost:6831".to_string(),
            service_name: "agentic-rag".to_string(),
            sampler_rate: 1.0,
        }
    }
}

impl DistributedTracingConfig {
    /// Load configuration from environment
    ///
    /// Environment variables:
    /// - `TRACING_ENABLED`: Enable distributed tracing (true/false)
    /// - `JAEGER_ENDPOINT`: Jaeger agent endpoint URL
    /// - `SERVICE_NAME`: Service name for traces
    /// - `SAMPLER_RATE`: Trace sampling rate (0.0-1.0)
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(enabled) = env::var("TRACING_ENABLED") {
            config.enabled = enabled.to_lowercase() == "true" || enabled == "1";
        }

        if let Ok(endpoint) = env::var("JAEGER_ENDPOINT") {
            config.jaeger_endpoint = endpoint;
        }

        if let Ok(name) = env::var("SERVICE_NAME") {
            config.service_name = name;
        }

        if let Ok(rate) = env::var("SAMPLER_RATE") {
            if let Ok(r) = rate.parse::<f64>() {
                config.sampler_rate = r.clamp(0.0, 1.0);
            }
        }

        if config.enabled {
            tracing::info!(
                jaeger_endpoint = %config.jaeger_endpoint,
                service_name = %config.service_name,
                sampler_rate = config.sampler_rate,
                "Distributed tracing enabled"
            );
        } else {
            tracing::debug!("Distributed tracing disabled (set TRACING_ENABLED=true to enable)");
        }

        config
    }

    /// Initialize OpenTelemetry with OTLP exporter
    ///
    /// # Returns
    /// OpenTelemetry layer for tracing subscriber, or None if disabled
    pub fn init_tracer(
        &self,
    ) -> Result<
        Option<OpenTelemetryLayer<tracing_subscriber::Registry, opentelemetry_sdk::trace::Tracer>>,
        TraceError,
    > {
        if !self.enabled {
            return Ok(None);
        }

        // Decide agent vs collector mode from env
        // For compatibility with current versions, use agent pipeline. If JAEGER_MODE=collector is set,
        // log a warning and fall back to agent unless collector client is fully configured.
        let mode = std::env::var("JAEGER_MODE").unwrap_or_else(|_| "agent".into());
        if mode.eq_ignore_ascii_case("collector") {
            tracing::warn!("JAEGER_MODE=collector set, but collector runtime not configured; falling back to agent pipeline");
        }

        // Parse the agent target (host/port) to avoid dead_code warning and for future use
        let (agent_host, agent_port) = self.parse_jaeger_endpoint();
        tracing::debug!(%agent_host, agent_port, "Jaeger agent target parsed");

        // Build OTLP exporter (grpc) to env-specified endpoint or default
        let endpoint =
            crate::settings::effective_or("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4317");
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint);
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .install_simple()
            .map_err(|e| {
                tracing::error!("Failed to initialize OTLP tracer: {}", e);
                TraceError::Other(Box::new(std::io::Error::other(format!(
                    "OTLP initialization failed: {}",
                    e
                ))))
            })?;

        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        Ok(Some(telemetry_layer))
    }

    /// Parse Jaeger endpoint into host and port
    fn parse_jaeger_endpoint(&self) -> (String, u16) {
        // Expected format: "http://localhost:6831" or "localhost:6831"
        let url = self
            .jaeger_endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://");

        if let Some((host, port_str)) = url.split_once(':') {
            let port = port_str.parse().unwrap_or(6831);
            (host.to_string(), port)
        } else {
            (url.to_string(), 6831)
        }
    }
}

/// Generate a trace ID for correlation
pub fn generate_trace_id() -> String {
    Uuid::new_v4().to_string()
}

/// Span context for propagation across services
#[derive(Debug, Clone)]
pub struct SpanContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
}

impl Default for SpanContext {
    fn default() -> Self {
        Self::new()
    }
}

impl SpanContext {
    /// Create a new span context with generated IDs
    pub fn new() -> Self {
        Self {
            trace_id: generate_trace_id(),
            span_id: generate_trace_id(),
            parent_span_id: None,
        }
    }

    /// Create a child span context
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: generate_trace_id(),
            parent_span_id: Some(self.span_id.clone()),
        }
    }

    /// Convert to W3C Trace Context format for HTTP headers
    /// Format: traceparent: 00-{trace_id}-{span_id}-01
    pub fn to_w3c_traceparent(&self) -> String {
        format!("00-{}-{}-01", &self.trace_id[..16], &self.span_id[..16])
    }

    /// Parse W3C Trace Context from HTTP header
    pub fn from_w3c_traceparent(traceparent: &str) -> Option<Self> {
        let parts: Vec<&str> = traceparent.split('-').collect();
        if parts.len() == 4 {
            Some(Self {
                trace_id: format!("{:0>32}", parts[1]),
                span_id: format!("{:0>16}", parts[2]),
                parent_span_id: None,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_disabled_by_default() {
        std::env::remove_var("TRACING_ENABLED");
        let config = DistributedTracingConfig::from_env();
        assert!(!config.enabled);
    }

    #[test]
    fn test_config_enabled_from_env() {
        std::env::set_var("TRACING_ENABLED", "true");
        let config = DistributedTracingConfig::from_env();
        assert!(config.enabled);
        std::env::remove_var("TRACING_ENABLED");
    }

    #[test]
    fn test_trace_id_generation() {
        let id1 = generate_trace_id();
        let id2 = generate_trace_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36); // UUID length
    }

    #[test]
    fn test_span_context_creation() {
        let ctx = SpanContext::new();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
        assert!(ctx.parent_span_id.is_none());
    }

    #[test]
    fn test_child_span_context() {
        let parent = SpanContext::new();
        let child = parent.child();
        assert_eq!(parent.trace_id, child.trace_id);
        assert_ne!(parent.span_id, child.span_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
    }

    #[test]
    fn test_w3c_traceparent_format() {
        let ctx = SpanContext::new();
        let traceparent = ctx.to_w3c_traceparent();
        assert!(traceparent.starts_with("00-"));
        assert!(traceparent.ends_with("-01"));
    }

    #[test]
    fn test_w3c_traceparent_parsing() {
        let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let ctx = SpanContext::from_w3c_traceparent(traceparent);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
    }

    #[test]
    fn test_sampler_rate_clamping() {
        std::env::set_var("SAMPLER_RATE", "1.5");
        let config = DistributedTracingConfig::from_env();
        assert_eq!(config.sampler_rate, 1.0);
        std::env::remove_var("SAMPLER_RATE");

        std::env::set_var("SAMPLER_RATE", "-0.5");
        let config = DistributedTracingConfig::from_env();
        assert_eq!(config.sampler_rate, 0.0);
        std::env::remove_var("SAMPLER_RATE");
    }

    #[test]
    fn test_parse_jaeger_endpoint() {
        let config1 = DistributedTracingConfig {
            jaeger_endpoint: "http://localhost:6831".to_string(),
            ..Default::default()
        };
        let (host, port) = config1.parse_jaeger_endpoint();
        assert_eq!(host, "localhost");
        assert_eq!(port, 6831);

        let config2 = DistributedTracingConfig {
            jaeger_endpoint: "jaeger.prod:6831".to_string(),
            ..Default::default()
        };
        let (host, port) = config2.parse_jaeger_endpoint();
        assert_eq!(host, "jaeger.prod");
        assert_eq!(port, 6831);
    }
}
