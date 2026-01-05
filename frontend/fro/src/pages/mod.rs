// src/pages/mod.rs
pub mod about;
pub mod config;
pub mod hardware;
pub mod home;
pub mod monitor;
pub mod not_found;
pub mod other;
pub mod parameters;
pub mod prompt;
pub mod sampling;

// Re-export so they can be used as `pages::Home`
pub use about::About;
pub use config::Config;
pub use hardware::ConfigHardware;
pub use home::Home;
pub use monitor::{
    MonitorAgentic, MonitorCache, MonitorIndex, MonitorLogs, MonitorOverview, MonitorRateLimits,
    MonitorRequests,
};
pub use not_found::PageNotFound;
pub use other::ConfigOther;
pub use parameters::Parameters;
pub use prompt::ConfigPrompt;
pub use sampling::ConfigSampling;
