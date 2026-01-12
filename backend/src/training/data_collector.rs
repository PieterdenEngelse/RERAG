// src/training/data_collector.rs
// Version: 1.0.0
//
// Collects and formats training data from RAG interactions for fine-tuning
// custom models with Unsloth. Uses Alpaca format for compatibility.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info, warn};

/// A single training example in QA format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExample {
    /// Unique identifier
    pub id: String,
    /// User query (the instruction)
    pub instruction: String,
    /// Retrieved context (optional input)
    pub context: Option<String>,
    /// Model response (the output)
    pub response: String,
    /// Quality score (1-5, from user feedback)
    pub quality_score: Option<u8>,
    /// Timestamp of collection
    pub timestamp: DateTime<Utc>,
    /// Source conversation ID (for grouping)
    pub conversation_id: Option<String>,
    /// Chat mode used (rag, llm, hybrid)
    pub mode: Option<String>,
    /// Model that generated the response
    pub model: Option<String>,
}

/// Alpaca format for Unsloth compatibility
/// This is the standard format expected by most fine-tuning frameworks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlpacaFormat {
    /// The task instruction (user's question)
    pub instruction: String,
    /// Additional context (RAG retrieved content)
    pub input: String,
    /// Expected output (model's response)
    pub output: String,
}

impl From<TrainingExample> for AlpacaFormat {
    fn from(example: TrainingExample) -> Self {
        AlpacaFormat {
            instruction: example.instruction,
            input: example.context.unwrap_or_default(),
            output: example.response,
        }
    }
}

impl From<&TrainingExample> for AlpacaFormat {
    fn from(example: &TrainingExample) -> Self {
        AlpacaFormat {
            instruction: example.instruction.clone(),
            input: example.context.clone().unwrap_or_default(),
            output: example.response.clone(),
        }
    }
}

/// Statistics about collected training data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrainingStats {
    /// Total examples collected
    pub total_examples: usize,
    /// Examples with quality score >= 4
    pub high_quality_count: usize,
    /// Examples with quality score >= 3
    pub usable_count: usize,
    /// Average quality score
    pub average_quality: f32,
    /// Whether we have enough data for training (500+ usable examples)
    pub ready_for_export: bool,
    /// Breakdown by mode
    pub by_mode: std::collections::HashMap<String, usize>,
    /// Last collection timestamp
    pub last_collected: Option<DateTime<Utc>>,
}

/// Training data collector with buffered writes and quality filtering
pub struct TrainingDataCollector {
    /// Path to the raw training data file
    output_path: PathBuf,
    /// In-memory buffer for batching writes
    buffer: Mutex<Vec<TrainingExample>>,
    /// Number of examples before auto-flush
    buffer_size: usize,
    /// Minimum quality score to keep (1-5)
    min_quality_score: u8,
    /// Whether collection is enabled
    enabled: bool,
}

impl TrainingDataCollector {
    /// Create a new collector with default settings
    pub fn new(output_path: PathBuf) -> Self {
        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let enabled = std::env::var("TRAINING_DATA_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let min_quality = std::env::var("TRAINING_MIN_QUALITY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);

        Self {
            output_path,
            buffer: Mutex::new(Vec::new()),
            buffer_size: 50, // Flush every 50 examples
            min_quality_score: min_quality,
            enabled,
        }
    }

    /// Create a collector with custom settings
    pub fn with_settings(
        output_path: PathBuf,
        buffer_size: usize,
        min_quality_score: u8,
        enabled: bool,
    ) -> Self {
        if let Some(parent) = output_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        Self {
            output_path,
            buffer: Mutex::new(Vec::new()),
            buffer_size,
            min_quality_score: min_quality_score.clamp(1, 5),
            enabled,
        }
    }

    /// Check if collection is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable collection
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Add a training example (buffers until flush)
    pub fn add_example(&self, example: TrainingExample) -> Result<(), std::io::Error> {
        if !self.enabled {
            debug!("Training data collection disabled, skipping example");
            return Ok(());
        }

        // Filter by quality if score is provided
        if let Some(score) = example.quality_score {
            if score < self.min_quality_score {
                debug!(
                    score = score,
                    min = self.min_quality_score,
                    "Skipping low-quality example"
                );
                return Ok(());
            }
        }

        let mut buffer = self.buffer.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;

        buffer.push(example);

        if buffer.len() >= self.buffer_size {
            info!(
                count = buffer.len(),
                "Auto-flushing training data buffer"
            );
            self.flush_internal(&mut buffer)?;
        }

        Ok(())
    }

    /// Flush buffer to disk
    pub fn flush(&self) -> Result<usize, std::io::Error> {
        let mut buffer = self.buffer.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        let count = buffer.len();
        self.flush_internal(&mut buffer)?;
        Ok(count)
    }

    fn flush_internal(&self, buffer: &mut Vec<TrainingExample>) -> Result<(), std::io::Error> {
        if buffer.is_empty() {
            return Ok(());
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.output_path)?;

        let mut writer = BufWriter::new(file);

        for example in buffer.drain(..) {
            // Write as JSONL (one JSON object per line)
            serde_json::to_writer(&mut writer, &example)?;
            writeln!(writer)?;
        }

        writer.flush()?;
        info!(path = ?self.output_path, "Training data flushed to disk");
        Ok(())
    }

    /// Export to Unsloth-compatible Alpaca JSONL format
    pub fn export_for_unsloth(&self, output_path: &PathBuf) -> Result<usize, std::io::Error> {
        // First flush any buffered data
        self.flush()?;

        // Read all examples
        let examples = self.load_all_examples()?;

        // Filter and convert to Alpaca format
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);
        let mut count = 0;

        for example in examples {
            // Only export examples that meet quality threshold
            if example
                .quality_score
                .map(|s| s >= self.min_quality_score)
                .unwrap_or(true)
            {
                let alpaca: AlpacaFormat = example.into();
                serde_json::to_writer(&mut writer, &alpaca)?;
                writeln!(writer)?;
                count += 1;
            }
        }

        writer.flush()?;
        info!(
            count = count,
            path = ?output_path,
            "Exported training data for Unsloth"
        );
        Ok(count)
    }

    /// Load all examples from disk
    pub fn load_all_examples(&self) -> Result<Vec<TrainingExample>, std::io::Error> {
        if !self.output_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.output_path)?;
        let reader = BufReader::new(file);
        let mut examples = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<TrainingExample>(&line) {
                Ok(example) => examples.push(example),
                Err(e) => {
                    warn!(error = %e, "Failed to parse training example, skipping");
                }
            }
        }

        Ok(examples)
    }

    /// Get statistics about collected training data
    pub fn get_stats(&self) -> Result<TrainingStats, std::io::Error> {
        let examples = self.load_all_examples()?;

        let total = examples.len();
        let mut high_quality = 0;
        let mut usable = 0;
        let mut quality_sum: u32 = 0;
        let mut quality_count = 0;
        let mut by_mode: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut last_collected: Option<DateTime<Utc>> = None;

        for example in &examples {
            // Count by quality
            if let Some(score) = example.quality_score {
                quality_sum += score as u32;
                quality_count += 1;
                if score >= 4 {
                    high_quality += 1;
                }
                if score >= 3 {
                    usable += 1;
                }
            } else {
                // No score = assume usable
                usable += 1;
            }

            // Count by mode
            if let Some(mode) = &example.mode {
                *by_mode.entry(mode.clone()).or_insert(0) += 1;
            }

            // Track latest timestamp
            if last_collected.map(|lc| example.timestamp > lc).unwrap_or(true) {
                last_collected = Some(example.timestamp);
            }
        }

        let average_quality = if quality_count > 0 {
            quality_sum as f32 / quality_count as f32
        } else {
            0.0
        };

        // Ready for export if we have 500+ usable examples
        let ready_for_export = usable >= 500;

        Ok(TrainingStats {
            total_examples: total,
            high_quality_count: high_quality,
            usable_count: usable,
            average_quality,
            ready_for_export,
            by_mode,
            last_collected,
        })
    }

    /// Clear all collected training data
    pub fn clear(&self) -> Result<(), std::io::Error> {
        // Clear buffer
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.clear();
        }

        // Remove file if exists
        if self.output_path.exists() {
            fs::remove_file(&self.output_path)?;
        }

        info!("Training data cleared");
        Ok(())
    }
}

impl Default for TrainingDataCollector {
    fn default() -> Self {
        let path = std::env::var("TRAINING_DATA_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_local_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("ag")
                    .join("training")
                    .join("raw_examples.jsonl")
            });

        Self::new(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_example(id: &str, score: Option<u8>) -> TrainingExample {
        TrainingExample {
            id: id.to_string(),
            instruction: format!("Test question {}", id),
            context: Some("Test context".to_string()),
            response: format!("Test answer {}", id),
            quality_score: score,
            timestamp: Utc::now(),
            conversation_id: None,
            mode: Some("hybrid".to_string()),
            model: Some("phi:latest".to_string()),
        }
    }

    #[test]
    fn test_training_example_to_alpaca() {
        let example = create_test_example("1", Some(5));
        let alpaca: AlpacaFormat = example.into();

        assert_eq!(alpaca.instruction, "Test question 1");
        assert_eq!(alpaca.input, "Test context");
        assert_eq!(alpaca.output, "Test answer 1");
    }

    #[test]
    fn test_collector_add_and_flush() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("training_data.jsonl");

        let collector = TrainingDataCollector::with_settings(path.clone(), 10, 1, true);

        // Add examples
        for i in 0..5 {
            let example = create_test_example(&i.to_string(), Some(4));
            collector.add_example(example).unwrap();
        }

        // Flush
        let count = collector.flush().unwrap();
        assert_eq!(count, 5);

        // Verify file exists and has content
        assert!(path.exists());
        let examples = collector.load_all_examples().unwrap();
        assert_eq!(examples.len(), 5);
    }

    #[test]
    fn test_quality_filtering() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("training_data.jsonl");

        // Min quality = 3
        let collector = TrainingDataCollector::with_settings(path.clone(), 10, 3, true);

        // Add examples with different scores
        collector
            .add_example(create_test_example("1", Some(5)))
            .unwrap();
        collector
            .add_example(create_test_example("2", Some(4)))
            .unwrap();
        collector
            .add_example(create_test_example("3", Some(3)))
            .unwrap();
        collector
            .add_example(create_test_example("4", Some(2)))
            .unwrap(); // Should be filtered
        collector
            .add_example(create_test_example("5", Some(1)))
            .unwrap(); // Should be filtered

        collector.flush().unwrap();

        let examples = collector.load_all_examples().unwrap();
        assert_eq!(examples.len(), 3); // Only scores 3, 4, 5
    }

    #[test]
    fn test_export_for_unsloth() {
        let dir = tempdir().unwrap();
        let raw_path = dir.path().join("raw.jsonl");
        let export_path = dir.path().join("export.jsonl");

        let collector = TrainingDataCollector::with_settings(raw_path, 10, 3, true);

        // Add examples
        for i in 0..10 {
            let example = create_test_example(&i.to_string(), Some(4));
            collector.add_example(example).unwrap();
        }
        collector.flush().unwrap();

        // Export
        let count = collector.export_for_unsloth(&export_path).unwrap();
        assert_eq!(count, 10);

        // Verify export format
        let content = fs::read_to_string(&export_path).unwrap();
        for line in content.lines() {
            let alpaca: AlpacaFormat = serde_json::from_str(line).unwrap();
            assert!(!alpaca.instruction.is_empty());
            assert!(!alpaca.output.is_empty());
        }
    }

    #[test]
    fn test_get_stats() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("training_data.jsonl");

        let collector = TrainingDataCollector::with_settings(path, 100, 1, true);

        // Add examples with various scores
        collector
            .add_example(create_test_example("1", Some(5)))
            .unwrap();
        collector
            .add_example(create_test_example("2", Some(4)))
            .unwrap();
        collector
            .add_example(create_test_example("3", Some(3)))
            .unwrap();
        collector
            .add_example(create_test_example("4", Some(2)))
            .unwrap();
        collector.flush().unwrap();

        let stats = collector.get_stats().unwrap();
        assert_eq!(stats.total_examples, 4);
        assert_eq!(stats.high_quality_count, 2); // scores 4 and 5
        assert_eq!(stats.usable_count, 3); // scores 3, 4, and 5
        assert!(!stats.ready_for_export); // Need 500+ examples
    }

    #[test]
    fn test_disabled_collector() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("training_data.jsonl");

        // Disabled collector
        let collector = TrainingDataCollector::with_settings(path.clone(), 10, 3, false);

        // Add examples (should be ignored)
        for i in 0..5 {
            collector
                .add_example(create_test_example(&i.to_string(), Some(5)))
                .unwrap();
        }
        collector.flush().unwrap();

        // File should not exist or be empty
        if path.exists() {
            let examples = collector.load_all_examples().unwrap();
            assert_eq!(examples.len(), 0);
        }
    }
}
