//! Monitoring configuration
//!
//! Loads from environment variables:
//! - RUST_LOG: Tracing level (debug, info, warn, error)
//! - MONITORING_ENABLED: Enable/disable monitoring (true/false)
//! - LOG_FORMAT: Output format (json or text)
//! - LOG_RETENTION_DAYS: How many days to keep logs (default: 7)
//! - LOG_DIR: Directory for log files (default: ~/.agentic-rag/logs)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable or disable monitoring
    pub enabled: bool,

    /// Log level (debug, info, warn, error)
    pub log_level: String,

    /// Log format (json or text)
    pub log_format: LogFormat,

    /// Directory for log files
    pub log_dir: PathBuf,

    /// How many days to retain logs
    pub log_retention_days: u32,

    /// Metrics scrape interval in seconds
    pub metrics_interval_secs: u64,

    /// Enable file logging
    pub enable_file_logging: bool,

    /// Enable console logging
    pub enable_console_logging: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Text,
}

impl LogFormat {
    pub fn as_str(&self) -> &str {
        match self {
            LogFormat::Json => "json",
            LogFormat::Text => "text",
        }
    }
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(LogFormat::Json),
            "text" => Ok(LogFormat::Text),
            _ => Err(format!("Unknown log format: {}", s)),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_level: "info".to_string(),
            log_format: LogFormat::Text,
            log_dir: Self::default_log_dir(),
            log_retention_days: 7,
            metrics_interval_secs: 15,
            enable_file_logging: true,
            enable_console_logging: true,
        }
    }
}

impl MonitoringConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(enabled) = std::env::var("MONITORING_ENABLED") {
            config.enabled = enabled.to_lowercase() == "true";
        }

        if let Some(log_level) = crate::settings::global().and_then(|s| s.effective("RUST_LOG")) {
            config.log_level = log_level;
        }

        if let Ok(log_format) = std::env::var("LOG_FORMAT") {
            if let Ok(format) = log_format.parse() {
                config.log_format = format;
            }
        }

        if let Ok(log_dir) = std::env::var("LOG_DIR") {
            config.log_dir = PathBuf::from(log_dir);
        }

        if let Ok(retention) = std::env::var("LOG_RETENTION_DAYS") {
            if let Ok(days) = retention.parse() {
                config.log_retention_days = days;
            }
        }

        if let Ok(interval) = std::env::var("METRICS_INTERVAL_SECS") {
            if let Ok(secs) = interval.parse() {
                config.metrics_interval_secs = secs;
            }
        }

        config
    }

    /// Get default log directory: ~/.agentic-rag/logs
    pub fn default_log_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(format!("{}/.agentic-rag/logs", home))
    }

    /// Ensure log directory exists
    ///
    /// INSTALLER IMPACT:
    /// - Creates directory if it doesn't exist
    /// - Sets permissions to 0755
    pub fn ensure_log_dir(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.log_dir.exists() {
            std::fs::create_dir_all(&self.log_dir)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(&self.log_dir, perms)?;
            }

            tracing::info!(
                path = %self.log_dir.display(),
                "Created log directory"
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MonitoringConfig::default();
        assert!(config.enabled);
        assert_eq!(config.log_level, "info");
        assert_eq!(config.log_format, LogFormat::Text);
        assert_eq!(config.log_retention_days, 7);
    }

    #[test]
    fn test_log_format_parsing() {
        assert_eq!("json".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("JSON".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("text".parse::<LogFormat>().unwrap(), LogFormat::Text);
        assert!("invalid".parse::<LogFormat>().is_err());
    }
}
