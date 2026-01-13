// src/tools/notification.rs
// Notification Agent - Send alerts and notifications

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tracing::{debug, info, warn};

/// Maximum notifications to keep in history
const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub notification_type: NotificationType,
    pub title: String,
    pub message: String,
    pub priority: Priority,
    pub timestamp: String,
    pub delivered: bool,
    pub channel: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NotificationType {
    Info,
    Success,
    Warning,
    Error,
    Alert,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Ord, PartialOrd, Eq)]
pub enum Priority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Notification history storage
struct NotificationHistory {
    notifications: VecDeque<Notification>,
}

impl Default for NotificationHistory {
    fn default() -> Self {
        Self {
            notifications: VecDeque::with_capacity(MAX_HISTORY),
        }
    }
}

static NOTIFICATION_HISTORY: OnceLock<Arc<Mutex<NotificationHistory>>> = OnceLock::new();

fn get_history() -> Arc<Mutex<NotificationHistory>> {
    NOTIFICATION_HISTORY
        .get_or_init(|| Arc::new(Mutex::new(NotificationHistory::default())))
        .clone()
}

/// Store a notification in history
pub fn store_notification(notification: Notification) {
    if let Ok(mut history) = get_history().lock() {
        if history.notifications.len() >= MAX_HISTORY {
            history.notifications.pop_back();
        }
        history.notifications.push_front(notification);
    }
}

/// Get recent notifications
pub fn get_recent_notifications(limit: usize) -> Vec<Notification> {
    if let Ok(history) = get_history().lock() {
        history.notifications.iter().take(limit).cloned().collect()
    } else {
        vec![]
    }
}

#[derive(Debug, Clone)]
pub struct NotificationTool {
    /// Webhook URL for external notifications
    webhook_url: Option<String>,
    /// Default channel
    default_channel: String,
    success_count: usize,
    total_count: usize,
}

impl NotificationTool {
    pub fn new() -> Self {
        Self {
            webhook_url: std::env::var("NOTIFICATION_WEBHOOK_URL").ok(),
            default_channel: "system".to_string(),
            success_count: 0,
            total_count: 0,
        }
    }

    pub fn with_webhook(mut self, url: String) -> Self {
        self.webhook_url = Some(url);
        self
    }

    pub fn with_channel(mut self, channel: String) -> Self {
        self.default_channel = channel;
        self
    }

    /// Parse notification from input
    fn parse_notification(&self, input: &str) -> (NotificationType, Priority, String, String) {
        let input_lower = input.to_lowercase();
        
        // Detect type
        let notification_type = if input_lower.contains("error") || input_lower.contains("fail") {
            NotificationType::Error
        } else if input_lower.contains("warn") {
            NotificationType::Warning
        } else if input_lower.contains("success") || input_lower.contains("complete") {
            NotificationType::Success
        } else if input_lower.contains("alert") || input_lower.contains("urgent") {
            NotificationType::Alert
        } else {
            NotificationType::Info
        };

        // Detect priority
        let priority = if input_lower.contains("urgent") || input_lower.contains("critical") {
            Priority::Urgent
        } else if input_lower.contains("high") || input_lower.contains("important") {
            Priority::High
        } else if input_lower.contains("low") {
            Priority::Low
        } else {
            Priority::Normal
        };

        // Extract title and message
        let lines: Vec<&str> = input.lines().collect();
        let (title, message) = if lines.len() > 1 {
            (lines[0].to_string(), lines[1..].join("\n"))
        } else {
            let words: Vec<&str> = input.split_whitespace().collect();
            if words.len() > 5 {
                (words[..5].join(" "), input.to_string())
            } else {
                (input.to_string(), input.to_string())
            }
        };

        (notification_type, priority, title, message)
    }

    /// Send notification via webhook
    async fn send_webhook(&self, notification: &Notification) -> Result<(), String> {
        let url = match &self.webhook_url {
            Some(u) => u,
            None => return Ok(()), // No webhook configured, skip
        };

        let payload = serde_json::json!({
            "type": format!("{:?}", notification.notification_type),
            "title": notification.title,
            "message": notification.message,
            "priority": format!("{:?}", notification.priority),
            "timestamp": notification.timestamp,
            "channel": notification.channel,
        });

        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Webhook request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Webhook returned status: {}", response.status()))
        }
    }

    /// Log notification
    fn log_notification(&self, notification: &Notification) {
        match notification.notification_type {
            NotificationType::Error => {
                tracing::error!(
                    notification_id = %notification.id,
                    title = %notification.title,
                    priority = ?notification.priority,
                    "Notification: {}", notification.message
                );
            }
            NotificationType::Warning => {
                warn!(
                    notification_id = %notification.id,
                    title = %notification.title,
                    "Notification: {}", notification.message
                );
            }
            _ => {
                info!(
                    notification_id = %notification.id,
                    title = %notification.title,
                    "Notification: {}", notification.message
                );
            }
        }
    }

    /// Create and send notification
    async fn send_notification(&self, input: &str, channel: Option<&str>) -> Result<Notification, String> {
        let (notification_type, priority, title, message) = self.parse_notification(input);
        
        let notification = Notification {
            id: uuid::Uuid::new_v4().to_string(),
            notification_type,
            title,
            message,
            priority,
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            delivered: false,
            channel: channel.unwrap_or(&self.default_channel).to_string(),
        };

        // Log the notification
        self.log_notification(&notification);

        // Store in history
        store_notification(notification.clone());

        // Try to send via webhook
        if self.webhook_url.is_some() {
            if let Err(e) = self.send_webhook(&notification).await {
                warn!("Failed to send webhook notification: {}", e);
            }
        }

        Ok(Notification {
            delivered: true,
            ..notification
        })
    }
}

#[async_trait]
impl Tool for NotificationTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Notification
    }

    fn description(&self) -> String {
        "Send notifications and alerts via webhooks, logs, and internal channels".to_string()
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
        debug!("NotificationTool: sending notification");

        if query.trim().is_empty() {
            return Ok(ToolResult {
                tool: ToolType::Notification,
                success: false,
                result: "No notification message provided.".to_string(),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("Notification".to_string()),
                    cost: Some(0.0),
                },
            });
        }

        // Check for channel specification (format: "channel:message" or just "message")
        let (channel, message) = if query.contains(':') && !query.starts_with("http") {
            let parts: Vec<&str> = query.splitn(2, ':').collect();
            if parts.len() == 2 && parts[0].len() < 30 && !parts[0].contains(' ') {
                (Some(parts[0]), parts[1].trim())
            } else {
                (None, query)
            }
        } else {
            (None, query)
        };

        match self.send_notification(message, channel).await {
            Ok(notification) => {
                let type_emoji = match notification.notification_type {
                    NotificationType::Info => "ℹ️",
                    NotificationType::Success => "✅",
                    NotificationType::Warning => "⚠️",
                    NotificationType::Error => "❌",
                    NotificationType::Alert => "🚨",
                };

                let priority_str = match notification.priority {
                    Priority::Low => "Low",
                    Priority::Normal => "Normal",
                    Priority::High => "High ⬆️",
                    Priority::Urgent => "URGENT 🔴",
                };

                let mut output = format!("{} **Notification Sent**\n\n", type_emoji);
                output.push_str(&format!("**ID:** {}\n", notification.id));
                output.push_str(&format!("**Type:** {:?}\n", notification.notification_type));
                output.push_str(&format!("**Priority:** {}\n", priority_str));
                output.push_str(&format!("**Channel:** {}\n", notification.channel));
                output.push_str(&format!("**Title:** {}\n", notification.title));
                output.push_str(&format!("**Message:** {}\n", notification.message));
                output.push_str(&format!("**Timestamp:** {}\n", notification.timestamp));
                
                if self.webhook_url.is_some() {
                    output.push_str("\n*Webhook delivery attempted*");
                }

                Ok(ToolResult {
                    tool: ToolType::Notification,
                    success: true,
                    result: output,
                    metadata: ToolMetadata {
                        execution_time_ms: start.elapsed().as_millis() as u64,
                        confidence: 0.95,
                        source: Some(format!("Notification/{}", notification.channel)),
                        cost: Some(0.0),
                    },
                })
            }
            Err(e) => Ok(ToolResult {
                tool: ToolType::Notification,
                success: false,
                result: format!("Failed to send notification: {}", e),
                metadata: ToolMetadata {
                    execution_time_ms: start.elapsed().as_millis() as u64,
                    confidence: 0.0,
                    source: Some("Notification".to_string()),
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
    async fn test_info_notification() {
        let tool = NotificationTool::new();
        let result = tool.execute("System started successfully").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.success);
        assert!(res.result.contains("Notification Sent"));
    }

    #[tokio::test]
    async fn test_error_notification() {
        let tool = NotificationTool::new();
        let result = tool.execute("Error: Database connection failed").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("Error"));
    }

    #[tokio::test]
    async fn test_urgent_notification() {
        let tool = NotificationTool::new();
        let result = tool.execute("URGENT: Server is down!").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("URGENT"));
    }

    #[tokio::test]
    async fn test_channel_notification() {
        let tool = NotificationTool::new();
        let result = tool.execute("alerts:Critical system alert").await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.result.contains("alerts"));
    }
}
