use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::runtime::Tokio as OtelTokioRuntime;
use opentelemetry_sdk::trace::TracerProvider;
use std::env;
use tracing::info;

#[derive(Debug, Clone)]
pub struct OtelConfig {
    pub service_name: String,
    pub otlp_export: bool,
    pub console_export: bool,
    pub otlp_endpoint: String,
    pub insecure: bool, // Skip TLS verification for self-signed certs
    /// Master enable switch for OTEL tracing. When false, OTEL is entirely disabled
    /// and no exporters or tracer providers are configured.
    pub enabled: bool,
}

impl OtelConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let enabled = env::var("OTEL_TRACES_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        OtelConfig {
            service_name: env::var("OTEL_SERVICE_NAME")
                .unwrap_or_else(|_| "agentic-rag".to_string()),
            otlp_export: env::var("OTEL_OTLP_EXPORT")
                .unwrap_or_else(|_| "false".to_string())
                .parse::<bool>()
                .unwrap_or(false),
            console_export: env::var("OTEL_CONSOLE_EXPORT")
                .unwrap_or_else(|_| "false".to_string())
                .parse::<bool>()
                .unwrap_or(false),
            otlp_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://127.0.0.1:4317".to_string()),
            insecure: env::var("OTEL_EXPORTER_OTLP_INSECURE")
                .unwrap_or_else(|_| "true".to_string()) // Default true for localhost dev
                .parse::<bool>()
                .unwrap_or(true),
            enabled,
        }
    }
}

pub fn init_otel(config: &OtelConfig) -> Result<OtelGuard, Box<dyn std::error::Error>> {
    if !config.enabled {
        // OTEL entirely disabled; return a no-op guard.
        info!("OpenTelemetry disabled via OTEL_TRACES_ENABLED=false");
        return Ok(OtelGuard { enabled: false });
    }

    info!(
        "Initializing OpenTelemetry: service={}, otlp_export={}, endpoint={}, insecure={}",
        config.service_name, config.otlp_export, config.otlp_endpoint, config.insecure
    );

    let mut provider_builder = TracerProvider::builder();

    // Add OTLP exporter if enabled
    if config.otlp_export {
        // Use gRPC protocol (only option in opentelemetry-otlp 0.14.0)
        // The endpoint should be the gRPC endpoint (e.g., http://localhost:4317)
        // For HTTPS with self-signed certs, use http:// and let the collector handle TLS

        info!("Configuring OTLP gRPC exporter...");

        let otlp_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(&config.otlp_endpoint)
            .build_span_exporter()?;

        let batch_processor =
            opentelemetry_sdk::trace::BatchSpanProcessor::builder(otlp_exporter, OtelTokioRuntime)
                .with_max_export_batch_size(512)
                .with_max_queue_size(2048)
                .build();

        provider_builder = provider_builder.with_span_processor(batch_processor);
        info!("✓ OTLP exporter configured: {}", config.otlp_endpoint);
    }

    // Add console exporter if enabled
    if config.console_export {
        let stdout_exporter = opentelemetry_stdout::SpanExporter::default();
        let batch_processor = opentelemetry_sdk::trace::BatchSpanProcessor::builder(
            stdout_exporter,
            OtelTokioRuntime,
        )
        .build();

        provider_builder = provider_builder.with_span_processor(batch_processor);
        info!("✓ Console exporter configured");
    }

    // Set resource with service name and version
    let resource = opentelemetry_sdk::Resource::new(vec![
        opentelemetry::KeyValue::new("service.name", config.service_name.clone()),
        opentelemetry::KeyValue::new(
            "service.version",
            env::var("OTEL_SERVICE_VERSION")
                .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
        ),
        opentelemetry::KeyValue::new(
            "deployment.environment",
            env::var("OTEL_ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        ),
    ]);

    let trace_config = opentelemetry_sdk::trace::Config::default().with_resource(resource);
    let provider = provider_builder.with_config(trace_config).build();
    global::set_tracer_provider(provider);

    info!("✓ OpenTelemetry initialized successfully");
    Ok(OtelGuard { enabled: true })
}

pub struct OtelGuard {
    enabled: bool,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if self.enabled {
            info!("Shutting down OpenTelemetry tracer provider...");
            global::shutdown_tracer_provider();
        }
    }
}
