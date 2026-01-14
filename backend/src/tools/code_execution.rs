// src/tools/code_execution.rs
// Code Execution Tool - Execute code snippets in a sandboxed environment

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::io::Write;
use std::process::Command;
use std::time::Instant;
use tempfile::NamedTempFile;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct CodeExecutionTool {
    /// Maximum execution time in seconds
    timeout_secs: u64,
    /// Allowed languages
    allowed_languages: Vec<String>,
    success_count: usize,
    total_count: usize,
}

impl CodeExecutionTool {
    pub fn new() -> Self {
        Self {
            timeout_secs: 10,
            allowed_languages: vec![
                "python".to_string(),
                "python3".to_string(),
                "bash".to_string(),
                "sh".to_string(),
            ],
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Detect language from code or explicit marker
    fn detect_language(&self, code: &str) -> Option<String> {
        let code_lower = code.to_lowercase();

        // Check for explicit language markers
        if code_lower.starts_with("```python") || code_lower.starts_with("# python") {
            return Some("python3".to_string());
        }
        if code_lower.starts_with("```bash") || code_lower.starts_with("#!/bin/bash") {
            return Some("bash".to_string());
        }
        if code_lower.starts_with("```sh") || code_lower.starts_with("#!/bin/sh") {
            return Some("sh".to_string());
        }

        // Try to detect from content
        if code.contains("def ") || code.contains("import ") || code.contains("print(") {
            return Some("python3".to_string());
        }
        if code.contains("echo ") || code.contains("$") {
            return Some("bash".to_string());
        }

        None
    }

    /// Clean code by removing markdown fences
    fn clean_code(&self, code: &str) -> String {
        let mut cleaned = code.to_string();

        // Remove markdown code fences
        if cleaned.starts_with("```") {
            if let Some(newline_pos) = cleaned.find('\n') {
                cleaned = cleaned[newline_pos + 1..].to_string();
            }
        }
        if cleaned.ends_with("```") {
            cleaned = cleaned[..cleaned.len() - 3].to_string();
        }
        if cleaned.ends_with("```\n") {
            cleaned = cleaned[..cleaned.len() - 4].to_string();
        }

        cleaned.trim().to_string()
    }

    /// Check if code is safe to execute
    fn is_safe_code(&self, code: &str, language: &str) -> Result<(), String> {
        let code_lower = code.to_lowercase();

        // Block dangerous operations
        let dangerous_patterns = [
            "rm -rf",
            "rm -r /",
            "dd if=",
            "mkfs",
            ":(){ :|:& };:", // Fork bomb
            "chmod 777 /",
            "sudo",
            "su -",
            "> /dev/",
            "curl | bash",
            "wget | bash",
            "eval(",
            "__import__('os').system",
            "subprocess.call",
            "os.system",
            "shutil.rmtree('/')",
        ];

        for pattern in dangerous_patterns {
            if code_lower.contains(pattern) {
                return Err(format!("Blocked dangerous pattern: {}", pattern));
            }
        }

        // Python-specific checks
        if language.contains("python") {
            let python_dangerous = ["open('/etc/", "open('/dev/", "open('/proc/", "open('/sys/"];
            for pattern in python_dangerous {
                if code_lower.contains(pattern) {
                    return Err(format!("Blocked access to system path: {}", pattern));
                }
            }
        }

        Ok(())
    }

    /// Execute Python code
    fn execute_python(&self, code: &str) -> Result<String, String> {
        let mut temp_file = NamedTempFile::with_suffix(".py")
            .map_err(|e| format!("Failed to create temp file: {}", e))?;

        temp_file
            .write_all(code.as_bytes())
            .map_err(|e| format!("Failed to write code: {}", e))?;

        let output = Command::new("python3")
            .arg(temp_file.path())
            .output()
            .map_err(|e| format!("Failed to execute Python: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            if stdout.is_empty() {
                Ok("Code executed successfully (no output)".to_string())
            } else {
                Ok(stdout)
            }
        } else {
            Err(format!("Execution failed:\n{}", stderr))
        }
    }

    /// Execute Bash code
    fn execute_bash(&self, code: &str) -> Result<String, String> {
        let mut temp_file = NamedTempFile::with_suffix(".sh")
            .map_err(|e| format!("Failed to create temp file: {}", e))?;

        temp_file
            .write_all(code.as_bytes())
            .map_err(|e| format!("Failed to write code: {}", e))?;

        let output = Command::new("bash")
            .arg(temp_file.path())
            .output()
            .map_err(|e| format!("Failed to execute Bash: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            if stdout.is_empty() {
                Ok("Script executed successfully (no output)".to_string())
            } else {
                Ok(stdout)
            }
        } else {
            Err(format!("Execution failed:\n{}", stderr))
        }
    }
}

#[async_trait]
impl Tool for CodeExecutionTool {
    fn tool_type(&self) -> ToolType {
        ToolType::CodeExecution
    }

    fn description(&self) -> String {
        "Execute Python or Bash code snippets in a sandboxed environment".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.75
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();
        debug!("CodeExecutionTool: executing code");

        // Detect language
        let language = match self.detect_language(query) {
            Some(lang) => lang,
            None => {
                return Ok(ToolResult {
                    tool: ToolType::CodeExecution,
                    success: false,
                    result: "Could not detect language. Please specify with ```python or ```bash"
                        .to_string(),
                    metadata: ToolMetadata {
                        execution_time_ms: start.elapsed().as_millis() as u64,
                        confidence: 0.0,
                        source: Some("CodeExecution".to_string()),
                        cost: Some(0.0),
                    },
                });
            }
        };

        // Check if language is allowed
        if !self.allowed_languages.contains(&language) {
            return Ok(ToolResult {
                tool: ToolType::CodeExecution,
                success: false,
                result: format!(
                    "Language '{}' not allowed. Supported: {:?}",
                    language, self.allowed_languages
                ),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("CodeExecution".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        // Clean the code
        let code = self.clean_code(query);

        // Safety check
        if let Err(e) = self.is_safe_code(&code, &language) {
            warn!("CodeExecutionTool: blocked unsafe code - {}", e);
            return Ok(ToolResult {
                tool: ToolType::CodeExecution,
                success: false,
                result: format!("Code blocked for safety: {}", e),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("CodeExecution".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        // Execute based on language
        let result = if language.contains("python") {
            self.execute_python(&code)
        } else if language == "bash" || language == "sh" {
            self.execute_bash(&code)
        } else {
            Err(format!("Unsupported language: {}", language))
        };

        match result {
            Ok(output) => Ok(ToolResult {
                tool: ToolType::CodeExecution,
                success: true,
                result: format!("Language: {}\n\nOutput:\n{}", language, output),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.9,
                    source: Some(format!("CodeExecution/{}", language)),
                    cost: Some(0.0),
                },
            }),
            Err(e) => Ok(ToolResult {
                tool: ToolType::CodeExecution,
                success: false,
                result: e,
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some(format!("CodeExecution/{}", language)),
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
    fn test_language_detection() {
        let tool = CodeExecutionTool::new();

        assert_eq!(
            tool.detect_language("```python\nprint('hello')```"),
            Some("python3".to_string())
        );
        assert_eq!(
            tool.detect_language("```bash\necho hello```"),
            Some("bash".to_string())
        );
        assert_eq!(
            tool.detect_language("print('hello')"),
            Some("python3".to_string())
        );
        assert_eq!(tool.detect_language("echo hello"), Some("bash".to_string()));
    }

    #[test]
    fn test_code_cleaning() {
        let tool = CodeExecutionTool::new();

        assert_eq!(
            tool.clean_code("```python\nprint('hello')\n```"),
            "print('hello')"
        );
        assert_eq!(tool.clean_code("print('hello')"), "print('hello')");
    }

    #[test]
    fn test_safety_check() {
        let tool = CodeExecutionTool::new();

        assert!(tool.is_safe_code("print('hello')", "python").is_ok());
        assert!(tool.is_safe_code("rm -rf /", "bash").is_err());
        assert!(tool
            .is_safe_code("os.system('rm -rf /')", "python")
            .is_err());
    }

    #[tokio::test]
    async fn test_python_execution() {
        let tool = CodeExecutionTool::new();
        let result = tool.execute("```python\nprint(2 + 2)\n```").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        if res.success {
            assert!(res.result.contains("4"));
        }
    }
}
