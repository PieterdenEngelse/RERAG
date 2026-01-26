// backend/src/graph/agent_memory_graph.rs
// Agent memory stored in Neo4j knowledge graph
//
// This module provides graph-based agent memory for:
// - Episodic memory with entity connections
// - Goal and task tracking with relationships
// - Pattern discovery across sessions

use crate::graph::client::Neo4jError;
use neo4rs::{query, Graph};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// Agent memory stored in Neo4j
pub struct AgentMemoryGraph {
    graph: Arc<Graph>,
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
        graph: Arc<Graph>,
        agent_id: String,
        agent_name: &str,
    ) -> Result<Self, Neo4jError> {
        // Ensure agent node exists
        let q = query(
            "MERGE (a:Agent {id: $id})
             SET a.name = $name,
                 a.last_active = datetime()
             ON CREATE SET a.created_at = datetime()",
        )
        .param("id", agent_id.clone())
        .param("name", agent_name.to_string());

        graph.run(q).await?;

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
    ) -> Result<(), Neo4jError> {
        let q = query(
            "MATCH (a:Agent {id: $agent_id})
             CREATE (e:Episode {
                id: $id,
                query: $query,
                response: $response,
                success: $success,
                chunks_used: $chunks_used,
                created_at: datetime()
             })
             CREATE (a)-[:EXPERIENCED]->(e)",
        )
        .param("agent_id", self.agent_id.clone())
        .param("id", episode_id.to_string())
        .param("query", query_text.to_string())
        .param("response", response.to_string())
        .param("success", success)
        .param("chunks_used", chunks_used as i64);

        self.graph.run(q).await?;
        debug!(episode_id = %episode_id, "Recorded episode in graph");
        Ok(())
    }

    /// Link an episode to chunks it used
    pub async fn link_episode_to_chunks(
        &self,
        episode_id: &str,
        chunk_ids: &[String],
    ) -> Result<(), Neo4jError> {
        if chunk_ids.is_empty() {
            return Ok(());
        }

        let q = query(
            "MATCH (e:Episode {id: $episode_id})
             UNWIND $chunk_ids AS chunk_id
             MATCH (c:Chunk {id: chunk_id})
             CREATE (e)-[:USED_CHUNK]->(c)",
        )
        .param("episode_id", episode_id.to_string())
        .param("chunk_ids", chunk_ids.to_vec());

        self.graph.run(q).await?;
        Ok(())
    }

    /// Link an episode to entities it mentioned
    pub async fn link_episode_to_entities(
        &self,
        episode_id: &str,
        entity_names: &[String],
    ) -> Result<(), Neo4jError> {
        if entity_names.is_empty() {
            return Ok(());
        }

        let normalized: Vec<String> = entity_names.iter().map(|e| e.to_lowercase()).collect();

        let q = query(
            "MATCH (e:Episode {id: $episode_id})
             UNWIND $entity_names AS entity_name
             MATCH (ent:Entity {normalized_name: entity_name})
             CREATE (e)-[:MENTIONED_ENTITY]->(ent)",
        )
        .param("episode_id", episode_id.to_string())
        .param("entity_names", normalized);

        self.graph.run(q).await?;
        Ok(())
    }

    /// Find similar past episodes using graph structure
    pub async fn find_similar_episodes(
        &self,
        query_entities: &[String],
        limit: usize,
    ) -> Result<Vec<SimilarEpisode>, Neo4jError> {
        if query_entities.is_empty() {
            return Ok(Vec::new());
        }

        let normalized: Vec<String> = query_entities.iter().map(|e| e.to_lowercase()).collect();

        let q = query(
            "MATCH (a:Agent {id: $agent_id})-[:EXPERIENCED]->(e:Episode)
             OPTIONAL MATCH (e)-[:MENTIONED_ENTITY]->(ent:Entity)
             WHERE ent.normalized_name IN $entities
             WITH e, collect(DISTINCT ent.name) as direct_entities, count(DISTINCT ent) as direct_count
             WHERE direct_count > 0
             RETURN e.id as id, 
                    e.query as query, 
                    e.response as response,
                    e.success as success, 
                    direct_count as similarity,
                    direct_entities as shared_entities
             ORDER BY similarity DESC
             LIMIT $limit",
        )
        .param("agent_id", self.agent_id.clone())
        .param("entities", normalized)
        .param("limit", limit as i64);

        let mut result = self.graph.execute(q).await?;
        let mut episodes = Vec::new();

        while let Ok(Some(row)) = result.next().await {
            let id: String = row.get("id").unwrap_or_default();
            let query_text: String = row.get("query").unwrap_or_default();
            let response: String = row.get("response").unwrap_or_default();
            let success: bool = row.get("success").unwrap_or(false);
            let similarity: i64 = row.get("similarity").unwrap_or(0);
            let shared_entities: Vec<String> = row.get("shared_entities").unwrap_or_default();

            episodes.push(SimilarEpisode {
                id,
                query: query_text,
                response,
                success,
                similarity: similarity as f32,
                shared_entities,
            });
        }

        Ok(episodes)
    }

    /// Discover patterns across episodes
    pub async fn discover_patterns(&self) -> Result<Vec<Pattern>, Neo4jError> {
        let q = query(
            "MATCH (a:Agent {id: $agent_id})-[:EXPERIENCED]->(e:Episode)
             MATCH (e)-[:MENTIONED_ENTITY]->(ent:Entity)
             WITH ent, count(e) as episode_count, 
                  sum(CASE WHEN e.success THEN 1 ELSE 0 END) as success_count
             WHERE episode_count >= 3
             RETURN ent.name as entity,
                    episode_count,
                    success_count,
                    toFloat(success_count) / episode_count as success_rate
             ORDER BY episode_count DESC
             LIMIT 20",
        )
        .param("agent_id", self.agent_id.clone());

        let mut result = self.graph.execute(q).await?;
        let mut patterns = Vec::new();

        while let Ok(Some(row)) = result.next().await {
            let entity: String = row.get("entity").unwrap_or_default();
            let episode_count: i64 = row.get("episode_count").unwrap_or(0);
            let success_count: i64 = row.get("success_count").unwrap_or(0);
            let success_rate: f64 = row.get("success_rate").unwrap_or(0.0);

            let pattern_type = if success_rate > 0.8 {
                "high_success"
            } else if success_rate < 0.3 {
                "low_success"
            } else {
                "normal"
            };

            patterns.push(Pattern {
                entity,
                episode_count: episode_count as usize,
                success_count: success_count as usize,
                success_rate: success_rate as f32,
                pattern_type: pattern_type.to_string(),
            });
        }

        Ok(patterns)
    }

    /// Create a goal in the graph
    pub async fn create_goal(
        &self,
        goal_id: &str,
        description: &str,
        status: &str,
    ) -> Result<(), Neo4jError> {
        let q = query(
            "MATCH (a:Agent {id: $agent_id})
             CREATE (g:Goal {
                id: $id,
                description: $description,
                status: $status,
                created_at: datetime()
             })
             CREATE (a)-[:HAS_GOAL]->(g)",
        )
        .param("agent_id", self.agent_id.clone())
        .param("id", goal_id.to_string())
        .param("description", description.to_string())
        .param("status", status.to_string());

        self.graph.run(q).await?;
        Ok(())
    }

    /// Update goal status
    pub async fn update_goal_status(&self, goal_id: &str, status: &str) -> Result<(), Neo4jError> {
        let q = query(
            "MATCH (g:Goal {id: $goal_id})
             SET g.status = $status,
                 g.updated_at = datetime()",
        )
        .param("goal_id", goal_id.to_string())
        .param("status", status.to_string());

        self.graph.run(q).await?;
        Ok(())
    }

    /// Get agent statistics
    pub async fn get_agent_stats(&self) -> Result<AgentStats, Neo4jError> {
        let q = query(
            "MATCH (a:Agent {id: $agent_id})
             OPTIONAL MATCH (a)-[:EXPERIENCED]->(e:Episode)
             WITH a, count(e) as episode_count,
                  sum(CASE WHEN e.success THEN 1 ELSE 0 END) as success_count
             OPTIONAL MATCH (a)-[:HAS_GOAL]->(g:Goal)
             WITH a, episode_count, success_count, count(g) as goal_count,
                  sum(CASE WHEN g.status = 'completed' THEN 1 ELSE 0 END) as completed_goals
             OPTIONAL MATCH (a)-[:REFLECTED]->(r:Reflection)
             RETURN episode_count,
                    success_count,
                    goal_count,
                    completed_goals,
                    count(r) as reflection_count",
        )
        .param("agent_id", self.agent_id.clone());

        let mut result = self.graph.execute(q).await?;

        if let Ok(Some(row)) = result.next().await {
            Ok(AgentStats {
                episode_count: row.get::<i64>("episode_count").unwrap_or(0) as usize,
                success_count: row.get::<i64>("success_count").unwrap_or(0) as usize,
                goal_count: row.get::<i64>("goal_count").unwrap_or(0) as usize,
                completed_goals: row.get::<i64>("completed_goals").unwrap_or(0) as usize,
                reflection_count: row.get::<i64>("reflection_count").unwrap_or(0) as usize,
            })
        } else {
            Ok(AgentStats::default())
        }
    }
}
