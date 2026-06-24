//! Platform-portable "open with default app" helper.
//!
//! `xdg-open` is Linux-only; Windows uses `cmd /c start`.

/// Open a URL or path with the OS default handler (browser, file manager, …).
pub fn open(target: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", target])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}
