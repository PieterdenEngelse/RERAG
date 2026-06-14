//! The components module contains all shared components for our app. Components are the building blocks of dioxus apps.
//! They can be used to defined common UI elements like buttons, forms, and modals. In this template, we define a Hero
//! component and an Echo component for fullstack apps to be used in our app.

pub mod config_nav;
pub mod header;
pub use header::Header;
mod nav_dropdown;
pub use nav_dropdown::{ActiveDropdown, DropdownItem, NavDropdown};
pub mod search;
pub use search::SearchBar;

pub mod monitor;
pub use monitor::*;

pub mod backend_selector;
pub use backend_selector::BackendSelector;

pub mod performance;
pub use performance::{
    LazyLoadState, LoadingSpinner, PaginationState, SkeletonLoader, VirtualScrollConfig,
};

pub mod embedding_toggle;
pub use embedding_toggle::EmbeddingToggle;

pub mod global_error_bar;
pub use global_error_bar::GlobalErrorBar;

pub mod home_settings_boards;
pub use home_settings_boards::HomeSettingsBoards;
