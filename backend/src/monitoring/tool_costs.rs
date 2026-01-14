// src/monitoring/tool_costs.rs
// Feature #6: Tool cost tracking

use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize)]
pub struct ToolCostStats {
    pub tool_type: String,
    pub total_cost: f32,
    pub executions: usize,
    pub avg_cost: f32,
    pub last_updated: String,
}

struct ToolCostState {
    totals: HashMap<String, InternalCostStats>,
}

#[derive(Debug, Clone)]
struct InternalCostStats {
    total_cost: f32,
    executions: usize,
    last_updated: chrono::DateTime<Utc>,
}

impl Default for ToolCostState {
    fn default() -> Self {
        Self {
            totals: HashMap::new(),
        }
    }
}

static COST_STATE: OnceLock<Mutex<ToolCostState>> = OnceLock::new();

fn get_state() -> &'static Mutex<ToolCostState> {
    COST_STATE.get_or_init(|| Mutex::new(ToolCostState::default()))
}

pub fn record_tool_cost(tool_type: &str, cost: f32) {
    if cost <= 0.0 {
        return;
    }

    if let Ok(mut state) = get_state().lock() {
        let entry = state
            .totals
            .entry(tool_type.to_string())
            .or_insert(InternalCostStats {
                total_cost: 0.0,
                executions: 0,
                last_updated: Utc::now(),
            });
        entry.total_cost += cost;
        entry.executions += 1;
        entry.last_updated = Utc::now();
    }
}

pub fn get_tool_costs() -> Vec<ToolCostStats> {
    if let Ok(state) = get_state().lock() {
        state
            .totals
            .iter()
            .map(|(tool_type, stats)| ToolCostStats {
                tool_type: tool_type.clone(),
                total_cost: stats.total_cost,
                executions: stats.executions,
                avg_cost: if stats.executions > 0 {
                    stats.total_cost / stats.executions as f32
                } else {
                    0.0
                },
                last_updated: stats.last_updated.to_rfc3339(),
            })
            .collect()
    } else {
        vec![]
    }
}
