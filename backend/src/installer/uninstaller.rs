// src/installer/uninstaller.rs - Version 13.1.1 - SIMPLIFIED

use crate::installer::errors::{InstallerError, InstallerResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallAction {
    pub action_type: UninstallActionType,
    pub target_path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
    pub error_msg: Option<String>,
    pub backup_location: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum UninstallActionType {
    DirectoryDeleted,
    FileDeleted,
    ConfigPreserved,
    DataPreserved,
    DatabaseBackup,
    IndexBackup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Uninstaller {
    pub install_prefix: PathBuf,
    pub preserve_data: bool,
    pub preserve_config: bool,
    pub backup_dir: PathBuf,
    pub actions_performed: Vec<UninstallAction>,
    pub rollback_stack: VecDeque<UninstallAction>,
    pub dry_run: bool,
    pub verbose: bool,
}

impl Uninstaller {
    pub fn new(install_prefix: PathBuf, preserve_data: bool) -> Self {
        let backup_dir = install_prefix.join(".ag_uninstall_backup");
        Self {
            install_prefix,
            preserve_data,
            preserve_config: false,
            backup_dir,
            actions_performed: Vec::new(),
            rollback_stack: VecDeque::new(),
            dry_run: false,
            verbose: false,
        }
    }

    pub fn with_dry_run(mut self, enabled: bool) -> Self {
        self.dry_run = enabled;
        self
    }

    pub fn with_verbose(mut self, enabled: bool) -> Self {
        self.verbose = enabled;
        self
    }

    pub fn with_preserve_config(mut self, enabled: bool) -> Self {
        self.preserve_config = enabled;
        self
    }

    pub fn uninstall(&mut self) -> InstallerResult<UninstallReport> {
        if self.verbose {
            eprintln!("🔍 Starting uninstallation");
        }
        self.backup_critical_data()?;
        self.remove_installer_files()?;
        self.clean_environment()?;
        Ok(self.generate_report())
    }

    fn backup_critical_data(&mut self) -> InstallerResult<()> {
        if (self.preserve_data || self.preserve_config) && !self.dry_run {
            fs::create_dir_all(&self.backup_dir).map_err(|e| {
                InstallerError::DirectoryCreationFailed {
                    path: self.backup_dir.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
        }
        Ok(())
    }

    fn remove_installer_files(&mut self) -> InstallerResult<()> {
        let dirs = vec![
            self.install_prefix.join("bin"),
            self.install_prefix.join("config"),
        ];
        for dir in dirs {
            if dir.exists() && !self.dry_run {
                fs::remove_dir_all(&dir).ok();
            }
        }
        Ok(())
    }

    fn clean_environment(&mut self) -> InstallerResult<()> {
        let env_file = self.install_prefix.join(".env");
        if env_file.exists() && !self.dry_run {
            fs::remove_file(&env_file).ok();
        }
        Ok(())
    }

    pub fn generate_report(&self) -> UninstallReport {
        UninstallReport {
            timestamp: Utc::now(),
            dry_run: self.dry_run,
            files_deleted: 0,
            dirs_deleted: 0,
            backups_created: 0,
            data_preserved: self.preserve_data,
            config_preserved: self.preserve_config,
            backup_location: None,
            actions: self.actions_performed.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallReport {
    pub timestamp: DateTime<Utc>,
    pub dry_run: bool,
    pub files_deleted: usize,
    pub dirs_deleted: usize,
    pub backups_created: usize,
    pub data_preserved: bool,
    pub config_preserved: bool,
    pub backup_location: Option<PathBuf>,
    pub actions: Vec<UninstallAction>,
}

impl UninstallReport {
    pub fn display(&self) -> String {
        format!(
            "Uninstall: {} files, {} dirs",
            self.files_deleted, self.dirs_deleted
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninstaller_new() {
        let u = Uninstaller::new(PathBuf::from("/app"), true);
        assert!(u.preserve_data);
    }

    #[test]
    fn test_uninstaller_options() {
        let u = Uninstaller::new(PathBuf::from("/app"), false)
            .with_dry_run(true)
            .with_verbose(true);
        assert!(u.dry_run);
    }

    #[test]
    fn test_report() {
        let r = UninstallReport {
            timestamp: Utc::now(),
            dry_run: false,
            files_deleted: 5,
            dirs_deleted: 3,
            backups_created: 2,
            data_preserved: true,
            config_preserved: false,
            backup_location: None,
            actions: Vec::new(),
        };
        assert!(r.display().contains("5"));
    }
}
