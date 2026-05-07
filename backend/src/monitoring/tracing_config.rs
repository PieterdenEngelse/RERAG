//! Structured logging with tracing
//!
//! Sets up:
//! - Console logging (for development)
//! - File logging with daily rotation (for production)
//! - Structured JSON logs (optional)
//! - Configurable log levels

use super::config::MonitoringConfig;
use tracing_appender::non_blocking;
use tracing_appender::rolling::daily;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Initialize tracing subscriber
///
/// INSTALLER IMPACT:
/// - Creates log directory
/// - Sets up file rotation
/// - Enables console output
///
/// Returns a guard that must be kept alive for the duration of the program.
/// Dropping the guard will stop file logging.
pub fn init_tracing(
    config: &MonitoringConfig,
) -> Result<Box<dyn std::any::Any>, Box<dyn std::error::Error>> {
    if !config.enabled {
        // No-op guard if monitoring disabled
        return Ok(Box::new(()));
    }

    // Ensure log directory exists
    config.ensure_log_dir()?;

    // Build env filter from RUST_LOG
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let registry = tracing_subscriber::registry().with(env_filter);

    // Console layer (always enabled)
    if config.enable_console_logging {
        let console_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_span_events(FmtSpan::CLOSE)
            .with_target(true)
            .with_file(true)
            .with_line_number(true);

        if config.enable_file_logging {
            // Both file and console
            let file_appender = daily(&config.log_dir, "backend.log");
            let (non_blocking_file, guard) = non_blocking(file_appender);

            let file_layer = fmt::layer()
                .with_writer(non_blocking_file)
                .with_ansi(false) // No ANSI codes in files
                .json(); // JSON format for files

            let subscriber = registry.with(console_layer).with(file_layer);
            let _ = subscriber.try_init();

            // Return the guard to keep file logging alive
            return Ok(Box::new(guard));
        } else {
            // Console only
            let _ = registry.with(console_layer).try_init();
        }
    } else if config.enable_file_logging {
        // File only
        let file_appender = daily(&config.log_dir, "backend.log");
        let (non_blocking_file, guard) = non_blocking(file_appender);

        let file_layer = fmt::layer()
            .with_writer(non_blocking_file)
            .with_ansi(false)
            .json();

        let _ = registry.with(file_layer).try_init();

        // Return the guard to keep file logging alive
        return Ok(Box::new(guard));
    }

    Ok(Box::new(()))
}

/// Log a request with structured fields
///
/// Usage:
/// ```rust,ignore
/// log_request("GET", "/search", 200, 45.5);
/// ```
#[macro_export]
macro_rules! log_request {
    ($method:expr, $path:expr, $status:expr, $duration_ms:expr) => {
        tracing::info!(
            method = $method,
            path = $path,
            status = $status,
            duration_ms = $duration_ms,
            "API Request"
        );
    };
}

/// Log a database query
#[macro_export]
macro_rules! log_db_query {
    ($query_type:expr, $duration_ms:expr) => {
        tracing::debug!(
            query_type = $query_type,
            duration_ms = $duration_ms,
            "Database Query"
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_initialization() {
        let config = MonitoringConfig::default();
        let result = init_tracing(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tracing_disabled() {
        let config = MonitoringConfig {
            enabled: false,
            ..Default::default()
        };
        let result = init_tracing(&config);
        assert!(result.is_ok());
    }
}
