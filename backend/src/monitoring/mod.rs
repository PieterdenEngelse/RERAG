//! Monitoring module for agentic-rag
//!
//! Provides:
//! - Structured logging with tracing
//! - Prometheus metrics collection
//! - Health checks endpoints
//! - Performance instrumentation
//!
//! INSTALLER IMPACT:
//! - Creates ~/.agentic-rag/logs/ directory
//! - Requires RUST_LOG environment variable
//! - Requires MONITORING_ENABLED=true environment variable
pub mod alerting_hooks;
pub mod canon_stats;
pub mod chunking_stats;
pub mod config;
pub mod config_phase15;
pub mod distributed_tracing;
pub mod extraction_stats;
pub mod handlers;
pub mod health;
pub mod histogram_config;
pub mod metrics;
pub mod metrics_config;
pub mod onnx_metrics;
pub mod otel_config;
pub mod performance_analysis;
pub mod pprof;
pub mod preprocess_stats;
pub mod rate_limit_middleware;
pub mod resource_attribution;
pub mod rig_stats;
pub mod tool_alerts;
pub mod tool_costs;
pub mod tool_dependencies;
pub mod tool_stats;
pub mod tool_trends;
pub mod trace_alerting;
pub mod trace_context;
pub mod trace_middleware;
pub mod tracing_config;
pub mod ui_metrics;

pub use crate::monitoring::metrics::{
    export_prometheus, observe_reindex_duration_ms, observe_search_latency_ms,
    refresh_retriever_gauges, APP_INFO, CACHE_HITS_TOTAL, CACHE_MISSES_TOTAL, DOCUMENTS_TOTAL,
    EXTRACTION_CHARS_TOTAL, EXTRACTION_OCR_TOTAL, EXTRACTION_TOTAL, INDEX_SIZE_BYTES,
    RATE_LIMIT_DROPS_BY_ROUTE, RATE_LIMIT_DROPS_TOTAL, REGISTRY, REINDEX_FAILURE_TOTAL,
    REINDEX_SUCCESS_TOTAL, SEARCH_LATENCY_MS, STARTUP_DURATION_MS, VECTORS_TOTAL,
};
pub use alerting_hooks::{AlertingHooksConfig, ReindexCompletionEvent};
pub use canon_stats::{
    flush as flush_canon_stats, forget_file as forget_canon_file, get_stats as get_canon_stats,
    get_stats_for as get_canon_stats_for, init as init_canon_stats,
    record_embed_ingestion as record_canon_embed_ingestion,
    record_embed_query as record_canon_embed_query, record_file_embed as record_canon_file_embed,
    record_file_index as record_canon_file_index,
    record_index_ingestion as record_canon_index_ingestion,
    record_index_query as record_canon_index_query, record_store as record_canon_store,
    CallSiteStats, CanonStats, StoreRecord,
};
pub use chunking_stats::{
    chunking_logging_enabled, chunking_snapshot_history, flush as flush_chunking_stats,
    init as init_chunking_stats, latest_chunking_snapshot, record_chunking_snapshot,
    set_chunking_history_capacity, set_chunking_logging_enabled, ChunkingStatsSnapshot,
    DetectionInfo,
};
pub use config::MonitoringConfig;
pub use extraction_stats::{
    forget_file as forget_extraction_file, get_stats as get_extraction_stats,
    get_stats_for as get_extraction_stats_for, load_history as load_extraction_history,
    record_format as record_extraction_format, record_ocr_attempted, record_ocr_no_pages,
    record_ocr_no_text, record_ocr_ok, record_ocr_unavailable, ExtractionStats, FileRecord,
};
pub use health::HealthStatus;
pub use histogram_config::HistogramBuckets;
pub use preprocess_stats::{
    flush as flush_preprocess_stats, get_stats as get_preprocess_stats,
    init as init_preprocess_stats, record_html as record_preprocess_html,
    record_passthrough as record_preprocess_passthrough,
    record_unicode as record_preprocess_unicode, PreprocessFileRecord, PreprocessStats,
};
pub use resource_attribution::{start_resource_attribution, ResourceAttributionConfig};
pub use tool_alerts::{
    acknowledge_alert, get_alert_config, get_alert_status, get_alerts, get_tool_alerts,
    record_and_check as record_tool_alert, set_alert_config, set_webhook_url, ToolAlert,
};
pub use tool_costs::{get_tool_costs, record_tool_cost, ToolCostStats};
pub use tool_dependencies::{
    get_tool_dependency_graph, record_tool_dependency, record_tool_dependency_str,
    ToolDependencyEdge, ToolDependencyGraph, ToolDependencyNode,
};
pub use tool_stats::{
    clear_history as clear_tool_history, get_llm_latency_stats, get_recent_executions,
    get_tool_stat, get_tool_stats, record_tool_execution, LlmLatencyStats, ToolAggregateStats,
    ToolExecution, ToolExecutionResponse, ToolStatsResponse,
};
pub use tool_trends::{
    compare_windows as compare_trends, get_all_trends, get_tool_trend, record_execution, ToolTrend,
};
pub use trace_alerting::{start_trace_alerting, TraceAlertingConfig, TraceAnomalyEvent};
pub use trace_context::{clear_trace_id, get_trace_id, set_trace_id};
pub use ui_metrics::{
    get_requests_snapshot, record_http_request, RequestChartPoint, RequestsSnapshot,
};

use once_cell::sync::OnceCell;
use std::sync::Arc;
use std::time::Instant;

/// Global health tracker for access from anywhere in the application
static GLOBAL_HEALTH_TRACKER: OnceCell<Arc<health::HealthTracker>> = OnceCell::new();

/// Initialize the global health tracker (call once at startup)
pub fn init_health_tracker() {
    let tracker = Arc::new(health::HealthTracker::new());
    let _ = GLOBAL_HEALTH_TRACKER.set(tracker);
    tracing::info!("Health tracker initialized");
}

/// Get the global health tracker (if initialized)
pub fn get_health_tracker() -> Option<&'static Arc<health::HealthTracker>> {
    GLOBAL_HEALTH_TRACKER.get()
}

/// Mark indexing as started (safe to call even if tracker not initialized)
pub fn mark_indexing_started() {
    if let Some(tracker) = GLOBAL_HEALTH_TRACKER.get() {
        tracker.start_indexing();
    }
}

/// Mark indexing as finished (safe to call even if tracker not initialized)
pub fn mark_indexing_finished() {
    if let Some(tracker) = GLOBAL_HEALTH_TRACKER.get() {
        tracker.finish_indexing();
    }
}

/// Mark LLM call as started (safe to call even if tracker not initialized)
pub fn mark_llm_started() {
    if let Some(tracker) = GLOBAL_HEALTH_TRACKER.get() {
        tracker.start_llm_call();
    }
}

/// Mark LLM call as finished (safe to call even if tracker not initialized)
pub fn mark_llm_finished() {
    if let Some(tracker) = GLOBAL_HEALTH_TRACKER.get() {
        tracker.finish_llm_call();
    }
}

/// Monitoring context shared across the application
#[derive(Clone)]
pub struct MonitoringContext {
    pub config: MonitoringConfig,
    pub health: Arc<health::HealthTracker>,
    pub startup_time: Instant,
}

impl MonitoringContext {
    /// Initialize monitoring system
    ///
    /// INSTALLER IMPACT:
    /// - Must be called before starting API server
    /// - Creates log directories
    /// - Initializes tracing subscriber
    pub fn new(config: MonitoringConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize tracing (logging)
        let _guard = tracing_config::init_tracing(&config)?;
        // Metrics registry initialized on first use by Lazy statics
        // Initialize health tracker
        let health = Arc::new(health::HealthTracker::new());

        // Store in global for access from indexing/LLM code
        let _ = GLOBAL_HEALTH_TRACKER.set(Arc::clone(&health));

        let startup_time = Instant::now();
        // Log effective histogram buckets at startup for visibility
        let search_buckets =
            crate::monitoring::metrics::__test_parse_buckets_env("SEARCH_HISTO_BUCKETS").unwrap_or(
                vec![1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
            );
        let reindex_buckets =
            crate::monitoring::metrics::__test_parse_buckets_env("REINDEX_HISTO_BUCKETS")
                .unwrap_or(vec![
                    50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0,
                ]);
        tracing::info!(
            ?search_buckets,
            ?reindex_buckets,
            "Monitoring system initialized with histogram buckets"
        );
        Ok(Self {
            config,
            health,
            startup_time,
        })
    }

    /// Record startup completion
    ///
    /// INSTALLER IMPACT:
    /// - Must be called after server is listening
    /// - Records startup duration in metrics
    /// - Marks system as ready
    pub fn startup_complete(&self) {
        let startup_duration = self.startup_time.elapsed();
        // You can record startup duration as a counter or gauge in Prometheus if desired.
        self.health.mark_ready();
        tracing::info!(
            duration_ms = startup_duration.as_millis(),
            "Application startup complete"
        );
    }

    /// Get current health status
    pub fn health_status(&self) -> HealthStatus {
        self.health.get_status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitoring_context_creation() {
        let config = MonitoringConfig::default();
        let ctx = MonitoringContext::new(config);
        assert!(ctx.is_ok());
    }
}
