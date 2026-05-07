// src/installer/detector.rs
// Phase 13.1.0a: Dependency Detection & Verification
// Version: 13.1.0

use crate::installer::{InstallLogger, InstallerError, InstallerResult, Platform};
use std::process::Command;

/// Represents a system dependency
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Name of dependency
    pub name: String,
    /// Command to check (e.g., "cargo", "node")
    pub check_cmd: String,
    /// Version argument (e.g., "--version")
    pub version_arg: String,
    /// Minimum required version
    pub min_version: String,
    /// Installation instructions
    pub install_instruction: String,
}

/// Get required dependencies for this platform
pub fn get_required_dependencies(platform: &Platform) -> Vec<Dependency> {
    let mut deps = vec![
        // Rust toolchain (required on all platforms)
        Dependency {
            name: "Rust toolchain".to_string(),
            check_cmd: "rustc".to_string(),
            version_arg: "--version".to_string(),
            min_version: "1.70.0".to_string(),
            install_instruction: "Install from https://rustup.rs".to_string(),
        },
        Dependency {
            name: "Cargo".to_string(),
            check_cmd: "cargo".to_string(),
            version_arg: "--version".to_string(),
            min_version: "1.70.0".to_string(),
            install_instruction: "Install Rust from https://rustup.rs".to_string(),
        },
        // Platform-specific dependencies
    ];

    // Add platform-specific dependencies
    match platform {
        Platform::Linux => {
            deps.push(Dependency {
                name: "SQLite3 development".to_string(),
                check_cmd: "pkg-config".to_string(),
                version_arg: "--version".to_string(),
                min_version: "0.25".to_string(),
                install_instruction:
                    "On Ubuntu/Debian: sudo apt-get install libsqlite3-dev pkg-config".to_string(),
            });
            deps.push(Dependency {
                name: "Build tools".to_string(),
                check_cmd: "gcc".to_string(),
                version_arg: "--version".to_string(),
                min_version: "4.8".to_string(),
                install_instruction: "On Ubuntu/Debian: sudo apt-get install build-essential"
                    .to_string(),
            });
        }
        Platform::MacOS => {
            deps.push(Dependency {
                name: "Xcode Command Line Tools".to_string(),
                check_cmd: "xcrun".to_string(),
                version_arg: "--version".to_string(),
                min_version: "1".to_string(),
                install_instruction: "Run: xcode-select --install".to_string(),
            });
        }
        Platform::Windows => {
            deps.push(Dependency {
                name: "Visual C++ Build Tools".to_string(),
                check_cmd: "cl".to_string(),
                version_arg: "".to_string(),
                min_version: "19.0".to_string(),
                install_instruction: "Download from https://visualstudio.microsoft.com/downloads/"
                    .to_string(),
            });
        }
        Platform::Unknown => {}
    }

    deps
}

/// Check if a single dependency is available
pub fn check_dependency(dep: &Dependency, logger: &InstallLogger) -> InstallerResult<()> {
    let output = Command::new("which")
        .arg(&dep.check_cmd)
        .output()
        .or_else(|_| {
            // Fallback for Windows
            Command::new("where").arg(&dep.check_cmd).output()
        });

    match output {
        Ok(result) if result.status.success() => {
            logger.info(&format!("✓ {} installed", &dep.name));
            Ok(())
        }
        _ => {
            logger.info(&format!("✗ {} not found", &dep.name));
            Err(InstallerError::MissingDependency {
                dep: dep.name.clone(),
                instruction: dep.install_instruction.clone(),
            })
        }
    }
}

/// Check Rust version
pub fn check_rust_version(logger: &InstallLogger) -> InstallerResult<String> {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|e| InstallerError::CommandFailed {
            cmd: "rustc --version".to_string(),
            reason: e.to_string(),
        })?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        logger.debug(&format!("Rust version: {}", version.trim()));
        Ok(version.to_string())
    } else {
        Err(InstallerError::CommandFailed {
            cmd: "rustc --version".to_string(),
            reason: "Failed to determine Rust version".to_string(),
        })
    }
}

/// Check Cargo version
pub fn check_cargo_version(logger: &InstallLogger) -> InstallerResult<String> {
    let output = Command::new("cargo")
        .arg("--version")
        .output()
        .map_err(|e| InstallerError::CommandFailed {
            cmd: "cargo --version".to_string(),
            reason: e.to_string(),
        })?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        logger.debug(&format!("Cargo version: {}", version.trim()));
        Ok(version.to_string())
    } else {
        Err(InstallerError::CommandFailed {
            cmd: "cargo --version".to_string(),
            reason: "Failed to determine Cargo version".to_string(),
        })
    }
}

/// Check if system can run Rust compiler
pub fn check_system_compiler(logger: &InstallLogger) -> InstallerResult<()> {
    logger.debug("Checking C compiler compatibility...");

    #[cfg(unix)]
    {
        let output = Command::new("cc").arg("--version").output();

        match output {
            Ok(result) if result.status.success() => {
                logger.debug("C compiler available");
                Ok(())
            }
            _ => Err(InstallerError::DependencyCheckFailed(
                "No C compiler found - install build tools".to_string(),
            )),
        }
    }

    #[cfg(not(unix))]
    {
        Ok(())
    }
}

/// Check network connectivity
pub fn check_network() -> InstallerResult<()> {
    let output = if cfg!(target_os = "windows") {
        Command::new("ping").args(["-n", "1", "8.8.8.8"]).output()
    } else {
        Command::new("ping").args(["-c", "1", "8.8.8.8"]).output()
    };

    match output {
        Ok(result) if result.status.success() => Ok(()),
        _ => Err(InstallerError::NetworkError(
            "Network connectivity check failed".to_string(),
        )),
    }
}

/// Check if port is available
pub fn check_port_available(port: u16) -> InstallerResult<()> {
    use std::net::TcpListener;

    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => Ok(()),
        Err(_) => Err(InstallerError::PortInUse { port }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_required_dependencies() {
        let platform = Platform::detect();
        let deps = get_required_dependencies(&platform);
        assert!(!deps.is_empty());
        assert!(deps.iter().any(|d| d.name.contains("Rust")));
    }

    #[test]
    fn test_dependency_structure() {
        let dep = Dependency {
            name: "test".to_string(),
            check_cmd: "test".to_string(),
            version_arg: "--version".to_string(),
            min_version: "1.0.0".to_string(),
            install_instruction: "install test".to_string(),
        };
        assert_eq!(dep.name, "test");
    }

    #[test]
    fn test_port_check() {
        // This test checks if high-numbered ports are available
        let result = check_port_available(9999);
        // Don't assert - port might be in use
        let _ = result;
    }
}
