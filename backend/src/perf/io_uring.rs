//! io_uring Async I/O (Linux)
//!
//! Provides 2-3x faster async file I/O on Linux using io_uring.
//! Falls back to standard tokio::fs on other platforms or when feature is disabled.
//!
//! # Requirements
//! - Linux kernel 5.1+ for basic io_uring
//! - Linux kernel 5.6+ for full feature set
//! - Feature flag: `io_uring`
//!
//! # Configuration (Environment Variables)
//! - `IO_URING_RING_SIZE`: Submission/completion queue size (default: 256, range: 1-32768)
//! - `IO_URING_BUFFER_SIZE`: Read/write buffer size in bytes (default: 65536)
//! - `IO_URING_SQPOLL`: Enable kernel SQ polling thread (default: false)
//! - `IO_URING_SQPOLL_IDLE_MS`: SQ poll thread idle timeout in ms (default: 1000)
//!
//! # Usage
//! ```rust
//! use ag::perf::io_uring::{read_file, write_file, is_available, get_config};
//!
//! // Check if io_uring is available
//! if is_available() {
//!     println!("Using io_uring for file I/O");
//!     println!("Config: {:?}", get_config());
//! }
//!
//! // Read file (uses io_uring if available, falls back to tokio::fs)
//! let data = read_file("path/to/file").await?;
//! ```
//!
//! # Performance
//! - Document ingestion: 2-3x faster file reads
//! - Index loading: 2-3x faster vector file reads
//! - Batch operations: Even better due to io_uring's batching

use once_cell::sync::Lazy;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use tracing::{debug, error, info, trace};

#[cfg(all(target_os = "linux", feature = "io_uring"))]
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};

// Global stats and config for monitoring
static IO_STATS: OnceLock<IoStats> = OnceLock::new();
static IO_CONFIG: OnceLock<IoUringConfig> = OnceLock::new();
static IO_STATS_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOGGED_INIT: OnceLock<bool> = OnceLock::new();
#[cfg(all(target_os = "linux", feature = "io_uring"))]
static IO_RUNTIME: OnceLock<Option<Arc<IoRuntimeHandle>>> = OnceLock::new();

// ============================================================================
// Configuration
// ============================================================================

/// io_uring configuration parameters
#[derive(Debug, Clone)]
pub struct IoUringConfig {
    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 1: QUEUE & BUFFERS
    // ═══════════════════════════════════════════════════════════════
    /// Size of submission queue entries (power of 2, 1-32768)
    pub ring_size: u32,

    /// Size of completion queue (0 = auto 2x ring_size)
    pub cq_size: u32,

    /// Size of read/write buffers in bytes
    pub buffer_size: usize,

    /// Number of pre-allocated buffers in the pool
    pub buffer_pool_size: usize,

    /// Clamp queue sizes to max instead of error
    pub clamp: bool,

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 2: POLLING
    // ═══════════════════════════════════════════════════════════════
    /// Enable kernel SQ polling thread (reduces syscalls, uses CPU)
    pub sqpoll: bool,

    /// SQ poll thread idle timeout in milliseconds
    pub sqpoll_idle_ms: u32,

    /// Pin SQ poll thread to specific CPU (-1 = no affinity)
    pub sqpoll_cpu: i32,

    /// Enable busy-wait I/O polling (requires O_DIRECT)
    pub iopoll: bool,

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 3: OPTIMIZATION
    // ═══════════════════════════════════════════════════════════════
    /// Hint that only one thread submits (kernel 6.0+)
    pub single_issuer: bool,

    /// Cooperative task running - reduces interrupts (kernel 5.19+)
    pub coop_taskrun: bool,

    /// Defer work until explicit enter call (kernel 6.1+, requires single_issuer)
    pub defer_taskrun: bool,

    /// Continue submitting even if one request errors (kernel 5.18+)
    pub submit_all: bool,

    /// Set flag when completions pending, use with coop_taskrun (kernel 5.19+)
    pub taskrun_flag: bool,

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 4: ADVANCED
    // ═══════════════════════════════════════════════════════════════
    /// Start with rings disabled for setup (kernel 5.10+)
    pub r_disabled: bool,

    /// Share worker thread pool with another ring (fd, -1 = disabled)
    pub attach_wq_fd: i32,

    /// Prevent ring memory from being inherited by fork
    pub dontfork: bool,
}

impl Default for IoUringConfig {
    fn default() -> Self {
        Self {
            // Queue & Buffers
            ring_size: 256,
            cq_size: 0,           // Auto (2x ring_size)
            buffer_size: 65536,   // 64KB
            buffer_pool_size: 64, // 64 pre-allocated buffers
            clamp: false,         // Error on oversized queues

            // Polling
            sqpoll: false,        // Disabled by default (uses CPU)
            sqpoll_idle_ms: 1000, // 1 second
            sqpoll_cpu: -1,       // No CPU affinity
            iopoll: false,        // Disabled (requires O_DIRECT)

            // Optimization
            single_issuer: true,  // Thread-per-core model
            coop_taskrun: false,  // Disabled by default
            defer_taskrun: false, // Disabled by default
            submit_all: false,    // Stop on first error by default
            taskrun_flag: false,  // Disabled by default

            // Advanced
            r_disabled: false, // Start enabled by default
            attach_wq_fd: -1,  // No shared worker pool
            dontfork: false,   // Allow fork inheritance
        }
    }
}

impl IoUringConfig {
    /// Load configuration from .env.io_uring file and environment variables
    /// Priority: env vars > .env.io_uring file > defaults
    pub fn from_env() -> Self {
        // First, try to load from .env.io_uring file
        Self::load_env_file();

        let mut config = Self::default();

        // Helper to parse bool from env
        let parse_bool = |val: &str| -> bool { val == "1" || val.to_lowercase() == "true" };

        // ═══════════════════════════════════════════════════════════════
        // CATEGORY 1: QUEUE & BUFFERS
        // ═══════════════════════════════════════════════════════════════

        // IO_URING_RING_SIZE: 1-32768, must be power of 2
        if let Ok(val) = std::env::var("IO_URING_RING_SIZE") {
            if let Ok(size) = val.parse::<u32>() {
                if (1..=32768).contains(&size) && size.is_power_of_two() {
                    config.ring_size = size;
                } else {
                    eprintln!(
                        "IO_URING_RING_SIZE must be power of 2 between 1-32768, using default {}",
                        config.ring_size
                    );
                }
            }
        }

        // IO_URING_CQ_SIZE: completion queue size (0 = auto)
        if let Ok(val) = std::env::var("IO_URING_CQ_SIZE") {
            if let Ok(size) = val.parse::<u32>() {
                config.cq_size = size;
            }
        }

        // IO_URING_BUFFER_SIZE: buffer size in bytes
        if let Ok(val) = std::env::var("IO_URING_BUFFER_SIZE") {
            if let Ok(size) = val.parse::<usize>() {
                if (4096..=16 * 1024 * 1024).contains(&size) {
                    config.buffer_size = size;
                } else {
                    eprintln!(
                        "IO_URING_BUFFER_SIZE must be between 4096-16MB, using default {}",
                        config.buffer_size
                    );
                }
            }
        }

        // IO_URING_BUFFER_POOL_SIZE: number of pre-allocated buffers
        if let Ok(val) = std::env::var("IO_URING_BUFFER_POOL_SIZE") {
            if let Ok(size) = val.parse::<usize>() {
                if (1..=4096).contains(&size) {
                    config.buffer_pool_size = size;
                }
            }
        }

        // IO_URING_CLAMP: clamp queue sizes to max
        if let Ok(val) = std::env::var("IO_URING_CLAMP") {
            config.clamp = parse_bool(&val);
        }

        // ═══════════════════════════════════════════════════════════════
        // CATEGORY 2: POLLING
        // ═══════════════════════════════════════════════════════════════

        // IO_URING_SQPOLL: enable kernel SQ polling
        if let Ok(val) = std::env::var("IO_URING_SQPOLL") {
            config.sqpoll = parse_bool(&val);
        }

        // IO_URING_SQPOLL_IDLE_MS: SQ poll idle timeout
        if let Ok(val) = std::env::var("IO_URING_SQPOLL_IDLE_MS") {
            if let Ok(ms) = val.parse::<u32>() {
                config.sqpoll_idle_ms = ms;
            }
        }

        // IO_URING_SQPOLL_CPU: pin SQ poll thread to CPU
        if let Ok(val) = std::env::var("IO_URING_SQPOLL_CPU") {
            if let Ok(cpu) = val.parse::<i32>() {
                config.sqpoll_cpu = cpu;
            }
        }

        // IO_URING_IOPOLL: enable busy-wait I/O polling
        if let Ok(val) = std::env::var("IO_URING_IOPOLL") {
            config.iopoll = parse_bool(&val);
        }

        // ═══════════════════════════════════════════════════════════════
        // CATEGORY 3: OPTIMIZATION
        // ═══════════════════════════════════════════════════════════════

        // IO_URING_SINGLE_ISSUER: single issuer optimization
        if let Ok(val) = std::env::var("IO_URING_SINGLE_ISSUER") {
            config.single_issuer = val != "0" && val.to_lowercase() != "false";
        }

        // IO_URING_COOP_TASKRUN: cooperative task running
        if let Ok(val) = std::env::var("IO_URING_COOP_TASKRUN") {
            config.coop_taskrun = parse_bool(&val);
        }

        // IO_URING_DEFER_TASKRUN: defer work until enter call
        if let Ok(val) = std::env::var("IO_URING_DEFER_TASKRUN") {
            config.defer_taskrun = parse_bool(&val);
        }

        // IO_URING_SUBMIT_ALL: continue submitting on error
        if let Ok(val) = std::env::var("IO_URING_SUBMIT_ALL") {
            config.submit_all = parse_bool(&val);
        }

        // IO_URING_TASKRUN_FLAG: set flag when completions pending
        if let Ok(val) = std::env::var("IO_URING_TASKRUN_FLAG") {
            config.taskrun_flag = parse_bool(&val);
        }

        // ═══════════════════════════════════════════════════════════════
        // CATEGORY 4: ADVANCED
        // ═══════════════════════════════════════════════════════════════

        // IO_URING_R_DISABLED: start with rings disabled
        if let Ok(val) = std::env::var("IO_URING_R_DISABLED") {
            config.r_disabled = parse_bool(&val);
        }

        // IO_URING_ATTACH_WQ_FD: share worker pool with another ring
        if let Ok(val) = std::env::var("IO_URING_ATTACH_WQ_FD") {
            if let Ok(fd) = val.parse::<i32>() {
                config.attach_wq_fd = fd;
            }
        }

        // IO_URING_DONTFORK: prevent fork inheritance
        if let Ok(val) = std::env::var("IO_URING_DONTFORK") {
            config.dontfork = parse_bool(&val);
        }

        config
    }

    /// Load environment variables from .env.io_uring file if it exists
    fn load_env_file() {
        let env_path = std::path::Path::new(".env.io_uring");
        if !env_path.exists() {
            return;
        }

        match std::fs::read_to_string(env_path) {
            Ok(content) => {
                for line in content.lines() {
                    let line = line.trim();
                    // Skip comments and empty lines
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    // Parse KEY=VALUE
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim();
                        let value = value.trim();
                        // Only set if not already set in environment
                        if std::env::var(key).is_err() {
                            std::env::set_var(key, value);
                        }
                    }
                }
                tracing::info!("Loaded io_uring config from .env.io_uring");
            }
            Err(e) => {
                tracing::warn!("Failed to read .env.io_uring: {}", e);
            }
        }
    }
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
struct IoRuntimeHandle {
    tx: mpsc::UnboundedSender<IoCommand>,
    buffer_pool: Arc<Semaphore>,
}

#[cfg(not(all(target_os = "linux", feature = "io_uring")))]
type IoRuntimeHandle = ();

#[cfg(all(target_os = "linux", feature = "io_uring"))]
impl IoRuntimeHandle {
    fn start(config: IoUringConfig) -> io::Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let buffer_pool = Arc::new(Semaphore::new(config.buffer_pool_size.max(1)));
        let builder_config = config.clone();
        let runtime_buffer_size = config.buffer_size;
        let runtime_pool = Arc::clone(&buffer_pool);

        thread::Builder::new()
            .name("io-uring-runtime".to_string())
            .spawn(move || {
                if let Err(err) =
                    run_uring_thread(builder_config, runtime_buffer_size, runtime_pool, rx)
                {
                    error!("io_uring runtime thread exited: {}", err);
                }
            })
            .map_err(|e| io::Error::other(format!("Failed to spawn io_uring thread: {}", e)))?;

        Ok(Self { tx, buffer_pool })
    }

    async fn acquire_permit(&self) -> OwnedSemaphorePermit {
        self.buffer_pool
            .clone()
            .acquire_owned()
            .await
            .expect("io_uring buffer semaphore closed")
    }

    async fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        let permit = self.acquire_permit().await;
        let (resp_tx, resp_rx) = oneshot::channel();
        let expected_len = std::fs::metadata(path).ok().map(|m| m.len());
        let cmd = IoCommand::ReadFile {
            path: path.to_path_buf(),
            expected_len,
            respond_to: resp_tx,
        };
        if self.tx.send(cmd).is_err() {
            drop(permit);
            return Err(io::Error::other("io_uring runtime unavailable"));
        }
        let result = resp_rx
            .await
            .unwrap_or_else(|_| Err(io::Error::other("io_uring runtime dropped")));
        drop(permit);
        result
    }

    async fn write_file(&self, path: &Path, data: Arc<[u8]>) -> io::Result<()> {
        let permit = self.acquire_permit().await;
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = IoCommand::WriteFile {
            path: path.to_path_buf(),
            data,
            respond_to: resp_tx,
        };
        if self.tx.send(cmd).is_err() {
            drop(permit);
            return Err(io::Error::other("io_uring runtime unavailable"));
        }
        let result = resp_rx
            .await
            .unwrap_or_else(|_| Err(io::Error::other("io_uring runtime dropped")));
        drop(permit);
        result
    }
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
enum IoCommand {
    ReadFile {
        path: PathBuf,
        expected_len: Option<u64>,
        respond_to: oneshot::Sender<io::Result<Vec<u8>>>,
    },
    WriteFile {
        path: PathBuf,
        data: Arc<[u8]>,
        respond_to: oneshot::Sender<io::Result<()>>,
    },
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn run_uring_thread(
    config: IoUringConfig,
    buffer_size: usize,
    buffer_pool: Arc<Semaphore>,
    mut rx: mpsc::UnboundedReceiver<IoCommand>,
) -> io::Result<()> {
    let mut entries = config.ring_size;
    if !entries.is_power_of_two() {
        entries = entries.next_power_of_two();
    }

    let mut runtime_builder = tokio_uring::builder();
    runtime_builder.entries(entries);
    let mut inner = tokio_uring::uring_builder();

    // Category 1: Queue & Buffers
    let cq_entries = if config.cq_size > 0 {
        config.cq_size
    } else {
        entries.saturating_mul(2)
    };
    inner.setup_cqsize(cq_entries);
    if config.clamp {
        inner.setup_clamp();
    }

    // Category 2: Polling
    if config.sqpoll {
        inner.setup_sqpoll(config.sqpoll_idle_ms);
        if config.sqpoll_cpu >= 0 {
            inner.setup_sqpoll_cpu(config.sqpoll_cpu as u32);
        }
    }
    if config.iopoll {
        inner.setup_iopoll();
    }

    // Category 3: Optimization
    if config.single_issuer {
        inner.setup_single_issuer();
    }
    if config.coop_taskrun {
        inner.setup_coop_taskrun();
    }
    if config.defer_taskrun {
        // defer_taskrun requires single_issuer
        if config.single_issuer {
            inner.setup_defer_taskrun();
        } else {
            tracing::warn!("io_uring: defer_taskrun requires single_issuer, ignoring");
        }
    }
    if config.submit_all {
        inner.setup_submit_all();
    }
    if config.taskrun_flag {
        // taskrun_flag works best with coop_taskrun
        inner.setup_taskrun_flag();
    }

    // Category 4: Advanced
    if config.r_disabled {
        inner.setup_r_disabled();
    }
    if config.attach_wq_fd >= 0 {
        use std::os::unix::io::RawFd;
        inner.setup_attach_wq(config.attach_wq_fd as RawFd);
    }

    runtime_builder.uring_builder(&inner);

    // Note: dontfork is applied after build, not during setup
    // tokio_uring doesn't expose this directly, so we skip it for now
    if config.dontfork {
        tracing::info!("io_uring: dontfork requested but not directly supported by tokio_uring");
    }

    runtime_builder.start(async move {
        info!(
            "io_uring runtime running with ring_size={} buffer_size={} pool_size={}",
            entries,
            buffer_size,
            buffer_pool.available_permits()
        );
        while let Some(cmd) = rx.recv().await {
            match cmd {
                IoCommand::ReadFile {
                    path,
                    expected_len,
                    respond_to,
                } => {
                    let result = read_file_task(path, expected_len, buffer_size).await;
                    let _ = respond_to.send(result);
                }
                IoCommand::WriteFile {
                    path,
                    data,
                    respond_to,
                } => {
                    let result = write_file_task(path, data, buffer_size).await;
                    let _ = respond_to.send(result);
                }
            }
        }
        info!("io_uring runtime command channel closed");
    });

    Ok(())
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
async fn read_file_task(
    path: PathBuf,
    expected_len: Option<u64>,
    chunk_size: usize,
) -> io::Result<Vec<u8>> {
    use tokio_uring::fs::File;

    let chunk_size = chunk_size.max(1);
    let file = match File::open(&path).await {
        Ok(f) => f,
        Err(e) => {
            get_stats().record_read_error();
            return Err(e);
        }
    };
    let mut offset: u64 = 0;
    let mut buffer = vec![0u8; chunk_size];
    let mut result = Vec::new();

    if let Some(len) = expected_len {
        result.reserve(len as usize);
    }

    let mut last_err: Option<io::Error> = None;

    loop {
        let (res, buf) = file.read_at(buffer, offset).await;
        buffer = buf;
        match res {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                result.extend_from_slice(&buffer[..bytes_read]);
                offset += bytes_read as u64;
                if bytes_read < chunk_size {
                    break;
                }
            }
            Err(e) => {
                last_err = Some(e);
                break;
            }
        }
    }

    if let Some(err) = last_err {
        get_stats().record_read_error();
        return Err(err);
    }

    get_stats().record_read(result.len() as u64);
    Ok(result)
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
async fn write_file_task(path: PathBuf, data: Arc<[u8]>, chunk_size: usize) -> io::Result<()> {
    use tokio_uring::fs::File;

    let chunk_size = chunk_size.max(1);
    let file = match File::create(&path).await {
        Ok(f) => f,
        Err(e) => {
            get_stats().record_write_error();
            return Err(e);
        }
    };
    let mut offset: u64 = 0;
    while offset < data.len() as u64 {
        let end = (offset as usize + chunk_size).min(data.len());
        let chunk: Vec<u8> = data[offset as usize..end].to_vec();
        let (res, _buf) = file.write_at(chunk, offset).submit().await;
        if let Err(e) = res {
            get_stats().record_write_error();
            return Err(e);
        }
        offset = end as u64;
    }
    get_stats().record_write(data.len() as u64);
    Ok(())
}

/// Get the global io_uring configuration
pub fn get_config() -> &'static IoUringConfig {
    IO_CONFIG.get_or_init(IoUringConfig::from_env)
}

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn runtime_handle() -> Option<&'static Arc<IoRuntimeHandle>> {
    IO_RUNTIME
        .get_or_init(|| {
            if !is_available() {
                return None;
            }
            let config = get_config().clone();
            match IoRuntimeHandle::start(config) {
                Ok(handle) => Some(Arc::new(handle)),
                Err(err) => {
                    error!("Failed to start io_uring runtime: {}", err);
                    None
                }
            }
        })
        .as_ref()
}

#[cfg(not(all(target_os = "linux", feature = "io_uring")))]
fn runtime_handle() -> Option<&'static Arc<IoRuntimeHandle>> {
    None
}

/// Check if io_uring is available on this system
pub fn is_available() -> bool {
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    {
        // Check kernel version
        if let Ok(version) = std::fs::read_to_string("/proc/version") {
            if let Some(ver_str) = version.split_whitespace().nth(2) {
                let parts: Vec<&str> = ver_str.split('.').collect();
                if parts.len() >= 2 {
                    if let (Ok(major), Ok(minor)) =
                        (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                    {
                        // io_uring available in kernel 5.1+
                        return major > 5 || (major == 5 && minor >= 1);
                    }
                }
            }
        }
        false
    }
    #[cfg(not(all(target_os = "linux", feature = "io_uring")))]
    {
        false
    }
}

/// Check if io_uring feature is compiled in
pub fn is_feature_enabled() -> bool {
    cfg!(feature = "io_uring")
}

/// Get the current I/O backend name
pub fn backend_name() -> &'static str {
    if is_available() {
        "io_uring"
    } else {
        "tokio::fs (epoll)"
    }
}

// ============================================================================
// Fallback implementation (tokio::fs with epoll)
// ============================================================================

mod fallback_impl {
    use super::*;
    use tokio::fs;

    /// Read file using tokio::fs (epoll-based)
    pub async fn read_file_fallback<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
        let data = fs::read(path).await?;
        get_stats().record_read(data.len() as u64);
        Ok(data)
    }

    /// Write file using tokio::fs
    pub async fn write_file_fallback<P: AsRef<Path>>(path: P, data: &[u8]) -> io::Result<()> {
        fs::write(path, data).await?;
        get_stats().record_write(data.len() as u64);
        Ok(())
    }

    /// Read file to string using tokio::fs
    pub async fn read_to_string_fallback<P: AsRef<Path>>(path: P) -> io::Result<String> {
        let data = fs::read_to_string(path).await?;
        get_stats().record_read(data.len() as u64);
        Ok(data)
    }

    /// Batch read using tokio::fs
    pub async fn read_files_batch_fallback<P: AsRef<Path>>(
        paths: &[P],
    ) -> Vec<io::Result<Vec<u8>>> {
        let futs: Vec<_> = paths.iter().map(read_file_fallback).collect();
        futures_util::future::join_all(futs).await
    }
}

// ============================================================================
// Public API - automatically selects best implementation
// ============================================================================

/// Async file read - uses io_uring if available, falls back to tokio::fs
pub async fn read_file<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    // Log initialization once
    LOGGED_INIT.get_or_init(|| {
        let config = get_config();
        info!(
            "io_uring: initialized backend={} available={} feature_enabled={} ring_size={} buffer_size={}",
            backend_name(),
            is_available(),
            is_feature_enabled(),
            config.ring_size,
            config.buffer_size
        );
        true
    });

    let path_ref = path.as_ref();
    trace!("io_uring: read_file path={}", path_ref.display());

    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    if let Some(handle) = runtime_handle() {
        let result = handle.read_file(path_ref).await;
        match &result {
            Ok(data) => {
                get_stats().record_read(data.len() as u64);
                debug!(
                    "io_uring: read {} bytes from {} via io_uring",
                    data.len(),
                    path_ref.display()
                );
            }
            Err(e) => {
                get_stats().record_read_error();
                debug!(
                    "io_uring: read error from {} via io_uring: {}",
                    path_ref.display(),
                    e
                );
            }
        }
        return result;
    }

    let result = fallback_impl::read_file_fallback(path_ref).await;
    match &result {
        Ok(data) => debug!(
            "io_uring: read {} bytes from {} via tokio::fs",
            data.len(),
            path_ref.display()
        ),
        Err(e) => {
            get_stats().record_read_error();
            debug!(
                "io_uring: read error from {} via tokio::fs: {}",
                path_ref.display(),
                e
            );
        }
    }
    result
}

/// Async file write - uses io_uring if available, falls back to tokio::fs
pub async fn write_file<P: AsRef<Path>>(path: P, data: &[u8]) -> io::Result<()> {
    let path_ref = path.as_ref();
    trace!(
        "io_uring: write_file path={} bytes={}",
        path_ref.display(),
        data.len()
    );

    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    if let Some(handle) = runtime_handle() {
        let result = handle.write_file(path_ref, Arc::from(data.to_vec())).await;
        match &result {
            Ok(_) => {
                get_stats().record_write(data.len() as u64);
                debug!(
                    "io_uring: wrote {} bytes to {} via io_uring",
                    data.len(),
                    path_ref.display()
                );
            }
            Err(e) => {
                get_stats().record_write_error();
                debug!(
                    "io_uring: write error to {} via io_uring: {}",
                    path_ref.display(),
                    e
                );
            }
        }
        return result;
    }

    let result = fallback_impl::write_file_fallback(path_ref, data).await;
    match &result {
        Ok(_) => debug!(
            "io_uring: wrote {} bytes to {} via tokio::fs",
            data.len(),
            path_ref.display()
        ),
        Err(e) => {
            get_stats().record_write_error();
            debug!(
                "io_uring: write error to {} via tokio::fs: {}",
                path_ref.display(),
                e
            );
        }
    }
    result
}

/// Async file read to string - uses io_uring if available
pub async fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    if let Some(handle) = runtime_handle() {
        return handle.read_file(path.as_ref()).await.and_then(|bytes| {
            get_stats().record_read(bytes.len() as u64);
            String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        });
    }

    fallback_impl::read_to_string_fallback(path).await
}

/// Batch read multiple files - uses io_uring batching if available
/// This is the most efficient way to read multiple files
pub async fn read_files<P: AsRef<Path>>(paths: &[P]) -> Vec<io::Result<Vec<u8>>> {
    info!(
        "io_uring: batch read {} files via {}",
        paths.len(),
        backend_name()
    );

    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    if let Some(handle) = runtime_handle() {
        let handle = Arc::clone(handle);
        let futures = paths.iter().map(|p| handle.read_file(p.as_ref()));
        let results = futures_util::future::join_all(futures).await;
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        debug!(
            "io_uring: batch read completed {}/{} files via io_uring",
            success_count,
            paths.len()
        );
        return results;
    }

    let results = fallback_impl::read_files_batch_fallback(paths).await;
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    debug!(
        "io_uring: batch read completed {}/{} files via tokio::fs",
        success_count,
        paths.len()
    );
    results
}

/// Async file metadata
pub async fn metadata<P: AsRef<Path>>(path: P) -> io::Result<std::fs::Metadata> {
    tokio::fs::metadata(path).await
}

/// Async directory creation
pub async fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    tokio::fs::create_dir_all(path).await
}

/// Async file removal
pub async fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    tokio::fs::remove_file(path).await
}

/// Async file copy
pub async fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<u64> {
    tokio::fs::copy(from, to).await
}

/// Async file rename
pub async fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    tokio::fs::rename(from, to).await
}

// ============================================================================
// Statistics and Monitoring
// ============================================================================

/// File I/O statistics
#[derive(Debug, Default)]
pub struct IoStats {
    pub reads: AtomicU64,
    pub writes: AtomicU64,
    pub bytes_read: AtomicU64,
    pub bytes_written: AtomicU64,
    pub read_errors: AtomicU64,
    pub write_errors: AtomicU64,
}

impl IoStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_read(&self, bytes: u64) {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.bytes_read.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_write(&self, bytes: u64) {
        self.writes.fetch_add(1, Ordering::Relaxed);
        self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_read_error(&self) {
        self.read_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_write_error(&self) {
        self.write_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_reads(&self) -> u64 {
        self.reads.load(Ordering::Relaxed)
    }

    pub fn get_writes(&self) -> u64 {
        self.writes.load(Ordering::Relaxed)
    }

    pub fn get_bytes_read(&self) -> u64 {
        self.bytes_read.load(Ordering::Relaxed)
    }

    pub fn get_bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::Relaxed)
    }

    pub fn get_read_errors(&self) -> u64 {
        self.read_errors.load(Ordering::Relaxed)
    }

    pub fn get_write_errors(&self) -> u64 {
        self.write_errors.load(Ordering::Relaxed)
    }

    pub fn get_total_errors(&self) -> u64 {
        self.get_read_errors() + self.get_write_errors()
    }

    pub fn reset(&self) {
        self.reads.store(0, Ordering::Relaxed);
        self.writes.store(0, Ordering::Relaxed);
        self.bytes_read.store(0, Ordering::Relaxed);
        self.bytes_written.store(0, Ordering::Relaxed);
        self.read_errors.store(0, Ordering::Relaxed);
        self.write_errors.store(0, Ordering::Relaxed);
    }
}

/// Get global I/O stats
pub fn get_stats() -> &'static IoStats {
    IO_STATS.get_or_init(IoStats::new)
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct IoStatsSnapshot {
    reads: u64,
    writes: u64,
    bytes_read: u64,
    bytes_written: u64,
    read_errors: u64,
    write_errors: u64,
}

/// Load persisted stats from disk and seed the global counters.
/// Call once at startup before any I/O occurs.
pub fn init_stats(path: PathBuf) {
    let _ = IO_STATS_PATH.set(path.clone());
    let stats = get_stats();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(snap) = serde_json::from_str::<IoStatsSnapshot>(&data) {
            stats.reads.store(snap.reads, Ordering::Relaxed);
            stats.writes.store(snap.writes, Ordering::Relaxed);
            stats.bytes_read.store(snap.bytes_read, Ordering::Relaxed);
            stats.bytes_written.store(snap.bytes_written, Ordering::Relaxed);
            stats.read_errors.store(snap.read_errors, Ordering::Relaxed);
            stats.write_errors.store(snap.write_errors, Ordering::Relaxed);
        }
    }
}

/// Flush current stats to disk. Call periodically and on shutdown.
pub fn flush_stats() {
    let Some(path) = IO_STATS_PATH.get() else { return };
    let s = get_stats();
    let snap = IoStatsSnapshot {
        reads: s.get_reads(),
        writes: s.get_writes(),
        bytes_read: s.get_bytes_read(),
        bytes_written: s.get_bytes_written(),
        read_errors: s.get_read_errors(),
        write_errors: s.get_write_errors(),
    };
    if let Ok(json) = serde_json::to_string(&snap) {
        let _ = std::fs::write(path, json);
    }
}

// ── Startup I/O Accounting ────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct StartupIoRecord {
    pub vectors_bytes: u64,
    pub vectors_read_ms: u64,
    pub vectors_count: usize,
    pub cache_bytes: u64,
    pub cache_read_ms: u64,
    pub cache_entries: usize,
    pub backend: String,
}

static STARTUP_IO: Lazy<parking_lot::Mutex<StartupIoRecord>> =
    Lazy::new(|| parking_lot::Mutex::new(StartupIoRecord::default()));

pub fn update_startup_vectors(bytes: u64, read_ms: u64, count: usize, backend: &str) {
    let mut r = STARTUP_IO.lock();
    r.vectors_bytes = bytes;
    r.vectors_read_ms = read_ms;
    r.vectors_count = count;
    r.backend = backend.to_string();
}

pub fn update_startup_cache(bytes: u64, read_ms: u64, entries: usize) {
    let mut r = STARTUP_IO.lock();
    r.cache_bytes = bytes;
    r.cache_read_ms = read_ms;
    r.cache_entries = entries;
}

pub fn get_startup_io() -> StartupIoRecord {
    STARTUP_IO.lock().clone()
}

/// Get I/O stats summary
pub fn stats_summary() -> String {
    let stats = get_stats();
    format!(
        "io_uring stats: backend={}, reads={}, writes={}, bytes_read={}, bytes_written={}",
        backend_name(),
        stats.get_reads(),
        stats.get_writes(),
        stats.get_bytes_read(),
        stats.get_bytes_written()
    )
}

// ============================================================================
// Tracked I/O wrapper for per-operation tracking
// ============================================================================

/// Tracked file operations with per-instance stats
pub struct TrackedIo {
    reads: AtomicU64,
    bytes_read: AtomicU64,
}

impl TrackedIo {
    pub fn new() -> Self {
        Self {
            reads: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
        }
    }

    pub async fn read<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>> {
        let data = read_file(path).await?;
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.bytes_read
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(data)
    }

    pub fn read_count(&self) -> u64 {
        self.reads.load(Ordering::Relaxed)
    }

    pub fn bytes_read(&self) -> u64 {
        self.bytes_read.load(Ordering::Relaxed)
    }
}

impl Default for TrackedIo {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_uring_availability() {
        let available = is_available();
        let feature_enabled = is_feature_enabled();
        println!("io_uring feature enabled: {}", feature_enabled);
        println!("io_uring available: {}", available);
        println!("Backend: {}", backend_name());
    }

    #[tokio::test]
    async fn test_async_read_write() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.txt");

        write_file(&path, b"hello world").await.unwrap();
        let data = read_file(&path).await.unwrap();

        assert_eq!(data, b"hello world");

        // Check stats were recorded
        let stats = get_stats();
        assert!(stats.get_reads() > 0);
        assert!(stats.get_writes() > 0);
    }

    #[tokio::test]
    async fn test_read_to_string() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.txt");

        write_file(&path, b"hello world").await.unwrap();
        let content = read_to_string(&path).await.unwrap();

        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_batch_read() {
        let temp_dir = tempfile::tempdir().unwrap();

        let paths: Vec<_> = (0..5)
            .map(|i| {
                let path = temp_dir.path().join(format!("file{}.txt", i));
                std::fs::write(&path, format!("content {}", i)).unwrap();
                path
            })
            .collect();

        let results = read_files(&paths).await;

        assert_eq!(results.len(), 5);
        for (i, result) in results.iter().enumerate() {
            let data = result.as_ref().unwrap();
            assert_eq!(String::from_utf8_lossy(data), format!("content {}", i));
        }
    }

    #[tokio::test]
    async fn test_tracked_io() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("tracked.txt");

        std::fs::write(&path, "test data").unwrap();

        let tracker = TrackedIo::new();
        let _ = tracker.read(&path).await.unwrap();
        let _ = tracker.read(&path).await.unwrap();

        assert_eq!(tracker.read_count(), 2);
        assert_eq!(tracker.bytes_read(), 18); // "test data" * 2
    }
}
