use chrono::Utc;
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct AgentMemory {
    conn: Connection,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryItem {
    pub id: i64,
    pub agent_id: String,
    pub memory_type: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemorySearchResult {
    pub item: MemoryItem,
    pub score: f32,
}

impl AgentMemory {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        // Legacy table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_memory (
                id INTEGER PRIMARY KEY,
                agent_id TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;
        // RAG memory table with vector stored as JSON text
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rag_memory (
                id INTEGER PRIMARY KEY,
                agent_id TEXT NOT NULL,
                memory_type TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                vector TEXT NOT NULL
            )",
            [],
        )?;
        // Goal/task helpers for chat commands
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                goal_id TEXT NOT NULL,
                task TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                completed_at INTEGER
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    // Simple append-only legacy store
    pub fn store(&self, agent_id: &str, content: &str, timestamp: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO agent_memory (agent_id, content, timestamp) VALUES (?1, ?2, ?3)",
            (agent_id, content, timestamp),
        )?;
        Ok(())
    }

    pub fn recall(&self, agent_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT content FROM agent_memory WHERE agent_id = ?1 ORDER BY timestamp DESC",
        )?;
        let rows = stmt.query_map([agent_id], |row| row.get(0))?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    // RAG memory: store with embedding
    pub fn store_rag(
        &self,
        agent_id: &str,
        memory_type: &str,
        content: &str,
        timestamp: &str,
    ) -> Result<()> {
        let vec = crate::embedder::embed(content);
        let vector_json = serde_json::to_string(&vec).unwrap_or("[]".to_string());
        self.conn.execute(
            "INSERT INTO rag_memory (agent_id, memory_type, content, timestamp, vector) VALUES (?1, ?2, ?3, ?4, ?5)",
            (agent_id, memory_type, content, timestamp, &vector_json),
        )?;
        Ok(())
    }

    pub fn forget_topic(&self, agent_id: &str, topic: &str) -> Result<usize> {
        let pattern = format!("%{}%", topic);
        let affected = self.conn.execute(
            "DELETE FROM rag_memory WHERE agent_id = ?1 AND content LIKE ?2",
            (agent_id, &pattern),
        )?;
        Ok(affected)
    }

    pub fn recall_rag(&self, agent_id: &str, limit: usize) -> Result<Vec<MemoryItem>> {
        let sql = format!(
            "SELECT id, agent_id, memory_type, content, timestamp FROM rag_memory WHERE agent_id = ?1 ORDER BY timestamp DESC LIMIT {}",
            limit
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([agent_id], |row| {
            Ok(MemoryItem {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                memory_type: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn add_note(&self, agent_id: &str, note: &str, timestamp: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO rag_memory (agent_id, memory_type, content, timestamp, vector) VALUES (?1, 'note', ?2, ?3, '[]')",
            (agent_id, note, timestamp),
        )?;
        Ok(())
    }

    pub fn create_subgoal(&self, goal_id: &str, text: &str) -> Result<String> {
        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO tasks (id, goal_id, task, status, created_at) VALUES (?1, ?2, ?3, 'pending', ?4)",
            (&task_id, goal_id, text, now),
        )?;
        Ok(task_id)
    }

    pub fn update_goal_status(&self, goal_id: &str, status: &str) -> Result<()> {
        let now = Utc::now().timestamp();
        self.conn.execute(
            "UPDATE goals SET status = ?1, completed_at = CASE WHEN ?1 IN ('completed','failed','abandoned') THEN ?2 ELSE completed_at END WHERE id = ?3",
            (status, now, goal_id),
        )?;
        Ok(())
    }

    pub fn latest_goal(&self, agent_id: &str) -> Result<Option<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, goal FROM goals WHERE agent_id = ?1 ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([agent_id])?;
        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let goal: String = row.get(1)?;
            Ok(Some((id, goal)))
        } else {
            Ok(None)
        }
    }

    pub fn search_rag(
        &self,
        agent_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<MemorySearchResult>> {
        // Load all memory vectors for this agent (keep it simple for now)
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, memory_type, content, timestamp, vector FROM rag_memory WHERE agent_id = ?1",
        )?;
        let rows = stmt.query_map([agent_id], |row| {
            let id: i64 = row.get(0)?;
            let agent_id: String = row.get(1)?;
            let memory_type: String = row.get(2)?;
            let content: String = row.get(3)?;
            let timestamp: String = row.get(4)?;
            let vector_json: String = row.get(5)?;
            let vector: Vec<f32> = serde_json::from_str(&vector_json).unwrap_or_default();
            Ok((
                MemoryItem {
                    id,
                    agent_id,
                    memory_type,
                    content,
                    timestamp,
                },
                vector,
            ))
        })?;

        let items: Vec<(MemoryItem, Vec<f32>)> = rows.filter_map(Result::ok).collect();
        let q_vec = crate::embedder::embed(query);
        let mut scored: Vec<MemorySearchResult> = items
            .into_iter()
            .map(|(item, vec)| {
                let score = cosine_similarity(&q_vec, &vec);
                MemorySearchResult { item, score }
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(top_k);
        Ok(scored)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let ma: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if ma == 0.0 || mb == 0.0 {
        0.0
    } else {
        dot / (ma * mb)
    }
}
