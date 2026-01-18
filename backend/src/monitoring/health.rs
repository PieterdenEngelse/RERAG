//! Health check endpoints
//!
//! Provides:
//! - GET /monitoring/health - Full health status
//! - GET /monitoring/ready - Readiness probe (K8s compatible)
//! - GET /monitoring/live - Liveness probe (K8s compatible)
//!
//! INSTALLER IMPACT:
//! - Installer must call /health endpoint to verify startup
//! - Should check "status" field == "healthy"

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::perf::CacheAligned;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComponentStatus {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "degraded")]
    Degraded,
    #[serde(rename = "unhealthy")]
    Unhealthy,
    #[serde(rename = "busy")]
    Busy,
}

impl std::fmt::Display for ComponentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentStatus::Healthy => write!(f, "healthy"),
            ComponentStatus::Degraded => write!(f, "degraded"),
            ComponentStatus::Unhealthy => write!(f, "unhealthy"),
            ComponentStatus::Busy => write!(f, "busy"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: ComponentStatus,
    pub timestamp: String,
    pub uptime_seconds: f64,
    pub components: ComponentHealth,
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load: Option<LoadMetrics>,
}

/// System load metrics for "busy" status detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadMetrics {
    /// CPU usage percentage (0-100 per core, can exceed 100 on multi-core)
    pub cpu_percent: f32,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
    /// Number of active long-running tasks (indexing, LLM calls, etc.)
    pub active_tasks: u32,
    /// Estimated queue depth for pending requests
    pub queue_depth: u32,
    /// Whether the system is currently indexing
    pub indexing: bool,
    /// Whether an LLM call is in progress
    pub llm_active: bool,
}

impl Default for LoadMetrics {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_percent: 0.0,
            active_tasks: 0,
            queue_depth: 0,
            indexing: false,
            llm_active: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub api: ComponentStatus,
    pub database: ComponentStatus,
    pub configuration: ComponentStatus,
    pub logging: ComponentStatus,
}

impl Default for ComponentHealth {
    fn default() -> Self {
        Self {
            api: ComponentStatus::Unhealthy,
            database: ComponentStatus::Unhealthy,
            configuration: ComponentStatus::Unhealthy,
            logging: ComponentStatus::Unhealthy,
        }
    }
}

use std::sync::atomic::AtomicU32;
// CacheAligned already imported above

/// Tracks application health
/// 
/// Atomics are cache-line aligned to prevent false sharing when
/// different threads update active_tasks vs check is_ready.
pub struct HealthTracker {
    is_ready: CacheAligned<AtomicBool>,
    is_live: CacheAligned<AtomicBool>,
    components: parking_lot::RwLock<ComponentHealth>,
    startup_time: std::time::Instant,
    // Load tracking - cache-line aligned
    active_tasks: CacheAligned<AtomicU32>,
    indexing: CacheAligned<AtomicBool>,
    llm_active: CacheAligned<AtomicBool>,
}

impl HealthTracker {
    /// Create new health tracker
    pub fn new() -> Self {
        Self {
            is_ready: CacheAligned::new(AtomicBool::new(false)),
            is_live: CacheAligned::new(AtomicBool::new(true)), // Always live until told otherwise
            components: parking_lot::RwLock::new(ComponentHealth::default()),
            startup_time: std::time::Instant::now(),
            active_tasks: CacheAligned::new(AtomicU32::new(0)),
            indexing: CacheAligned::new(AtomicBool::new(false)),
            llm_active: CacheAligned::new(AtomicBool::new(false)),
        }
    }

    /// Mark system as ready
    ///
    /// INSTALLER IMPACT:
    /// - Call after all components initialized
    /// - /ready endpoint will return 200 after this
    pub fn mark_ready(&self) {
        self.is_ready.store(true, Ordering::SeqCst);
        tracing::info!("System marked as ready");
    }

    /// Mark system as not ready
    pub fn mark_not_ready(&self) {
        self.is_ready.store(false, Ordering::SeqCst);
        tracing::warn!("System marked as not ready");
    }

    /// Mark system as not live (will restart if running in container)
    pub fn mark_not_live(&self) {
        self.is_live.store(false, Ordering::SeqCst);
        tracing::error!("System marked as not live");
    }

    /// Update component status
    pub fn set_component_status(&self, component: &str, status: ComponentStatus) {
        let mut components = self.components.write();
        match component {
            "api" => components.api = status.clone(),
            "database" => components.database = status.clone(),
            "configuration" => components.configuration = status.clone(),
            "logging" => components.logging = status.clone(),
            _ => {}
        }
        tracing::debug!(component, status = %status, "Component status updated");
    }

    /// Mark indexing as started
    pub fn start_indexing(&self) {
        self.indexing.store(true, Ordering::SeqCst);
        self.active_tasks.fetch_add(1, Ordering::SeqCst);
        tracing::debug!("Indexing started");
    }

    /// Mark indexing as finished
    pub fn finish_indexing(&self) {
        self.indexing.store(false, Ordering::SeqCst);
        self.active_tasks.fetch_sub(1, Ordering::SeqCst);
        tracing::debug!("Indexing finished");
    }

    /// Mark LLM call as started
    pub fn start_llm_call(&self) {
        self.llm_active.store(true, Ordering::SeqCst);
        self.active_tasks.fetch_add(1, Ordering::SeqCst);
        tracing::debug!("LLM call started");
    }

    /// Mark LLM call as finished
    pub fn finish_llm_call(&self) {
        self.llm_active.store(false, Ordering::SeqCst);
        self.active_tasks.fetch_sub(1, Ordering::SeqCst);
        tracing::debug!("LLM call finished");
    }

    /// Increment active tasks
    pub fn start_task(&self) {
        self.active_tasks.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement active tasks
    pub fn finish_task(&self) {
        self.active_tasks.fetch_sub(1, Ordering::SeqCst);
    }

    /// Check if system is busy
    pub fn is_busy(&self) -> bool {
        self.indexing.load(Ordering::SeqCst)
            || self.llm_active.load(Ordering::SeqCst)
            || self.active_tasks.load(Ordering::SeqCst) > 2
    }

    /// Get current load metrics
    pub fn get_load_metrics(&self) -> LoadMetrics {
        LoadMetrics {
            cpu_percent: 0.0,    // Would need sysinfo crate for real CPU usage
            memory_percent: 0.0, // Would need sysinfo crate for real memory usage
            active_tasks: self.active_tasks.load(Ordering::SeqCst),
            queue_depth: 0, // Would need request queue tracking
            indexing: self.indexing.load(Ordering::SeqCst),
            llm_active: self.llm_active.load(Ordering::SeqCst),
        }
    }

    /// Get current health status
    pub fn get_status(&self) -> HealthStatus {
        let components = self.components.read().clone();
        let load = self.get_load_metrics();
        let is_busy = self.is_busy();

        // Overall status is worst component status, but can be "busy" if under load
        let overall_status = match components {
            _ if components.api == ComponentStatus::Unhealthy
                || components.database == ComponentStatus::Unhealthy
                || components.configuration == ComponentStatus::Unhealthy =>
            {
                ComponentStatus::Unhealthy
            }
            _ if is_busy => ComponentStatus::Busy,
            _ if components.api == ComponentStatus::Degraded
                || components.database == ComponentStatus::Degraded
                || components.configuration == ComponentStatus::Degraded =>
            {
                ComponentStatus::Degraded
            }
            _ => ComponentStatus::Healthy,
        };

        let uptime = self.startup_time.elapsed().as_secs_f64();

        let message = if is_busy {
            Some(format!(
                "System busy: {} active tasks{}{}",
                load.active_tasks,
                if load.indexing { ", indexing" } else { "" },
                if load.llm_active {
                    ", LLM processing"
                } else {
                    ""
                }
            ))
        } else {
            None
        };

        HealthStatus {
            status: overall_status,
            timestamp: chrono::Utc::now().to_rfc3339(),
            uptime_seconds: uptime,
            components,
            message,
            load: Some(load),
        }
    }

    /// Check if system is ready
    pub fn is_ready(&self) -> bool {
        self.is_ready.load(Ordering::SeqCst)
    }

    /// Check if system is live
    pub fn is_live(&self) -> bool {
        self.is_live.load(Ordering::SeqCst)
    }
}

impl Default for HealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_tracker_creation() {
        let tracker = HealthTracker::new();
        assert!(!tracker.is_ready());
        assert!(tracker.is_live());
    }

    #[test]
    fn test_mark_ready() {
        let tracker = HealthTracker::new();
        tracker.mark_ready();
        assert!(tracker.is_ready());
    }

    #[test]
    fn test_component_status_update() {
        let tracker = HealthTracker::new();
        tracker.set_component_status("api", ComponentStatus::Healthy);
        tracker.set_component_status("database", ComponentStatus::Degraded);

        let status = tracker.get_status();
        assert_eq!(status.components.api, ComponentStatus::Healthy);
        assert_eq!(status.components.database, ComponentStatus::Degraded);
    }

    #[test]
    fn test_overall_health_calculation() {
        let tracker = HealthTracker::new();

        // All healthy
        tracker.set_component_status("api", ComponentStatus::Healthy);
        tracker.set_component_status("database", ComponentStatus::Healthy);
        tracker.set_component_status("configuration", ComponentStatus::Healthy);
        tracker.set_component_status("logging", ComponentStatus::Healthy);

        assert_eq!(tracker.get_status().status, ComponentStatus::Healthy);

        // One degraded
        tracker.set_component_status("database", ComponentStatus::Degraded);
        assert_eq!(tracker.get_status().status, ComponentStatus::Degraded);

        // One unhealthy
        tracker.set_component_status("api", ComponentStatus::Unhealthy);
        assert_eq!(tracker.get_status().status, ComponentStatus::Unhealthy);
    }
}
