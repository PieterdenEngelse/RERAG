// File: src/monitoring/histogram_config.rs
// Phase 15 Step 4: Configurability – Logging and Metrics
// Version: 1.1.0
// Location: src/monitoring/histogram_config.rs
//
// Purpose: Configure Prometheus histogram buckets via environment variables
// Allows operators to tune metrics collection for their specific needs
//
// Behavior:
// - Env vars: SEARCH_HISTO_BUCKETS, REINDEX_HISTO_BUCKETS
// - Comma-separated numbers (ms). Example: "1,2,5,10,20,50,100,250,500,1000"
// - Lenient parsing: invalid tokens are ignored with a warning; valid tokens are kept
// - If no valid tokens remain or env not set, defaults are used
// - Parsed buckets are sorted ascending and deduplicated
//
// Examples:
//   SEARCH_HISTO_BUCKETS="10,abc, , -,100" -> [10.0, 100.0]
//   REINDEX_HISTO_BUCKETS=",,," or unset -> defaults

use std::env;

/// Default histogram buckets in milliseconds for search operations
pub const DEFAULT_SEARCH_BUCKETS: &[f64] = &[
    10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
];

/// Default histogram buckets in milliseconds for reindex operations
pub const DEFAULT_REINDEX_BUCKETS: &[f64] = &[
    100.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 30000.0, 60000.0,
];

/// Configuration for histogram buckets - loaded from environment at startup
#[derive(Debug, Clone)]
pub struct HistogramBuckets {
    pub search_buckets: Vec<f64>,
    pub reindex_buckets: Vec<f64>,
}

impl HistogramBuckets {
    /// Load histogram bucket configuration from environment variables.
    ///
    /// Environment variables:
    /// - `SEARCH_HISTO_BUCKETS`: Comma-separated millisecond thresholds for search histogram
    ///   Example: `SEARCH_HISTO_BUCKETS=10,50,100,500,1000`
    ///   Falls back to DEFAULT_SEARCH_BUCKETS if not set or invalid
    ///
    /// - `REINDEX_HISTO_BUCKETS`: Comma-separated millisecond thresholds for reindex histogram
    ///   Example: `REINDEX_HISTO_BUCKETS=500,2000,5000,30000`
    ///   Falls back to DEFAULT_REINDEX_BUCKETS if not set or invalid
    ///
    /// # Returns
    /// A new HistogramBuckets instance with values from environment or defaults
    ///
    /// # Panics
    /// Never panics; invalid values are logged as warnings and defaults are used
    pub fn from_env() -> Self {
        let search_buckets = Self::parse_buckets("SEARCH_HISTO_BUCKETS", DEFAULT_SEARCH_BUCKETS);
        let reindex_buckets = Self::parse_buckets("REINDEX_HISTO_BUCKETS", DEFAULT_REINDEX_BUCKETS);

        tracing::info!(
            search_buckets = ?search_buckets,
            reindex_buckets = ?reindex_buckets,
            "Histogram buckets configured"
        );

        Self {
            search_buckets,
            reindex_buckets,
        }
    }

    /// Parse histogram buckets from an environment variable.
    ///
    /// Format: Comma-separated numbers (floats or integers)
    /// Example: "10,50,100,500,1000"
    ///
    /// # Arguments
    /// * `env_var` - Environment variable name to read
    /// * `default_buckets` - Fallback buckets if env var not set or invalid
    ///
    /// # Returns
    /// Vector of f64 buckets, sorted in ascending order
    fn parse_buckets(env_var: &str, default_buckets: &[f64]) -> Vec<f64> {
        match env::var(env_var) {
            Ok(value) => {
                let mut had_invalid = false;
                let mut buckets: Vec<f64> = value
                    .split(',')
                    .filter_map(|s| {
                        let t = s.trim();
                        if t.is_empty() {
                            had_invalid = true;
                            return None;
                        }
                        match t.parse::<f64>() {
                            Ok(v) => Some(v),
                            Err(_) => {
                                had_invalid = true;
                                None
                            }
                        }
                    })
                    .collect();

                if buckets.is_empty() {
                    tracing::warn!(
                        env_var,
                        raw_value = ?value,
                        "No valid histogram buckets parsed; using defaults"
                    );
                    return default_buckets.to_vec();
                }

                if had_invalid {
                    tracing::warn!(
                        env_var,
                        raw_value = ?value,
                        parsed_buckets = ?buckets,
                        "Some invalid tokens skipped; using parsed valid values"
                    );
                }

                // Sort and ensure uniqueness
                buckets.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                buckets.dedup();
                buckets
            }
            Err(_) => {
                tracing::debug!(
                    env_var,
                    "Environment variable not set, using default histogram buckets"
                );
                default_buckets.to_vec()
            }
        }
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
    fn test_parse_returns_defaults_when_env_unset() {
        let var = "TEST_VAR_UNSET";
        unset(var);
        let result = HistogramBuckets::parse_buckets(var, &[10.0, 50.0, 100.0]);
        assert_eq!(result, vec![10.0, 50.0, 100.0]);
    }

    #[test]
    fn test_parse_valid_custom_buckets_sorted_and_deduped() {
        let var = "TEST_VAR_VALID";
        env::set_var(var, "100, 50, 50, 10, 25");
        let result = HistogramBuckets::parse_buckets(var, &[1.0, 2.0, 3.0]);
        assert_eq!(result, vec![10.0, 25.0, 50.0, 100.0]);
        unset(var);
    }

    #[test]
    fn test_parse_with_invalid_tokens_is_lenient_and_keeps_valids() {
        let var = "TEST_VAR_INVALID";
        env::set_var(var, "10,abc, , -,100");
        // Lenient behavior: ignore invalid tokens, keep valid ones
        let result = HistogramBuckets::parse_buckets(var, &[10.0, 20.0, 30.0]);
        assert_eq!(result, vec![10.0, 100.0]);
        unset(var);
    }

    #[test]
    fn test_from_env_uses_defaults_when_unset() {
        unset("SEARCH_HISTO_BUCKETS");
        unset("REINDEX_HISTO_BUCKETS");
        let cfg = HistogramBuckets::from_env();
        assert_eq!(cfg.search_buckets, DEFAULT_SEARCH_BUCKETS);
        assert_eq!(cfg.reindex_buckets, DEFAULT_REINDEX_BUCKETS);
    }

    #[test]
    fn test_from_env_uses_custom_when_valid() {
        // Clean up first
        unset("SEARCH_HISTO_BUCKETS");
        unset("REINDEX_HISTO_BUCKETS");

        env::set_var("SEARCH_HISTO_BUCKETS", "1,2,5,10");
        env::set_var("REINDEX_HISTO_BUCKETS", "50,100,250,500,1000");

        let cfg = HistogramBuckets::from_env();

        assert_eq!(cfg.search_buckets, vec![1.0, 2.0, 5.0, 10.0]);
        assert_eq!(cfg.reindex_buckets, vec![50.0, 100.0, 250.0, 500.0, 1000.0]);

        // Clean up after
        unset("SEARCH_HISTO_BUCKETS");
        unset("REINDEX_HISTO_BUCKETS");
    }

    #[test]
    fn test_default_search_buckets_sorted() {
        let sorted = DEFAULT_SEARCH_BUCKETS.to_vec();
        assert!(sorted.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn test_default_reindex_buckets_sorted() {
        let sorted = DEFAULT_REINDEX_BUCKETS.to_vec();
        assert!(sorted.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn test_parse_with_invalid_tokens_keeps_valid() {
        let var = "TEST_VAR_INVALID2";
        env::set_var(var, "10, abc, 50, 100");
        // Should keep 10, 50, 100 (skip abc)
        let result = HistogramBuckets::parse_buckets(var, &[1.0, 2.0]);
        assert_eq!(result, vec![10.0, 50.0, 100.0]);
        unset(var);
    }
}
