//! Section-reassembly (Auto → PointerRag) statistics.
//!
//! Counts how often Auto mode chose each route and what came back from
//! section hydration. Modelled on `rig_stats`: atomic counters for
//! lifetime totals, plus a small Mutex-guarded ring buffer of the last
//! 100 Pointer routes so the monitor page can show recent gap /
//! threshold / fallback shape.

use crate::agent::{AutoRoute, PointerHydration};
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const HISTORY_SIZE: usize = 100;

// ── Lifetime atomic counters ──────────────────────────────────────────────────

static AUTO_QUERIES_TOTAL: AtomicUsize = AtomicUsize::new(0);
static AUTO_ROUTE_POINTER: AtomicUsize = AtomicUsize::new(0);
static AUTO_ROUTE_STRICT: AtomicUsize = AtomicUsize::new(0);
static AUTO_ROUTE_HYBRID: AtomicUsize = AtomicUsize::new(0);

static POINTER_SECTIONS_HYDRATED: AtomicUsize = AtomicUsize::new(0);
static POINTER_CHUNKS_IN_TOTAL: AtomicUsize = AtomicUsize::new(0);
static POINTER_FB_NO_SECTION_ID: AtomicUsize = AtomicUsize::new(0);
static POINTER_FB_FETCH_EMPTY: AtomicUsize = AtomicUsize::new(0);
static POINTER_FB_LOCK_FAILED: AtomicUsize = AtomicUsize::new(0);

// ── Ring buffer of recent Pointer routes ──────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct PointerHistoryEntry {
    pub recorded_at_ms: u64,
    pub chunks_in: usize,
    pub sections_hydrated: usize,
    pub fb_no_section_id: usize,
    pub fb_fetch_empty: usize,
    pub fb_lock_failed: usize,
    pub gap: f32,
    pub threshold: f32,
}

impl PointerHistoryEntry {
    fn total_fallbacks(&self) -> usize {
        self.fb_no_section_id + self.fb_fetch_empty + self.fb_lock_failed
    }
}

static HISTORY: Lazy<Mutex<VecDeque<PointerHistoryEntry>>> =
    Lazy::new(|| Mutex::new(VecDeque::with_capacity(HISTORY_SIZE)));

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ── Recorders called from agent.rs ────────────────────────────────────────────

/// Called once per Auto-mode routing decision (i.e. after `auto_route`
/// returns, regardless of which arm wins). Always increments
/// `AUTO_QUERIES_TOTAL` and exactly one of the per-route counters. Gap
/// and threshold for the decision are captured per-Pointer-route by
/// `record_pointer_hydration` instead — keeping them out of this hot
/// path means a Strict/Hybrid route doesn't allocate or lock.
pub fn record_auto_route(route: AutoRoute) {
    AUTO_QUERIES_TOTAL.fetch_add(1, Ordering::Relaxed);
    match route {
        AutoRoute::PointerHydration => &AUTO_ROUTE_POINTER,
        AutoRoute::Strict => &AUTO_ROUTE_STRICT,
        AutoRoute::Hybrid => &AUTO_ROUTE_HYBRID,
    }
    .fetch_add(1, Ordering::Relaxed);
}

/// Called only on the Pointer arm, after `hydrate_pointer_sections`
/// returns. Adds to the lifetime hydration totals and pushes a fresh
/// ring-buffer entry. `record_auto_route` is expected to have already
/// fired for the same query — do NOT increment route counters here.
pub fn record_pointer_hydration(
    chunks_in: usize,
    hydration: &PointerHydration,
    gap: f32,
    threshold: f32,
) {
    POINTER_CHUNKS_IN_TOTAL.fetch_add(chunks_in, Ordering::Relaxed);
    POINTER_SECTIONS_HYDRATED.fetch_add(hydration.hydrated, Ordering::Relaxed);
    POINTER_FB_NO_SECTION_ID.fetch_add(hydration.fb_no_section_id, Ordering::Relaxed);
    POINTER_FB_FETCH_EMPTY.fetch_add(hydration.fb_fetch_empty, Ordering::Relaxed);
    POINTER_FB_LOCK_FAILED.fetch_add(hydration.fb_lock_failed, Ordering::Relaxed);

    let entry = PointerHistoryEntry {
        recorded_at_ms: now_ms(),
        chunks_in,
        sections_hydrated: hydration.hydrated,
        fb_no_section_id: hydration.fb_no_section_id,
        fb_fetch_empty: hydration.fb_fetch_empty,
        fb_lock_failed: hydration.fb_lock_failed,
        gap,
        threshold,
    };

    if let Ok(mut guard) = HISTORY.lock() {
        if guard.len() >= HISTORY_SIZE {
            guard.pop_front();
        }
        guard.push_back(entry);
    }
}

// ── Snapshot ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct PointerStatsSnapshot {
    pub auto_queries_total: usize,
    pub route_pointer_total: usize,
    pub route_strict_total: usize,
    pub route_hybrid_total: usize,
    /// `route_pointer_total / auto_queries_total * 100`, or 0 when no
    /// Auto queries have been recorded yet.
    pub pointer_route_pct: f64,

    pub sections_hydrated_total: usize,
    pub chunks_in_total: usize,
    /// Hydration attempts that produced section text, as a percentage of
    /// all hydration attempts (success + every fallback bucket). 0 when
    /// no attempts have happened yet.
    pub hydration_success_rate_pct: f64,
    pub fb_no_section_id_total: usize,
    pub fb_fetch_empty_total: usize,
    pub fb_lock_failed_total: usize,

    /// Most recent ≤100 Pointer routes, newest first.
    pub recent: Vec<PointerHistoryEntry>,
    /// Mean `gap` over `recent`.
    pub avg_gap: f32,
    /// Mean `threshold` over `recent`. Surfaces threshold churn from
    /// users moving the Pointer-trigger slider.
    pub avg_threshold: f32,
    /// Fraction of `recent` routes that completed with zero fallbacks.
    pub clean_pointer_route_pct: f64,
}

pub fn snapshot() -> PointerStatsSnapshot {
    let auto_queries = AUTO_QUERIES_TOTAL.load(Ordering::Relaxed);
    let pointer = AUTO_ROUTE_POINTER.load(Ordering::Relaxed);
    let strict = AUTO_ROUTE_STRICT.load(Ordering::Relaxed);
    let hybrid = AUTO_ROUTE_HYBRID.load(Ordering::Relaxed);

    let hydrated = POINTER_SECTIONS_HYDRATED.load(Ordering::Relaxed);
    let chunks_in = POINTER_CHUNKS_IN_TOTAL.load(Ordering::Relaxed);
    let fb_no_section = POINTER_FB_NO_SECTION_ID.load(Ordering::Relaxed);
    let fb_fetch_empty = POINTER_FB_FETCH_EMPTY.load(Ordering::Relaxed);
    let fb_lock_failed = POINTER_FB_LOCK_FAILED.load(Ordering::Relaxed);

    let pointer_route_pct = if auto_queries > 0 {
        (pointer as f64 / auto_queries as f64) * 100.0
    } else {
        0.0
    };

    let attempts = hydrated + fb_no_section + fb_fetch_empty + fb_lock_failed;
    let hydration_success_rate_pct = if attempts > 0 {
        (hydrated as f64 / attempts as f64) * 100.0
    } else {
        0.0
    };

    let (recent, avg_gap, avg_threshold, clean_pct) = match HISTORY.lock() {
        Ok(guard) => {
            let n = guard.len();
            if n == 0 {
                (Vec::new(), 0.0_f32, 0.0_f32, 0.0_f64)
            } else {
                let sum_gap: f32 = guard.iter().map(|e| e.gap).sum();
                let sum_thr: f32 = guard.iter().map(|e| e.threshold).sum();
                let clean = guard.iter().filter(|e| e.total_fallbacks() == 0).count();
                let recent: Vec<PointerHistoryEntry> = guard.iter().rev().cloned().collect();
                (
                    recent,
                    sum_gap / n as f32,
                    sum_thr / n as f32,
                    (clean as f64 / n as f64) * 100.0,
                )
            }
        }
        Err(_) => (Vec::new(), 0.0, 0.0, 0.0),
    };

    PointerStatsSnapshot {
        auto_queries_total: auto_queries,
        route_pointer_total: pointer,
        route_strict_total: strict,
        route_hybrid_total: hybrid,
        pointer_route_pct,
        sections_hydrated_total: hydrated,
        chunks_in_total: chunks_in,
        hydration_success_rate_pct,
        fb_no_section_id_total: fb_no_section,
        fb_fetch_empty_total: fb_fetch_empty,
        fb_lock_failed_total: fb_lock_failed,
        recent,
        avg_gap,
        avg_threshold,
        clean_pointer_route_pct: clean_pct,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Counters and the ring buffer are process-globals; tests must run
    /// one at a time so deltas (and history length) are deterministic.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset() {
        AUTO_QUERIES_TOTAL.store(0, Ordering::Relaxed);
        AUTO_ROUTE_POINTER.store(0, Ordering::Relaxed);
        AUTO_ROUTE_STRICT.store(0, Ordering::Relaxed);
        AUTO_ROUTE_HYBRID.store(0, Ordering::Relaxed);
        POINTER_SECTIONS_HYDRATED.store(0, Ordering::Relaxed);
        POINTER_CHUNKS_IN_TOTAL.store(0, Ordering::Relaxed);
        POINTER_FB_NO_SECTION_ID.store(0, Ordering::Relaxed);
        POINTER_FB_FETCH_EMPTY.store(0, Ordering::Relaxed);
        POINTER_FB_LOCK_FAILED.store(0, Ordering::Relaxed);
        if let Ok(mut guard) = HISTORY.lock() {
            guard.clear();
        }
    }

    fn mk_hydration(
        hydrated: usize,
        fb_no_section_id: usize,
        fb_fetch_empty: usize,
        fb_lock_failed: usize,
    ) -> PointerHydration {
        PointerHydration {
            context: String::new(),
            hydrated,
            fb_no_section_id,
            fb_fetch_empty,
            fb_lock_failed,
        }
    }

    #[test]
    fn snapshot_starts_empty() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        let s = snapshot();
        assert_eq!(s.auto_queries_total, 0);
        assert_eq!(s.route_pointer_total, 0);
        assert_eq!(s.route_strict_total, 0);
        assert_eq!(s.route_hybrid_total, 0);
        assert_eq!(s.pointer_route_pct, 0.0);
        assert_eq!(s.sections_hydrated_total, 0);
        assert_eq!(s.hydration_success_rate_pct, 0.0);
        assert!(s.recent.is_empty());
        assert_eq!(s.avg_gap, 0.0);
        assert_eq!(s.clean_pointer_route_pct, 0.0);
    }

    #[test]
    fn route_counts_partition_auto_queries() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        record_auto_route(AutoRoute::PointerHydration);
        record_auto_route(AutoRoute::PointerHydration);
        record_auto_route(AutoRoute::Strict);
        record_auto_route(AutoRoute::Hybrid);
        let s = snapshot();
        assert_eq!(s.auto_queries_total, 4);
        assert_eq!(s.route_pointer_total, 2);
        assert_eq!(s.route_strict_total, 1);
        assert_eq!(s.route_hybrid_total, 1);
        assert!((s.pointer_route_pct - 50.0).abs() < 1e-9);
    }

    #[test]
    fn hydration_records_history_and_aggregates() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        // Two Pointer routes: one fully clean, one with one fetch-empty
        // fallback. record_auto_route is what callers actually pair with
        // record_pointer_hydration, so mirror that here.
        record_auto_route(AutoRoute::PointerHydration);
        record_pointer_hydration(5, &mk_hydration(3, 0, 0, 0), 0.6, 0.5);
        record_auto_route(AutoRoute::PointerHydration);
        record_pointer_hydration(7, &mk_hydration(2, 1, 1, 0), 0.8, 0.5);

        let s = snapshot();
        assert_eq!(s.sections_hydrated_total, 5);
        assert_eq!(s.chunks_in_total, 12);
        assert_eq!(s.fb_no_section_id_total, 1);
        assert_eq!(s.fb_fetch_empty_total, 1);
        assert_eq!(s.fb_lock_failed_total, 0);
        assert_eq!(s.recent.len(), 2);
        // Newest-first ordering.
        assert_eq!(s.recent[0].chunks_in, 7);
        assert_eq!(s.recent[1].chunks_in, 5);
        // avg_gap = (0.6 + 0.8) / 2
        assert!((s.avg_gap - 0.7).abs() < 1e-6);
        assert!((s.avg_threshold - 0.5).abs() < 1e-6);
        // 1 of 2 routes had zero fallbacks.
        assert!((s.clean_pointer_route_pct - 50.0).abs() < 1e-9);
        // hydration_success_rate: 5 hydrated / (5 + 1 + 1 + 0) attempts.
        let expected = 5.0_f64 / 7.0_f64 * 100.0;
        assert!((s.hydration_success_rate_pct - expected).abs() < 1e-9);
    }
}
