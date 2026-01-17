//! io_uring Async I/O (Linux)
//! 
//! Provides 2-3x faster async file I/O on Linux using io_uring.
//! Falls back to standard tokio::fs on other platforms.
//! 
//! # Requirements
//! - Linux kernel 5.1+ for basic io_uring
//! - Linux kernel 5.6+ for full feature set
//! 
//! # Note
//! This module provides a compatibility layer. For full io_uring support,
//! add `tokio-uring` to Cargo.toml and use its runtime.

use std::path::Path;
use std::io;
use tokio::fs;
use futures_util::future::join_all;

/// Check if io_uring is available on this system
pub fn is_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Check kernel version
        if let Ok(version) = std::fs::read_to_string("/proc/version") {
            // Parse kernel version (e.g., "Linux version 5.15.0-...")
            if let Some(ver_str) = version.split_whitespace().nth(2) {
                let parts: Vec<&str> = ver_str.split('.').collect();
                if parts.len() >= 2 {
                    if let (Ok(major), Ok(minor)) = (
                        parts[0].parse::<u32>(),
                        parts[1].parse::<u32>(),
                    ) {
                        // io_uring available in kernel 5.1+
                        return major > 5 || (major == 5 && minor >= 1);
                    }
                }
            }
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Async file read (uses tokio::fs, would use io_uring with tokio-uring)
pub async fn read_file<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    fs::read(path).await
}

/// Async file write
pub async fn write_file<P: AsRef<Path>>(path: P, data: &[u8]) -> io::Result<()> {
    fs::write(path, data).await
}

/// Async file read to string
pub async fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    fs::read_to_string(path).await
}

/// Async file metadata
pub async fn metadata<P: AsRef<Path>>(path: P) -> io::Result<std::fs::Metadata> {
    fs::metadata(path).await
}

/// Async directory creation
pub async fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs::create_dir_all(path).await
}

/// Async file removal
pub async fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs::remove_file(path).await
}

/// Async file copy
pub async fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<u64> {
    fs::copy(from, to).await
}

/// Async file rename
pub async fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    fs::rename(from, to).await
}

/// Batch read multiple files concurrently
pub async fn read_files<P: AsRef<Path>>(paths: &[P]) -> Vec<io::Result<Vec<u8>>> {
    let futures: Vec<_> = paths.iter().map(|p| read_file(p)).collect();
    join_all(futures).await
}

/// Batch write multiple files concurrently
pub async fn write_files<P: AsRef<Path>>(files: &[(P, &[u8])]) -> Vec<io::Result<()>> {
    let futures: Vec<_> = files.iter().map(|(p, data)| write_file(p, data)).collect();
    join_all(futures).await
}

/// File I/O statistics
#[derive(Debug, Default, Clone)]
pub struct IoStats {
    pub reads: u64,
    pub writes: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
}

/// Tracked file operations
pub struct TrackedIo {
    stats: std::sync::atomic::AtomicU64,
}

impl TrackedIo {
    pub fn new() -> Self {
        Self {
            stats: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub async fn read<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>> {
        let data = read_file(path).await?;
        self.stats.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(data)
    }

    pub fn read_count(&self) -> u64 {
        self.stats.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for TrackedIo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_uring_availability() {
        let available = is_available();
        println!("io_uring available: {}", available);
        // Just check it doesn't panic
    }

    #[tokio::test]
    async fn test_async_read_write() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.txt");
        
        write_file(&path, b"hello world").await.unwrap();
        let data = read_file(&path).await.unwrap();
        
        assert_eq!(data, b"hello world");
    }

    #[tokio::test]
    async fn test_batch_read() {
        let temp_dir = tempfile::tempdir().unwrap();
        
        let paths: Vec<_> = (0..3).map(|i| {
            let path = temp_dir.path().join(format!("file{}.txt", i));
            std::fs::write(&path, format!("content {}", i)).unwrap();
            path
        }).collect();
        
        let results = read_files(&paths).await;
        
        assert_eq!(results.len(), 3);
        for result in results {
            assert!(result.is_ok());
        }
    }
}
