// src/pages/mod.rs
pub mod about;
pub mod config_chunker;
pub mod config_corpus;
pub mod config_embedding;
pub mod config_io_uring;
pub mod config_runtime;
pub mod docu;
pub mod docu_index;
pub mod falkordb;
pub mod hardware;
pub mod home;
pub mod memories;
pub mod monitor;
pub mod ner;
pub mod not_found;
pub mod onnx;
pub mod onnx_help;
pub mod other;
pub mod parameters;
pub mod redis;
pub mod terms;
pub mod train;

// Re-export so they can be used as `pages::Home`
pub use about::About;
pub use config_chunker::ConfigChunker;
pub use config_corpus::ConfigCorpus;
pub use config_embedding::ConfigEmbedding;
pub use config_io_uring::ConfigIoUring;
pub use config_runtime::ConfigRuntime;
pub use docu::Docu;
pub use docu_index::DocuIndex;
pub use docu_index::{
    DocuAgPipeline, DocuAgglutinative, DocuBias, DocuBm25, DocuBpeUnigram, DocuCanonicalization,
    DocuDetrLayout, DocuEmbeddings, DocuEntitiesProduction, DocuFileWatcher, DocuIoUring,
    DocuKnowledgeGraphs, DocuLoraExport, DocuOnnx, DocuOnnxParams, DocuRig, DocuRkyv, DocuTantivy,
    DocuThreads, DocuTokenizersGeneral,
};
pub use falkordb::ConfigFalkorDb;
pub use hardware::ConfigHardware;
pub use home::Home;
pub use memories::ConfigMemories;
pub use monitor::{
    MonitorAgSystemd, MonitorAgentic, MonitorCache, MonitorChunks, MonitorDatastores,
    MonitorDocker, MonitorGrafanaServices, MonitorIndex, MonitorKnowledgeGraph, MonitorLogs,
    MonitorObservations, MonitorOnnx, MonitorOnnxStatus, MonitorRag, MonitorRateLimits,
    MonitorRequests, MonitorTip, MonitorTools,
};
pub use ner::ConfigNer;
pub use not_found::PageNotFound;
pub use onnx::ConfigOnnx;
pub use other::ConfigOther;
pub use parameters::Parameters;
pub use redis::ConfigRedis;
pub use terms::ConfigTerms;
pub use train::Train;
