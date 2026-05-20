//! Lifecycle actions: self re-exec.
//!
//! `restart_self` replaces ag's process image with a fresh copy of the same
//! binary. Works in every deployment — no systemd, no docker, no supervisor.
//! Used to apply boot-bound setting overrides without depending on a host
//! service manager.

use tracing::{error, info};

/// Replace this process with a fresh copy of the same binary, same argv.
/// On success this function never returns. On failure (e.g. argv empty,
/// binary missing) it returns the underlying I/O error.
#[cfg(unix)]
pub fn restart_self() -> std::io::Error {
    use std::os::unix::process::CommandExt;

    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.is_empty() {
        let err = std::io::Error::other("argv is empty — cannot re-exec");
        error!("restart_self: {err}");
        return err;
    }
    info!("restart_self: re-executing {:?}", args[0]);
    let err = std::process::Command::new(&args[0]).args(&args[1..]).exec();
    error!("restart_self: exec failed: {err}");
    err
}

/// Windows fallback: spawn a new process and exit. Not as clean as `execve`
/// (PID changes, briefly two processes), but functionally equivalent.
#[cfg(windows)]
pub fn restart_self() -> std::io::Error {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.is_empty() {
        let err = std::io::Error::other("argv is empty — cannot re-exec");
        error!("restart_self: {err}");
        return err;
    }
    info!("restart_self: spawning {:?} and exiting", args[0]);
    match std::process::Command::new(&args[0])
        .args(&args[1..])
        .spawn()
    {
        Ok(_) => {
            std::process::exit(0);
        }
        Err(e) => {
            error!("restart_self: spawn failed: {e}");
            e
        }
    }
}
