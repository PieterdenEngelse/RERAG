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
    /// `docker --version` (the CLI *client*) when the binary is on PATH,
    /// `None` otherwise. Presence of the CLI does **not** imply a running
    /// engine — see `docker_engine_version`.
    pub docker_present: Option<String>,
    /// `docker version --format {{.Server.Version}}` — the engine/daemon
    /// (`dockerd`) version, set only when the daemon is actually reachable.
    /// `Some` ⇒ a working engine the compose stack can use; `None` with a
    /// `Some(docker_present)` ⇒ the CLI is installed but the daemon isn't
    /// running (e.g. Docker Desktop not started). The compose stack needs
    /// the engine, not just the CLI, so detection probes both.
    pub docker_engine_version: Option<String>,
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
    /// Windows only. `wsl --status` exited 0 → WSL2 feature is enabled.
    /// Gates whether the WSL2 Docker option appears in the DockerMissing
    /// prompt. Always `false` on Linux (field exists on both platforms so
    /// the struct shape stays shared). NOTE: `wsl --status` exits 0 the
    /// instant the feature is staged — even while a reboot is still
    /// pending — so this alone is **not** "usable now"; combine it with
    /// `wsl2_reboot_pending` via [`DetectionResult::wsl2_ready_now`].
    pub wsl2_available: bool,
    /// Windows only. `true` when a Windows servicing reboot is pending — the
    /// state where the WSL2 feature reads as enabled (`wsl --status` exits 0)
    /// but Virtual Machine Platform isn't live until the machine restarts.
    /// `wsl --install` stages exactly such an operation, so right after
    /// enabling WSL2 this is `true` until the reboot. Read from the Component
    /// Based Servicing reboot marker (non-elevated). Always `false` on Linux.
    pub wsl2_reboot_pending: bool,
    /// Windows only. `wsl -d ag-ubuntu -- docker --version` succeeded →
    /// Docker Engine is already installed inside the ag-managed distro.
    pub wsl2_docker_version: Option<String>,
    /// Windows only. The ag-managed WSL2 distro (`ag-ubuntu`) already
    /// exists → `Some("ag-ubuntu")`. Reinstalls reuse it.
    pub wsl2_distro_name: Option<String>,
    /// Windows only. `true` only when virtualization is **off at the
    /// firmware level** and no hypervisor is running — i.e.
    /// `Win32_ComputerSystem.HypervisorPresent` is false AND
    /// `Win32_Processor.VirtualizationFirmwareEnabled` is explicitly false.
    /// In that state `wsl --install` reports success but WSL2 still can't
    /// start after a reboot (error `0x80370102`), so the "enable WSL2" path
    /// would burn a restart for nothing — and Docker Desktop can't run
    /// either. Gates enablement off and drives the "enable VT-x/AMD-V in
    /// BIOS first" guidance. Conservative: any uncertainty (property
    /// unreadable, probe failed, hypervisor already present) leaves this
    /// `false` so we never false-block a machine that's actually fine.
    /// Always `false` on Linux.
    pub virtualization_blocked: bool,
}

impl DetectionResult {
    /// `true` when WSL2 is enabled **and usable right now** — the feature is
    /// on (`wsl2_available`) and no servicing reboot is pending. The installer
    /// offers the no-restart "lightweight" Docker path only in this state;
    /// `wsl2_available && wsl2_reboot_pending` routes to the enable/restart
    /// path instead, because WSL2 can't actually run until the machine reboots
    /// even though `wsl --status` already exits 0. Always `false` on Linux.
    pub fn wsl2_ready_now(&self) -> bool {
        self.wsl2_available && !self.wsl2_reboot_pending
    }
}

/// Runs every probe (orchestrator body lives in
/// `platform::{linux,windows}::run_detection`), sending a `()` on `progress`
/// as each probe completes so the detection screen can advance a progress
/// bar. The number of ticks to expect is
/// [`crate::platform::DETECTION_PROBE_COUNT`]. Pass a throwaway sender when
/// progress isn't needed.
pub async fn run_with_progress(
    progress: tokio::sync::mpsc::UnboundedSender<()>,
) -> DetectionResult {
    crate::platform::run_detection(Some(progress)).await
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
    ///     cargo test -p ag-installer -- --ignored --nocapture \
    ///         detection::tests::print_real_result
    /// (no `--lib`: ag-installer is a binary-only crate, so `--lib` errors
    /// with "no library targets found").
    #[tokio::test]
    #[ignore]
    async fn print_real_result() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let result = run_with_progress(tx).await;
        println!("\n--- DetectionResult ---");
        println!("docker_present     {:?}", result.docker_present);
        println!("docker_engine_version {:?}", result.docker_engine_version);
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
        println!("wsl2_available     {}", result.wsl2_available);
        println!("wsl2_reboot_pending {}", result.wsl2_reboot_pending);
        println!("wsl2_docker_version {:?}", result.wsl2_docker_version);
        println!("wsl2_distro_name   {:?}", result.wsl2_distro_name);
        println!("virtualization_blocked {}", result.virtualization_blocked);

        let rows = crate::app::detection_rows(&result);
        println!("\n--- Detection screen rows ---");
        for row in &rows {
            let mark = match row.status {
                crate::app::DetectionStatus::Ok => '✓',
                crate::app::DetectionStatus::Info => '○',
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
                println!(
                    "  - {:?} → default {:?}",
                    id,
                    id.default_choice(Some(&result))
                );
            }
        }
    }
}
