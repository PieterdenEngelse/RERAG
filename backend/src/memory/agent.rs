// src/memory/agent.rs
// Phase 6: Agent Memory Layer
// Episodic memory, goal tracking, reflection mechanisms

use chrono::Utc;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info};
use uuid::Uuid;

use crate::embedder::EmbeddingService;
use crate::memory::{VectorRecord, VectorStore};

/// Agent state and identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

/// Goal tracked by agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub agent_id: String,
    pub goal: String,
    pub status: GoalStatus,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GoalStatus {
    Active,
    Completed,
    Failed,
    Paused,
    Abandoned,
}

impl GoalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            GoalStatus::Active => "active",
            GoalStatus::Completed => "completed",
            GoalStatus::Failed => "failed",
            GoalStatus::Paused => "paused",
            GoalStatus::Abandoned => "abandoned",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Self {
        match value {
            "completed" => GoalStatus::Completed,
            "failed" => GoalStatus::Failed,
            "paused" => GoalStatus::Paused,
            "abandoned" => GoalStatus::Abandoned,
            _ => GoalStatus::Active,
        }
    }
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Task (sub-goal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub goal_id: String,
    pub task: String,
    pub status: TaskStatus,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Episodic memory - single interaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub agent_id: String,
    pub query: String,
    pub response: String,
    pub context_chunks_used: usize,
    pub success: bool,
    pub created_at: i64,
}

/// Reflection - analysis of past episodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reflection {
    pub id: String,
    pub agent_id: String,
    pub reflection_type: ReflectionType,
    pub insight: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReflectionType {
    Success,
    Failure,
    Pattern,
    Improvement,
}

impl std::fmt::Display for ReflectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReflectionType::Success => write!(f, "success"),
            ReflectionType::Failure => write!(f, "failure"),
            ReflectionType::Pattern => write!(f, "pattern"),
            ReflectionType::Improvement => write!(f, "improvement"),
        }
    }
}

/// Agent Memory Layer - manages goals, episodes, reflections
pub struct AgentMemoryLayer {
    agent: Agent,
    db_path: PathBuf,
    vector_store: std::sync::Arc<tokio::sync::RwLock<VectorStore>>,
    embedding_service: std::sync::Arc<EmbeddingService>,
}

impl AgentMemoryLayer {
    /// Create new agent memory layer
    pub fn new(
        agent_id: String,
        agent_name: String,
        db_path: PathBuf,
        vector_store: std::sync::Arc<tokio::sync::RwLock<VectorStore>>,
        embedding_service: std::sync::Arc<EmbeddingService>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let agent = Agent {
            id: agent_id,
            name: agent_name,
            created_at: Utc::now().timestamp(),
        };

        let memory = Self {
            agent,
            db_path,
            vector_store,
            embedding_service,
        };

        memory.init_db()?;
        info!(agent_id = %memory.agent.id, "Agent memory layer initialized");
        Ok(memory)
    }

    /// Initialize SQLite schema
    fn init_db(&self) -> Result<(), Box<dyn std::error::Error>> {
        let conn = Connection::open(&self.db_path)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS goals (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                completed_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                goal_id TEXT NOT NULL,
                task TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                completed_at INTEGER,
                FOREIGN KEY(goal_id) REFERENCES goals(id)
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

            CREATE INDEX IF NOT EXISTS idx_goals_agent ON goals(agent_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_goal ON tasks(goal_id);
            CREATE INDEX IF NOT EXISTS idx_episodes_agent ON episodes(agent_id);
            CREATE INDEX IF NOT EXISTS idx_reflections_agent ON reflections(agent_id);
            ",
        )?;

        Ok(())
    }

    /// Set a new goal
    pub fn set_goal(&self, goal_text: String) -> Result<Goal, Box<dyn std::error::Error>> {
        let conn = Connection::open(&self.db_path)?;
        let goal_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO goals (id, agent_id, goal, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&goal_id, &self.agent.id, &goal_text, "active", now],
        )?;

        info!(agent_id = %self.agent.id, goal_id = %goal_id, "Goal set");

        Ok(Goal {
            id: goal_id,
            agent_id: self.agent.id.clone(),
            goal: goal_text,
            status: GoalStatus::Active,
            created_at: now,
            completed_at: None,
        })
    }

    /// Complete a goal
    pub fn complete_goal(&self, goal_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = Connection::open(&self.db_path)?;
        let now = Utc::now().timestamp();

        conn.execute(
            "UPDATE goals SET status = ?1, completed_at = ?2 WHERE id = ?3",
            params!["completed", now, goal_id],
        )?;

        info!(agent_id = %self.agent.id, goal_id = %goal_id, "Goal completed");
        Ok(())
    }

    /// Record an episode (query + response + result)
    pub async fn record_episode(
        &self,
        query: String,
        response: String,
        context_chunks_used: usize,
        success: bool,
    ) -> Result<Episode, Box<dyn std::error::Error>> {
        let episode_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        // Store in vector store for semantic search
        let embedding = self.embedding_service.embed_text(&query).await;
        let record = VectorRecord::new(
            episode_id.clone(),
            self.agent.id.clone(),
            query.clone(),
            embedding,
            0,
            query.split_whitespace().count(),
            "agent_episode".to_string(),
            now,
        );

        {
            let mut store = self.vector_store.write().await;
            store.add_record(record).await?;
        }

        // Store in SQLite
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO episodes (id, agent_id, query, response, context_chunks_used, success, created_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &episode_id,
                &self.agent.id,
                &query,
                &response,
                context_chunks_used,
                success as i32,
                now
            ],
        )?;

        info!(agent_id = %self.agent.id, episode_id = %episode_id, success = success, "Episode recorded");

        Ok(Episode {
            id: episode_id,
            agent_id: self.agent.id.clone(),
            query,
            response,
            context_chunks_used,
            success,
            created_at: now,
        })
    }

    /// Recall similar episodes from semantic search
    pub async fn recall_similar_episodes(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<Episode>, Box<dyn std::error::Error>> {
        let embedding = self.embedding_service.embed_query(query).await;
        // FIXED: Changed from read() to write() because search() needs mutable access for LRU tracking
        let mut store = self.vector_store.write().await;
        let results = store.search(&embedding, top_k).await?;

        // Fetch full episode details from SQLite
        let conn = Connection::open(&self.db_path)?;
        let mut episodes = Vec::new();

        for result in results {
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, query, response, context_chunks_used, success, created_at 
                 FROM episodes WHERE id = ?1",
            )?;
            let episode = stmt.query_row(params![&result.chunk_id], |row| {
                Ok(Episode {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    query: row.get(2)?,
                    response: row.get(3)?,
                    context_chunks_used: row.get(4)?,
                    success: row.get::<_, i32>(5)? != 0,
                    created_at: row.get(6)?,
                })
            })?;
            episodes.push(episode);
        }

        debug!(query = %query, found = episodes.len(), "Similar past queries found");
        Ok(episodes)
    }

    /// Reflect on episodes - analyze patterns
    pub fn reflect_on_episodes(&self) -> Result<Reflection, Box<dyn std::error::Error>> {
        let conn = Connection::open(&self.db_path)?;

        // Get recent episodes
        let mut stmt = conn.prepare(
            "SELECT COUNT(*), SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) 
             FROM episodes WHERE agent_id = ?1 AND created_at > ?2",
        )?;

        let one_day_ago = Utc::now().timestamp() - (24 * 3600);
        let (total, successful): (usize, usize) = stmt
            .query_row(params![&self.agent.id, one_day_ago], |row| {
                Ok((row.get(0)?, row.get(1).unwrap_or(0)))
            })?;

        let success_rate = if total > 0 {
            (successful as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let reflection_type = if success_rate > 80.0 {
            ReflectionType::Success
        } else if success_rate < 50.0 {
            ReflectionType::Failure
        } else {
            ReflectionType::Pattern
        };

        let insight = format!(
            "Last 24h: {} total episodes, {} successful ({:.1}% success rate)",
            total, successful, success_rate
        );

        let reflection_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO reflections (id, agent_id, reflection_type, insight, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&reflection_id, &self.agent.id, reflection_type.to_string(), &insight, now],
        )?;

        info!(agent_id = %self.agent.id, insight = %insight, "Reflection recorded");

        Ok(Reflection {
            id: reflection_id,
            agent_id: self.agent.id.clone(),
            reflection_type,
            insight,
            created_at: now,
        })
    }

    /// Get all active goals
    pub fn get_active_goals(&self) -> Result<Vec<Goal>, Box<dyn std::error::Error>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, goal, status, created_at, completed_at FROM goals 
             WHERE agent_id = ?1 AND status = ?2",
        )?;

        let goals = stmt.query_map(params![&self.agent.id, "active"], |row| {
            Ok(Goal {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                goal: row.get(2)?,
                status: GoalStatus::Active,
                created_at: row.get(4)?,
                completed_at: row.get(5)?,
            })
        })?;

        Ok(goals.collect::<SqlResult<Vec<_>>>()?)
    }

    /// Get agent context (all memory for decision making)
    pub fn get_agent_context(&self) -> Result<AgentContext, Box<dyn std::error::Error>> {
        let conn = Connection::open(&self.db_path)?;

        // Get active goals
        let goals = self.get_active_goals()?;

        // Get recent episodes
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, query, response, context_chunks_used, success, created_at 
             FROM episodes WHERE agent_id = ?1 ORDER BY created_at DESC LIMIT 10",
        )?;
        let episodes = stmt.query_map(params![&self.agent.id], |row| {
            Ok(Episode {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                query: row.get(2)?,
                response: row.get(3)?,
                context_chunks_used: row.get(4)?,
                success: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
            })
        })?;

        // Get recent reflections
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, reflection_type, insight, created_at 
             FROM reflections WHERE agent_id = ?1 ORDER BY created_at DESC LIMIT 5",
        )?;
        let reflections = stmt.query_map(params![&self.agent.id], |row| {
            Ok(Reflection {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                reflection_type: match row.get::<_, String>(2)?.as_str() {
                    "success" => ReflectionType::Success,
                    "failure" => ReflectionType::Failure,
                    "pattern" => ReflectionType::Pattern,
                    "improvement" => ReflectionType::Improvement,
                    _ => ReflectionType::Pattern,
                },
                insight: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        Ok(AgentContext {
            agent_id: self.agent.id.clone(),
            active_goals: goals,
            recent_episodes: episodes.collect::<SqlResult<Vec<_>>>()?,
            recent_reflections: reflections.collect::<SqlResult<Vec<_>>>()?,
        })
    }
}

/// Agent context for decision making
#[derive(Debug, Serialize)]
pub struct AgentContext {
    pub agent_id: String,
    pub active_goals: Vec<Goal>,
    pub recent_episodes: Vec<Episode>,
    pub recent_reflections: Vec<Reflection>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_memory() -> (AgentMemoryLayer, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path().to_path_buf();

        let vector_store = std::sync::Arc::new(tokio::sync::RwLock::new(
            VectorStore::with_defaults().unwrap(),
        ));
        let embedding_service = std::sync::Arc::new(EmbeddingService::new(
            crate::embedder::EmbeddingConfig::default(),
        ));

        let memory = AgentMemoryLayer::new(
            "test-agent".to_string(),
            "Test Agent".to_string(),
            db_path,
            vector_store,
            embedding_service,
        )
        .unwrap();

        (memory, temp_file)
    }

    #[test]
    fn test_agent_memory_creation() {
        let (memory, _temp) = create_test_memory();
        assert_eq!(memory.agent.id, "test-agent");
        assert_eq!(memory.agent.name, "Test Agent");
    }

    #[test]
    fn test_set_and_get_goals() {
        let (memory, _temp) = create_test_memory();

        let goal = memory
            .set_goal("Find information about Rust".to_string())
            .unwrap();
        assert_eq!(goal.status, GoalStatus::Active);

        let goals = memory.get_active_goals().unwrap();
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].goal, "Find information about Rust");
    }

    #[test]
    fn test_complete_goal() {
        let (memory, _temp) = create_test_memory();

        let goal = memory.set_goal("Test goal".to_string()).unwrap();
        memory.complete_goal(&goal.id).unwrap();

        let goals = memory.get_active_goals().unwrap();
        assert_eq!(goals.len(), 0);
    }

    #[tokio::test]
    async fn test_record_episode() {
        let (memory, _temp) = create_test_memory();

        let episode = memory
            .record_episode(
                "What is Rust?".to_string(),
                "Rust is a systems programming language.".to_string(),
                3,
                true,
            )
            .await
            .unwrap();

        assert!(episode.success);
        assert_eq!(episode.context_chunks_used, 3);
    }

    #[test]
    fn test_reflect_on_episodes() {
        let (memory, _temp) = create_test_memory();

        let reflection = memory.reflect_on_episodes().unwrap();
        assert!(!reflection.insight.is_empty());
    }
}
