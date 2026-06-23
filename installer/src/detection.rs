//! Detection result struct + thin orchestrator wrapper.
//!
//! The probe bodies live under `crate::platform::{linux,windows}` —
//! lifted there in PR 1.3 so the same orchestrator shape can be reused
//! on Windows in PR 2 (where probes shell out to `schtasks` / `winreg`
//! / `fs2` / raw RESP `PING` instead of `systemctl` / `/proc` /
//! `redis-cli`). The `DetectionResult` shape itself stays shared — the
//! UI tree reads it without knowing which OS produced it.
//!
//! See `docs/bin3 §Phase C` for the spec and `docs/wininstall.md §2` for
//! the Windows probe mapping.
//!
//! Detection is best-effort: a missing tool is information, not a crash;
//! every probe returns the "not present" value (`false`, `None`, `0`)
//! rather than propagating errors.

pub const BACKEND_PORT: u16 = 3010;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DetectionResult {
    /// `docker --version` string when present, `None` otherwise.
    pub docker_present: Option<String>,
    /// Linux: `systemctl --user is-active ollama` exits 0.
    /// Windows: `http://127.0.0.1:11434/api/tags` responds 2xx.
    pub ollama_active: bool,
    /// `docker compose ls` lists a project named `ag`.
    pub compose_up: bool,
    /// Linux: `falkordb.service` is active. Windows: `docker inspect
    /// ag-falkordb` reports healthy.
    pub falkordb_healthy: bool,
    /// `~/.config/ag/ag.env` (Linux) or `%APPDATA%\ag\ag.env` (Windows) exists.
    pub ag_env_exists: bool,
    /// Something is bound on `BACKEND_PORT`. Linux uses `ss -tln`; Windows
    /// uses a `TcpListener::bind` probe.
    pub backend_port_busy: bool,
    /// `redis-cli -p 6379 ping` returns PONG and the listener isn't our
    /// own ag-redis container. Windows uses a raw RESP `*1\r\n$4\r\nPING`
    /// probe (no `redis-cli` by default).
    pub system_redis: bool,
    /// Linux only — active native observability units
    /// (loki / tempo / otelcol). Always `vec![]` on Windows.
    pub native_obs: Vec<String>,
    /// Linux: `~/.config/systemd/user/ag.service` exists but is missing
    /// load-bearing lines from our template (likely hand-edited).
    /// Windows: `schtasks /Query /TN ag /XML` returns a Command element
    /// that doesn't point at `%LOCALAPPDATA%\ag\bin\ag-start.cmd`.
    pub ag_service_drift: bool,
    /// Free space in GB on the install volume. Linux: `df -BG $HOME`.
    /// Windows: `fs2::available_space(parent_of_ag_home) >> 30`.
    pub disk_free_gb: u64,
    /// Total physical RAM in GB. Linux: `/proc/meminfo`. Windows:
    /// `sysinfo::System::new().total_memory() >> 30`.
    pub ram_gb: u64,
    /// Linux: `/etc/os-release` PRETTY_NAME. Windows: registry
    /// `ProductName` + `DisplayVersion`. `None` if unobtainable.
    pub distro: Option<String>,
}

/// Runs every probe; orchestrator body lives in
/// `platform::{linux,windows}::run_detection`. This wrapper keeps the
/// existing `detection::run().await` call shape (used by
/// `ui/detection_screen.rs`) intact across the PR 1 refactor.
pub async fn run() -> DetectionResult {
    crate::platform::run_detection().await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Acceptance harness for the Phase C spec: runs the real probes against
    /// whatever host the test is invoked on and prints the result so the
    /// developer can eyeball it against `install-linux.sh --dry-run` output.
    ///
    /// `#[ignore]` so it never runs in normal CI / pre-commit — the values
    /// are host-specific and meaningless on a clean GitHub runner. Invoke
    /// explicitly with:
    ///     cargo test -p ag-installer --lib -- --ignored --nocapture \
    ///         detection::tests::print_real_result
    #[tokio::test]
    #[ignore]
    async fn print_real_result() {
        let result = run().await;
        println!("\n--- DetectionResult ---");
        println!("docker_present     {:?}", result.docker_present);
        println!("ollama_active      {}", result.ollama_active);
        println!("compose_up         {}", result.compose_up);
        println!("falkordb_healthy   {}", result.falkordb_healthy);
        println!("ag_env_exists      {}", result.ag_env_exists);
        println!("backend_port_busy  {}", result.backend_port_busy);
        println!("system_redis       {}", result.system_redis);
        println!("native_obs         {:?}", result.native_obs);
        println!("ag_service_drift   {}", result.ag_service_drift);
        println!("disk_free_gb       {}", result.disk_free_gb);
        println!("ram_gb             {}", result.ram_gb);
        println!("distro             {:?}", result.distro);

        let rows = crate::app::detection_rows(&result);
        println!("\n--- Detection screen rows ---");
        for row in &rows {
            let mark = match row.status {
                crate::app::DetectionStatus::Ok => '✓',
                crate::app::DetectionStatus::Warn => '⚠',
            };
            println!("  {mark}  {:<22} {}", row.label, row.value);
        }

        let prompts = crate::prompts::required_prompts(&result);
        println!("\n--- Prompts that will fire ---");
        if prompts.is_empty() {
            println!("  (none)");
        } else {
            for id in prompts {
                println!("  - {:?} → default {:?}", id, id.default_choice());
            }
        }
    }
}
