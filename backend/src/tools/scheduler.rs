// src/tools/scheduler.rs
// Feature #17: SchedulerTool - lightweight task/reminder scheduling

use async_trait::async_trait;
use chrono::{DateTime, Duration, Local, NaiveTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

use crate::tools::{Tool, ToolMetadata, ToolResult, ToolType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub description: String,
    pub scheduled_for: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

static TASKS: OnceLock<Mutex<Vec<ScheduledTask>>> = OnceLock::new();

fn tasks_store() -> &'static Mutex<Vec<ScheduledTask>> {
    TASKS.get_or_init(|| Mutex::new(Vec::new()))
}

pub struct SchedulerTool {
    success_rate: f32,
}

impl SchedulerTool {
    pub fn new() -> Self {
        Self { success_rate: 0.9 }
    }

    fn schedule_task(&self, description: &str, when: DateTime<Utc>) -> ScheduledTask {
        let task = ScheduledTask {
            id: Uuid::new_v4().to_string(),
            description: description.trim().to_string(),
            scheduled_for: when,
            created_at: Utc::now(),
        };
        if let Ok(mut tasks) = tasks_store().lock() {
            tasks.push(task.clone());
            tasks.sort_by_key(|t| t.scheduled_for);
        }
        task
    }

    fn list_tasks(&self, limit: usize) -> Vec<ScheduledTask> {
        tasks_store()
            .lock()
            .map(|tasks| tasks.iter().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    fn parse_schedule(&self, query: &str) -> (String, DateTime<Utc>) {
        let lower = query.to_lowercase();
        let default_description = query.trim().to_string();
        let now = Utc::now();

        if let Some(idx) = lower.find(" in ") {
            let desc = query[..idx].trim();
            let remainder = query[idx + 4..].trim();
            if let Some(dt) = parse_relative_time(remainder) {
                return (
                    if desc.is_empty() {
                        default_description
                    } else {
                        desc.to_string()
                    },
                    now + dt,
                );
            }
        }

        if let Some(idx) = lower.find(" at ") {
            let (prefix, time_part) = query.split_at(idx);
            let desc = prefix.trim();
            let time_str = time_part[4..].trim();
            if let Some(dt) = parse_time_of_day(&lower, time_str) {
                return (
                    if desc.is_empty() {
                        default_description
                    } else {
                        desc.to_string()
                    },
                    dt,
                );
            }
        }

        // default: schedule one hour from now
        (default_description, now + Duration::minutes(60))
    }
}

fn parse_relative_time(text: &str) -> Option<Duration> {
    let re = Regex::new(r"(?i)(\d+)\s*(minute|minutes|hour|hours|day|days)").ok()?;
    let caps = re.captures(text)?;
    let value: i64 = caps.get(1)?.as_str().parse().ok()?;
    let unit = caps.get(2)?.as_str().to_lowercase();
    match unit.as_str() {
        "minute" | "minutes" => Some(Duration::minutes(value)),
        "hour" | "hours" => Some(Duration::hours(value)),
        "day" | "days" => Some(Duration::days(value)),
        _ => None,
    }
}

fn parse_time_of_day(lower_query: &str, time_str: &str) -> Option<DateTime<Utc>> {
    let clean = time_str.trim();
    let base_date = if lower_query.contains("tomorrow") {
        Local::now().date_naive() + chrono::Days::new(1)
    } else {
        Local::now().date_naive()
    };

    let parsed = NaiveTime::parse_from_str(clean, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(clean, "%H"))
        .ok()?;
    let local_dt = base_date.and_time(parsed);
    local_dt
        .and_local_timezone(Local)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
}

#[async_trait]
impl Tool for SchedulerTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Scheduler
    }

    fn description(&self) -> String {
        "Schedule lightweight reminders such as 'schedule standup in 30 minutes' or list upcoming tasks.".to_string()
    }

    fn success_rate(&self) -> f32 {
        self.success_rate
    }

    async fn execute(&self, query: &str) -> Result<ToolResult, String> {
        let lower = query.to_lowercase();
        let start = std::time::Instant::now();

        let response = if lower.contains("list") || lower.contains("show") {
            let tasks = self.list_tasks(10);
            if tasks.is_empty() {
                "No scheduled tasks. Use 'schedule <task> in 30 minutes' to add one.".to_string()
            } else {
                let mut lines = Vec::new();
                for task in tasks {
                    lines.push(format!(
                        "{} – {}",
                        task.scheduled_for.format("%Y-%m-%d %H:%M UTC"),
                        task.description
                    ));
                }
                lines.join("\n")
            }
        } else {
            let (description, when) = self.parse_schedule(query);
            let task = self.schedule_task(&description, when);
            format!(
                "Scheduled '{}' at {} (id: {}). Use 'schedule list' to review.",
                task.description,
                task.scheduled_for.format("%Y-%m-%d %H:%M UTC"),
                task.id
            )
        };

        Ok(ToolResult {
            tool: ToolType::Scheduler,
            success: true,
            result: response,
            metadata: ToolMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                confidence: 0.9,
                source: Some("in-memory-scheduler".to_string()),
                cost: Some(0.0),
            },
        })
    }

    fn update_success(&mut self, success: bool) {
        if success {
            self.success_rate = (self.success_rate * 0.95) + 0.05;
        } else {
            self.success_rate *= 0.95;
        }
    }
}
