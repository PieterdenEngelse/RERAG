// src/tools/tool_chain_executor.rs
// Features #1, #2, #11: Tool chaining API, retry with backoff, parallel execution

use crate::tools::tool_cache::{cache_result, get_cached};
use crate::tools::tool_composer::{ChainPlan, ExecutionStep, ToolChain, ToolComposer};
use crate::tools::tool_executor::ToolExecutor;
use crate::tools::tool_permissions::{
    check_chain_permission, check_parallel_permission, check_permission,
};
use crate::tools::tool_rate_limiter::{check_rate_limit, RateLimitResult};
use crate::tools::{ToolResult, ToolType};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Configuration for chain execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainExecutionConfig {
    /// Maximum number of steps in a chain
    pub max_steps: usize,
    /// Maximum total execution time in seconds
    pub max_execution_time_secs: u64,
    /// Enable parallel execution for independent steps
    pub enable_parallel: bool,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// API key for permission checks
    pub api_key: Option<String>,
}

impl Default for ChainExecutionConfig {
    fn default() -> Self {
        Self {
            max_steps: 10,
            max_execution_time_secs: 60,
            enable_parallel: true,
            retry_config: RetryConfig::default(),
            api_key: None,
        }
    }
}

/// Retry configuration with exponential backoff (#2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: usize,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Whether to retry on rate limit
    pub retry_on_rate_limit: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            retry_on_rate_limit: true,
        }
    }
}

/// Result of chain execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainExecutionResult {
    pub success: bool,
    pub chain: ToolChain,
    pub error: Option<String>,
    pub total_execution_time_ms: u64,
    pub steps_executed: usize,
    pub steps_from_cache: usize,
    pub parallel_groups: usize,
    pub retries_used: usize,
}

/// Chain executor with advanced features
pub struct ToolChainExecutor;

impl ToolChainExecutor {
    /// Execute a complete tool chain
    pub async fn execute_chain(
        query: &str,
        config: ChainExecutionConfig,
    ) -> Result<ChainExecutionResult, String> {
        let start = Instant::now();
        let mut retries_used = 0;
        let mut steps_from_cache = 0;

        // Plan the chain
        let plan = ToolComposer::plan_chain(query);

        // Check chain permission
        let perm_result =
            check_chain_permission(config.api_key.as_deref(), plan.total_planned_steps);
        if !perm_result.is_allowed() {
            return Err(format!("Chain permission denied: {:?}", perm_result));
        }

        // Validate chain length
        if plan.total_planned_steps > config.max_steps {
            return Err(format!(
                "Chain too long: {} steps (max: {})",
                plan.total_planned_steps, config.max_steps
            ));
        }

        info!(
            query = query,
            steps = plan.total_planned_steps,
            "Starting chain execution"
        );

        // Create chain from plan
        let mut chain = ToolComposer::create_chain_from_plan(&plan);

        // Identify parallel groups
        let parallel_groups =
            if config.enable_parallel && check_parallel_permission(config.api_key.as_deref()) {
                Self::identify_parallel_groups(&plan)
            } else {
                // Sequential execution - each step is its own group
                (0..plan.total_planned_steps).map(|i| vec![i]).collect()
            };

        let num_parallel_groups = parallel_groups.len();

        // Execute groups
        let mut previous_result: Option<String> = None;
        let mut previous_tool: Option<ToolType> = None;

        for group in parallel_groups {
            // Check timeout
            if start.elapsed().as_secs() > config.max_execution_time_secs {
                return Err("Chain execution timeout".to_string());
            }

            if group.len() == 1 {
                // Sequential execution
                let step_idx = group[0];
                let step = &mut chain.steps[step_idx];
                if let Some(prev_tool) = previous_tool.as_ref() {
                    crate::monitoring::record_tool_dependency(prev_tool, &step.tool);
                }

                let (result, from_cache, retries) =
                    Self::execute_step_with_retry(step, previous_result.as_deref(), &config)
                        .await?;

                step.result = Some(result.result.clone());
                step.execution_time_ms = result.metadata.execution_time_ms;
                step.confidence = result.metadata.confidence;

                if from_cache {
                    steps_from_cache += 1;
                }
                retries_used += retries;
                previous_tool = Some(step.tool.clone());
                previous_result = Some(result.result);
            } else {
                // Parallel execution (#11)
                if let Some(prev_tool) = previous_tool.as_ref() {
                    for &step_idx in &group {
                        crate::monitoring::record_tool_dependency(
                            prev_tool,
                            &chain.steps[step_idx].tool,
                        );
                    }
                }

                let futures: Vec<_> =
                    group
                        .iter()
                        .map(|&step_idx| {
                            let step = chain.steps[step_idx].clone();
                            let prev = previous_result.clone();
                            let cfg = config.clone();
                            async move {
                                Self::execute_step_with_retry(&step, prev.as_deref(), &cfg).await
                            }
                        })
                        .collect();

                let results = join_all(futures).await;

                for (i, result) in results.into_iter().enumerate() {
                    let step_idx = group[i];
                    match result {
                        Ok((tool_result, from_cache, retries)) => {
                            chain.steps[step_idx].result = Some(tool_result.result.clone());
                            chain.steps[step_idx].execution_time_ms =
                                tool_result.metadata.execution_time_ms;
                            chain.steps[step_idx].confidence = tool_result.metadata.confidence;
                            if from_cache {
                                steps_from_cache += 1;
                            }
                            retries_used += retries;
                        }
                        Err(e) => {
                            warn!(step = step_idx, error = %e, "Parallel step failed");
                            chain.steps[step_idx].result = Some(format!("Error: {}", e));
                            chain.steps[step_idx].confidence = 0.0;
                        }
                    }
                }

                // Combine results for next group
                let combined: Vec<String> = group
                    .iter()
                    .filter_map(|&idx| chain.steps[idx].result.clone())
                    .collect();
                previous_result = Some(combined.join("\n"));
                previous_tool = group.last().map(|idx| chain.steps[*idx].tool.clone());
            }
        }

        // Calculate final metrics
        let total_time = start.elapsed().as_millis() as u64;
        let confidences: Vec<f32> = chain.steps.iter().map(|s| s.confidence).collect();
        chain.total_confidence = ToolComposer::calculate_aggregate_confidence(&confidences);
        chain.total_execution_time_ms = total_time;
        chain.final_answer = ToolComposer::compose_answer(&chain);

        let steps_executed = chain.steps.iter().filter(|s| s.result.is_some()).count();

        info!(
            steps = steps_executed,
            from_cache = steps_from_cache,
            retries = retries_used,
            time_ms = total_time,
            "Chain execution completed"
        );

        Ok(ChainExecutionResult {
            success: true,
            chain,
            error: None,
            total_execution_time_ms: total_time,
            steps_executed,
            steps_from_cache,
            parallel_groups: num_parallel_groups,
            retries_used,
        })
    }

    /// Execute a single step with retry and caching
    async fn execute_step_with_retry(
        step: &ExecutionStep,
        previous_result: Option<&str>,
        config: &ChainExecutionConfig,
    ) -> Result<(ToolResult, bool, usize), String> {
        let tool_type_str = step.tool.to_string();

        // Check permission
        let perm = check_permission(config.api_key.as_deref(), &tool_type_str);
        if !perm.is_allowed() {
            return Err(format!("Permission denied for tool: {}", tool_type_str));
        }

        // Check cache first
        if let Some(cached) = get_cached(&tool_type_str, &step.query) {
            debug!(tool = %tool_type_str, "Using cached result");
            return Ok((cached, true, 0));
        }

        // Execute with retry
        let mut retries = 0;
        let mut delay = Duration::from_millis(config.retry_config.initial_delay_ms);

        loop {
            // Check rate limit
            match check_rate_limit(&tool_type_str) {
                RateLimitResult::Allowed => {}
                RateLimitResult::Limited { retry_after, .. } => {
                    if config.retry_config.retry_on_rate_limit
                        && retries < config.retry_config.max_retries
                    {
                        debug!(
                            tool = %tool_type_str,
                            retry_after_ms = retry_after.as_millis(),
                            "Rate limited, waiting"
                        );
                        sleep(retry_after).await;
                        retries += 1;
                        continue;
                    } else {
                        return Err(format!("Rate limit exceeded for tool: {}", tool_type_str));
                    }
                }
            }

            // Execute tool
            match ToolExecutor::execute_tool(&step.tool, &step.query, previous_result).await {
                Ok(result) => {
                    // Cache successful result
                    if result.success {
                        cache_result(&tool_type_str, &step.query, &result);
                    }
                    return Ok((result, false, retries));
                }
                Err(e) => {
                    if retries < config.retry_config.max_retries {
                        warn!(
                            tool = %tool_type_str,
                            retry = retries + 1,
                            error = %e,
                            "Tool execution failed, retrying"
                        );
                        sleep(delay).await;
                        delay = Duration::from_millis(
                            (delay.as_millis() as f64 * config.retry_config.backoff_multiplier)
                                .min(config.retry_config.max_delay_ms as f64)
                                as u64,
                        );
                        retries += 1;
                    } else {
                        return Err(format!(
                            "Tool {} failed after {} retries: {}",
                            tool_type_str, retries, e
                        ));
                    }
                }
            }
        }
    }

    /// Identify groups of steps that can run in parallel
    fn identify_parallel_groups(plan: &ChainPlan) -> Vec<Vec<usize>> {
        if !plan.is_multi_step || plan.planned_steps.len() <= 1 {
            return (0..plan.planned_steps.len()).map(|i| vec![i]).collect();
        }

        // Simple heuristic: steps with same "level" can run in parallel
        // For now, we identify independent steps by checking if they don't
        // depend on each other's output

        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut current_group: Vec<usize> = Vec::new();

        for (i, step) in plan.planned_steps.iter().enumerate() {
            // Check if this step depends on previous results
            let depends_on_previous = step.purpose.to_lowercase().contains("result")
                || step.purpose.to_lowercase().contains("previous")
                || step.purpose.to_lowercase().contains("then");

            if depends_on_previous && !current_group.is_empty() {
                groups.push(current_group);
                current_group = vec![i];
            } else {
                current_group.push(i);
            }
        }

        if !current_group.is_empty() {
            groups.push(current_group);
        }

        groups
    }

    /// Execute a single tool with all features (rate limit, cache, retry)
    pub async fn execute_single(
        tool_type: &ToolType,
        query: &str,
        config: Option<ChainExecutionConfig>,
    ) -> Result<ToolResult, String> {
        let config = config.unwrap_or_default();
        let tool_type_str = tool_type.to_string();

        // Check permission
        let perm = check_permission(config.api_key.as_deref(), &tool_type_str);
        if !perm.is_allowed() {
            return Err(format!("Permission denied for tool: {}", tool_type_str));
        }

        // Check cache
        if let Some(cached) = get_cached(&tool_type_str, query) {
            return Ok(cached);
        }

        // Check rate limit
        match check_rate_limit(&tool_type_str) {
            RateLimitResult::Allowed => {}
            RateLimitResult::Limited { retry_after, .. } => {
                if config.retry_config.retry_on_rate_limit {
                    sleep(retry_after).await;
                } else {
                    return Err("Rate limit exceeded".to_string());
                }
            }
        }

        // Execute with retry
        let step = ExecutionStep {
            step: 1,
            tool: tool_type.clone(),
            query: query.to_string(),
            formatted_query: None,
            result: None,
            confidence: 0.0,
            execution_time_ms: 0,
            metadata_extra: None,
        };

        let (result, _, _) = Self::execute_step_with_retry(&step, None, &config).await?;
        Ok(result)
    }
}

/// Request for chain execution API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainExecutionRequest {
    pub query: String,
    #[serde(default)]
    pub max_steps: Option<usize>,
    #[serde(default)]
    pub enable_parallel: Option<bool>,
    #[serde(default)]
    pub max_retries: Option<usize>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

impl ChainExecutionRequest {
    pub fn to_config(&self) -> ChainExecutionConfig {
        let mut config = ChainExecutionConfig::default();
        if let Some(max_steps) = self.max_steps {
            config.max_steps = max_steps;
        }
        if let Some(enable_parallel) = self.enable_parallel {
            config.enable_parallel = enable_parallel;
        }
        if let Some(max_retries) = self.max_retries {
            config.retry_config.max_retries = max_retries;
        }
        if let Some(timeout) = self.timeout_secs {
            config.max_execution_time_secs = timeout;
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_chain() {
        let result =
            ToolChainExecutor::execute_chain("Calculate 5 + 3", ChainExecutionConfig::default())
                .await;

        assert!(result.is_ok());
        let chain_result = result.unwrap();
        assert!(chain_result.success);
        assert!(chain_result.steps_executed > 0);
    }

    #[tokio::test]
    async fn test_single_execution() {
        let result = ToolChainExecutor::execute_single(&ToolType::Calculator, "10 * 5", None).await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_parallel_groups() {
        let plan = ToolComposer::plan_chain("Find papers and calculate 5 + 3");
        let groups = ToolChainExecutor::identify_parallel_groups(&plan);
        assert!(!groups.is_empty());
    }
}
