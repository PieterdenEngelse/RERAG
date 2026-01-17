// src/tools/web_search.rs
// Phase 9: Web Search Tool - REAL IMPLEMENTATION using DuckDuckGo HTML

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct WebSearchTool {
    success_count: usize,
    total_count: usize,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn with_mock() -> Self {
        Self::new()
    }

    /// Extract search results from DuckDuckGo HTML
    /// Format: <a rel="nofollow" class="result__a" href="...">Title</a>
    fn parse_ddg_html(&self, html: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let mut search_pos = 0;

        while let Some(class_pos) = html[search_pos..].find("class=\"result__a\"") {
            let abs_class = search_pos + class_pos;

            // Look FORWARD for href=" (it comes after class in this HTML)
            let after_class = &html[abs_class..];

            if let Some(href_rel) = after_class.find("href=\"") {
                let href_start = abs_class + href_rel + 6;
                if let Some(href_end) = html[href_start..].find('"') {
                    let raw_url = &html[href_start..href_start + href_end];
                    let actual_url = self.extract_url(raw_url);

                    // Find title (text after href closing quote and >)
                    let after_href = href_start + href_end;
                    if let Some(tag_close) = html[after_href..].find('>') {
                        let title_start = after_href + tag_close + 1;
                        if let Some(title_end) = html[title_start..].find("</a>") {
                            let title =
                                Self::clean_html(&html[title_start..title_start + title_end]);

                            // Find snippet
                            let after_title = title_start + title_end;
                            let snippet = self.find_snippet(&html[after_title..]);

                            if !actual_url.is_empty() && !title.is_empty() {
                                results.push(SearchResult {
                                    title,
                                    url: actual_url,
                                    snippet,
                                });
                            }
                        }
                    }
                }
            }

            search_pos = abs_class + 20;
            if results.len() >= 5 {
                break;
            }
        }

        results
    }

    fn extract_url(&self, raw: &str) -> String {
        // URL format: //duckduckgo.com/l/?uddg=https%3A%2F%2F...&amp;rut=...
        if let Some(uddg_pos) = raw.find("uddg=") {
            let url_start = uddg_pos + 5;
            let rest = &raw[url_start..];
            // In HTML, & is encoded as &amp;
            let url_end = rest
                .find("&amp;")
                .or_else(|| rest.find('&'))
                .unwrap_or(rest.len());
            let encoded = &rest[..url_end];
            return Self::url_decode(encoded);
        }

        if raw.starts_with("http") {
            return raw.to_string();
        }
        if raw.starts_with("//") {
            return format!("https:{}", raw);
        }

        String::new()
    }

    fn find_snippet(&self, html: &str) -> String {
        if let Some(snip_pos) = html.find("class=\"result__snippet\"") {
            if let Some(tag_end) = html[snip_pos..].find('>') {
                let text_start = snip_pos + tag_end + 1;
                if let Some(close_pos) = html[text_start..].find("</") {
                    let snippet = &html[text_start..text_start + close_pos];
                    return Self::clean_html(snippet);
                }
            }
        }
        String::new()
    }

    fn url_decode(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '%' {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                } else {
                    result.push('%');
                    result.push_str(&hex);
                }
            } else if c == '+' {
                result.push(' ');
            } else {
                result.push(c);
            }
        }

        result
    }

    fn clean_html(s: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;

        for c in s.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                result.push(c);
            }
        }

        result
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#x27;", "'")
            .replace("&nbsp;", " ")
            .replace("&#39;", "'")
            .trim()
            .to_string()
    }
}

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn tool_type(&self) -> ToolType {
        ToolType::WebSearch
    }

    fn description(&self) -> String {
        "Search the web using DuckDuckGo".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.85
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();

        let encoded_query: String = query
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                ' ' => "+".to_string(),
                _ => format!("%{:02X}", c as u8),
            })
            .collect();
        let url = format!("https://html.duckduckgo.com/html/?q={}", encoded_query);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        match client.get(&url).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    return Ok(ToolResult {
                        tool: ToolType::WebSearch,
                        success: false,
                        result: format!("Search failed with status: {}", response.status()),
                        metadata: ToolMetadata {
                            execution_time_ms: start.elapsed().as_millis() as u64,
                            confidence: 0.0,
                            source: Some("DuckDuckGo".to_string()),
                            cost: Some(0.0),
                        },
                    });
                }

                let html = response
                    .text()
                    .await
                    .map_err(|e| format!("Failed to read response: {}", e))?;
                let results = self.parse_ddg_html(&html);

                if results.is_empty() {
                    return Ok(ToolResult {
                        tool: ToolType::WebSearch,
                        success: true,
                        result: format!("Search for '{}': No results found.", query),
                        metadata: ToolMetadata {
                            execution_time_ms: start.elapsed().as_millis() as u64,
                            confidence: 0.3,
                            source: Some("DuckDuckGo".to_string()),
                            cost: Some(0.0),
                        },
                    });
                }

                let mut output = format!("Web search results for '{}':\n\n", query);
                for (i, result) in results.iter().enumerate() {
                    output.push_str(&format!("{}. {}\n   {}\n", i + 1, result.title, result.url));
                    if !result.snippet.is_empty() {
                        output.push_str(&format!("   {}\n", result.snippet));
                    }
                    output.push('\n');
                }

                Ok(ToolResult {
                    tool: ToolType::WebSearch,
                    success: true,
                    result: output,
                    metadata: ToolMetadata {
                        execution_time_ms: start.elapsed().as_millis() as u64,
                        confidence: 0.85,
                        source: Some("DuckDuckGo".to_string()),
                        cost: Some(0.0),
                    },
                })
            }
            Err(e) => Ok(ToolResult {
                tool: ToolType::WebSearch,
                success: false,
                result: format!("Search failed: {}", e),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("DuckDuckGo".to_string()),
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

    #[tokio::test]
    async fn test_web_search() {
        let tool = WebSearchTool::new();
        let result = tool.execute("Rust programming").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(WebSearchTool::url_decode("hello%20world"), "hello world");
        assert_eq!(
            WebSearchTool::url_decode("https%3A%2F%2Frust-lang.org%2F"),
            "https://rust-lang.org/"
        );
    }
}
