//! UI screens and shared components for the installer.

pub mod components;
pub mod detection_screen;
pub mod first_run_form;
pub mod progress;
pub mod prompts;
pub mod shell_open;
pub mod summary_screen;
pub mod welcome;

pub use detection_screen::DetectionScreen;
pub use first_run_form::FirstRunForm;
pub use progress::ProgressScreen;
pub use prompts::PromptsScreen;
pub use summary_screen::SummaryScreen;
pub use welcome::Welcome;
