//! Install path resolution.
//!
//! All install destinations derive from `$HOME` (matching
//! `installers/install-linux.sh`'s scheme: `$HOME/.local/bin`,
//! `$HOME/.local/lib`, `$HOME/.config/ag`, …). Sandbox testing on this
//! box uses `HOME=/tmp/ag-test cargo run -p ag-installer`, which
//! redirects every path here without touching the real ag install.
//!
//! `AG_HOME` is the only env-var override the bash installer exposes;
//! we honor it here too so `AG_HOME=/somewhere cargo run` still works.
//!
//! `SKIP_SYSTEMCTL=1` is *not* a path override — it gates the systemctl
//! shellouts in install_steps. Documented here because the sandbox
//! recipe needs it set alongside `HOME`.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Paths {
    /// `$AG_HOME` or `$HOME/.local/share/ag`. Holds runtime state: data/,
    /// index/, db/, logs/, web/, falkordb/, falkordb/data/.
    pub ag_home: PathBuf,
    /// `$HOME/.local/bin`. `ag` binary lands here.
    pub bin_dir: PathBuf,
    /// `$HOME/.local/lib`. `libtika_native.so` lands here.
    pub lib_dir: PathBuf,
    /// `$HOME/.config/ag`. `ag.env`, `docker-compose.yml` live here.
    pub config_dir: PathBuf,
    /// `$HOME/.config/systemd/user`. The three rendered .service files
    /// and the ag.service.d/ drop-in dir live here.
    pub systemd_user_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        let ag_home = std::env::var("AG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".local/share/ag"));
        Paths {
            bin_dir: home.join(".local/bin"),
            lib_dir: home.join(".local/lib"),
            config_dir: home.join(".config/ag"),
            systemd_user_dir: home.join(".config/systemd/user"),
            ag_home,
        }
    }

    pub fn ag_env(&self) -> PathBuf {
        self.config_dir.join("ag.env")
    }

    pub fn docker_compose(&self) -> PathBuf {
        self.config_dir.join("docker-compose.yml")
    }

    pub fn ag_service(&self) -> PathBuf {
        self.systemd_user_dir.join("ag.service")
    }

    pub fn ag_stack_service(&self) -> PathBuf {
        self.systemd_user_dir.join("ag-stack.service")
    }

    pub fn falkordb_service(&self) -> PathBuf {
        self.systemd_user_dir.join("falkordb.service")
    }

    pub fn ag_service_drop_in_dir(&self) -> PathBuf {
        self.systemd_user_dir.join("ag.service.d")
    }

    pub fn install_log(&self, timestamp_utc: &str) -> PathBuf {
        self.ag_home.join("logs").join(format!("install-{timestamp_utc}.log"))
    }
}

/// True when `SKIP_SYSTEMCTL` is set (any non-empty value). Sandbox tests
/// set this so the `systemctl --user` shellouts log what they would do
/// instead of touching the real user systemd.
pub fn skip_systemctl() -> bool {
    std::env::var("SKIP_SYSTEMCTL")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}
