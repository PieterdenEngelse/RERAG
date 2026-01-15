// src/monitoring/tool_dependencies.rs
// Feature #7: Tool dependency graph tracking

use crate::tools::ToolType;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize)]
pub struct ToolDependencyNode {
    pub tool_type: String,
    pub executions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDependencyEdge {
    pub from: String,
    pub to: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDependencyGraph {
    pub nodes: Vec<ToolDependencyNode>,
    pub edges: Vec<ToolDependencyEdge>,
}

struct DependencyState {
    edges: HashMap<(String, String), usize>,
    nodes: HashMap<String, usize>,
}

impl Default for DependencyState {
    fn default() -> Self {
        Self {
            edges: HashMap::new(),
            nodes: HashMap::new(),
        }
    }
}

static DEP_STATE: OnceLock<Mutex<DependencyState>> = OnceLock::new();

fn get_state() -> &'static Mutex<DependencyState> {
    DEP_STATE.get_or_init(|| Mutex::new(DependencyState::default()))
}

pub fn record_tool_dependency(from: &ToolType, to: &ToolType) {
    if from == to {
        return;
    }

    if let Ok(mut state) = get_state().lock() {
        let from_name = from.to_string();
        let to_name = to.to_string();
        *state
            .edges
            .entry((from_name.clone(), to_name.clone()))
            .or_insert(0) += 1;
        *state.nodes.entry(from_name).or_insert(0) += 1;
        *state.nodes.entry(to_name).or_insert(0) += 1;
    }
}

/// Record tool dependency using string names (for use outside of ToolType enum)
pub fn record_tool_dependency_str(from: &str, to: &str) {
    if from == to {
        return;
    }

    if let Ok(mut state) = get_state().lock() {
        *state
            .edges
            .entry((from.to_string(), to.to_string()))
            .or_insert(0) += 1;
        *state.nodes.entry(from.to_string()).or_insert(0) += 1;
        *state.nodes.entry(to.to_string()).or_insert(0) += 1;
    }
}

pub fn get_tool_dependency_graph() -> ToolDependencyGraph {
    if let Ok(state) = get_state().lock() {
        let nodes = state
            .nodes
            .iter()
            .map(|(tool_type, executions)| ToolDependencyNode {
                tool_type: tool_type.clone(),
                executions: *executions,
            })
            .collect();

        let edges = state
            .edges
            .iter()
            .map(|((from, to), count)| ToolDependencyEdge {
                from: from.clone(),
                to: to.clone(),
                count: *count,
            })
            .collect();

        ToolDependencyGraph { nodes, edges }
    } else {
        ToolDependencyGraph {
            nodes: vec![],
            edges: vec![],
        }
    }
}
