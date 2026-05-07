//! Cache-line aligned wrapper to prevent false sharing
//!
//! False sharing occurs when multiple threads access different variables
//! that happen to reside on the same CPU cache line (typically 64 bytes).
//! When one thread writes to its variable, it invalidates the entire cache
//! line for all other cores, causing expensive cache coherency traffic.
//!
//! This module provides a `CacheAligned<T>` wrapper that ensures each
//! wrapped value occupies its own cache line.

use std::ops::{Deref, DerefMut};

/// Cache line size on most modern x86_64 and ARM64 processors
pub const CACHE_LINE_SIZE: usize = 64;

/// A wrapper that aligns its contents to a cache line boundary.
///
/// This prevents false sharing when multiple `CacheAligned` values
/// are stored adjacently in memory (e.g., in a struct or array).
///
/// # Example
///
/// ```rust
/// use std::sync::atomic::{AtomicU64, Ordering};
/// use crate::perf::cache_aligned::CacheAligned;
///
/// struct Stats {
///     // Each counter gets its own cache line
///     reads: CacheAligned<AtomicU64>,
///     writes: CacheAligned<AtomicU64>,
/// }
///
/// impl Stats {
///     fn new() -> Self {
///         Self {
///             reads: CacheAligned::new(AtomicU64::new(0)),
///             writes: CacheAligned::new(AtomicU64::new(0)),
///         }
///     }
/// }
/// ```
#[repr(align(64))]
#[derive(Debug)]
pub struct CacheAligned<T> {
    value: T,
}

impl<T> CacheAligned<T> {
    /// Create a new cache-aligned wrapper
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Consume the wrapper and return the inner value
    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T: Default> Default for CacheAligned<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone> Clone for CacheAligned<T> {
    fn clone(&self) -> Self {
        Self::new(self.value.clone())
    }
}

impl<T> Deref for CacheAligned<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for CacheAligned<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> AsRef<T> for CacheAligned<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T> AsMut<T> for CacheAligned<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

// Implement From for easy construction
impl<T> From<T> for CacheAligned<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn test_alignment() {
        // Verify the struct is 64-byte aligned
        assert_eq!(mem::align_of::<CacheAligned<u64>>(), 64);
    }

    #[test]
    fn test_size() {
        // Size should be at least 64 bytes due to alignment
        assert!(mem::size_of::<CacheAligned<u64>>() >= 64);
    }

    #[test]
    fn test_adjacent_no_sharing() {
        // Two adjacent CacheAligned values should not share a cache line
        struct TwoCounters {
            a: CacheAligned<AtomicU64>,
            b: CacheAligned<AtomicU64>,
        }

        let counters = TwoCounters {
            a: CacheAligned::new(AtomicU64::new(0)),
            b: CacheAligned::new(AtomicU64::new(0)),
        };

        // Get addresses
        let addr_a = &*counters.a as *const AtomicU64 as usize;
        let addr_b = &*counters.b as *const AtomicU64 as usize;

        // They should be at least 64 bytes apart
        let distance = addr_a.abs_diff(addr_b);

        assert!(
            distance >= 64,
            "Adjacent CacheAligned values are only {} bytes apart",
            distance
        );
    }

    #[test]
    fn test_deref() {
        let counter = CacheAligned::new(AtomicU64::new(42));
        assert_eq!(counter.load(Ordering::Relaxed), 42);
        counter.store(100, Ordering::Relaxed);
        assert_eq!(counter.load(Ordering::Relaxed), 100);
    }
}
