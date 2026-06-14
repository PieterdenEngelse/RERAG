//! UI screens and shared components for the installer.

pub mod components;
pub mod welcome;
pub mod detection_screen;
pub mod prompts;
pub mod progress;
pub mod first_run_form;
pub mod summary_screen;

pub use welcome::Welcome;
pub use detection_screen::DetectionScreen;
pub use prompts::PromptsScreen;
pub use progress::ProgressScreen;
pub use first_run_form::FirstRunForm;
pub use summary_screen::SummaryScreen;
