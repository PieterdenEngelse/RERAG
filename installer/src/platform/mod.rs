//! Platform boundary for the installer.
//!
//! Everything OS-specific — path resolution, detection probes, the bodies
//! of install_steps that shell out to systemd/schtasks/docker, first-run
//! service start, uninstall cleanup — lives behind this module. The
//! Dioxus UI, prompt model, progress events, update check, and bundled-
//! asset resolution stay shared and call into the cfg-selected
//! implementation here.
//!
//! Subsequent PR1.* tasks lift Linux code from `detection.rs`,
//! `install_steps.rs`, `first_run.rs`, and `uninstall.rs` into
//! `platform::linux`. PR2 fills in `platform::windows`.

#[cfg(unix)]
mod linux;

#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use linux::{
    apply_falkordb_password, copy_artifacts, disk_free_gb, ensure_install_tree, install_service,
    install_stack, run_detection, skip_systemctl, start_ag, uninstall_managed, uninstall_targets,
    Paths,
};

#[cfg(windows)]
pub use windows::{
    apply_falkordb_password, copy_artifacts, disk_free_gb, enable_wsl2, ensure_install_tree,
    install_docker, install_docker_wsl2, install_service, install_stack, register_wsl2_resume,
    run_detection, skip_systemctl, start_ag, uninstall_managed, uninstall_targets, Paths,
    WslEnableOutcome,
};
