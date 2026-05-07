// File: src/monitoring/config_phase15.rs
// Phase 15 Step 4: Configurability – Logging and Metrics
// Version: 1.0.0 (Production-only)
// Location: src/monitoring/config_phase15.rs
//
// Purpose: Unified production configuration for histogram buckets
// Simplifies passing histogram configuration through the application

use crate::monitoring::HistogramBuckets;

/// Production monitoring configuration for Phase 15 Step 4
///
/// Combines histogram bucket configuration
/// into a single, easy-to-use configuration object
#[derive(Debug, Clone)]
pub struct MonitoringConfigPhase15 {
    /// Histogram bucket configuration
    pub histogram_config: HistogramBuckets,
}

impl MonitoringConfigPhase15 {
    /// Load histogram configuration from environment
    ///
    /// Loads histogram bucket configuration from environment variables.
    ///
    /// # Returns
    /// Fully initialized monitoring configuration
    ///
    /// # Example
    /// ```ignore
    /// let config = MonitoringConfigPhase15::from_env();
    /// println!("Search buckets: {:?}", config.histogram_config.search_buckets);
    /// ```
    pub fn from_env() -> Self {
        let histogram_config = HistogramBuckets::from_env();

        tracing::info!(
            search_buckets = ?histogram_config.search_buckets,
            reindex_buckets = ?histogram_config.reindex_buckets,
            "Monitoring Phase 15 configuration loaded"
        );

        Self { histogram_config }
    }

    /// Create a configuration for production environment
    ///
    /// Presets:
    /// - Search buckets: [50, 100, 500, 1000, 5000] ms
    /// - Reindex buckets: [1000, 5000, 30000, 60000] ms
    ///
    /// These values are appropriate for high-volume production deployments
    pub fn production() -> Self {
        Self {
            histogram_config: HistogramBuckets {
                search_buckets: vec![50.0, 100.0, 500.0, 1000.0, 5000.0],
                reindex_buckets: vec![1000.0, 5000.0, 30000.0, 60000.0],
            },
        }
    }

    /// Get summary of configuration for logging
    ///
    /// # Returns
    /// String suitable for debug/info logging that shows all settings
    pub fn summary(&self) -> String {
        format!(
            "MonitoringConfig(buckets=[search:{}ms, reindex:{}ms])",
            self.histogram_config
                .search_buckets
                .iter()
                .copied()
                .map(|b| format!("{}", b as i32))
                .collect::<Vec<_>>()
                .join(","),
            self.histogram_config
                .reindex_buckets
                .iter()
                .copied()
                .map(|b| format!("{}", b as i32))
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

impl Default for MonitoringConfigPhase15 {
    /// Default configuration uses environment variables
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn unset(var: &str) {
        env::remove_var(var);
    }

    #[test]
    fn test_production_config() {
        let config = MonitoringConfigPhase15::production();
        assert_eq!(config.histogram_config.search_buckets.len(), 5);
        assert_eq!(config.histogram_config.reindex_buckets.len(), 4);
        assert_eq!(config.histogram_config.search_buckets[0], 50.0);
        assert_eq!(config.histogram_config.reindex_buckets[0], 1000.0);
    }

    #[test]
    fn test_from_env_with_defaults() {
        unset("SEARCH_HISTO_BUCKETS");
        unset("REINDEX_HISTO_BUCKETS");
        let config = MonitoringConfigPhase15::from_env();
        assert!(!config.histogram_config.search_buckets.is_empty());
        assert!(!config.histogram_config.reindex_buckets.is_empty());
    }

    #[test]
    fn test_summary_format() {
        let config = MonitoringConfigPhase15::production();
        let summary = config.summary();
        assert!(summary.contains("MonitoringConfig"));
        assert!(summary.contains("search:"));
        assert!(summary.contains("reindex:"));
        assert!(summary.contains("50"));
        assert!(summary.contains("1000"));
    }
}
