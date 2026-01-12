// src/installer/health.rs
// Phase 13.1.2: Health Monitoring & Auto-restart
// Version: 13.1.2

use crate::db::path_resolver;
use crate::installer::{InstallLogger, InstallerError, InstallerResult};
use std::net::TcpStream;
use std::time::{Duration, SystemTime};

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub backend_healthy: bool,
    pub frontend_healthy: bool,
    pub database_healthy: bool,
    pub timestamp: SystemTime,
    pub last_failure: Option<String>,
}

/// Health monitor configuration
#[derive(Debug, Clone)]
pub struct HealthMonitorConfig {
    pub check_interval: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub backend_port: u16,
    pub frontend_port: u16,
    pub db_path: String,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_secs(5),
            backend_port: 3010,
            frontend_port: 3011,
            db_path: path_resolver::agent_db_path_string(),
        }
    }
}

pub struct HealthMonitor {
    config: HealthMonitorConfig,
    logger: InstallLogger,
}

impl HealthMonitor {
    /// Create new health monitor
    pub fn new(config: HealthMonitorConfig, logger: InstallLogger) -> Self {
        Self { config, logger }
    }

    /// Check if backend is healthy
    pub fn check_backend(&self) -> bool {
        match TcpStream::connect(format!("127.0.0.1:{}", self.config.backend_port)) {
            Ok(_) => {
                self.logger.debug("✓ Backend port responding");
                true
            }
            Err(_) => {
                self.logger.debug("✗ Backend port not responding");
                false
            }
        }
    }

    /// Check if frontend is healthy
    pub fn check_frontend(&self) -> bool {
        match TcpStream::connect(format!("127.0.0.1:{}", self.config.frontend_port)) {
            Ok(_) => {
                self.logger.debug("✓ Frontend port responding");
                true
            }
            Err(_) => {
                self.logger.debug("✗ Frontend port not responding");
                false
            }
        }
    }

    /// Check if database is accessible
    pub fn check_database(&self) -> bool {
        match std::fs::metadata(&self.config.db_path) {
            Ok(metadata) => {
                if metadata.is_file() {
                    self.logger.debug("✓ Database file accessible");
                    true
                } else {
                    self.logger.debug("✗ Database path is not a file");
                    false
                }
            }
            Err(_) => {
                self.logger.debug("✗ Database file not found");
                false
            }
        }
    }

    /// Run full health check
    pub fn check_all(&self) -> HealthStatus {
        let backend_healthy = self.check_backend();
        let frontend_healthy = self.check_frontend();
        let database_healthy = self.check_database();

        let last_failure = match (backend_healthy, frontend_healthy, database_healthy) {
            (true, true, true) => None,
            (false, _, _) => Some("Backend unhealthy".to_string()),
            (_, false, _) => Some("Frontend unhealthy".to_string()),
            (_, _, false) => Some("Database unhealthy".to_string()),
        };

        HealthStatus {
            backend_healthy,
            frontend_healthy,
            database_healthy,
            timestamp: SystemTime::now(),
            last_failure,
        }
    }

    /// Check health with retries
    pub fn check_with_retries(&self) -> InstallerResult<HealthStatus> {
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            let status = self.check_all();

            if status.backend_healthy && status.frontend_healthy && status.database_healthy {
                self.logger
                    .info(&format!("✓ All services healthy (attempt {})", attempt));
                return Ok(status);
            }

            if attempt < self.config.max_retries {
                self.logger.info(&format!(
                    "Health check failed (attempt {}/{}), retrying in {:?}",
                    attempt, self.config.max_retries, self.config.retry_delay
                ));
                std::thread::sleep(self.config.retry_delay);
            } else {
                last_error = status.last_failure;
            }
        }

        Err(InstallerError::Other(format!(
            "Health check failed after {} attempts: {:?}",
            self.config.max_retries, last_error
        )))
    }

    /// Report health status
    pub fn report(&self, status: &HealthStatus) {
        self.logger.info("📊 Health Report:");
        self.logger.info(&format!(
            "  Backend: {}",
            if status.backend_healthy { "✓" } else { "✗" }
        ));
        self.logger.info(&format!(
            "  Frontend: {}",
            if status.frontend_healthy {
                "✓"
            } else {
                "✗"
            }
        ));
        self.logger.info(&format!(
            "  Database: {}",
            if status.database_healthy {
                "✓"
            } else {
                "✗"
            }
        ));

        if let Some(failure) = &status.last_failure {
            self.logger.info(&format!("  Last failure: {}", failure));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_config_default() {
        let config = HealthMonitorConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.backend_port, 3010);
    }

    #[test]
    fn test_health_status() {
        let status = HealthStatus {
            backend_healthy: true,
            frontend_healthy: true,
            database_healthy: true,
            timestamp: SystemTime::now(),
            last_failure: None,
        };
        assert!(status.backend_healthy);
    }
}
