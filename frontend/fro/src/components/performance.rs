//! Performance utilities for the frontend
//!
//! This module provides:
//! - Lazy loading helpers
//! - Virtual scrolling components
//! - Debouncing utilities
//! - Memoization helpers

use dioxus::prelude::*;
use std::time::Duration;

/// Configuration for virtual scrolling
#[derive(Clone, Debug)]
pub struct VirtualScrollConfig {
    /// Height of each item in pixels
    pub item_height: f64,
    /// Number of items to render above/below the visible area
    pub overscan: usize,
    /// Total number of items
    pub total_items: usize,
    /// Height of the container in pixels
    pub container_height: f64,
}

impl Default for VirtualScrollConfig {
    fn default() -> Self {
        Self {
            item_height: 40.0,
            overscan: 5,
            total_items: 0,
            container_height: 400.0,
        }
    }
}

/// Calculate which items should be rendered for virtual scrolling
pub fn calculate_visible_range(scroll_top: f64, config: &VirtualScrollConfig) -> (usize, usize) {
    let start_index = (scroll_top / config.item_height).floor() as usize;
    let visible_count = (config.container_height / config.item_height).ceil() as usize;

    let start = start_index.saturating_sub(config.overscan);
    let end = (start_index + visible_count + config.overscan).min(config.total_items);

    (start, end)
}

/// Virtual scroll helper - calculates visible items for a scrollable list
///
/// Usage in your component:
/// ```rust
/// let scroll_top = use_signal(|| 0.0f64);
/// let config = VirtualScrollConfig { total_items: items.len(), ..Default::default() };
/// let (start, end) = calculate_visible_range(*scroll_top.read(), &config);
///
/// rsx! {
///     div {
///         class: "overflow-y-auto",
///         style: "height: 400px;",
///         for index in start..end {
///             div { "Item {index}" }
///         }
///     }
/// }
/// ```
///
/// Note: For full virtual scrolling, you'd need to:
/// 1. Track scroll position via JS interop
/// 2. Apply transform to offset visible items
/// 3. Set total height for proper scrollbar

/// Lazy loading state
#[derive(Clone, Debug, PartialEq)]
pub enum LazyLoadState<T> {
    /// Not yet loaded
    Pending,
    /// Currently loading
    Loading,
    /// Successfully loaded
    Loaded(T),
    /// Failed to load
    Error(String),
}

impl<T> LazyLoadState<T> {
    pub fn is_loading(&self) -> bool {
        matches!(self, LazyLoadState::Loading)
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, LazyLoadState::Loaded(_))
    }

    pub fn data(&self) -> Option<&T> {
        match self {
            LazyLoadState::Loaded(data) => Some(data),
            _ => None,
        }
    }
}

/// Pagination state for lazy loading lists
#[derive(Clone, Debug, Default)]
pub struct PaginationState {
    /// Current page (0-indexed)
    pub page: usize,
    /// Items per page
    pub page_size: usize,
    /// Total items (if known)
    pub total_items: Option<usize>,
    /// Whether there are more items to load
    pub has_more: bool,
}

impl PaginationState {
    pub fn new(page_size: usize) -> Self {
        Self {
            page: 0,
            page_size,
            total_items: None,
            has_more: true,
        }
    }

    pub fn offset(&self) -> usize {
        self.page * self.page_size
    }

    pub fn next_page(&mut self) {
        if self.has_more {
            self.page += 1;
        }
    }

    pub fn total_pages(&self) -> Option<usize> {
        self.total_items
            .map(|total| (total + self.page_size - 1) / self.page_size)
    }
}

/// Debounce helper for search inputs
/// Returns a closure that will only execute after the specified delay
pub fn use_debounce<T: Clone + 'static>(
    delay_ms: u64,
    callback: impl Fn(T) + 'static,
) -> impl Fn(T) {
    // In a real implementation, this would use a timer
    // For now, this is a placeholder that shows the pattern
    move |value: T| {
        callback(value);
    }
}

/// Loading spinner component
#[component]
pub fn LoadingSpinner(size: Option<&'static str>) -> Element {
    let size_class = size.unwrap_or("w-8 h-8");

    rsx! {
        div {
            class: "flex items-center justify-center",
            svg {
                class: "{size_class} animate-spin text-blue-500",
                xmlns: "http://www.w3.org/2000/svg",
                fill: "none",
                view_box: "0 0 24 24",
                circle {
                    class: "opacity-25",
                    cx: "12",
                    cy: "12",
                    r: "10",
                    stroke: "currentColor",
                    stroke_width: "4",
                }
                path {
                    class: "opacity-75",
                    fill: "currentColor",
                    d: "M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z",
                }
            }
        }
    }
}

/// Skeleton loader for content placeholders
#[component]
pub fn SkeletonLoader(lines: Option<usize>, class: Option<&'static str>) -> Element {
    let line_count = lines.unwrap_or(3);
    let extra_class = class.unwrap_or("");

    rsx! {
        div {
            class: "animate-pulse space-y-2 {extra_class}",
            for i in 0..line_count {
                div {
                    class: "h-4 bg-gray-700 rounded",
                    style: if i == line_count - 1 { "width: 75%;" } else { "width: 100%;" },
                }
            }
        }
    }
}

/// Intersection observer placeholder for lazy loading
/// In a real implementation, this would use the Intersection Observer API
#[component]
pub fn LazyLoadTrigger(on_visible: EventHandler<()>) -> Element {
    // This is a placeholder - in production, you'd use JS interop
    // to implement actual intersection observer
    rsx! {
        div {
            class: "h-1 w-full",
            // When this element becomes visible, trigger loading
        }
    }
}
