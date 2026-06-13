//! Shared UI primitives used across screens.

pub mod nav_footer;
pub mod step_list;
pub mod log_view;
pub mod prompt_radio;
pub mod status_icon;

pub use nav_footer::NavFooter;
pub use step_list::StepListView;
pub use log_view::LogView;
pub use prompt_radio::PromptRadio;
pub use status_icon::{StatusIcon, IconKind};
