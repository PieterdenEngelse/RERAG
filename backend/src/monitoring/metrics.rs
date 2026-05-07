use once_cell::sync::Lazy;
use prometheus::{
    core::Collector, Encoder, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, Opts, Registry, TextEncoder,
};

// Global Prometheus registry
pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

fn service_and_env() -> (String, String) {
    let service = std::env::var("APP_SERVICE")
        .ok()
        .unwrap_or_else(|| env!("APP_SERVICE_DEFAULT").to_string());
    let env_name = std::env::var("APP_ENV")
        .ok()
        .unwrap_or_else(|| env!("APP_ENV_DEFAULT").to_string());
    (service, env_name)
}

// App info gauge (const)
pub static APP_INFO: Lazy<IntGauge> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let g = IntGauge::with_opts(
        Opts::new("app_info", "Application info gauge")
            .const_label("app", &service)
            .const_label("service", &service)
            .const_label("env", &env_name)
            .const_label("version", env!("CARGO_PKG_VERSION"))
            .const_label("git_sha", env!("GIT_SHA"))
            .const_label("build_time", env!("BUILD_TIME")),
    )
    .unwrap();
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

// Startup duration
pub static STARTUP_DURATION_MS: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new(
        "startup_duration_ms",
        "Application startup duration in milliseconds",
    )
    .unwrap();
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

// Reindex metrics
pub static REINDEX_SUCCESS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    let c = IntCounter::new(
        "reindex_success_total",
        "Total successful reindex operations",
    )
    .unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

pub static REINDEX_FAILURE_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    let c = IntCounter::new("reindex_failure_total", "Total failed reindex operations").unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

#[doc(hidden)]
pub fn __test_parse_buckets_env(var: &str) -> Option<Vec<f64>> {
    parse_buckets_env(var)
}

fn parse_buckets_env(var: &str) -> Option<Vec<f64>> {
    match std::env::var(var) {
        Ok(val) if !val.trim().is_empty() => {
            let mut parsed: Vec<f64> = Vec::new();
            for tok in val.split(',') {
                let t = tok.trim();
                if t.is_empty() {
                    continue;
                }
                match t.parse::<f64>() {
                    Ok(v) if v > 0.0 => parsed.push(v),
                    _ => {
                        tracing::warn!(env_var = %var, token = %t, "Invalid histogram bucket value; ignoring");
                        return None;
                    }
                }
            }
            if parsed.is_empty() {
                None
            } else {
                parsed.sort_by(|a, b| a.partial_cmp(b).unwrap());
                Some(parsed)
            }
        }
        _ => None,
    }
}

pub static REINDEX_DURATION_MS: Lazy<Histogram> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let default = vec![50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];
    let buckets = parse_buckets_env("REINDEX_HISTO_BUCKETS").unwrap_or(default);
    let mut opts = HistogramOpts::new("reindex_duration_ms", "Reindex duration in milliseconds")
        .buckets(buckets);
    opts.common_opts = opts
        .common_opts
        .const_label("service", service)
        .const_label("env", env_name);
    let h = Histogram::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(h.clone())).ok();
    h
});

// Search metrics
pub static SEARCH_LATENCY_MS: Lazy<Histogram> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let default = vec![1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 250.0, 500.0, 1000.0];
    let buckets = parse_buckets_env("SEARCH_HISTO_BUCKETS").unwrap_or(default);
    let mut opts =
        HistogramOpts::new("search_latency_ms", "Search latency in milliseconds").buckets(buckets);
    opts.common_opts = opts
        .common_opts
        .const_label("service", service)
        .const_label("env", env_name);
    let h = Histogram::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(h.clone())).ok();
    h
});

// Embedding metrics
pub static EMBEDDING_LATENCY_MS: Lazy<Histogram> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let default = vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0];
    let buckets = parse_buckets_env("EMBEDDING_HISTO_BUCKETS").unwrap_or(default);
    let mut opts = HistogramOpts::new(
        "embedding_latency_ms",
        "Embedding generation latency in milliseconds",
    )
    .buckets(buckets);
    opts.common_opts = opts
        .common_opts
        .const_label("service", service)
        .const_label("env", env_name);
    let h = Histogram::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(h.clone())).ok();
    h
});

pub static EMBEDDING_BATCH_SIZE: Lazy<Histogram> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let buckets = vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0];
    let mut opts = HistogramOpts::new(
        "embedding_batch_size",
        "Number of texts per embedding batch",
    )
    .buckets(buckets);
    opts.common_opts = opts
        .common_opts
        .const_label("service", service)
        .const_label("env", env_name);
    let h = Histogram::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(h.clone())).ok();
    h
});

pub static EMBEDDING_CACHE_HITS: Lazy<IntCounter> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let c = IntCounter::with_opts(
        Opts::new("embedding_cache_hits_total", "Total embedding cache hits")
            .const_label("service", service)
            .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

pub static EMBEDDING_CACHE_MISSES: Lazy<IntCounter> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let c = IntCounter::with_opts(
        Opts::new(
            "embedding_cache_misses_total",
            "Total embedding cache misses",
        )
        .const_label("service", service)
        .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

pub static EMBEDDING_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let c = IntCounter::with_opts(
        Opts::new("embedding_total", "Total embeddings generated")
            .const_label("service", service)
            .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

// Inference gateway metrics
pub static INFERENCE_PERMITS_ACQUIRED: Lazy<IntCounterVec> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let opts = Opts::new(
        "inference_permits_acquired_total",
        "Total inference permits acquired",
    )
    .const_label("service", service)
    .const_label("env", env_name);
    let cv = IntCounterVec::new(opts, &["type"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

pub static INFERENCE_PERMITS_REJECTED: Lazy<IntCounterVec> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let opts = Opts::new(
        "inference_permits_rejected_total",
        "Total inference permits rejected (timeout/unavailable)",
    )
    .const_label("service", service)
    .const_label("env", env_name);
    let cv = IntCounterVec::new(opts, &["type"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

pub static INFERENCE_PERMITS_AVAILABLE: Lazy<prometheus::IntGaugeVec> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let opts = Opts::new(
        "inference_permits_available",
        "Currently available inference permits",
    )
    .const_label("service", service)
    .const_label("env", env_name);
    let gv = prometheus::IntGaugeVec::new(opts, &["type"]).unwrap();
    REGISTRY.register(Box::new(gv.clone())).ok();
    gv
});

pub static CACHE_HITS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let c = IntCounter::with_opts(
        Opts::new("cache_hits_total", "Total cache hits")
            .const_label("service", service)
            .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

pub static CACHE_MISSES_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let c = IntCounter::with_opts(
        Opts::new("cache_misses_total", "Total cache misses")
            .const_label("service", service)
            .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

pub static RATE_LIMIT_DROPS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let c = IntCounter::with_opts(
        Opts::new(
            "rate_limit_drops_total",
            "Total requests dropped due to rate limit",
        )
        .const_label("service", service)
        .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

pub static RATE_LIMIT_DROPS_BY_ROUTE: Lazy<IntCounterVec> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let opts = Opts::new(
        "rate_limit_drops_by_route_total",
        "Rate limit drops partitioned by route",
    )
    .const_label("service", service)
    .const_label("env", env_name);
    let cv = IntCounterVec::new(opts, &["route"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

// State gauges
pub static DOCUMENTS_TOTAL: Lazy<IntGauge> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let g = IntGauge::with_opts(
        Opts::new("documents_total", "Total number of indexed documents")
            .const_label("service", service)
            .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

pub static VECTORS_TOTAL: Lazy<IntGauge> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let g = IntGauge::with_opts(
        Opts::new("vectors_total", "Total number of vectors")
            .const_label("service", service)
            .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

pub static INDEX_SIZE_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let g = IntGauge::with_opts(
        Opts::new("index_size_bytes", "Index size in bytes (approximate)")
            .const_label("service", service)
            .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

pub static CACHE_HIT_RATE_PERCENT: Lazy<IntGauge> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let g = IntGauge::with_opts(
        Opts::new(
            "search_cache_hit_rate_percent",
            "Search cache hit rate (0-100)",
        )
        .const_label("service", service)
        .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

pub static SEARCH_TOP_K_GAUGE: Lazy<IntGauge> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let g = IntGauge::with_opts(
        Opts::new(
            "search_top_k_config",
            "Configured keyword search top-k limit",
        )
        .const_label("service", service)
        .const_label("env", env_name),
    )
    .unwrap();
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

pub static REQUEST_LATENCY_MS: Lazy<prometheus::HistogramVec> = Lazy::new(|| {
    use prometheus::{histogram_opts, HistogramVec};
    let (service, env_name) = service_and_env();
    let mut opts = histogram_opts!("request_latency_ms", "HTTP request latency in milliseconds");
    opts.common_opts = opts
        .common_opts
        .const_label("service", service)
        .const_label("env", env_name);
    let hv = HistogramVec::new(opts, &["method", "route", "status_class"]).unwrap();
    REGISTRY.register(Box::new(hv.clone())).ok();
    hv
});

pub static MANUAL_OBS_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let opts = Opts::new(
        "manual_observation_requests_total",
        "Manual observation handler calls",
    )
    .const_label("service", service)
    .const_label("env", env_name);
    let cv = IntCounterVec::new(opts, &["endpoint", "status"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

pub static MANUAL_OBS_LATENCY_MS: Lazy<HistogramVec> = Lazy::new(|| {
    use prometheus::histogram_opts;
    let (service, env_name) = service_and_env();
    let mut opts = histogram_opts!(
        "manual_observation_latency_ms",
        "Latency for manual observation endpoints"
    );
    opts.common_opts = opts
        .common_opts
        .const_label("service", service)
        .const_label("env", env_name);
    let hv = HistogramVec::new(opts, &["endpoint"]).unwrap();
    REGISTRY.register(Box::new(hv.clone())).ok();
    hv
});

// Helper to update gauges from retriever
pub fn refresh_retriever_gauges(retriever: &crate::retriever::Retriever) {
    DOCUMENTS_TOTAL.set(retriever.metrics.total_documents_indexed as i64);
    VECTORS_TOTAL.set(retriever.metrics.total_vectors as i64);
    set_search_top_k(retriever.current_search_top_k() as i64);
    set_cache_hit_rate_percent(retriever.metrics.cache_hit_rate());
    if let Ok(size) = retriever.metrics.get_index_size_bytes() {
        INDEX_SIZE_BYTES.set(size as i64);
    }
}

pub fn set_cache_hit_rate_percent(hit_rate_fraction: f64) {
    let hit_rate = (hit_rate_fraction * 100.0).round() as i64;
    CACHE_HIT_RATE_PERCENT.set(hit_rate.clamp(0, 100));
}

pub fn set_search_top_k(top_k: i64) {
    SEARCH_TOP_K_GAUGE.set(top_k.max(1));
}

// Observe search latency in ms
pub fn observe_search_latency_ms(duration_ms: f64) {
    SEARCH_LATENCY_MS.observe(duration_ms);
}

// Record reindex duration in ms
pub fn observe_reindex_duration_ms(duration_ms: f64) {
    REINDEX_DURATION_MS.observe(duration_ms);
}

// Observe embedding latency in ms
pub fn observe_embedding_latency_ms(duration_ms: f64) {
    EMBEDDING_LATENCY_MS.observe(duration_ms);
}

// Extraction metrics
pub static EXTRACTION_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let opts = Opts::new(
        "extraction_total",
        "Text extraction attempts by format and status (ok/empty)",
    );
    let cv = IntCounterVec::new(opts, &["format", "status"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

pub static EXTRACTION_CHARS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let opts = Opts::new(
        "extraction_chars_total",
        "Total characters extracted per format",
    );
    let cv = IntCounterVec::new(opts, &["format"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

pub static EXTRACTION_OCR_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let opts = Opts::new(
        "extraction_ocr_total",
        "OCR pipeline events (attempted/ok/no_text/no_pages/unavailable)",
    );
    let cv = IntCounterVec::new(opts, &["status"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

// Record embedding batch size
pub fn observe_embedding_batch_size(size: usize) {
    EMBEDDING_BATCH_SIZE.observe(size as f64);
}

// Record embedding cache hit
pub fn record_embedding_cache_hit() {
    EMBEDDING_CACHE_HITS.inc();
}

// Record embedding cache miss
pub fn record_embedding_cache_miss() {
    EMBEDDING_CACHE_MISSES.inc();
}

// Record embedding generated
pub fn record_embedding_generated(count: u64) {
    EMBEDDING_TOTAL.inc_by(count);
}

/// Get embedding cache stats snapshot
pub fn embedding_cache_stats() -> (u64, u64) {
    (EMBEDDING_CACHE_HITS.get(), EMBEDDING_CACHE_MISSES.get())
}

// Record inference permit acquired
pub fn record_inference_permit_acquired(permit_type: &str) {
    INFERENCE_PERMITS_ACQUIRED
        .with_label_values(&[permit_type])
        .inc();
}

// Record inference permit rejected
pub fn record_inference_permit_rejected(permit_type: &str) {
    INFERENCE_PERMITS_REJECTED
        .with_label_values(&[permit_type])
        .inc();
}

// Update available permits gauge
pub fn set_inference_permits_available(permit_type: &str, count: i64) {
    INFERENCE_PERMITS_AVAILABLE
        .with_label_values(&[permit_type])
        .set(count);
}

/// Refresh inference gateway gauges
pub fn refresh_inference_gateway_gauges() {
    let stats = crate::inference_gateway::gateway_stats();
    set_inference_permits_available("embedding", stats.embedding_permits_available as i64);
    set_inference_permits_available("llm", stats.llm_permits_available as i64);
}

// Exporter for Prometheus text format
pub fn export_prometheus() -> String {
    let metric_families = REGISTRY.gather();
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    if encoder.encode(&metric_families, &mut buffer).is_ok() {
        String::from_utf8(buffer).unwrap_or_default()
    } else {
        "".to_string()
    }
}

/// Snapshot total cache hit/miss counters
pub fn cache_hit_miss_counts() -> (i64, i64) {
    (
        CACHE_HITS_TOTAL.get() as i64,
        CACHE_MISSES_TOTAL.get() as i64,
    )
}

/// Snapshot total rate limit drops counter
pub fn rate_limit_drop_total() -> i64 {
    RATE_LIMIT_DROPS_TOTAL.get() as i64
}

/// Snapshot per-route rate limit drops from the counter vec
pub fn rate_limit_drops_by_route_snapshot() -> Vec<(String, i64)> {
    let mut out = Vec::new();
    for family in RATE_LIMIT_DROPS_BY_ROUTE.collect().into_iter() {
        if family.get_name() != "rate_limit_drops_by_route_total" {
            continue;
        }
        for metric in family.get_metric() {
            let mut route = "unknown".to_string();
            for label in metric.get_label() {
                if label.get_name() == "route" {
                    route = label.get_value().to_string();
                    break;
                }
            }
            let drops = metric.get_counter().get_value() as i64;
            out.push((route, drops));
        }
    }
    out
}

pub fn record_manual_observation(endpoint: &str, success: bool, duration_ms: f64) {
    let status = if success { "ok" } else { "err" };
    MANUAL_OBS_REQUESTS_TOTAL
        .with_label_values(&[endpoint, status])
        .inc();
    MANUAL_OBS_LATENCY_MS
        .with_label_values(&[endpoint])
        .observe(duration_ms);
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ManualObservationMetricSnapshot {
    pub endpoint: String,
    pub ok: u64,
    pub err: u64,
    pub latency_p50: f64,
    pub latency_p90: f64,
}

pub fn manual_observation_metrics_snapshot() -> Vec<ManualObservationMetricSnapshot> {
    use prometheus::proto::MetricFamily;

    let mut counters: std::collections::HashMap<String, (u64, u64)> =
        std::collections::HashMap::new();
    let mut latencies: std::collections::HashMap<String, Vec<f64>> =
        std::collections::HashMap::new();

    let gather = MANUAL_OBS_REQUESTS_TOTAL.collect();
    for family in gather.iter() {
        if family.get_name() != "manual_observation_requests_total" {
            continue;
        }
        for metric in family.get_metric() {
            let mut endpoint = "unknown".to_string();
            let mut status = "ok".to_string();
            for label in metric.get_label() {
                if label.get_name() == "endpoint" {
                    endpoint = label.get_value().to_string();
                }
                if label.get_name() == "status" {
                    status = label.get_value().to_string();
                }
            }
            let entry = counters.entry(endpoint).or_insert((0, 0));
            let value = metric.get_counter().get_value() as u64;
            if status == "ok" {
                entry.0 = value;
            } else {
                entry.1 = value;
            }
        }
    }

    let gather_latency: Vec<MetricFamily> = MANUAL_OBS_LATENCY_MS.collect();
    for family in gather_latency.iter() {
        if family.get_name() != "manual_observation_latency_ms" {
            continue;
        }
        for metric in family.get_metric() {
            let mut endpoint = "unknown".to_string();
            for label in metric.get_label() {
                if label.get_name() == "endpoint" {
                    endpoint = label.get_value().to_string();
                }
            }
            let histogram = metric.get_histogram();
            for bucket in histogram.get_bucket() {
                // Not a true percentile; approximate using cumulative counts
                // since we lack raw samples. Use upper_bound as representative.
                // We'll store bucket upper bounds repeated by count.
                let entry = latencies.entry(endpoint.clone()).or_default();
                entry.push(bucket.get_upper_bound());
            }
        }
    }

    counters
        .into_iter()
        .map(|(endpoint, (ok, err))| {
            let mut latency_vec = latencies.remove(&endpoint).unwrap_or_default();
            latency_vec.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            fn percentile(data: &[f64], pct: f64) -> f64 {
                if data.is_empty() {
                    return 0.0;
                }
                let idx = ((data.len() as f64 - 1.0) * pct).round() as usize;
                data[idx.min(data.len() - 1)]
            }
            ManualObservationMetricSnapshot {
                endpoint,
                ok,
                err,
                latency_p50: percentile(&latency_vec, 0.5),
                latency_p90: percentile(&latency_vec, 0.9),
            }
        })
        .collect()
}

// ============================================================================
// 3-LAYER MEMORY SEARCH METRICS (SEARCH.md)
// ============================================================================

/// Layer-specific request counter for 3-layer memory search
/// Labels: layer (search|timeline|fetch), status (ok|err)
pub static MEMORY_SEARCH_LAYER_REQUESTS: Lazy<IntCounterVec> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let opts = Opts::new(
        "memory_search_requests_total",
        "Memory search requests by layer (search/timeline/fetch)",
    )
    .const_label("service", service)
    .const_label("env", env_name);
    let cv = IntCounterVec::new(opts, &["layer", "status"]).unwrap();
    REGISTRY.register(Box::new(cv.clone())).ok();
    cv
});

/// Layer-specific latency histogram for 3-layer memory search
pub static MEMORY_SEARCH_LAYER_LATENCY_MS: Lazy<HistogramVec> = Lazy::new(|| {
    use prometheus::histogram_opts;
    let (service, env_name) = service_and_env();
    let buckets = vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0];
    let mut opts = histogram_opts!(
        "memory_search_latency_ms",
        "Latency for memory search layers in milliseconds"
    );
    opts.common_opts = opts
        .common_opts
        .const_label("service", service)
        .const_label("env", env_name);
    opts.buckets = buckets;
    let hv = HistogramVec::new(opts, &["layer"]).unwrap();
    REGISTRY.register(Box::new(hv.clone())).ok();
    hv
});

/// Estimated tokens saved by using 3-layer approach
pub static MEMORY_SEARCH_TOKENS_SAVED: Lazy<IntCounter> = Lazy::new(|| {
    let (service, env_name) = service_and_env();
    let opts = Opts::new(
        "memory_search_tokens_saved_total",
        "Estimated tokens saved by using 3-layer memory search",
    )
    .const_label("service", service)
    .const_label("env", env_name);
    let c = IntCounter::with_opts(opts).unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

/// Record a memory search layer request
/// layer: "search" | "timeline" | "fetch"
pub fn record_memory_search_layer(layer: &str, success: bool, duration_ms: f64) {
    let status = if success { "ok" } else { "err" };
    MEMORY_SEARCH_LAYER_REQUESTS
        .with_label_values(&[layer, status])
        .inc();
    MEMORY_SEARCH_LAYER_LATENCY_MS
        .with_label_values(&[layer])
        .observe(duration_ms);
}

/// Record estimated tokens saved (e.g., when using Layer 1 instead of Layer 3)
pub fn record_tokens_saved(tokens: u64) {
    MEMORY_SEARCH_TOKENS_SAVED.inc_by(tokens);
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemorySearchLayerStats {
    pub layer: String,
    pub requests_ok: u64,
    pub requests_err: u64,
    pub latency_p50_ms: f64,
    pub latency_p99_ms: f64,
}

/// Get snapshot of 3-layer memory search metrics
pub fn memory_search_layer_stats() -> Vec<MemorySearchLayerStats> {
    let mut stats = Vec::new();

    for layer in &["search", "timeline", "fetch"] {
        let ok = MEMORY_SEARCH_LAYER_REQUESTS
            .with_label_values(&[layer, "ok"])
            .get();
        let err = MEMORY_SEARCH_LAYER_REQUESTS
            .with_label_values(&[layer, "err"])
            .get();

        // Get histogram data for latency percentiles
        let histogram = MEMORY_SEARCH_LAYER_LATENCY_MS.with_label_values(&[layer]);
        let sample_count = histogram.get_sample_count();
        let sample_sum = histogram.get_sample_sum();
        let avg_latency = if sample_count > 0 {
            sample_sum / sample_count as f64
        } else {
            0.0
        };

        stats.push(MemorySearchLayerStats {
            layer: layer.to_string(),
            requests_ok: ok,
            requests_err: err,
            latency_p50_ms: avg_latency,       // Approximation
            latency_p99_ms: avg_latency * 2.0, // Rough estimate
        });
    }

    stats
}

/// Get total tokens saved
pub fn memory_search_tokens_saved_total() -> u64 {
    MEMORY_SEARCH_TOKENS_SAVED.get()
}
