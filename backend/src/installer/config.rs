// src/installer/config.rs
// Version: 13.1.1
// Installation configuration management with installer impact tracking

use crate::installer::errors::InstallerResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Installation phase tracking for impact analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InstallationPhase {
    /// Pre-installation phase
    PreFlight,
    /// Directories being created
    DirectoriesCreated,
    /// Configuration files written
    ConfigurationWritten,
    /// Backend services initialized
    BackendInitialized,
    /// Frontend assets deployed
    FrontendDeployed,
    /// Database initialized
    DatabaseInitialized,
    /// Index system initialized
    IndexInitialized,
    /// Installation complete
    Completed,
    /// Installation failed - rollback needed
    Failed,
}

impl std::fmt::Display for InstallationPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreFlight => write!(f, "PreFlight"),
            Self::DirectoriesCreated => write!(f, "DirectoriesCreated"),
            Self::ConfigurationWritten => write!(f, "ConfigurationWritten"),
            Self::BackendInitialized => write!(f, "BackendInitialized"),
            Self::FrontendDeployed => write!(f, "FrontendDeployed"),
            Self::DatabaseInitialized => write!(f, "DatabaseInitialized"),
            Self::IndexInitialized => write!(f, "IndexInitialized"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

/// Installation impact tracking - comprehensive record of all changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerImpact {
    /// Directories created during installation
    pub directories_created: Vec<PathBuf>,

    /// Configuration files written
    pub config_files_written: Vec<PathBuf>,

    /// Database files created
    pub database_files: Vec<PathBuf>,

    /// Index files created
    pub index_files: Vec<PathBuf>,

    /// Environment variables set
    pub env_vars_set: HashMap<String, String>,

    /// Service ports claimed
    pub ports_claimed: HashMap<String, u16>,

    /// Installation timestamp
    pub installed_at: String,

    /// Installation phase reached
    pub current_phase: InstallationPhase,

    /// Total installation duration (seconds)
    pub duration_secs: u64,

    /// Rollback state - can we safely uninstall?
    pub is_rollbackable: bool,

    /// Preserve user data on uninstall
    pub preserve_data_on_uninstall: bool,
}

impl Default for InstallerImpact {
    fn default() -> Self {
        Self {
            directories_created: Vec::new(),
            config_files_written: Vec::new(),
            database_files: Vec::new(),
            index_files: Vec::new(),
            env_vars_set: HashMap::new(),
            ports_claimed: HashMap::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            current_phase: InstallationPhase::PreFlight,
            duration_secs: 0,
            is_rollbackable: true,
            preserve_data_on_uninstall: true,
        }
    }
}

impl InstallerImpact {
    /// Record a created directory for rollback tracking
    pub fn track_directory(&mut self, path: PathBuf) {
        if !self.directories_created.contains(&path) {
            self.directories_created.push(path);
        }
    }

    /// Record a written configuration file
    pub fn track_config_file(&mut self, path: PathBuf) {
        if !self.config_files_written.contains(&path) {
            self.config_files_written.push(path);
        }
    }

    /// Record database file creation
    pub fn track_database_file(&mut self, path: PathBuf) {
        if !self.database_files.contains(&path) {
            self.database_files.push(path);
        }
    }

    /// Record index file creation
    pub fn track_index_file(&mut self, path: PathBuf) {
        if !self.index_files.contains(&path) {
            self.index_files.push(path);
        }
    }

    /// Record a claimed port
    pub fn track_port(&mut self, service: String, port: u16) {
        self.ports_claimed.insert(service, port);
    }

    /// Record an environment variable set
    pub fn track_env_var(&mut self, key: String, value: String) {
        self.env_vars_set.insert(key, value);
    }

    /// Transition to next installation phase
    pub fn advance_phase(&mut self, phase: InstallationPhase) {
        self.current_phase = phase;
    }

    /// Get all created resources for rollback purposes
    pub fn get_rollback_items(&self) -> Vec<PathBuf> {
        let mut items = self.directories_created.clone();
        items.extend(self.config_files_written.clone());
        items.extend(self.database_files.clone());
        items.extend(self.index_files.clone());
        items
    }

    /// Generate installation impact report
    pub fn generate_report(&self) -> String {
        let phase_str = format!("{}", self.current_phase);
        format!(
            r#"
╔═══════════════════════════════════════════════════════════════════════╗
║           INSTALLATION IMPACT REPORT - Phase 13.1.1                   ║
╚═══════════════════════════════════════════════════════════════════════╝

Installation Time:   {}
Duration:           {} seconds
Current Phase:      {}
Rollbackable:       {}
Preserve Data:      {}

DIRECTORIES CREATED: ({})
{}

CONFIGURATION FILES: ({})
{}

DATABASE FILES: ({})
{}

INDEX FILES: ({})
{}

ENVIRONMENT VARIABLES: ({})
{}

PORTS ALLOCATED: ({})
{}

╚═══════════════════════════════════════════════════════════════════════╝
"#,
            self.installed_at,
            self.duration_secs,
            phase_str,
            self.is_rollbackable,
            self.preserve_data_on_uninstall,
            self.directories_created.len(),
            self.directories_created
                .iter()
                .map(|p| format!("  ├─ {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n"),
            self.config_files_written.len(),
            self.config_files_written
                .iter()
                .map(|p| format!("  ├─ {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n"),
            self.database_files.len(),
            self.database_files
                .iter()
                .map(|p| format!("  ├─ {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n"),
            self.index_files.len(),
            self.index_files
                .iter()
                .map(|p| format!("  ├─ {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n"),
            self.env_vars_set.len(),
            self.env_vars_set
                .iter()
                .map(|(k, v)| format!("  ├─ {} = {}", k, v))
                .collect::<Vec<_>>()
                .join("\n"),
            self.ports_claimed.len(),
            self.ports_claimed
                .iter()
                .map(|(s, p)| format!("  ├─ {} : {}", s, p))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// Save impact report to file
    pub fn save_report(&self, path: &PathBuf) -> InstallerResult<()> {
        let report = self.generate_report();
        std::fs::write(path, report)?;
        Ok(())
    }
}

/// Main installation configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerConfig {
    /// Installation prefix directory
    pub install_prefix: PathBuf,

    /// Data directory for documents and indices
    pub data_dir: PathBuf,

    /// Configuration directory
    pub config_dir: PathBuf,

    /// Cache directory
    pub cache_dir: PathBuf,

    /// Log directory
    pub log_dir: PathBuf,

    /// Backend configuration directory
    pub backend_config_dir: PathBuf,

    /// Frontend build directory
    pub frontend_build_dir: PathBuf,

    /// Backend port
    pub backend_port: u16,

    /// Frontend port
    pub frontend_port: u16,

    /// API host
    pub api_host: String,

    /// Enable development mode
    pub dev_mode: bool,

    /// Enable logging
    pub enable_logging: bool,

    /// Log level
    pub log_level: String,

    /// Maximum document size in MB
    pub max_document_size_mb: u32,

    /// Chunk size for document processing
    pub chunk_size: usize,

    /// Vector dimension
    pub vector_dimension: usize,

    /// Database path
    pub db_path: PathBuf,

    /// Index path
    pub index_path: PathBuf,

    /// Installation timestamp
    pub installed_at: String,

    /// Installer version
    pub installer_version: String,

    /// Installation impact tracking
    pub impact: InstallerImpact,

    /// Preserve user data on uninstall
    pub preserve_data_on_uninstall: bool,

    /// Configuration metadata version
    pub config_version: String,
}

impl Default for InstallerConfig {
    fn default() -> Self {
        let home = dirs::home_dir().expect("Could not determine home directory");
        let install_prefix = home.join(".ag");

        Self {
            data_dir: install_prefix.join("data"),
            config_dir: install_prefix.join("config"),
            cache_dir: install_prefix.join("cache"),
            log_dir: install_prefix.join("logs"),
            backend_config_dir: install_prefix.join("config/backend"),
            frontend_build_dir: install_prefix.join("frontend/build"),
            install_prefix,
            backend_port: 8000,
            frontend_port: 3011,
            api_host: "127.0.0.1".to_string(),
            dev_mode: false,
            enable_logging: true,
            log_level: "info".to_string(),
            max_document_size_mb: 100,
            chunk_size: 512,
            vector_dimension: 384,
            db_path: home.join(".ag/data/agent.db"),
            index_path: home.join(".ag/data/tantivy_index"),
            installed_at: chrono::Utc::now().to_rfc3339(),
            installer_version: "13.1.1".to_string(),
            impact: InstallerImpact::default(),
            preserve_data_on_uninstall: true,
            config_version: "13.1.1".to_string(),
        }
    }
}

impl InstallerConfig {
    /// Create configuration with custom prefix
    #[allow(clippy::field_reassign_with_default)]
    pub fn with_prefix(prefix: PathBuf) -> Self {
        let mut config = Self::default();
        config.install_prefix = prefix.clone();
        config.data_dir = prefix.join("data");
        config.config_dir = prefix.join("config");
        config.cache_dir = prefix.join("cache");
        config.log_dir = prefix.join("logs");
        config.backend_config_dir = prefix.join("config/backend");
        config.frontend_build_dir = prefix.join("frontend/build");
        config.db_path = prefix.join("data/agent.db");
        config.index_path = prefix.join("data/tantivy_index");
        config
    }

    /// Load configuration from file
    pub fn from_file(path: &PathBuf) -> InstallerResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self, path: &PathBuf) -> InstallerResult<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get all required directories
    pub fn get_directories(&self) -> Vec<&PathBuf> {
        vec![
            &self.install_prefix,
            &self.data_dir,
            &self.config_dir,
            &self.backend_config_dir,
            &self.cache_dir,
            &self.log_dir,
            &self.frontend_build_dir,
        ]
    }

    /// Get API URL
    pub fn api_url(&self) -> String {
        format!("http://{}:{}", self.api_host, self.backend_port)
    }

    /// Get frontend URL
    pub fn frontend_url(&self) -> String {
        format!("http://{}:{}", self.api_host, self.frontend_port)
    }

    /// Validate configuration
    pub fn validate(&self) -> InstallerResult<()> {
        // Check port ranges
        if self.backend_port == 0 || self.backend_port == self.frontend_port {
            return Err(
                crate::installer::errors::InstallerError::InvalidConfiguration(
                    "Invalid port configuration".to_string(),
                ),
            );
        }

        // Check directory paths
        if self.data_dir.as_os_str().is_empty() {
            return Err(
                crate::installer::errors::InstallerError::InvalidConfiguration(
                    "Data directory not set".to_string(),
                ),
            );
        }

        // Check document size
        if self.max_document_size_mb == 0 {
            return Err(
                crate::installer::errors::InstallerError::InvalidConfiguration(
                    "Maximum document size must be > 0".to_string(),
                ),
            );
        }

        // Check chunk size
        if self.chunk_size == 0 {
            return Err(
                crate::installer::errors::InstallerError::InvalidConfiguration(
                    "Chunk size must be > 0".to_string(),
                ),
            );
        }

        // Check vector dimension
        if self.vector_dimension == 0 {
            return Err(
                crate::installer::errors::InstallerError::InvalidConfiguration(
                    "Vector dimension must be > 0".to_string(),
                ),
            );
        }

        Ok(())
    }

    /// Generate .env file content
    pub fn generate_env_content(&self) -> String {
        format!(
            r#"# RERAG Configuration - Generated by Installer v{}
# Installation Date: {}
# Configuration Version: {}

# ────────────────────────────────────────────────────────────────────────
# BACKEND CONFIGURATION
# ────────────────────────────────────────────────────────────────────────
BACKEND_HOST={}
BACKEND_PORT={}
API_URL={}

# ────────────────────────────────────────────────────────────────────────
# FRONTEND CONFIGURATION
# ────────────────────────────────────────────────────────────────────────
FRONTEND_PORT={}

# ────────────────────────────────────────────────────────────────────────
# DATA & STORAGE PATHS
# ────────────────────────────────────────────────────────────────────────
DATA_DIR={}
CONFIG_DIR={}
BACKEND_CONFIG_DIR={}
FRONTEND_BUILD_DIR={}
CACHE_DIR={}
LOG_DIR={}

# ────────────────────────────────────────────────────────────────────────
# DATABASE & INDEX
# ────────────────────────────────────────────────────────────────────────
DATABASE_PATH={}
INDEX_PATH={}

# ────────────────────────────────────────────────────────────────────────
# PROCESSING PARAMETERS
# ────────────────────────────────────────────────────────────────────────
CHUNK_SIZE={}
VECTOR_DIMENSION={}
MAX_DOCUMENT_SIZE_MB={}

# ────────────────────────────────────────────────────────────────────────
# LOGGING
# ────────────────────────────────────────────────────────────────────────
LOG_LEVEL={}
ENABLE_LOGGING={}

# ────────────────────────────────────────────────────────────────────────
# DEVELOPMENT
# ────────────────────────────────────────────────────────────────────────
DEV_MODE={}

# ────────────────────────────────────────────────────────────────────────
# INSTALLER METADATA
# ────────────────────────────────────────────────────────────────────────
INSTALLER_VERSION={}
INSTALLED_AT={}
PRESERVE_DATA_ON_UNINSTALL={}
"#,
            self.installer_version,
            chrono::Utc::now().to_rfc3339(),
            self.config_version,
            self.api_host,
            self.backend_port,
            self.api_url(),
            self.frontend_port,
            self.data_dir.display(),
            self.config_dir.display(),
            self.backend_config_dir.display(),
            self.frontend_build_dir.display(),
            self.cache_dir.display(),
            self.log_dir.display(),
            self.db_path.display(),
            self.index_path.display(),
            self.chunk_size,
            self.vector_dimension,
            self.max_document_size_mb,
            self.log_level,
            self.enable_logging,
            self.dev_mode,
            self.installer_version,
            self.installed_at,
            self.preserve_data_on_uninstall,
        )
    }

    /// Generate JSON configuration
    pub fn to_json(&self) -> InstallerResult<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Generate configuration summary for CLI output
    pub fn generate_summary(&self) -> String {
        format!(
            r#"
╔═══════════════════════════════════════════════════════════════════════╗
║              INSTALLATION CONFIGURATION SUMMARY v13.1.1               ║
╚═══════════════════════════════════════════════════════════════════════╝

INSTALLATION PATHS:
  Install Prefix:     {}
  Data Directory:     {}
  Config Directory:   {}
  Log Directory:      {}

SERVICE ENDPOINTS:
  Backend API:        {}
  Frontend:           {}

PROCESSING PARAMETERS:
  Chunk Size:         {}
  Vector Dimension:   {}
  Max Document Size:  {} MB

DATABASE & INDEX:
  Database:           {}
  Index:              {}

DEVELOPMENT:
  Dev Mode:           {}
  Logging Enabled:    {}
  Log Level:          {}

INSTALLER:
  Version:            {}
  Installed At:       {}

╚═══════════════════════════════════════════════════════════════════════╝
"#,
            self.install_prefix.display(),
            self.data_dir.display(),
            self.config_dir.display(),
            self.log_dir.display(),
            self.api_url(),
            self.frontend_url(),
            self.chunk_size,
            self.vector_dimension,
            self.max_document_size_mb,
            self.db_path.display(),
            self.index_path.display(),
            self.dev_mode,
            self.enable_logging,
            self.log_level,
            self.installer_version,
            self.installed_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = InstallerConfig::default();
        assert_eq!(config.backend_port, 8000);
        assert_eq!(config.frontend_port, 3011);
        assert_eq!(config.installer_version, "13.1.1");
    }

    #[test]
    fn test_config_validation() {
        let config = InstallerConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_api_url() {
        let config = InstallerConfig::default();
        assert_eq!(config.api_url(), "http://127.0.0.1:8000");
    }

    #[test]
    fn test_frontend_url() {
        let config = InstallerConfig::default();
        assert_eq!(config.frontend_url(), "http://127.0.0.1:3011");
    }

    #[test]
    fn test_env_generation() {
        let config = InstallerConfig::default();
        let env = config.generate_env_content();
        assert!(env.contains("BACKEND_PORT=8000"));
        assert!(env.contains("FRONTEND_PORT=3011"));
        assert!(env.contains("INSTALLER_VERSION=13.1.1"));
    }

    #[test]
    fn test_config_with_prefix() {
        let prefix = PathBuf::from("/custom/path");
        let config = InstallerConfig::with_prefix(prefix.clone());
        assert_eq!(config.install_prefix, prefix);
        assert!(config.data_dir.to_string_lossy().contains("custom/path"));
    }

    #[test]
    fn test_installer_impact_tracking() {
        let mut impact = InstallerImpact::default();
        impact.track_directory(PathBuf::from("/test/dir"));
        impact.track_port("backend".to_string(), 8000);
        impact.track_env_var("TEST".to_string(), "value".to_string());

        assert_eq!(impact.directories_created.len(), 1);
        assert_eq!(impact.ports_claimed.len(), 1);
        assert_eq!(impact.env_vars_set.len(), 1);
    }

    #[test]
    fn test_installation_phase_display() {
        assert_eq!(format!("{}", InstallationPhase::PreFlight), "PreFlight");
        assert_eq!(format!("{}", InstallationPhase::Completed), "Completed");
    }

    #[test]
    fn test_config_summary_generation() {
        let config = InstallerConfig::default();
        let summary = config.generate_summary();
        assert!(summary.contains("INSTALLATION CONFIGURATION SUMMARY v13.1.1"));
        assert!(summary.contains("Backend API:"));
    }
}
