// src/api/agentic_monitor_routes.rs
// Agentic Monitoring API Endpoints
// Provides stats for the frontend Agentic monitoring dashboard

use actix_web::{web, HttpResponse, Result as ActixResult};
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::monitoring::tool_alerts;
use crate::monitoring::tool_alerts::ToolAlertStatus;
use crate::monitoring::tool_trends::{self, TimeWindow};
use crate::tools::tool_cache::ToolCacheStats;
use crate::tools::tool_rate_limiter::{get_all_rate_limit_status, ToolRateLimitStatus};

// ============ Response Types ============

#[derive(Debug, Serialize)]
pub struct AgentStatsResponse {
    pub active_agents: usize,
    pub agent_names: Vec<String>,
    pub episodes_total: usize,
    pub episodes_last_hour: usize,
    pub success_rate: f64,
    pub active_goals: usize,
    pub completed_goals: usize,
    pub failed_goals: usize,
    pub total_reflections: usize,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct EpisodeEntry {
    pub id: String,
    pub agent_id: String,
    pub query: String,
    pub response: String,
    pub context_chunks_used: usize,
    pub success: bool,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct EpisodesResponse {
    pub episodes: Vec<EpisodeEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct GoalEntry {
    pub id: String,
    pub agent_id: String,
    pub goal: String,
    pub status: String,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct GoalsResponse {
    pub goals: Vec<GoalEntry>,
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
pub struct ReflectionEntry {
    pub id: String,
    pub agent_id: String,
    pub reflection_type: String,
    pub insight: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct ReflectionsResponse {
    pub reflections: Vec<ReflectionEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct MemoryStatsResponse {
    pub total_episodes: usize,
    pub total_rag_memories: usize,
    pub unique_agents: usize,
    pub oldest_episode_timestamp: Option<i64>,
    pub newest_episode_timestamp: Option<i64>,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ToolStatsResponse {
    pub tool_executions: usize,
    pub avg_confidence: f64,
    pub fallback_rate: f64,
    pub tool_distribution: Vec<ToolUsageEntry>,
    pub llm_latency: crate::monitoring::LlmLatencyStats,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ToolUsageEntry {
    pub tool_name: String,
    pub count: usize,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct ToolCacheResponse {
    pub request_id: String,
    pub caches: Vec<ToolCacheStats>,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ToolRateLimitResponse {
    pub request_id: String,
    pub statuses: Vec<ToolRateLimitStatus>,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolTrendsResponse {
    pub request_id: String,
    pub window: String,
    pub trends: Vec<tool_trends::ToolTrend>,
}

#[derive(Debug, Serialize)]
pub struct ToolAlertsResponse {
    pub request_id: String,
    pub alerts: Vec<tool_alerts::ToolAlert>,
    pub status: Vec<ToolAlertStatus>,
}

#[derive(Debug, Serialize)]
pub struct ToolCostResponse {
    pub request_id: String,
    pub timestamp: String,
    pub costs: Vec<crate::monitoring::ToolCostStats>,
}

#[derive(Debug, Serialize)]
pub struct ToolDependencyResponse {
    pub request_id: String,
    pub graph: crate::monitoring::ToolDependencyGraph,
}

#[derive(Debug, Deserialize)]
pub struct LimitQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize)]
pub struct TrendQuery {
    pub window: Option<String>,
}

fn default_limit() -> usize {
    20
}

fn parse_time_window(param: Option<String>) -> TimeWindow {
    match param
        .unwrap_or_else(|| "hour".to_string())
        .to_lowercase()
        .as_str()
    {
        "day" | "24h" => TimeWindow::Day,
        "week" | "7d" => TimeWindow::Week,
        _ => TimeWindow::Hour,
    }
}

// ============ Database Helpers ============

fn ensure_tables(conn: &Connection) {
    let _ = conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS goals (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            goal TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            completed_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS episodes (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            query TEXT NOT NULL,
            response TEXT NOT NULL,
            context_chunks_used INTEGER NOT NULL,
            success INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS reflections (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            reflection_type TEXT NOT NULL,
            insight TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        ",
    );
}

pub fn get_agent_db_connection() -> Option<Connection> {
    // Try multiple possible paths
    let path = crate::db::path_resolver::agent_db_path();
    match Connection::open(path) {
        Ok(conn) => {
            debug!("Connected to agent database at: {}", path.display());
            ensure_tables(&conn);
            Some(conn)
        }
        Err(err) => {
            warn!("Could not connect to agent database: {}", err);
            None
        }
    }
}

// ============ Endpoint Handlers ============

/// GET /monitoring/agents/stats
/// Returns aggregate statistics about agents
pub async fn get_agent_stats() -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(HttpResponse::Ok().json(AgentStatsResponse {
                active_agents: 1,
                agent_names: vec!["default".to_string()],
                episodes_total: 0,
                episodes_last_hour: 0,
                success_rate: 0.0,
                active_goals: 0,
                completed_goals: 0,
                failed_goals: 0,
                total_reflections: 0,
                timestamp: Utc::now().to_rfc3339(),
            }));
        }
    };

    // Count unique agents and collect their names
    let active_agents: usize = conn
        .query_row("SELECT COUNT(DISTINCT agent_id) FROM episodes", [], |row| {
            row.get(0)
        })
        .unwrap_or(1);

    let agent_names: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT DISTINCT agent_id FROM episodes ORDER BY agent_id")
            .unwrap_or_else(|_| conn.prepare("SELECT 'default'").unwrap());
        stmt.query_map([], |row| row.get(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_else(|_| vec!["default".to_string()])
    };

    // Total episodes
    let episodes_total: usize = conn
        .query_row("SELECT COUNT(*) FROM episodes", [], |row| row.get(0))
        .unwrap_or(0);

    // Episodes in last hour
    let one_hour_ago = Utc::now().timestamp() - 3600;
    let episodes_last_hour: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM episodes WHERE created_at > ?1",
            [one_hour_ago],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Success rate
    let (total, successful): (usize, usize) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END), 0) FROM episodes",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0));

    let success_rate = if total > 0 {
        (successful as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // Goal counts
    let active_goals: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM goals WHERE status = 'active'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let completed_goals: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM goals WHERE status = 'completed'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let failed_goals: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM goals WHERE status = 'failed'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Total reflections
    let total_reflections: usize = conn
        .query_row("SELECT COUNT(*) FROM reflections", [], |row| row.get(0))
        .unwrap_or(0);

    Ok(HttpResponse::Ok().json(AgentStatsResponse {
        active_agents: active_agents.max(1),
        agent_names: if agent_names.is_empty() {
            vec!["default".to_string()]
        } else {
            agent_names
        },
        episodes_total,
        episodes_last_hour,
        success_rate,
        active_goals,
        completed_goals,
        failed_goals,
        total_reflections,
        timestamp: Utc::now().to_rfc3339(),
    }))
}

/// GET /monitoring/agents/episodes
/// Returns recent episodes
pub async fn get_recent_episodes(query: web::Query<LimitQuery>) -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(HttpResponse::Ok().json(EpisodesResponse {
                episodes: vec![],
                total: 0,
            }));
        }
    };

    let limit = query.limit.min(100);

    let mut stmt = match conn.prepare(
        "SELECT id, agent_id, query, response, context_chunks_used, success, created_at 
         FROM episodes ORDER BY created_at DESC LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => {
            return Ok(HttpResponse::Ok().json(EpisodesResponse {
                episodes: vec![],
                total: 0,
            }));
        }
    };

    let episodes: Vec<EpisodeEntry> = stmt
        .query_map([limit], |row| {
            Ok(EpisodeEntry {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                query: row.get(2)?,
                response: row.get(3)?,
                context_chunks_used: row.get(4)?,
                success: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default();

    let total: usize = conn
        .query_row("SELECT COUNT(*) FROM episodes", [], |row| row.get(0))
        .unwrap_or(0);

    Ok(HttpResponse::Ok().json(EpisodesResponse { episodes, total }))
}

// ============ Goal Management Endpoints ============

#[derive(Debug, Deserialize)]
pub struct CreateGoalRequest {
    pub goal: String,
    #[serde(default = "default_agent_id")]
    pub agent_id: String,
}

fn default_agent_id() -> String {
    "default".to_string()
}

#[derive(Debug, Serialize)]
pub struct CreateGoalResponse {
    pub id: String,
    pub goal: String,
    pub status: String,
    pub agent_id: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct GoalActionResponse {
    pub status: String,
    pub message: String,
}

/// POST /agent/goals
/// Create a new goal
pub async fn create_goal(req: web::Json<CreateGoalRequest>) -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(
                HttpResponse::InternalServerError().json(GoalActionResponse {
                    status: "error".to_string(),
                    message: "Database not available".to_string(),
                }),
            );
        }
    };

    let goal_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    match conn.execute(
        "INSERT INTO goals (id, agent_id, goal, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![&goal_id, &req.agent_id, &req.goal, "active", now],
    ) {
        Ok(_) => Ok(HttpResponse::Created().json(CreateGoalResponse {
            id: goal_id,
            goal: req.goal.clone(),
            status: "active".to_string(),
            agent_id: req.agent_id.clone(),
            created_at: now,
        })),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(GoalActionResponse {
                status: "error".to_string(),
                message: format!("Failed to create goal: {}", e),
            }),
        ),
    }
}

/// POST /agent/goals/{goal_id}/complete
/// Mark a goal as completed
pub async fn complete_goal(goal_id: web::Path<String>) -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(
                HttpResponse::InternalServerError().json(GoalActionResponse {
                    status: "error".to_string(),
                    message: "Database not available".to_string(),
                }),
            );
        }
    };

    let now = chrono::Utc::now().timestamp();

    match conn.execute(
        "UPDATE goals SET status = ?1, completed_at = ?2 WHERE id = ?3",
        rusqlite::params!["completed", now, goal_id.as_str()],
    ) {
        Ok(rows) if rows > 0 => Ok(HttpResponse::Ok().json(GoalActionResponse {
            status: "success".to_string(),
            message: format!("Goal {} marked as completed", goal_id),
        })),
        Ok(_) => Ok(HttpResponse::NotFound().json(GoalActionResponse {
            status: "error".to_string(),
            message: format!("Goal {} not found", goal_id),
        })),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(GoalActionResponse {
                status: "error".to_string(),
                message: format!("Failed to complete goal: {}", e),
            }),
        ),
    }
}

/// POST /agent/goals/{goal_id}/fail
/// Mark a goal as failed
pub async fn fail_goal(goal_id: web::Path<String>) -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(
                HttpResponse::InternalServerError().json(GoalActionResponse {
                    status: "error".to_string(),
                    message: "Database not available".to_string(),
                }),
            );
        }
    };

    let now = chrono::Utc::now().timestamp();

    match conn.execute(
        "UPDATE goals SET status = ?1, completed_at = ?2 WHERE id = ?3",
        rusqlite::params!["failed", now, goal_id.as_str()],
    ) {
        Ok(rows) if rows > 0 => Ok(HttpResponse::Ok().json(GoalActionResponse {
            status: "success".to_string(),
            message: format!("Goal {} marked as failed", goal_id),
        })),
        Ok(_) => Ok(HttpResponse::NotFound().json(GoalActionResponse {
            status: "error".to_string(),
            message: format!("Goal {} not found", goal_id),
        })),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(GoalActionResponse {
                status: "error".to_string(),
                message: format!("Failed to mark goal as failed: {}", e),
            }),
        ),
    }
}

/// GET /agent/goals
/// Get all active goals
pub async fn get_active_goals() -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(HttpResponse::Ok().json(Vec::<GoalEntry>::new()));
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT id, agent_id, goal, status, created_at, completed_at 
         FROM goals WHERE status = 'active' ORDER BY created_at DESC",
    ) {
        Ok(s) => s,
        Err(_) => {
            return Ok(HttpResponse::Ok().json(Vec::<GoalEntry>::new()));
        }
    };

    let goals: Vec<GoalEntry> = stmt
        .query_map([], |row| {
            Ok(GoalEntry {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                goal: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                completed_at: row.get(5)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default();

    Ok(HttpResponse::Ok().json(goals))
}

/// GET /monitoring/agents/goals
/// Returns all goals with status breakdown
pub async fn get_goals() -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(HttpResponse::Ok().json(GoalsResponse {
                goals: vec![],
                active: 0,
                completed: 0,
                failed: 0,
            }));
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT id, agent_id, goal, status, created_at, completed_at 
         FROM goals ORDER BY created_at DESC LIMIT 50",
    ) {
        Ok(s) => s,
        Err(_) => {
            return Ok(HttpResponse::Ok().json(GoalsResponse {
                goals: vec![],
                active: 0,
                completed: 0,
                failed: 0,
            }));
        }
    };

    let goals: Vec<GoalEntry> = stmt
        .query_map([], |row| {
            Ok(GoalEntry {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                goal: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                completed_at: row.get(5)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default();

    let active = goals.iter().filter(|g| g.status == "active").count();
    let completed = goals.iter().filter(|g| g.status == "completed").count();
    let failed = goals.iter().filter(|g| g.status == "failed").count();

    Ok(HttpResponse::Ok().json(GoalsResponse {
        goals,
        active,
        completed,
        failed,
    }))
}

/// GET /monitoring/agents/reflections
/// Returns recent reflections
pub async fn get_reflections(query: web::Query<LimitQuery>) -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(HttpResponse::Ok().json(ReflectionsResponse {
                reflections: vec![],
                total: 0,
            }));
        }
    };

    let limit = query.limit.min(50);

    let mut stmt = match conn.prepare(
        "SELECT id, agent_id, reflection_type, insight, created_at 
         FROM reflections ORDER BY created_at DESC LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => {
            return Ok(HttpResponse::Ok().json(ReflectionsResponse {
                reflections: vec![],
                total: 0,
            }));
        }
    };

    let reflections: Vec<ReflectionEntry> = stmt
        .query_map([limit], |row| {
            Ok(ReflectionEntry {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                reflection_type: row.get(2)?,
                insight: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default();

    let total: usize = conn
        .query_row("SELECT COUNT(*) FROM reflections", [], |row| row.get(0))
        .unwrap_or(0);

    Ok(HttpResponse::Ok().json(ReflectionsResponse { reflections, total }))
}

/// GET /monitoring/memory/stats
/// Returns memory system statistics
pub async fn get_memory_stats() -> ActixResult<HttpResponse> {
    let conn = match get_agent_db_connection() {
        Some(c) => c,
        None => {
            return Ok(HttpResponse::Ok().json(MemoryStatsResponse {
                total_episodes: 0,
                total_rag_memories: 0,
                unique_agents: 1,
                oldest_episode_timestamp: None,
                newest_episode_timestamp: None,
                timestamp: Utc::now().to_rfc3339(),
            }));
        }
    };

    let total_episodes: usize = conn
        .query_row("SELECT COUNT(*) FROM episodes", [], |row| row.get(0))
        .unwrap_or(0);

    // Try to get RAG memory count from rag_memory table
    let total_rag_memories: usize = conn
        .query_row("SELECT COUNT(*) FROM rag_memory", [], |row| row.get(0))
        .unwrap_or(0);

    let unique_agents: usize = conn
        .query_row("SELECT COUNT(DISTINCT agent_id) FROM episodes", [], |row| {
            row.get(0)
        })
        .unwrap_or(1);

    let oldest_episode_timestamp: Option<i64> = conn
        .query_row("SELECT MIN(created_at) FROM episodes", [], |row| row.get(0))
        .ok();

    let newest_episode_timestamp: Option<i64> = conn
        .query_row("SELECT MAX(created_at) FROM episodes", [], |row| row.get(0))
        .ok();

    Ok(HttpResponse::Ok().json(MemoryStatsResponse {
        total_episodes,
        total_rag_memories,
        unique_agents: unique_agents.max(1),
        oldest_episode_timestamp,
        newest_episode_timestamp,
        timestamp: Utc::now().to_rfc3339(),
    }))
}

/// GET /monitoring/tools/stats
/// Returns tool usage statistics from the monitoring system
pub async fn get_tool_stats() -> ActixResult<HttpResponse> {
    let stats = crate::monitoring::get_tool_stats();

    let total_executions: usize = stats.iter().map(|s| s.total_calls).sum();
    let total_confidence: f32 = stats
        .iter()
        .map(|s| s.avg_confidence * s.total_calls as f32)
        .sum();
    let avg_confidence = if total_executions > 0 {
        total_confidence / total_executions as f32
    } else {
        0.0
    };

    let total_failures: usize = stats.iter().map(|s| s.failure_count).sum();
    let fallback_rate = if total_executions > 0 {
        total_failures as f64 / total_executions as f64
    } else {
        0.0
    };

    let tool_distribution: Vec<ToolUsageEntry> = stats
        .iter()
        .map(|s| ToolUsageEntry {
            tool_name: s.tool_type.clone(),
            count: s.total_calls,
            percentage: if total_executions > 0 {
                s.total_calls as f64 / total_executions as f64 * 100.0
            } else {
                0.0
            },
        })
        .collect();

    Ok(HttpResponse::Ok().json(ToolStatsResponse {
        tool_executions: total_executions,
        avg_confidence: avg_confidence as f64,
        fallback_rate,
        tool_distribution,
        llm_latency: crate::monitoring::get_llm_latency_stats(),
        timestamp: Utc::now().to_rfc3339(),
    }))
}

/// GET /monitoring/tools/executions
/// Returns recent tool executions for the monitoring dashboard
pub async fn get_tool_executions(query: web::Query<LimitQuery>) -> ActixResult<HttpResponse> {
    let executions = crate::monitoring::get_recent_executions(query.limit);

    Ok(
        HttpResponse::Ok().json(crate::monitoring::ToolExecutionResponse {
            status: "ok".to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            count: executions.len(),
            executions,
        }),
    )
}

/// Tool availability info
#[derive(Debug, Serialize)]
pub struct AvailableTool {
    pub name: String,
    pub description: String,
    pub status: String, // "active", "placeholder", "disabled"
    pub icon: String,
    pub category: String,
}

/// GET /monitoring/tools/available
/// Returns list of all available tools with their status
pub async fn get_available_tools() -> ActixResult<HttpResponse> {
    // Define all tools with their actual status
    let tools = vec![
        // Core retrieval tools - ACTIVE
        AvailableTool {
            name: "SemanticSearch".into(),
            description: "Search indexed documents using semantic similarity (vector search)"
                .into(),
            status: "active".into(),
            icon: "📚".into(),
            category: "retrieval".into(),
        },
        AvailableTool {
            name: "LLMGenerate".into(),
            description: "Generate text responses using Ollama LLM (phi, llama, etc.)".into(),
            status: "active".into(),
            icon: "🤖".into(),
            category: "llm".into(),
        },
        AvailableTool {
            name: "Calculator".into(),
            description: "Mathematical calculations and arithmetic operations".into(),
            status: "active".into(),
            icon: "🧮".into(),
            category: "utility".into(),
        },
        AvailableTool {
            name: "Memory".into(),
            description: "Store and retrieve agent memory (RAG context, goals, reflections)".into(),
            status: "active".into(),
            icon: "🧠".into(),
            category: "agent".into(),
        },
        AvailableTool {
            name: "Summarizer".into(),
            description: "Summarize long text into concise summaries".into(),
            status: "active".into(),
            icon: "📝".into(),
            category: "text".into(),
        },
        AvailableTool {
            name: "QueryRewriter".into(),
            description: "Rewrite queries for better search results".into(),
            status: "active".into(),
            icon: "✏️".into(),
            category: "text".into(),
        },
        AvailableTool {
            name: "Classifier".into(),
            description: "Classify text into categories".into(),
            status: "active".into(),
            icon: "🏷️".into(),
            category: "text".into(),
        },
        AvailableTool {
            name: "SentimentAnalyzer".into(),
            description: "Analyze sentiment of text (positive/negative/neutral)".into(),
            status: "active".into(),
            icon: "😊".into(),
            category: "text".into(),
        },
        AvailableTool {
            name: "EntityExtractor".into(),
            description: "Extract named entities (people, places, organizations)".into(),
            status: "active".into(),
            icon: "🔍".into(),
            category: "text".into(),
        },
        AvailableTool {
            name: "SpellChecker".into(),
            description: "Check and correct spelling errors".into(),
            status: "active".into(),
            icon: "✅".into(),
            category: "text".into(),
        },
        AvailableTool {
            name: "Translator".into(),
            description: "Translate text between languages (local, no API keys)".into(),
            status: "active".into(),
            icon: "🌐".into(),
            category: "text".into(),
        },
        AvailableTool {
            name: "FileAnalyzer".into(),
            description: "Analyze file contents and metadata".into(),
            status: "active".into(),
            icon: "📄".into(),
            category: "utility".into(),
        },
        AvailableTool {
            name: "Notification".into(),
            description: "Send notifications and alerts".into(),
            status: "active".into(),
            icon: "🔔".into(),
            category: "utility".into(),
        },
        AvailableTool {
            name: "Scheduler".into(),
            description: "Schedule tasks and reminders".into(),
            status: "active".into(),
            icon: "📅".into(),
            category: "utility".into(),
        },
        // External tools
        AvailableTool {
            name: "WebSearch".into(),
            description: "Search the web using DuckDuckGo (no API key needed)".into(),
            status: "active".into(),
            icon: "🔍".into(),
            category: "external".into(),
        },
        AvailableTool {
            name: "URLFetch".into(),
            description: "Fetch and extract text content from URLs".into(),
            status: "active".into(),
            icon: "🌐".into(),
            category: "utility".into(),
        },
        AvailableTool {
            name: "DatabaseQuery".into(),
            description: "Execute read-only SQL queries on the agent database".into(),
            status: "active".into(),
            icon: "🗄️".into(),
            category: "utility".into(),
        },
        AvailableTool {
            name: "CodeExecution".into(),
            description: "Execute Python or Bash code snippets (sandboxed)".into(),
            status: "active".into(),
            icon: "💻".into(),
            category: "utility".into(),
        },
        AvailableTool {
            name: "ImageGeneration".into(),
            description: "Generate images from text descriptions (requires API)".into(),
            status: "placeholder".into(),
            icon: "🎨".into(),
            category: "external".into(),
        },
    ];

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "request_id": uuid::Uuid::new_v4().to_string(),
        "tools": tools,
        "total": tools.len(),
        "active": tools.iter().filter(|t| t.status == "active").count(),
        "placeholder": tools.iter().filter(|t| t.status == "placeholder").count(),
        "timestamp": Utc::now().to_rfc3339(),
    })))
}

/// GET /monitoring/tools/cache
/// Returns cache stats for each tool
pub async fn get_tool_cache_stats_endpoint() -> ActixResult<HttpResponse> {
    let caches = crate::tools::tool_cache::get_cache_stats();
    Ok(HttpResponse::Ok().json(ToolCacheResponse {
        request_id: uuid::Uuid::new_v4().to_string(),
        caches,
        timestamp: Utc::now().to_rfc3339(),
    }))
}

/// GET /monitoring/tools/rate-limits
/// Returns per-tool rate limit information
pub async fn get_tool_rate_limits_endpoint() -> ActixResult<HttpResponse> {
    let statuses = get_all_rate_limit_status();
    Ok(HttpResponse::Ok().json(ToolRateLimitResponse {
        request_id: uuid::Uuid::new_v4().to_string(),
        statuses,
        timestamp: Utc::now().to_rfc3339(),
    }))
}

/// GET /monitoring/tools/trends
/// Returns tool performance trends for the requested window
pub async fn get_tool_trends_endpoint(query: web::Query<TrendQuery>) -> ActixResult<HttpResponse> {
    let window = parse_time_window(query.window.clone());
    let trends = tool_trends::get_all_trends(window);
    Ok(HttpResponse::Ok().json(ToolTrendsResponse {
        request_id: uuid::Uuid::new_v4().to_string(),
        window: format!("{:?}", window),
        trends,
    }))
}

/// GET /monitoring/tools/alerts
/// Returns recent tool alerts and current statuses
pub async fn get_tool_alerts_endpoint(query: web::Query<LimitQuery>) -> ActixResult<HttpResponse> {
    let alerts = tool_alerts::get_alerts(query.limit);
    let status = tool_alerts::get_alert_status();
    Ok(HttpResponse::Ok().json(ToolAlertsResponse {
        request_id: uuid::Uuid::new_v4().to_string(),
        alerts,
        status,
    }))
}

/// GET /monitoring/tools/costs
/// Returns aggregate tool cost information
pub async fn get_tool_costs_endpoint() -> ActixResult<HttpResponse> {
    let costs = crate::monitoring::get_tool_costs();
    Ok(HttpResponse::Ok().json(ToolCostResponse {
        request_id: uuid::Uuid::new_v4().to_string(),
        timestamp: Utc::now().to_rfc3339(),
        costs,
    }))
}

/// GET /monitoring/tools/dependencies
pub async fn get_tool_dependencies_endpoint() -> ActixResult<HttpResponse> {
    let graph = crate::monitoring::get_tool_dependency_graph();
    Ok(HttpResponse::Ok().json(ToolDependencyResponse {
        request_id: uuid::Uuid::new_v4().to_string(),
        graph,
    }))
}

/// GET /monitoring/agents/rig-stats
/// Returns Rig agentic-mode counters and token budget stats
pub async fn get_rig_stats() -> ActixResult<HttpResponse> {
    let snap = crate::monitoring::rig_stats::snapshot();
    Ok(HttpResponse::Ok().json(snap))
}

/// GET /monitoring/agents/pointer-stats
/// Returns Auto-mode routing counters and section-reassembly hydration stats.
pub async fn get_pointer_stats() -> ActixResult<HttpResponse> {
    let snap = crate::monitoring::pointer_stats::snapshot();
    Ok(HttpResponse::Ok().json(snap))
}

// ============ Route Configuration ============

pub fn configure_agentic_monitor_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/monitoring/agents")
            .route("/stats", web::get().to(get_agent_stats))
            .route("/episodes", web::get().to(get_recent_episodes))
            .route("/goals", web::get().to(get_goals))
            .route("/reflections", web::get().to(get_reflections))
            .route("/rig-stats", web::get().to(get_rig_stats))
            .route("/pointer-stats", web::get().to(get_pointer_stats)),
    )
    .service(web::scope("/monitoring/memory").route("/stats", web::get().to(get_memory_stats)))
    .service(
        web::scope("/monitoring/tools")
            .route("/stats", web::get().to(get_tool_stats))
            .route("/executions", web::get().to(get_tool_executions))
            .route("/available", web::get().to(get_available_tools))
            .route("/cache", web::get().to(get_tool_cache_stats_endpoint))
            .route("/rate-limits", web::get().to(get_tool_rate_limits_endpoint))
            .route("/trends", web::get().to(get_tool_trends_endpoint))
            .route("/alerts", web::get().to(get_tool_alerts_endpoint))
            .route("/costs", web::get().to(get_tool_costs_endpoint))
            .route(
                "/dependencies",
                web::get().to(get_tool_dependencies_endpoint),
            ),
    );
}
