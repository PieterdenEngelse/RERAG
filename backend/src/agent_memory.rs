use chrono::Utc;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::ManualObservationOrder;

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
pub struct ManualObservation {
    pub id: String,
    pub entry_type: String,
    pub title: String,
    pub narrative: String,
    pub facts: Vec<String>,
    pub concepts: Vec<String>,
    pub files_read: Vec<String>,
    pub files_modified: Vec<String>,
    pub author: Option<String>,
    pub project: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManualObservationSummary {
    pub id: String,
    pub entry_type: String,
    pub title: String,
    pub project: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManualObservationSearchHit {
    pub summary: ManualObservationSummary,
    pub score: f32,
    pub snippet: Option<String>,
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

        // Manual observation table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS manual_observations (
                id TEXT PRIMARY KEY,
                entry_type TEXT NOT NULL,
                title TEXT NOT NULL,
                narrative TEXT NOT NULL,
                facts TEXT NOT NULL,
                concepts TEXT NOT NULL,
                files_read TEXT NOT NULL,
                files_modified TEXT NOT NULL,
                author TEXT,
                project TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Migration: add project column if it doesn't exist
        let _ = conn.execute(
            "ALTER TABLE manual_observations ADD COLUMN project TEXT",
            [],
        );

        // Full-text search virtual table for manual observations
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS manual_observations_fts USING fts5(
                id UNINDEXED,
                title,
                narrative,
                facts,
                concepts,
                files,
                content='manual_observations',
                content_rowid='rowid'
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

    pub fn create_manual_observation(
        &self,
        entry_type: &str,
        title: &str,
        narrative: &str,
        facts: &[String],
        concepts: &[String],
        files_read: &[String],
        files_modified: &[String],
        author: Option<&str>,
        project: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let facts_json = serde_json::to_string(facts).unwrap_or_else(|_| "[]".to_string());
        let concepts_json = serde_json::to_string(concepts).unwrap_or_else(|_| "[]".to_string());
        let files_read_json =
            serde_json::to_string(files_read).unwrap_or_else(|_| "[]".to_string());
        let files_modified_json =
            serde_json::to_string(files_modified).unwrap_or_else(|_| "[]".to_string());

        self.conn.execute(
            "INSERT INTO manual_observations (
                id, entry_type, title, narrative, facts, concepts, files_read, files_modified, author, project, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &id,
                entry_type,
                title,
                narrative,
                facts_json,
                concepts_json,
                files_read_json,
                files_modified_json,
                author,
                project,
                &now,
                &now
            ],
        )?;

        let facts_blob = facts.join("\n");
        let concepts_blob = concepts.join(",");
        let files_blob = format!("{};{}", files_read.join(","), files_modified.join(","));
        self.conn.execute(
            "INSERT INTO manual_observations_fts(rowid, id, title, narrative, facts, concepts, files)
             VALUES (last_insert_rowid(), ?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &id,
                title,
                narrative,
                &facts_blob,
                &concepts_blob,
                &files_blob
            ],
        )?;

        Ok(id)
    }

    pub fn list_manual_observations(
        &self,
        entry_type: Option<&str>,
        project: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ManualObservationSummary>> {
        let mut query = String::from(
            "SELECT id, entry_type, title, project, created_at FROM manual_observations WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ty) = entry_type {
            query.push_str(&format!(" AND entry_type = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(ty.to_string()));
        }
        if let Some(proj) = project {
            query.push_str(&format!(" AND project = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(proj.to_string()));
        }
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ?{}",
            params_vec.len() + 1
        ));
        params_vec.push(Box::new(limit as i64));

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params_vec.iter().map(|b| &**b)),
            |row| {
                Ok(ManualObservationSummary {
                    id: row.get(0)?,
                    entry_type: row.get(1)?,
                    title: row.get(2)?,
                    project: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )?;

        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_manual_observation(&self, id: &str) -> Result<Option<ManualObservation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entry_type, title, narrative, facts, concepts, files_read, files_modified, author, project, created_at, updated_at FROM manual_observations WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let facts_json: String = row.get(4)?;
            let concepts_json: String = row.get(5)?;
            let files_read_json: String = row.get(6)?;
            let files_modified_json: String = row.get(7)?;
            Ok(Some(ManualObservation {
                id: row.get(0)?,
                entry_type: row.get(1)?,
                title: row.get(2)?,
                narrative: row.get(3)?,
                facts: serde_json::from_str(&facts_json).unwrap_or_default(),
                concepts: serde_json::from_str(&concepts_json).unwrap_or_default(),
                files_read: serde_json::from_str(&files_read_json).unwrap_or_default(),
                files_modified: serde_json::from_str(&files_modified_json).unwrap_or_default(),
                author: row.get(8)?,
                project: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_manual_observation(
        &self,
        id: &str,
        entry_type: &str,
        title: &str,
        narrative: &str,
        facts: &[String],
        concepts: &[String],
        files_read: &[String],
        files_modified: &[String],
        author: Option<&str>,
        project: Option<&str>,
    ) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let facts_json = serde_json::to_string(facts).unwrap_or_else(|_| "[]".to_string());
        let concepts_json = serde_json::to_string(concepts).unwrap_or_else(|_| "[]".to_string());
        let files_read_json =
            serde_json::to_string(files_read).unwrap_or_else(|_| "[]".to_string());
        let files_modified_json =
            serde_json::to_string(files_modified).unwrap_or_else(|_| "[]".to_string());

        let updated = self.conn.execute(
            "UPDATE manual_observations SET entry_type = ?1, title = ?2, narrative = ?3, facts = ?4, concepts = ?5, files_read = ?6, files_modified = ?7, author = ?8, project = ?9, updated_at = ?10 WHERE id = ?11",
            params![
                entry_type,
                title,
                narrative,
                facts_json,
                concepts_json,
                files_read_json,
                files_modified_json,
                author,
                project,
                &now,
                id
            ],
        )?;

        if updated > 0 {
            self.conn.execute(
                "DELETE FROM manual_observations_fts WHERE id = ?1",
                params![id],
            )?;
            let facts_blob = facts.join("\n");
            let concepts_blob = concepts.join(",");
            let files_blob = format!("{};{}", files_read.join(","), files_modified.join(","));
            self.conn.execute(
                "INSERT INTO manual_observations_fts(rowid, id, title, narrative, facts, concepts, files)
                 SELECT rowid, id, ?2, ?3, ?4, ?5, ?6 FROM manual_observations WHERE id = ?1",
                params![
                    id,
                    title,
                    narrative,
                    &facts_blob,
                    &concepts_blob,
                    &files_blob
                ],
            )?;
        }

        Ok(updated > 0)
    }

    pub fn delete_manual_observation(&self, id: &str) -> Result<bool> {
        self.conn.execute(
            "DELETE FROM manual_observations_fts WHERE id = ?1",
            params![id],
        )?;
        let deleted = self
            .conn
            .execute("DELETE FROM manual_observations WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }

    pub fn search_manual_observations(
        &self,
        query: &str,
        entry_type: Option<&str>,
        project: Option<&str>,
        date_start: Option<&str>,
        date_end: Option<&str>,
        order: ManualObservationOrder,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ManualObservationSearchHit>> {
        let mut sql = String::from(
            "SELECT mo.id, mo.entry_type, mo.title, mo.project, mo.created_at, fts.rank, snippet(manual_observations_fts, 2, '<b>', '</b>', '...', 10) 
             FROM manual_observations_fts fts
             JOIN manual_observations mo ON mo.rowid = fts.rowid
             WHERE manual_observations_fts MATCH ?1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        params_vec.push(Box::new(query.to_string()));
        if let Some(ty) = entry_type {
            sql.push_str(&format!(" AND mo.entry_type = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(ty.to_string()));
        }
        if let Some(proj) = project {
            sql.push_str(&format!(" AND mo.project = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(proj.to_string()));
        }
        if let Some(start) = date_start {
            sql.push_str(&format!(" AND mo.created_at >= ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(start.to_string()));
        }
        if let Some(end) = date_end {
            sql.push_str(&format!(" AND mo.created_at <= ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(end.to_string()));
        }
        match order {
            ManualObservationOrder::Relevance => sql.push_str(" ORDER BY fts.rank"),
            ManualObservationOrder::Newest => sql.push_str(" ORDER BY mo.created_at DESC"),
            ManualObservationOrder::Oldest => sql.push_str(" ORDER BY mo.created_at ASC"),
        }
        sql.push_str(&format!(" LIMIT ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(limit as i64));
        sql.push_str(&format!(" OFFSET ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(offset as i64));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params_vec.iter().map(|b| &**b)),
            |row| {
                Ok(ManualObservationSearchHit {
                    summary: ManualObservationSummary {
                        id: row.get(0)?,
                        entry_type: row.get(1)?,
                        title: row.get(2)?,
                        project: row.get(3)?,
                        created_at: row.get(4)?,
                    },
                    score: row.get::<_, f32>(5).unwrap_or_default(),
                    snippet: row.get(6).ok(),
                })
            },
        )?;

        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn timeline_manual_observations(
        &self,
        anchor_id: Option<&str>,
        query: Option<&str>,
        entry_type: Option<&str>,
        project: Option<&str>,
        depth_before: usize,
        depth_after: usize,
    ) -> Result<Vec<ManualObservationSummary>> {
        let anchor = if let Some(id) = anchor_id {
            self.get_manual_observation(id)?
        } else if let Some(q) = query {
            self.search_manual_observations(
                q,
                entry_type,
                project,
                None,
                None,
                ManualObservationOrder::Relevance,
                1,
                0,
            )?
            .into_iter()
            .next()
            .and_then(|hit| self.get_manual_observation(&hit.summary.id).transpose())
            .transpose()?
        } else {
            None
        };

        let anchor = match anchor {
            Some(obs) => obs,
            None => return Ok(Vec::new()),
        };

        // Build query with optional project filter
        let mut results = Vec::new();

        // Fetch observations before anchor
        let mut before_items: Vec<ManualObservationSummary> = if let Some(proj) = project {
            let mut stmt = self.conn.prepare(
                "SELECT id, entry_type, title, project, created_at FROM manual_observations WHERE created_at < ?1 AND project = ?3 ORDER BY created_at DESC LIMIT ?2"
            )?;
            let rows = stmt.query_map(
                params![&anchor.created_at, depth_before as i64, proj],
                |row| {
                    Ok(ManualObservationSummary {
                        id: row.get(0)?,
                        entry_type: row.get(1)?,
                        title: row.get(2)?,
                        project: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )?;
            rows.filter_map(Result::ok).collect()
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, entry_type, title, project, created_at FROM manual_observations WHERE created_at < ?1 ORDER BY created_at DESC LIMIT ?2"
            )?;
            let rows = stmt.query_map(params![&anchor.created_at, depth_before as i64], |row| {
                Ok(ManualObservationSummary {
                    id: row.get(0)?,
                    entry_type: row.get(1)?,
                    title: row.get(2)?,
                    project: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            rows.filter_map(Result::ok).collect()
        };
        before_items.reverse();
        results.extend(before_items);

        results.push(ManualObservationSummary {
            id: anchor.id.clone(),
            entry_type: anchor.entry_type.clone(),
            title: anchor.title.clone(),
            project: anchor.project.clone(),
            created_at: anchor.created_at.clone(),
        });

        // Fetch observations after anchor
        let after_items: Vec<ManualObservationSummary> = if let Some(proj) = project {
            let mut stmt = self.conn.prepare(
                "SELECT id, entry_type, title, project, created_at FROM manual_observations WHERE created_at > ?1 AND project = ?3 ORDER BY created_at ASC LIMIT ?2"
            )?;
            let rows = stmt.query_map(
                params![&anchor.created_at, depth_after as i64, proj],
                |row| {
                    Ok(ManualObservationSummary {
                        id: row.get(0)?,
                        entry_type: row.get(1)?,
                        title: row.get(2)?,
                        project: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )?;
            rows.filter_map(Result::ok).collect()
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, entry_type, title, project, created_at FROM manual_observations WHERE created_at > ?1 ORDER BY created_at ASC LIMIT ?2"
            )?;
            let rows = stmt.query_map(params![&anchor.created_at, depth_after as i64], |row| {
                Ok(ManualObservationSummary {
                    id: row.get(0)?,
                    entry_type: row.get(1)?,
                    title: row.get(2)?,
                    project: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            rows.filter_map(Result::ok).collect()
        };
        results.extend(after_items);

        Ok(results)
    }

    pub fn fetch_manual_observations(&self, ids: &[String]) -> Result<Vec<ManualObservation>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = ids
            .iter()
            .enumerate()
            .map(|(idx, _)| format!("?{}", idx + 1))
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            "SELECT id, entry_type, title, narrative, facts, concepts, files_read, files_modified, author, project, created_at, updated_at FROM manual_observations WHERE id IN ({})",
            placeholders
        );
        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
            let facts_json: String = row.get(4)?;
            let concepts_json: String = row.get(5)?;
            let files_read_json: String = row.get(6)?;
            let files_modified_json: String = row.get(7)?;
            Ok(ManualObservation {
                id: row.get(0)?,
                entry_type: row.get(1)?,
                title: row.get(2)?,
                narrative: row.get(3)?,
                facts: serde_json::from_str(&facts_json).unwrap_or_default(),
                concepts: serde_json::from_str(&concepts_json).unwrap_or_default(),
                files_read: serde_json::from_str(&files_read_json).unwrap_or_default(),
                files_modified: serde_json::from_str(&files_modified_json).unwrap_or_default(),
                author: row.get(8)?,
                project: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
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
