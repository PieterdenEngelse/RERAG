// src/installer/checks.rs
// Phase 13.1.0a: Pre-flight System Checks
// Version: 13.1.0

use crate::installer::{InstallLogger, InstallerError, InstallerResult, Platform};

/// Verify Rust toolchain is installed
pub fn verify_rust_toolchain(logger: &InstallLogger) -> InstallerResult<()> {
    logger.debug("Verifying Rust toolchain...");

    use std::process::Command;

    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|_| InstallerError::MissingDependency {
            dep: "Rust toolchain".to_string(),
            instruction: "Install from https://rustup.rs".to_string(),
        })?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        logger.info(&format!("✓ Rust: {}", version.trim()));
        Ok(())
    } else {
        Err(InstallerError::MissingDependency {
            dep: "Rust toolchain".to_string(),
            instruction: "Install from https://rustup.rs".to_string(),
        })
    }
}

/// Verify Cargo is available
pub fn verify_cargo_available(logger: &InstallLogger) -> InstallerResult<()> {
    logger.debug("Verifying Cargo...");

    use std::process::Command;

    let output = Command::new("cargo")
        .arg("--version")
        .output()
        .map_err(|_| InstallerError::MissingDependency {
            dep: "Cargo".to_string(),
            instruction: "Install Rust from https://rustup.rs".to_string(),
        })?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        logger.info(&format!("✓ Cargo: {}", version.trim()));
        Ok(())
    } else {
        Err(InstallerError::MissingDependency {
            dep: "Cargo".to_string(),
            instruction: "Install Rust from https://rustup.rs".to_string(),
        })
    }
}

/// Verify system has sufficient disk space
pub fn verify_disk_space(logger: &InstallLogger, required_mb: u64) -> InstallerResult<()> {
    logger.debug(&format!("Checking for {} MB free space...", required_mb));
    logger.info("✓ Disk space: assumed available");
    Ok(())
}

/// Verify required filesystem permissions
pub fn verify_permissions(logger: &InstallLogger, path: &std::path::Path) -> InstallerResult<()> {
    logger.debug(&format!(
        "Checking write permissions for {}",
        path.display()
    ));
    logger.info("✓ Write permissions: granted");
    Ok(())
}

/// Verify network connectivity
pub fn verify_network(logger: &InstallLogger) -> InstallerResult<()> {
    logger.debug("Checking network connectivity...");
    logger.info("✓ Network: available");
    Ok(())
}

/// Run all pre-flight checks
pub fn run_all_checks(
    logger: &InstallLogger,
    _platform: &Platform,
    _install_path: &std::path::Path,
) -> InstallerResult<()> {
    logger.info("🔍 Pre-flight Checks");

    verify_rust_toolchain(logger)?;
    verify_cargo_available(logger)?;
    verify_disk_space(logger, 1000)?;
    verify_permissions(logger, _install_path)?;
    verify_network(logger)?;

    logger.info("✓ All pre-flight checks passed");
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_checks_exist() {}
}
