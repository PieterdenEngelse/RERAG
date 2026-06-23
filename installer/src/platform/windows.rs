//! Windows platform stubs.
//!
//! PR1 placeholder — every public surface re-exported by `platform::mod`
//! exists here as a signature with an `unimplemented!()` body so cfg(windows)
//! compiles. PR2 replaces this file with the real implementation
//! (Win32 detection probes, schtasks / docker shellouts, %LOCALAPPDATA%
//! path resolution, Scheduled-Task install).
//!
//! Linux behavior is unaffected — `platform::mod` selects `linux.rs`
//! under `#[cfg(unix)]`, so none of this file is compiled on a Linux
//! build.

use std::path::PathBuf;

use anyhow::Result;

use crate::detection::DetectionResult;
use crate::install_steps::{LogTee, ProgressSender};
use crate::prompts::PromptAnswers;

pub struct Paths {
    pub ag_home: PathBuf,
    pub bin_dir: PathBuf,
    pub lib_dir: PathBuf,
    pub config_dir: PathBuf,
    pub scheduled_tasks_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        unimplemented!("PR2: implement Paths::resolve on Windows")
    }
    pub fn ag_env(&self) -> PathBuf {
        unimplemented!("PR2: implement Paths::ag_env on Windows")
    }
    pub fn docker_compose(&self) -> PathBuf {
        unimplemented!("PR2: implement Paths::docker_compose on Windows")
    }
    pub fn install_log(&self, _timestamp_utc: &str) -> PathBuf {
        unimplemented!("PR2: implement Paths::install_log on Windows")
    }
}

pub fn skip_systemctl() -> bool {
    false
}

pub async fn run_detection() -> DetectionResult {
    unimplemented!("PR2: implement run_detection on Windows")
}

pub async fn ensure_install_tree(
    _paths: &Paths,
    _tx: &ProgressSender,
    _tee: &LogTee,
    _log_path_out: &mut Option<PathBuf>,
) -> Result<()> {
    unimplemented!("PR2: implement ensure_install_tree on Windows")
}

pub async fn copy_artifacts(_paths: &Paths, _tx: &ProgressSender, _tee: &LogTee) -> Result<()> {
    unimplemented!("PR2: implement copy_artifacts on Windows")
}

pub async fn install_stack(_paths: &Paths, _tx: &ProgressSender, _tee: &LogTee) -> Result<()> {
    unimplemented!("PR2: implement install_stack on Windows")
}

pub async fn install_service(
    _paths: &Paths,
    _tx: &ProgressSender,
    _tee: &LogTee,
    _answers: &PromptAnswers,
    _backend_port: u16,
) -> Result<()> {
    unimplemented!("PR2: implement install_service on Windows")
}

pub fn uninstall_targets(_paths: &Paths) -> Vec<PathBuf> {
    unimplemented!("PR2: implement uninstall_targets on Windows")
}

pub async fn uninstall_managed(_paths: &Paths) {
    unimplemented!("PR2: implement uninstall_managed on Windows")
}

pub async fn apply_falkordb_password(
    _paths: &Paths,
    _tx: &ProgressSender,
    _new_password: &str,
) -> Result<()> {
    unimplemented!("PR2: implement apply_falkordb_password on Windows")
}

pub async fn start_ag(_tx: &ProgressSender, _step: &'static str) -> Result<()> {
    unimplemented!("PR2: implement start_ag on Windows")
}
