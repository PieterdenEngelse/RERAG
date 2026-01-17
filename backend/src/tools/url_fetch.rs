// src/tools/url_fetch.rs
// Phase 9: URL Fetch Tool Implementation - REAL IMPLEMENTATION

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct URLFetchTool {
    success_count: usize,
    total_count: usize,
}

impl URLFetchTool {
    pub fn new() -> Self {
        Self {
            success_count: 0,
            total_count: 0,
        }
    }

    fn extract_url(&self, query: &str) -> Option<String> {
        let query = query.trim();

        // If the query itself is a URL
        if query.starts_with("http://") || query.starts_with("https://") {
            // Find end of URL (space or end of string)
            let end = query
                .find(|c: char| c.is_whitespace())
                .unwrap_or(query.len());
            return Some(query[..end].to_string());
        }

        // Look for URL in the query
        for word in query.split_whitespace() {
            if word.starts_with("http://") || word.starts_with("https://") {
                return Some(word.to_string());
            }
        }

        None
    }

    fn extract_text_from_html(&self, html: &str) -> String {
        // Simple HTML text extraction - remove tags and decode entities
        let mut result = String::new();
        let mut in_tag = false;
        let mut in_script = false;
        let mut in_style = false;

        let mut i = 0;
        let chars: Vec<char> = html.chars().collect();

        while i < chars.len() {
            let ch = chars[i];

            // Check for script/style tags
            if i + 7 < chars.len() {
                let slice: String = chars[i..i + 7].iter().collect();
                let slice_lower = slice.to_lowercase();
                if slice_lower == "<script" {
                    in_script = true;
                } else if slice_lower == "</scrip" {
                    in_script = false;
                } else if slice_lower == "<style "
                    || (i + 6 < chars.len()
                        && chars[i..i + 6].iter().collect::<String>().to_lowercase() == "<style")
                {
                    in_style = true;
                } else if slice_lower == "</style" {
                    in_style = false;
                }
            }

            if ch == '<' {
                in_tag = true;
            } else if ch == '>' {
                in_tag = false;
                // Add space after block elements
                if result
                    .chars()
                    .last()
                    .map(|c| !c.is_whitespace())
                    .unwrap_or(false)
                {
                    result.push(' ');
                }
            } else if !in_tag && !in_script && !in_style {
                // Decode common HTML entities
                if ch == '&' && i + 1 < chars.len() {
                    let rest: String = chars[i..].iter().take(10).collect();
                    if rest.starts_with("&amp;") {
                        result.push('&');
                        i += 4;
                    } else if rest.starts_with("&lt;") {
                        result.push('<');
                        i += 3;
                    } else if rest.starts_with("&gt;") {
                        result.push('>');
                        i += 3;
                    } else if rest.starts_with("&quot;") {
                        result.push('"');
                        i += 5;
                    } else if rest.starts_with("&nbsp;") {
                        result.push(' ');
                        i += 5;
                    } else if rest.starts_with("&#") {
                        // Numeric entity
                        if let Some(semi) = rest.find(';') {
                            let num_str = &rest[2..semi];
                            if let Ok(num) = if num_str.starts_with('x') {
                                u32::from_str_radix(&num_str[1..], 16)
                            } else {
                                num_str.parse()
                            } {
                                if let Some(c) = char::from_u32(num) {
                                    result.push(c);
                                    i += semi;
                                }
                            }
                        }
                    } else {
                        result.push(ch);
                    }
                } else {
                    result.push(ch);
                }
            }
            i += 1;
        }

        // Clean up whitespace
        let cleaned: String = result.split_whitespace().collect::<Vec<_>>().join(" ");

        cleaned
    }
}

#[async_trait]
impl Tool for URLFetchTool {
    fn tool_type(&self) -> ToolType {
        ToolType::URLFetch
    }

    fn description(&self) -> String {
        "Fetch and extract text content from URLs".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.80
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();

        // Extract URL from query
        let url = match self.extract_url(query) {
            Some(u) => u,
            None => {
                return Ok(ToolResult {
                    tool: ToolType::URLFetch,
                    success: false,
                    result: "No valid URL found in query. Please provide a URL starting with http:// or https://".to_string(),
                    metadata: ToolMetadata {
                        execution_time_ms: start.elapsed().as_millis() as u64,
                        confidence: 0.0,
                        source: None,
                        cost: Some(0.0),
                    },
                });
            }
        };

        // Fetch the URL
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (compatible; AgBot/1.0)")
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        match client.get(&url).send().await {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    return Ok(ToolResult {
                        tool: ToolType::URLFetch,
                        success: false,
                        result: format!("HTTP error: {} for URL {}", status, url),
                        metadata: ToolMetadata {
                            execution_time_ms: start.elapsed().as_millis() as u64,
                            confidence: 0.0,
                            source: Some(url),
                            cost: Some(0.01),
                        },
                    });
                }

                let content_type = response
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();

                let body = response
                    .text()
                    .await
                    .map_err(|e| format!("Failed to read response: {}", e))?;

                // Extract text based on content type
                let text = if content_type.contains("text/html") {
                    self.extract_text_from_html(&body)
                } else if content_type.contains("text/plain")
                    || content_type.contains("application/json")
                {
                    body.clone()
                } else {
                    // For other types, just return first part
                    body.chars().take(2000).collect()
                };

                // Truncate if too long
                let preview = if text.len() > 3000 {
                    format!(
                        "{}... [truncated, {} total chars]",
                        &text[..3000],
                        text.len()
                    )
                } else {
                    text.clone()
                };

                Ok(ToolResult {
                    tool: ToolType::URLFetch,
                    success: true,
                    result: format!("Fetched from {}\n\n{}", url, preview),
                    metadata: ToolMetadata {
                        execution_time_ms: start.elapsed().as_millis() as u64,
                        confidence: 0.90,
                        source: Some(url),
                        cost: Some(0.01),
                    },
                })
            }
            Err(e) => Ok(ToolResult {
                tool: ToolType::URLFetch,
                success: false,
                result: format!("Failed to fetch {}: {}", url, e),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some(url),
                    cost: Some(0.0),
                },
            }),
        }
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
    fn test_extract_url() {
        let tool = URLFetchTool::new();

        assert_eq!(
            tool.extract_url("https://example.com"),
            Some("https://example.com".to_string())
        );

        assert_eq!(
            tool.extract_url("Fetch https://example.com/page"),
            Some("https://example.com/page".to_string())
        );

        assert_eq!(tool.extract_url("No URL here"), None);
    }

    #[tokio::test]
    async fn test_url_fetch_no_url() {
        let tool = URLFetchTool::new();
        let result = tool.execute("No URL here").await;
        assert!(result.is_ok());
        assert!(!result.unwrap().success);
    }
}
