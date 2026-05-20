
use std::env;
use opentelemetry::global;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_sdk::runtime::Tokio;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct OtelConfig {
    pub service_name: String,
    pub otlp_export: bool,
    pub console_export: bool,
    pub otlp_endpoint: String,
    pub batch_queue_size: usize,
    pub batch_scheduled_delay_ms: u64,
}

impl OtelConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
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
            otlp_endpoint: crate::settings::effective_or(
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                "http://127.0.0.1:4318",
            ),
            batch_queue_size: 512,
            batch_scheduled_delay_ms: 5000,
        }
    }

    #[cfg(test)]
    pub fn new_test() -> Self {
        OtelConfig {
            service_name: "test-service".to_string(),
            otlp_export: false,
            console_export: false,
            otlp_endpoint: "http://127.0.0.1:4318".to_string(),
            batch_queue_size: 512,
            batch_scheduled_delay_ms: 5000,
        }
    }
}

pub fn init_otel(config: &OtelConfig) -> Result<OtelGuard, Box<dyn std::error::Error>> {
    info!(
        service = %config.service_name,
        otlp_export = config.otlp_export,
        console_export = config.console_export,
        "Initializing OpenTelemetry"
    );

    // Set resource with service name
    let resource = opentelemetry_sdk::Resource::new(vec![
        opentelemetry::KeyValue::new("service.name", config.service_name.clone()),
        opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ]);

    // Create base tracer provider with resource
    let mut provider_builder = TracerProvider::builder().with_resource(resource);

    // Add OTLP exporter if enabled
    if config.otlp_export {
        use opentelemetry_otlp::WithExportConfig;
        
        info!(endpoint = %config.otlp_endpoint, "Configuring OTLP exporter");
        
        match Self::setup_otlp_exporter(config) {
            Ok(processor) => {
                provider_builder = provider_builder.with_span_processor(processor);
                info!("✓ OTLP exporter configured: {}", config.otlp_endpoint);
            }
            Err(e) => {
                warn!("Failed to configure OTLP exporter: {}", e);
            }
        }
    }

    // Add console exporter if enabled
    if config.console_export {
        match Self::setup_console_exporter() {
            Ok(processor) => {
                provider_builder = provider_builder.with_span_processor(processor);
                info!("✓ Console exporter configured");
            }
            Err(e) => {
                warn!("Failed to configure console exporter: {}", e);
            }
        }
    }

    let provider = provider_builder.build();
    global::set_tracer_provider(provider);

    info!("✓ OpenTelemetry initialized");
    Ok(OtelGuard)
}

impl OtelConfig {
    fn setup_otlp_exporter(
        config: &OtelConfig,
    ) -> Result<opentelemetry_sdk::trace::BatchSpanProcessor<Tokio>, Box<dyn std::error::Error>> {
        use opentelemetry_otlp::WithExportConfig;
        use std::time::Duration;
        
        let otlp_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(config.otlp_endpoint.clone())
            .with_timeout(Duration::from_secs(10))
            .build_span_exporter()?;

        let batch_processor = opentelemetry_sdk::trace::BatchSpanProcessor::builder(
            otlp_exporter,
            Tokio,
        )
            .with_max_queue_size(config.batch_queue_size)
            .with_scheduled_delay(Duration::from_millis(config.batch_scheduled_delay_ms))
            .build();

        Ok(batch_processor)
    }

    fn setup_console_exporter() -> Result<opentelemetry_sdk::trace::BatchSpanProcessor<Tokio>, Box<dyn std::error::Error>> {
        use std::time::Duration;
        
        let console_exporter = opentelemetry_stdout::SpanExporter::default();

        let batch_processor = opentelemetry_sdk::trace::BatchSpanProcessor::builder(
            console_exporter,
            Tokio,
        )
            .with_max_queue_size(512)
            .with_scheduled_delay(Duration::from_millis(1000))
            .build();

        Ok(batch_processor)
    }
}

pub struct OtelGuard;

impl Drop for OtelGuard {
    fn drop(&mut self) {
        let _ = global::shutdown_tracer_provider();
    }
}
