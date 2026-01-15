// src/tools/calculator.rs - PRODUCTION
// Phase 9: Calculator Tool Implementation

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CalculatorTool {
    success_count: usize,
    total_count: usize,
}

impl CalculatorTool {
    pub fn new() -> Self {
        Self {
            success_count: 0,
            total_count: 0,
        }
    }

    fn evaluate_expression(&self, expr: &str) -> Result<String, String> {
        let expr = expr.trim();

        // Handle standalone numbers first
        if let Ok(num) = expr.parse::<f64>() {
            return Ok(num.to_string());
        }

        // Tokenize the expression
        let tokens = self.tokenize(expr)?;
        
        // Evaluate with proper precedence (shunting-yard style)
        let result = self.evaluate_tokens(&tokens)?;
        Ok(result.to_string())
    }
    
    fn tokenize(&self, expr: &str) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        let mut num_buf = String::new();
        
        for ch in expr.chars() {
            if ch.is_whitespace() {
                if !num_buf.is_empty() {
                    tokens.push(Token::Number(num_buf.parse().map_err(|_| "Invalid number")?));
                    num_buf.clear();
                }
                continue;
            }
            
            if ch.is_ascii_digit() || ch == '.' || (ch == '-' && num_buf.is_empty() && (tokens.is_empty() || matches!(tokens.last(), Some(Token::Op(_))))) {
                num_buf.push(ch);
            } else if "+-*/".contains(ch) {
                if !num_buf.is_empty() {
                    tokens.push(Token::Number(num_buf.parse().map_err(|_| "Invalid number")?));
                    num_buf.clear();
                }
                tokens.push(Token::Op(ch));
            } else {
                return Err(format!("Invalid character: {}", ch));
            }
        }
        
        if !num_buf.is_empty() {
            tokens.push(Token::Number(num_buf.parse().map_err(|_| "Invalid number")?));
        }
        
        Ok(tokens)
    }
    
    fn evaluate_tokens(&self, tokens: &[Token]) -> Result<f64, String> {
        if tokens.is_empty() {
            return Err("Empty expression".to_string());
        }
        
        // First pass: handle * and /
        let mut intermediate: Vec<Token> = Vec::new();
        let mut i = 0;
        
        while i < tokens.len() {
            match &tokens[i] {
                Token::Op('*') | Token::Op('/') => {
                    let op = if let Token::Op(c) = tokens[i] { c } else { unreachable!() };
                    let left = match intermediate.pop() {
                        Some(Token::Number(n)) => n,
                        _ => return Err("Invalid expression".to_string()),
                    };
                    let right = match tokens.get(i + 1) {
                        Some(Token::Number(n)) => *n,
                        _ => return Err("Invalid expression".to_string()),
                    };
                    let result = if op == '*' { left * right } else {
                        if right == 0.0 { return Err("Division by zero".to_string()); }
                        left / right
                    };
                    intermediate.push(Token::Number(result));
                    i += 2;
                }
                other => {
                    intermediate.push(other.clone());
                    i += 1;
                }
            }
        }
        
        // Second pass: handle + and -
        let mut result = match intermediate.first() {
            Some(Token::Number(n)) => *n,
            _ => return Err("Invalid expression".to_string()),
        };
        
        let mut j = 1;
        while j < intermediate.len() {
            match (&intermediate[j], intermediate.get(j + 1)) {
                (Token::Op('+'), Some(Token::Number(n))) => {
                    result += n;
                    j += 2;
                }
                (Token::Op('-'), Some(Token::Number(n))) => {
                    result -= n;
                    j += 2;
                }
                _ => return Err("Invalid expression".to_string()),
            }
        }
        
        Ok(result)
    }
}

#[derive(Debug, Clone)]
enum Token {
    Number(f64),
    Op(char),
}

#[async_trait]
impl Tool for CalculatorTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Calculator
    }

    fn description(&self) -> String {
        "Perform mathematical calculations and arithmetic operations".to_string()
    }

    fn success_rate(&self) -> f32 {
        if self.total_count == 0 {
            0.95
        } else {
            self.success_count as f32 / self.total_count as f32
        }
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let start = Instant::now();

        match self.evaluate_expression(query) {
            Ok(result) => Ok(ToolResult {
                tool: ToolType::Calculator,
                success: true,
                result: format!("{} = {}", query, result),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.99,
                    source: Some("Calculator".to_string()),
                    cost: Some(0.0),
                },
            }),
            Err(_) => Ok(ToolResult {
                tool: ToolType::Calculator,
                success: false,
                result: format!("Could not evaluate: {}", query),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("Calculator".to_string()),
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
    async fn test_calculator_add() {
        let tool = CalculatorTool::new();
        let result = tool.execute("5 + 3").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("8"));
    }

    #[tokio::test]
    async fn test_calculator_multiply() {
        let tool = CalculatorTool::new();
        let result = tool.execute("6 * 7").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("42"));
    }

    #[tokio::test]
    async fn test_calculator_standalone_number() {
        let tool = CalculatorTool::new();
        let result = tool.execute("5").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.success);
        assert!(res.result.contains("5"));
    }
}
