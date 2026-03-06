//! Quantization helpers for model memory estimation and recommendations
//!
//! This module provides utilities to:
//! - Parse model size from model names (e.g., "llama3:7b" → 7B parameters)
//! - Calculate memory requirements for different quantization levels
//! - Recommend appropriate quantization based on available system memory

use serde::{Deserialize, Serialize};

/// GGUF Quantization levels with their properties
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuantizationLevel {
    /// Quantization name (e.g., "q4_0", "q8_0")
    pub name: &'static str,
    /// Bits per weight
    pub bits_per_weight: f64,
    /// Human-readable description
    pub description: &'static str,
    /// Quality rating (1-5, 5 being best)
    pub quality: u8,
    /// Speed rating (1-5, 5 being fastest)
    pub speed: u8,
}

/// All supported quantization levels
pub const QUANTIZATION_LEVELS: &[QuantizationLevel] = &[
    QuantizationLevel {
        name: "q4_0",
        bits_per_weight: 4.5,
        description: "4-bit, fastest, lowest quality",
        quality: 2,
        speed: 5,
    },
    QuantizationLevel {
        name: "q4_k_s",
        bits_per_weight: 4.5,
        description: "4-bit K-quant small, good balance",
        quality: 3,
        speed: 5,
    },
    QuantizationLevel {
        name: "q4_k_m",
        bits_per_weight: 4.8,
        description: "4-bit K-quant medium, better quality",
        quality: 4,
        speed: 4,
    },
    QuantizationLevel {
        name: "q5_0",
        bits_per_weight: 5.5,
        description: "5-bit, balanced",
        quality: 3,
        speed: 4,
    },
    QuantizationLevel {
        name: "q5_k_s",
        bits_per_weight: 5.5,
        description: "5-bit K-quant small",
        quality: 4,
        speed: 4,
    },
    QuantizationLevel {
        name: "q5_k_m",
        bits_per_weight: 5.7,
        description: "5-bit K-quant medium",
        quality: 4,
        speed: 3,
    },
    QuantizationLevel {
        name: "q6_k",
        bits_per_weight: 6.6,
        description: "6-bit K-quant, high quality",
        quality: 5,
        speed: 3,
    },
    QuantizationLevel {
        name: "q8_0",
        bits_per_weight: 8.5,
        description: "8-bit, highest quality quantized",
        quality: 5,
        speed: 2,
    },
    QuantizationLevel {
        name: "f16",
        bits_per_weight: 16.0,
        description: "16-bit float, full precision",
        quality: 5,
        speed: 1,
    },
];

/// Parsed model size information
#[derive(Clone, Debug)]
pub struct ModelSize {
    /// Number of parameters in billions
    pub params_billions: f64,
    /// Original size string (e.g., "7b", "3.8b")
    pub size_string: String,
}

/// Parse model size from model name
///
/// Examples:
/// - "llama3:7b" → 7.0B
/// - "phi3:3.8b" → 3.8B
/// - "qwen2:1.5b" → 1.5B
/// - "mistral:7b-instruct-q4_k_m" → 7.0B
pub fn parse_model_size(model_name: &str) -> Option<ModelSize> {
    // Use the simple parser (no regex needed, WASM compatible)
    parse_model_size_simple(model_name)
}

/// Calculate memory requirement for a model with given quantization
///
/// Formula: (params * bits_per_weight) / 8 + overhead
/// Overhead accounts for KV cache, activations, etc. (~1.5GB typical)
pub fn calculate_memory_gb(params_billions: f64, quant: &QuantizationLevel) -> f64 {
    let base_memory = (params_billions * quant.bits_per_weight) / 8.0;
    let overhead = 1.5; // KV cache, activations, OS overhead
    base_memory + overhead
}

/// Memory recommendation result
#[derive(Clone, Debug)]
pub struct QuantizationRecommendation {
    pub level: &'static QuantizationLevel,
    pub estimated_memory_gb: f64,
    pub fits_in_memory: bool,
    pub is_recommended: bool,
    pub warning: Option<String>,
}

/// Get quantization recommendations for a model given available memory
pub fn get_recommendations(
    params_billions: f64,
    available_memory_gb: f64,
) -> Vec<QuantizationRecommendation> {
    let mut recommendations: Vec<QuantizationRecommendation> = QUANTIZATION_LEVELS
        .iter()
        .map(|level| {
            let estimated = calculate_memory_gb(params_billions, level);
            let fits = estimated <= available_memory_gb;
            let warning = if !fits {
                Some(format!(
                    "Requires ~{:.1} GB, you have {:.1} GB available",
                    estimated, available_memory_gb
                ))
            } else {
                None
            };

            QuantizationRecommendation {
                level,
                estimated_memory_gb: estimated,
                fits_in_memory: fits,
                is_recommended: false,
                warning,
            }
        })
        .collect();

    // Mark the best fitting option as recommended
    // Prefer highest quality that fits
    for rec in recommendations.iter_mut().rev() {
        if rec.fits_in_memory {
            rec.is_recommended = true;
            break;
        }
    }

    // If nothing fits, recommend the smallest
    if !recommendations.iter().any(|r| r.is_recommended) {
        if let Some(first) = recommendations.first_mut() {
            first.is_recommended = true;
            first.warning = Some(format!(
                "Model may be too large. Consider a smaller model ({}B → {}B)",
                params_billions,
                (params_billions / 2.0).max(1.0)
            ));
        }
    }

    recommendations
}

/// Simple model size parser without regex (for WASM compatibility)
pub fn parse_model_size_simple(model_name: &str) -> Option<ModelSize> {
    let name_lower = model_name.to_lowercase();

    // Look for patterns like "7b", "3.8b", "1.5b", "70b"
    let chars: Vec<char> = name_lower.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Find start of a number
        if chars[i].is_ascii_digit() {
            let start = i;
            let mut has_decimal = false;

            // Consume digits and optional decimal
            while i < chars.len()
                && (chars[i].is_ascii_digit() || (chars[i] == '.' && !has_decimal))
            {
                if chars[i] == '.' {
                    has_decimal = true;
                }
                i += 1;
            }

            // Check if followed by 'b'
            if i < chars.len() && chars[i] == 'b' {
                let num_str: String = chars[start..i].iter().collect();
                if let Ok(size) = num_str.parse::<f64>() {
                    if size > 0.0 && size < 1000.0 {
                        return Some(ModelSize {
                            params_billions: size,
                            size_string: format!("{}B", size),
                        });
                    }
                }
            }
        }
        i += 1;
    }

    // Fallback for known models
    if name_lower.contains("phi") && (name_lower.contains("mini") || name_lower.contains("3")) {
        return Some(ModelSize {
            params_billions: 3.8,
            size_string: "3.8B".to_string(),
        });
    }
    if name_lower.contains("tinyllama") {
        return Some(ModelSize {
            params_billions: 1.1,
            size_string: "1.1B".to_string(),
        });
    }
    if name_lower.contains("gemma") && name_lower.contains("2b") {
        return Some(ModelSize {
            params_billions: 2.0,
            size_string: "2B".to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_size() {
        assert_eq!(
            parse_model_size_simple("llama3:7b")
                .unwrap()
                .params_billions,
            7.0
        );
        assert_eq!(
            parse_model_size_simple("phi3:3.8b")
                .unwrap()
                .params_billions,
            3.8
        );
        assert_eq!(
            parse_model_size_simple("qwen2:1.5b")
                .unwrap()
                .params_billions,
            1.5
        );
        assert_eq!(
            parse_model_size_simple("mistral:7b-instruct-q4_k_m")
                .unwrap()
                .params_billions,
            7.0
        );
    }

    #[test]
    fn test_memory_calculation() {
        let q4 = &QUANTIZATION_LEVELS[0]; // q4_0
        let mem = calculate_memory_gb(7.0, q4);
        assert!(mem > 3.0 && mem < 6.0); // ~3.9 + 1.5 overhead
    }
}
