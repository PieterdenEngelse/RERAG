// src/training/mod.rs
// Phase 20: Custom Model Training Data Collection
//
// This module provides infrastructure for collecting high-quality
// training data from RAG interactions, which can be used to fine-tune
// custom models using Unsloth.

pub mod data_collector;
pub mod feedback;

pub use data_collector::{AlpacaFormat, TrainingDataCollector, TrainingExample, TrainingStats};
pub use feedback::QualityScore;
