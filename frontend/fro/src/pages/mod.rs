// src/pages/mod.rs
pub mod about;
pub mod config;
pub mod config_io_uring;
pub mod docu;
pub mod docu_index;
pub mod hardware;
pub mod home;
pub mod memories;
pub mod monitor;
pub mod neo4j;
pub mod not_found;
pub mod onnx;
pub mod onnx_help;
pub mod other;
pub mod parameters;
pub mod prompt;
pub mod sampling;
pub mod train;

// Re-export so they can be used as `pages::Home`
pub use about::About;
pub use config::Config;
pub use config_io_uring::ConfigIoUring;
pub use docu::Docu;
pub use docu_index::DocuIndex;
pub use docu_index::{
    DocuEmbeddings, DocuKnowledgeGraphs, DocuOnnx, DocuOnnxParams, DocuIoUring,
    DocuBias, DocuThreads, DocuEntitiesProduction, DocuAgPipeline, DocuLoraExport,
    DocuNeo4j, DocuTantivy, DocuBm25,
};
pub use hardware::ConfigHardware;
pub use home::Home;
pub use memories::ConfigMemories;
pub use monitor::{
    MonitorAgSystemd, MonitorAgentic, MonitorGrafanaServices, MonitorCache, MonitorDocker, MonitorIndex,
    MonitorKnowledgeGraph, MonitorOnnx, MonitorOnnxStatus, MonitorLogs, MonitorObservations, MonitorOverview, MonitorRag,
    MonitorRateLimits, MonitorRequests, MonitorTools,
};
pub use neo4j::ConfigNeo4j;
pub use not_found::PageNotFound;
pub use onnx::ConfigOnnx;
pub use other::ConfigOther;
pub use parameters::Parameters;
pub use prompt::ConfigPrompt;
pub use sampling::ConfigSampling;
pub use train::Train;
