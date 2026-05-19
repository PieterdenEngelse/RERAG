// backend/src/graph/agent_memory_graph.rs
// Agent memory stored in the FalkorDB knowledge graph.
//
// This module provides graph-based agent memory for:
// - Episodic memory with entity connections
// - Goal and task tracking with relationships
// - Pattern discovery across sessions
//
// FalkorDB notes: `datetime()` is replaced by app-supplied epoch-millis
// (`$now`); timestamps are stored as plain `i64`.

use crate::graph::client::{
    lit, now_millis, row_bool, row_f64, row_i64, row_str, row_str_vec, GraphHandle, GraphClientError,
};
use crate::params;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Agent memory stored in the knowledge graph.
pub struct AgentMemoryGraph {
    graph: GraphHandle,
    agent_id: String,
}

/// A similar episode found through graph search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarEpisode {
    pub id: String,
    pub query: String,
    pub response: String,
    pub success: bool,
    pub similarity: f32,
    pub shared_entities: Vec<String>,
}

/// A pattern discovered in agent memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub entity: String,
    pub episode_count: usize,
    pub success_count: usize,
    pub success_rate: f32,
    pub pattern_type: String,
}

/// Agent statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStats {
    pub episode_count: usize,
    pub success_count: usize,
    pub goal_count: usize,
    pub completed_goals: usize,
    pub reflection_count: usize,
}

impl AgentStats {
    pub fn success_rate(&self) -> f32 {
        if self.episode_count == 0 {
            0.0
        } else {
            self.success_count as f32 / self.episode_count as f32
        }
    }

    pub fn goal_completion_rate(&self) -> f32 {
        if self.goal_count == 0 {
            0.0
        } else {
            self.completed_goals as f32 / self.goal_count as f32
        }
    }
}

impl AgentMemoryGraph {
    /// Create a new agent memory graph
    pub async fn new(
        graph: GraphHandle,
        agent_id: String,
        agent_name: &str,
    ) -> Result<Self, GraphClientError> {
        let params = params! {
            "id" => lit::str(&agent_id),
            "name" => lit::str(agent_name),
            "now" => lit::int(now_millis()),
        };
        graph
            .run(
                "MERGE (a:Agent {id: $id})
                 SET a.name = $name,
                     a.last_active = $now
                 ON CREATE SET a.created_at = $now",
                &params,
            )
            .await?;

        info!(agent_id = %agent_id, "Initialized agent memory graph");
        Ok(Self { graph, agent_id })
    }

    /// Record an episode in the graph
    pub async fn record_episode(
        &self,
        episode_id: &str,
        query_text: &str,
        response: &str,
        success: bool,
        chunks_used: usize,
    ) -> Result<(), GraphClientError> {
        let params = params! {
            "agent_id" => lit::str(&self.agent_id),
            "id" => lit::str(episode_id),
            "query" => lit::str(query_text),
            "response" => lit::str(response),
            "success" => lit::bool(success),
            "chunks_used" => lit::int(chunks_used as i64),
            "now" => lit::int(now_millis()),
        };
        self.graph
            .run(
                "MATCH (a:Agent {id: $agent_id})
                 CREATE (e:Episode {
                    id: $id,
                    query: $query,
                    response: $response,
                    success: $success,
                    chunks_used: $chunks_used,
                    created_at: $now
                 })
                 CREATE (a)-[:EXPERIENCED]->(e)",
                &params,
            )
            .await?;
        debug!(episode_id = %episode_id, "Recorded episode in graph");
        Ok(())
    }

    /// Link an episode to chunks it used
    pub async fn link_episode_to_chunks(
        &self,
        episode_id: &str,
        chunk_ids: &[String],
    ) -> Result<(), GraphClientError> {
        if chunk_ids.is_empty() {
            return Ok(());
        }
        let params = params! {
            "episode_id" => lit::str(episode_id),
            "chunk_ids" => lit::str_list(chunk_ids),
        };
        self.graph
            .run(
                "MATCH (e:Episode {id: $episode_id})
                 UNWIND $chunk_ids AS chunk_id
                 MATCH (c:Chunk {id: chunk_id})
                 CREATE (e)-[:USED_CHUNK]->(c)",
                &params,
            )
            .await?;
        Ok(())
    }

    /// Link an episode to entities it mentioned
    pub async fn link_episode_to_entities(
        &self,
        episode_id: &str,
        entity_names: &[String],
    ) -> Result<(), GraphClientError> {
        if entity_names.is_empty() {
            return Ok(());
        }
        let normalized: Vec<String> = entity_names.iter().map(|e| e.to_lowercase()).collect();
        let params = params! {
            "episode_id" => lit::str(episode_id),
            "entity_names" => lit::str_list(&normalized),
        };
        self.graph
            .run(
                "MATCH (e:Episode {id: $episode_id})
                 UNWIND $entity_names AS entity_name
                 MATCH (ent:Entity {normalized_name: entity_name})
                 CREATE (e)-[:MENTIONED_ENTITY]->(ent)",
                &params,
            )
            .await?;
        Ok(())
    }

    /// Find similar past episodes using graph structure
    pub async fn find_similar_episodes(
        &self,
        query_entities: &[String],
        limit: usize,
    ) -> Result<Vec<SimilarEpisode>, GraphClientError> {
        if query_entities.is_empty() {
            return Ok(Vec::new());
        }
        let normalized: Vec<String> = query_entities.iter().map(|e| e.to_lowercase()).collect();
        let params = params! {
            "agent_id" => lit::str(&self.agent_id),
            "entities" => lit::str_list(&normalized),
            "limit" => lit::int(limit as i64),
        };
        let rows = self
            .graph
            .query(
                "MATCH (a:Agent {id: $agent_id})-[:EXPERIENCED]->(e:Episode)
                 OPTIONAL MATCH (e)-[:MENTIONED_ENTITY]->(ent:Entity)
                 WHERE ent.normalized_name IN $entities
                 WITH e, collect(DISTINCT ent.name) AS direct_entities,
                      count(DISTINCT ent) AS direct_count
                 WHERE direct_count > 0
                 RETURN e.id AS id,
                        e.query AS query,
                        e.response AS response,
                        e.success AS success,
                        direct_count AS similarity,
                        direct_entities AS shared_entities
                 ORDER BY similarity DESC
                 LIMIT $limit",
                &params,
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| SimilarEpisode {
                id: row_str(&row, 0),
                query: row_str(&row, 1),
                response: row_str(&row, 2),
                success: row_bool(&row, 3),
                similarity: row_i64(&row, 4) as f32,
                shared_entities: row_str_vec(&row, 5),
            })
            .collect())
    }

    /// Discover patterns across episodes
    pub async fn discover_patterns(&self) -> Result<Vec<Pattern>, GraphClientError> {
        let params = params! { "agent_id" => lit::str(&self.agent_id) };
        let rows = self
            .graph
            .query(
                "MATCH (a:Agent {id: $agent_id})-[:EXPERIENCED]->(e:Episode)
                 MATCH (e)-[:MENTIONED_ENTITY]->(ent:Entity)
                 WITH ent, count(e) AS episode_count,
                      sum(CASE WHEN e.success THEN 1 ELSE 0 END) AS success_count
                 WHERE episode_count >= 3
                 RETURN ent.name AS entity,
                        episode_count,
                        success_count,
                        toFloat(success_count) / episode_count AS success_rate
                 ORDER BY episode_count DESC
                 LIMIT 20",
                &params,
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let success_rate = row_f64(&row, 3, 0.0);
                let pattern_type = if success_rate > 0.8 {
                    "high_success"
                } else if success_rate < 0.3 {
                    "low_success"
                } else {
                    "normal"
                };
                Pattern {
                    entity: row_str(&row, 0),
                    episode_count: row_i64(&row, 1) as usize,
                    success_count: row_i64(&row, 2) as usize,
                    success_rate: success_rate as f32,
                    pattern_type: pattern_type.to_string(),
                }
            })
            .collect())
    }

    /// Create a goal in the graph
    pub async fn create_goal(
        &self,
        goal_id: &str,
        description: &str,
        status: &str,
    ) -> Result<(), GraphClientError> {
        let params = params! {
            "agent_id" => lit::str(&self.agent_id),
            "id" => lit::str(goal_id),
            "description" => lit::str(description),
            "status" => lit::str(status),
            "now" => lit::int(now_millis()),
        };
        self.graph
            .run(
                "MATCH (a:Agent {id: $agent_id})
                 CREATE (g:Goal {
                    id: $id,
                    description: $description,
                    status: $status,
                    created_at: $now
                 })
                 CREATE (a)-[:HAS_GOAL]->(g)",
                &params,
            )
            .await?;
        Ok(())
    }

    /// Update goal status
    pub async fn update_goal_status(&self, goal_id: &str, status: &str) -> Result<(), GraphClientError> {
        let params = params! {
            "goal_id" => lit::str(goal_id),
            "status" => lit::str(status),
            "now" => lit::int(now_millis()),
        };
        self.graph
            .run(
                "MATCH (g:Goal {id: $goal_id})
                 SET g.status = $status,
                     g.updated_at = $now",
                &params,
            )
            .await?;
        Ok(())
    }

    /// Get agent statistics
    pub async fn get_agent_stats(&self) -> Result<AgentStats, GraphClientError> {
        let params = params! { "agent_id" => lit::str(&self.agent_id) };
        let rows = self
            .graph
            .query(
                "MATCH (a:Agent {id: $agent_id})
                 OPTIONAL MATCH (a)-[:EXPERIENCED]->(e:Episode)
                 WITH a, count(e) AS episode_count,
                      sum(CASE WHEN e.success THEN 1 ELSE 0 END) AS success_count
                 OPTIONAL MATCH (a)-[:HAS_GOAL]->(g:Goal)
                 WITH a, episode_count, success_count, count(g) AS goal_count,
                      sum(CASE WHEN g.status = 'completed' THEN 1 ELSE 0 END) AS completed_goals
                 OPTIONAL MATCH (a)-[:REFLECTED]->(r:Reflection)
                 RETURN episode_count,
                        success_count,
                        goal_count,
                        completed_goals,
                        count(r) AS reflection_count",
                &params,
            )
            .await?;

        if let Some(row) = rows.first() {
            Ok(AgentStats {
                episode_count: row_i64(row, 0) as usize,
                success_count: row_i64(row, 1) as usize,
                goal_count: row_i64(row, 2) as usize,
                completed_goals: row_i64(row, 3) as usize,
                reflection_count: row_i64(row, 4) as usize,
            })
        } else {
            Ok(AgentStats::default())
        }
    }
}
