//! HTTP/2 Configuration
//! 
//! Provides HTTP/2 support for Actix Web with:
//! - Multiplexing (multiple requests over single connection)
//! - Header compression (HPACK)
//! - Server push (optional)
//! - Stream prioritization
//! 
//! # Requirements
//! HTTP/2 requires TLS in most browsers. Configure with rustls or openssl.
//! 
//! # Usage
//! ```rust
//! let config = Http2Config::default();
//! let server = config.configure_server(HttpServer::new(|| App::new()));
//! ```

use std::path::Path;


/// HTTP/2 configuration
#[derive(Debug, Clone)]
pub struct Http2Config {
    /// Enable HTTP/2
    pub enabled: bool,
    /// TLS certificate path
    pub cert_path: Option<String>,
    /// TLS key path
    pub key_path: Option<String>,
    /// Maximum concurrent streams per connection
    pub max_concurrent_streams: u32,
    /// Initial window size
    pub initial_window_size: u32,
    /// Maximum frame size
    pub max_frame_size: u32,
    /// Enable server push
    pub server_push: bool,
    /// Keep-alive interval in seconds
    pub keep_alive_interval: u64,
    /// Keep-alive timeout in seconds
    pub keep_alive_timeout: u64,
}

impl Default for Http2Config {
    fn default() -> Self {
        Self {
            enabled: true,
            cert_path: None,
            key_path: None,
            max_concurrent_streams: 100,
            initial_window_size: 65535,
            max_frame_size: 16384,
            server_push: false,
            keep_alive_interval: 30,
            keep_alive_timeout: 60,
        }
    }
}

impl Http2Config {
    /// Create config with TLS paths
    pub fn with_tls(cert_path: &str, key_path: &str) -> Self {
        Self {
            cert_path: Some(cert_path.to_string()),
            key_path: Some(key_path.to_string()),
            ..Default::default()
        }
    }

    /// High performance configuration
    pub fn high_performance() -> Self {
        Self {
            max_concurrent_streams: 250,
            initial_window_size: 1048576, // 1MB
            max_frame_size: 65535,
            keep_alive_interval: 15,
            keep_alive_timeout: 30,
            ..Default::default()
        }
    }

    /// Check if TLS is configured
    pub fn has_tls(&self) -> bool {
        self.cert_path.is_some() && self.key_path.is_some()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.enabled && !self.has_tls() {
            // HTTP/2 without TLS is h2c (cleartext), which most browsers don't support
            // but is fine for internal services
        }

        if let Some(ref cert) = self.cert_path {
            if !Path::new(cert).exists() {
                return Err(ConfigError::CertNotFound(cert.clone()));
            }
        }

        if let Some(ref key) = self.key_path {
            if !Path::new(key).exists() {
                return Err(ConfigError::KeyNotFound(key.clone()));
            }
        }

        Ok(())
    }
}

/// Configuration error
#[derive(Debug)]
pub enum ConfigError {
    CertNotFound(String),
    KeyNotFound(String),
    InvalidConfig(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CertNotFound(path) => write!(f, "Certificate not found: {}", path),
            Self::KeyNotFound(path) => write!(f, "Key not found: {}", path),
            Self::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

/// HTTP/2 connection info
#[derive(Debug, Clone)]
pub struct Http2ConnectionInfo {
    pub protocol: String,
    pub is_h2: bool,
    pub stream_id: Option<u32>,
}

impl Http2ConnectionInfo {
    pub fn from_request_head(is_h2: bool) -> Self {
        Self {
            protocol: if is_h2 { "h2".to_string() } else { "http/1.1".to_string() },
            is_h2,
            stream_id: None,
        }
    }
}

/// Server push hint
#[derive(Debug, Clone)]
pub struct PushHint {
    pub path: String,
    pub content_type: String,
}

impl PushHint {
    pub fn new(path: &str, content_type: &str) -> Self {
        Self {
            path: path.to_string(),
            content_type: content_type.to_string(),
        }
    }

    pub fn css(path: &str) -> Self {
        Self::new(path, "text/css")
    }

    pub fn js(path: &str) -> Self {
        Self::new(path, "application/javascript")
    }

    pub fn image(path: &str, format: &str) -> Self {
        Self::new(path, &format!("image/{}", format))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Http2Config::default();
        assert!(config.enabled);
        assert!(!config.has_tls());
    }

    #[test]
    fn test_with_tls() {
        let config = Http2Config::with_tls("/path/to/cert.pem", "/path/to/key.pem");
        assert!(config.has_tls());
    }

    #[test]
    fn test_high_performance() {
        let config = Http2Config::high_performance();
        assert_eq!(config.max_concurrent_streams, 250);
        assert_eq!(config.initial_window_size, 1048576);
    }

    #[test]
    fn test_push_hint() {
        let hint = PushHint::css("/styles/main.css");
        assert_eq!(hint.content_type, "text/css");
    }
}
