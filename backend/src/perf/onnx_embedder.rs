//! ONNX Runtime Embedder

use std::path::Path;
use tracing::{info, warn};

#[cfg(feature = "onnx")]
use ort::session::Session;
#[cfg(feature = "onnx")]
use ort::value::Tensor;

pub type EmbeddingVector = Vec<f32>;

#[derive(Debug, Clone)]
pub struct OnnxConfig {
    pub model_path: String,
    pub max_length: usize,
    pub embedding_dim: usize,
    pub num_threads: usize,
}

impl Default for OnnxConfig {
    fn default() -> Self {
        Self {
            model_path: "models/embedding_model.onnx".to_string(),
            max_length: 512,
            embedding_dim: 384,
            num_threads: 4,
        }
    }
}

#[derive(Debug)]
pub enum OnnxError {
    ModelNotFound(String),
    SessionCreationFailed(String),
    InferenceFailed(String),
}

impl std::fmt::Display for OnnxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelNotFound(p) => write!(f, "Model not found: {}", p),
            Self::SessionCreationFailed(e) => write!(f, "Session failed: {}", e),
            Self::InferenceFailed(e) => write!(f, "Inference failed: {}", e),
        }
    }
}

impl std::error::Error for OnnxError {}

pub struct SimpleTokenizer {
    max_length: usize,
}

impl SimpleTokenizer {
    pub fn new(max_length: usize) -> Self {
        Self { max_length }
    }

    pub fn encode_i64(&self, text: &str) -> (Vec<i64>, Vec<i64>) {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut input_ids = vec![101i64]; // CLS
        let mut attention_mask = vec![1i64];

        for word in words.iter().take(self.max_length - 2) {
            let hash = seahash::hash(word.as_bytes());
            input_ids.push((hash % 29522 + 1000) as i64);
            attention_mask.push(1);
        }

        input_ids.push(102); // SEP
        attention_mask.push(1);

        input_ids.resize(self.max_length, 0);
        attention_mask.resize(self.max_length, 0);

        (input_ids, attention_mask)
    }
}

#[cfg(feature = "onnx")]
pub struct OnnxEmbedder {
    config: OnnxConfig,
    tokenizer: SimpleTokenizer,
    session: Session,
}

#[cfg(not(feature = "onnx"))]
pub struct OnnxEmbedder {
    config: OnnxConfig,
    tokenizer: SimpleTokenizer,
}

#[cfg(feature = "onnx")]
impl OnnxEmbedder {
    pub fn new(config: OnnxConfig) -> Result<Self, OnnxError> {
        if !Path::new(&config.model_path).exists() {
            return Err(OnnxError::ModelNotFound(config.model_path.clone()));
        }

        info!(model = %config.model_path, "Initializing ONNX embedder");

        let _ = ort::init().with_name("ag").commit();

        let session = Session::builder()
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?
            .with_intra_threads(config.num_threads)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?
            .commit_from_file(&config.model_path)
            .map_err(|e| OnnxError::SessionCreationFailed(e.to_string()))?;

        info!("ONNX session created");

        Ok(Self {
            tokenizer: SimpleTokenizer::new(config.max_length),
            config,
            session,
        })
    }

    pub fn embed(&mut self, texts: &[&str]) -> Result<Vec<EmbeddingVector>, OnnxError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = texts.len();
        let seq_len = self.config.max_length;

        // Tokenize
        let mut all_input_ids = Vec::with_capacity(batch_size * seq_len);
        let mut all_attention_mask = Vec::with_capacity(batch_size * seq_len);

        for text in texts {
            let (ids, mask) = self.tokenizer.encode_i64(text);
            all_input_ids.extend(ids);
            all_attention_mask.extend(mask);
        }

        // Create tensors using Tensor::from_array
        let input_ids_tensor = Tensor::from_array((
            vec![batch_size as i64, seq_len as i64],
            all_input_ids
        )).map_err(|e| OnnxError::InferenceFailed(e.to_string()))?;

        let attention_mask_tensor = Tensor::from_array((
            vec![batch_size as i64, seq_len as i64],
            all_attention_mask
        )).map_err(|e| OnnxError::InferenceFailed(e.to_string()))?;

        // Run inference
        let outputs = self.session.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor
        ]).map_err(|e| OnnxError::InferenceFailed(e.to_string()))?;

        // Get first output
        let output = &outputs[0];
        let (shape, data) = output.try_extract_tensor::<f32>()
            .map_err(|e| OnnxError::InferenceFailed(e.to_string()))?;

        let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();

        // Extract embeddings
        let embeddings = match dims.as_slice() {
            [b, _s, h] => {
                // [batch, seq, hidden] - take CLS token
                (0..*b).map(|i| {
                    let start = i * dims[1] * dims[2];
                    data[start..start + *h].to_vec()
                }).collect()
            }
            [b, h] => {
                // [batch, hidden]
                (0..*b).map(|i| {
                    let start = i * *h;
                    data[start..start + *h].to_vec()
                }).collect()
            }
            _ => return Err(OnnxError::InferenceFailed(format!("Bad shape: {:?}", dims)))
        };

        Ok(embeddings)
    }

    pub fn embed_one(&mut self, text: &str) -> Result<EmbeddingVector, OnnxError> {
        self.embed(&[text]).map(|mut v| v.pop().unwrap_or_default())
    }

    pub fn dimension(&self) -> usize { self.config.embedding_dim }
    pub fn model_path(&self) -> &str { &self.config.model_path }
}

#[cfg(not(feature = "onnx"))]
impl OnnxEmbedder {
    pub fn new(config: OnnxConfig) -> Result<Self, OnnxError> {
        if !Path::new(&config.model_path).exists() {
            return Err(OnnxError::ModelNotFound(config.model_path.clone()));
        }
        warn!("ONNX feature not enabled");
        Ok(Self {
            tokenizer: SimpleTokenizer::new(config.max_length),
            config,
        })
    }

    pub fn embed(&self, texts: &[&str]) -> Result<Vec<EmbeddingVector>, OnnxError> {
        Ok(texts.iter().map(|t| {
            let h = seahash::hash(t.as_bytes());
            let mut v = vec![0f32; self.config.embedding_dim];
            for i in 0..v.len() {
                v[i] = ((seahash::hash(&[h.to_le_bytes(), (i as u64).to_le_bytes()].concat()) as f32) / u64::MAX as f32) * 2.0 - 1.0;
            }
            let n: f32 = v.iter().map(|x| x*x).sum::<f32>().sqrt();
            if n > 0.0 { v.iter_mut().for_each(|x| *x /= n); }
            v
        }).collect())
    }

    pub fn embed_one(&self, text: &str) -> Result<EmbeddingVector, OnnxError> {
        self.embed(&[text]).map(|mut v| v.pop().unwrap_or_default())
    }

    pub fn dimension(&self) -> usize { self.config.embedding_dim }
    pub fn model_path(&self) -> &str { &self.config.model_path }
}

pub fn is_onnx_enabled() -> bool { cfg!(feature = "onnx") }
