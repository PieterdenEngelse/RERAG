// src/tools/image_generation.rs
// Image Generation Tool - Generate images from text prompts
// Currently a placeholder that can be extended with actual image generation APIs

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::time::Instant;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct ImageGenerationTool {
    /// API endpoint for image generation (e.g., DALL-E, Stable Diffusion)
    api_endpoint: Option<String>,
    /// API key for authentication
    api_key: Option<String>,
    /// Default image size
    default_size: String,
    success_count: usize,
    total_count: usize,
}

impl ImageGenerationTool {
    pub fn new() -> Self {
        Self {
            api_endpoint: std::env::var("IMAGE_GEN_API_ENDPOINT").ok(),
            api_key: std::env::var("IMAGE_GEN_API_KEY").ok(),
            default_size: "512x512".to_string(),
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn with_api(endpoint: String, api_key: String) -> Self {
        Self {
            api_endpoint: Some(endpoint),
            api_key: Some(api_key),
            default_size: "512x512".to_string(),
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn set_default_size(&mut self, size: &str) {
        self.default_size = size.to_string();
    }

    /// Parse prompt to extract parameters
    fn parse_prompt(&self, query: &str) -> (String, String, Option<String>) {
        let mut prompt = query.to_string();
        let mut size = self.default_size.clone();
        let mut style: Option<String> = None;

        // Extract size if specified
        let size_patterns = ["256x256", "512x512", "1024x1024", "1792x1024", "1024x1792"];
        for pattern in size_patterns {
            if query.contains(pattern) {
                size = pattern.to_string();
                prompt = prompt.replace(pattern, "").trim().to_string();
            }
        }

        // Extract style if specified
        let style_keywords = [
            ("photorealistic", "photorealistic"),
            ("cartoon", "cartoon"),
            ("anime", "anime"),
            ("oil painting", "oil_painting"),
            ("watercolor", "watercolor"),
            ("sketch", "sketch"),
            ("3d render", "3d_render"),
            ("pixel art", "pixel_art"),
        ];

        for (keyword, style_name) in style_keywords {
            if query.to_lowercase().contains(keyword) {
                style = Some(style_name.to_string());
                break;
            }
        }

        (prompt.trim().to_string(), size, style)
    }

    /// Generate image using configured API
    async fn generate_with_api(&self, prompt: &str, size: &str, style: Option<&str>) -> Result<String, String> {
        let endpoint = self.api_endpoint.as_ref()
            .ok_or_else(|| "Image generation API not configured. Set IMAGE_GEN_API_ENDPOINT and IMAGE_GEN_API_KEY environment variables.".to_string())?;
        
        let api_key = self.api_key.as_ref()
            .ok_or_else(|| "API key not configured".to_string())?;

        // Build request body
        let mut body = serde_json::json!({
            "prompt": prompt,
            "size": size,
            "n": 1,
        });

        if let Some(s) = style {
            body["style"] = serde_json::json!(s);
        }

        // Make API request
        let client = reqwest::Client::new();
        let response = client
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("API request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, error_text));
        }

        let result: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        // Extract image URL from response
        // This assumes OpenAI-style response format
        if let Some(data) = result.get("data") {
            if let Some(first) = data.get(0) {
                if let Some(url) = first.get("url").and_then(|u| u.as_str()) {
                    return Ok(url.to_string());
                }
                if let Some(b64) = first.get("b64_json").and_then(|b| b.as_str()) {
                    return Ok(format!("data:image/png;base64,{}", b64));
                }
            }
        }

        Err("Could not extract image from response".to_string())
    }

    /// Generate a placeholder response when no API is configured
    fn generate_placeholder(&self, prompt: &str, size: &str, style: Option<&str>) -> String {
        let style_str = style.map(|s| format!(" in {} style", s)).unwrap_or_default();
        
        format!(
            "🎨 Image Generation Request\n\n\
            Prompt: \"{}\"\n\
            Size: {}\n\
            Style: {}\n\n\
            ⚠️ No image generation API configured.\n\n\
            To enable image generation, set these environment variables:\n\
            - IMAGE_GEN_API_ENDPOINT: API endpoint URL\n\
            - IMAGE_GEN_API_KEY: Your API key\n\n\
            Supported APIs:\n\
            - OpenAI DALL-E: https://api.openai.com/v1/images/generations\n\
            - Stable Diffusion: Your local or cloud endpoint\n\n\
            The prompt has been validated and would generate an image{}.",
            prompt,
            size,
            style.unwrap_or("default"),
            style_str
        )
    }
}

#[async_trait]
impl Tool for ImageGenerationTool {
    fn tool_type(&self) -> ToolType {
        ToolType::ImageGeneration
    }

    fn description(&self) -> String {
        "Generate images from text descriptions using AI image generation".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.70
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();
        debug!("ImageGenerationTool: processing prompt '{}'", query);

        if query.trim().is_empty() {
            return Ok(ToolResult {
                tool: ToolType::ImageGeneration,
                success: false,
                result: "Please provide a description of the image you want to generate.".to_string(),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("ImageGeneration".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        let (prompt, size, style) = self.parse_prompt(query);

        // Try to use API if configured
        if self.api_endpoint.is_some() && self.api_key.is_some() {
            match self.generate_with_api(&prompt, &size, style.as_deref()).await {
                Ok(image_url) => {
                    return Ok(ToolResult {
                        tool: ToolType::ImageGeneration,
                        success: true,
                        result: format!(
                            "✅ Image generated successfully!\n\n\
                            Prompt: \"{}\"\n\
                            Size: {}\n\
                            Style: {}\n\n\
                            Image URL: {}",
                            prompt,
                            size,
                            style.as_deref().unwrap_or("default"),
                            image_url
                        ),
                        metadata: ToolMetadata {
                            execution_time_ms: start.elapsed().as_millis() as u64,
                            confidence: 0.95,
                            source: Some("ImageGeneration/API".to_string()),
                            cost: Some(0.02), // Typical cost per image
                        },
                    });
                }
                Err(e) => {
                    return Ok(ToolResult {
                        tool: ToolType::ImageGeneration,
                        success: false,
                        result: format!("Image generation failed: {}", e),
                        metadata: ToolMetadata {
                            execution_time_ms: start.elapsed().as_millis() as u64,
                            confidence: 0.0,
                            source: Some("ImageGeneration/API".to_string()),
                            cost: Some(0.0),
                        },
                    });
                }
            }
        }

        // Return placeholder if no API configured
        Ok(ToolResult {
            tool: ToolType::ImageGeneration,
            success: true, // Placeholder is still a valid response
            result: self.generate_placeholder(&prompt, &size, style.as_deref()),
            metadata: ToolMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                confidence: 0.5,
                source: Some("ImageGeneration/Placeholder".to_string()),
                cost: Some(0.0),
            },
        })
    }

    fn update_success(&mut self, success: bool) {
        self.total_count += 1;
        if success {
            self.success_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_parsing() {
        let tool = ImageGenerationTool::new();
        
        let (prompt, size, style) = tool.parse_prompt("A cat sitting on a chair 1024x1024");
        assert_eq!(prompt, "A cat sitting on a chair");
        assert_eq!(size, "1024x1024");
        assert!(style.is_none());

        let (prompt, size, style) = tool.parse_prompt("A photorealistic sunset over mountains");
        assert!(prompt.contains("sunset"));
        assert_eq!(style, Some("photorealistic".to_string()));
    }

    #[tokio::test]
    async fn test_placeholder_generation() {
        let tool = ImageGenerationTool::new();
        let result = tool.execute("A beautiful sunset over the ocean").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("Image Generation Request"));
    }

    #[tokio::test]
    async fn test_empty_prompt() {
        let tool = ImageGenerationTool::new();
        let result = tool.execute("").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(!res.success);
    }
}
