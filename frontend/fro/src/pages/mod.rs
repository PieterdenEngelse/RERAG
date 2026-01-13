// src/pages/mod.rs
pub mod about;
pub mod config;
pub mod hardware;
pub mod home;
pub mod memories;
pub mod monitor;
pub mod not_found;
pub mod other;
pub mod parameters;
pub mod prompt;
pub mod sampling;
pub mod train;

// Re-export so they can be used as `pages::Home`
pub use about::About;
pub use config::Config;
pub use hardware::ConfigHardware;
pub use home::Home;
pub use memories::ConfigMemories;
pub use monitor::{
    MonitorAgentic, MonitorCache, MonitorIndex, MonitorLogs, MonitorObservations, MonitorOverview,
    MonitorRag, MonitorRateLimits, MonitorRequests, MonitorTools,
};
pub use not_found::PageNotFound;
pub use other::ConfigOther;
pub use parameters::Parameters;
pub use prompt::ConfigPrompt;
pub use sampling::ConfigSampling;
pub use train::Train;
