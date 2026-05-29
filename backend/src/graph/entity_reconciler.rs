// backend/src/graph/entity_reconciler.rs
//
// Collapses surface-form variants of the same real-world entity into one
// canonical :Entity node, replacing the naive `lowercase(name) MERGE` keying
// in KnowledgeBuilder::add_entity_mention.
//
// Three-state decision per candidate:
//   * top vector score ≥ auto_merge_threshold → merge, no LLM call
//   * top vector score ≥ llm_review_threshold → ask LLM YES/NEW
//   * otherwise                               → create new entity
//
// On any failure (FalkorDB vector index missing, LLM unreachable) the
// reconciler falls back to "create new" so ingest never blocks on it.

use crate::graph::client::{lit, now_millis, row_f64, row_str, GraphClientError, GraphHandle};
use crate::graph::config::EntityExtractionSettings;
use crate::params;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct EntityCandidate {
    pub name: String,
    pub entity_type: String,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct CanonicalEntity {
    pub id: String,
    pub created: bool,
    /// Surface form that was supplied for this mention (kept for alias tracking).
    pub surface_form: String,
}

#[derive(Debug, Default)]
pub struct ReconcilerStats {
    pub auto_merges: AtomicUsize,
    pub llm_merges: AtomicUsize,
    pub llm_news: AtomicUsize,
    pub auto_news: AtomicUsize,
    pub llm_calls: AtomicUsize,
    pub fallbacks: AtomicUsize,
}

impl ReconcilerStats {
    pub fn snapshot(&self) -> ReconcilerStatsSnapshot {
        ReconcilerStatsSnapshot {
            auto_merges: self.auto_merges.load(Ordering::Relaxed),
            llm_merges: self.llm_merges.load(Ordering::Relaxed),
            llm_news: self.llm_news.load(Ordering::Relaxed),
            auto_news: self.auto_news.load(Ordering::Relaxed),
            llm_calls: self.llm_calls.load(Ordering::Relaxed),
            fallbacks: self.fallbacks.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReconcilerStatsSnapshot {
    pub auto_merges: usize,
    pub llm_merges: usize,
    pub llm_news: usize,
    pub auto_news: usize,
    pub llm_calls: usize,
    pub fallbacks: usize,
}

pub struct EntityReconciler {
    graph: GraphHandle,
    settings: EntityExtractionSettings,
    stats: ReconcilerStats,
    llm_budget_remaining: AtomicUsize,
}

impl EntityReconciler {
    /// Build a reconciler scoped to a single document ingest.  The per-doc
    /// LLM tiebreak budget resets per instance.
    pub fn new(graph: GraphHandle, settings: EntityExtractionSettings) -> Self {
        let llm_budget = settings.reconcile_llm_review_max_per_doc;
        Self {
            graph,
            settings,
            stats: ReconcilerStats::default(),
            llm_budget_remaining: AtomicUsize::new(llm_budget),
        }
    }

    pub fn stats(&self) -> ReconcilerStatsSnapshot {
        self.stats.snapshot()
    }

    /// Resolve a candidate to a canonical :Entity node id, creating the node
    /// if no acceptable match exists.  `snippet` is the surrounding chunk text
    /// that gets stored as the definition snippet for future tiebreaks.
    pub async fn reconcile(
        &self,
        candidate: EntityCandidate,
        snippet: &str,
    ) -> Result<CanonicalEntity, GraphClientError> {
        // Name + type only — snippet was dominating the embedding and
        // dropping same-entity cosine from ~0.98 to ~0.85, defeating the
        // auto_merge threshold. Snippet still flows into `definition_snippet`
        // (for the LLM tiebreak in llm_says_same) via create_entity below.
        let qtext = format!("{} ({})", candidate.name, candidate.entity_type);
        let embedding = crate::embedder::embed(&qtext);

        // Top-k nearest existing entities of the same type.
        let neighbours = match self
            .top_k_neighbours(&embedding, &candidate.entity_type)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                debug!(error = %e, "vector lookup failed; falling back to create-new");
                self.stats.fallbacks.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }
        };

        if let Some(top) = neighbours.first() {
            if top.score >= self.settings.reconcile_auto_merge_threshold as f64 {
                self.stats.auto_merges.fetch_add(1, Ordering::Relaxed);
                self.touch_existing(&top.id, &candidate.name).await?;
                return Ok(CanonicalEntity {
                    id: top.id.clone(),
                    created: false,
                    surface_form: candidate.name,
                });
            }
            if top.score >= self.settings.reconcile_llm_review_threshold as f64
                && self.llm_budget_remaining.load(Ordering::Relaxed) > 0
            {
                self.llm_budget_remaining.fetch_sub(1, Ordering::Relaxed);
                self.stats.llm_calls.fetch_add(1, Ordering::Relaxed);
                if self
                    .llm_says_same(&candidate.name, snippet, &top.name, &top.snippet)
                    .await
                {
                    self.stats.llm_merges.fetch_add(1, Ordering::Relaxed);
                    self.touch_existing(&top.id, &candidate.name).await?;
                    return Ok(CanonicalEntity {
                        id: top.id.clone(),
                        created: false,
                        surface_form: candidate.name,
                    });
                } else {
                    self.stats.llm_news.fetch_add(1, Ordering::Relaxed);
                    // Fall through to create new.
                }
            } else {
                self.stats.auto_news.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            self.stats.auto_news.fetch_add(1, Ordering::Relaxed);
        }

        // Create new :Entity with the candidate's embedding + snippet.
        let new_id = uuid::Uuid::new_v4().to_string();
        self.create_entity(&new_id, &candidate, snippet, &embedding)
            .await?;
        Ok(CanonicalEntity {
            id: new_id,
            created: true,
            surface_form: candidate.name,
        })
    }

    async fn top_k_neighbours(
        &self,
        embedding: &[f32],
        entity_type: &str,
    ) -> Result<Vec<NeighbourRow>, GraphClientError> {
        let mut params = std::collections::HashMap::new();
        params.insert("type".to_string(), lit::str(entity_type));

        // vecf32(...) must be inlined into the query body — FalkorDB's bound-params
        // parser rejects the constructor as a parameter value. See lit::vecf32 docs.
        let cypher = format!(
            "CALL db.idx.vector.queryNodes('Entity', 'embedding', {k}, {qvec}) \
             YIELD node, score \
             WHERE node.entity_type = $type \
             RETURN node.id AS id, node.name AS name, \
                    coalesce(node.definition_snippet, '') AS snippet, score",
            k = self.settings.reconcile_vector_topk as i64,
            qvec = lit::vecf32(embedding),
        );
        let rows = self.graph.query(&cypher, &params).await?;
        Ok(rows
            .into_iter()
            .map(|r| NeighbourRow {
                id: row_str(&r, 0),
                name: row_str(&r, 1),
                snippet: row_str(&r, 2),
                // FalkorDB's 'cosine' vector index returns cosine *distance*
                // (0 = identical, higher = farther apart), not similarity as
                // the docs imply. Convert here so the rest of the reconciler
                // can keep using similarity-style threshold checks against
                // reconcile_auto_merge_threshold / reconcile_llm_review_threshold.
                // Default 1.0 (max distance → 0 similarity) on missing column.
                score: 1.0 - row_f64(&r, 3, 1.0),
            })
            .collect())
    }

    async fn create_entity(
        &self,
        id: &str,
        candidate: &EntityCandidate,
        snippet: &str,
        embedding: &[f32],
    ) -> Result<(), GraphClientError> {
        let snippet_trimmed = snippet.chars().take(1024).collect::<String>();
        let params = params! {
            "id" => lit::str(id),
            "name" => lit::str(&candidate.name),
            "normalized" => lit::str(&candidate.name.trim().to_lowercase()),
            "type" => lit::str(&candidate.entity_type),
            "snippet" => lit::str(&snippet_trimmed),
            "aliases" => lit::str_list(&[candidate.name.clone()]),
            "now" => lit::int(now_millis()),
        };
        // vecf32(...) must be inlined — see lit::vecf32 docs.
        let cypher = format!(
            "CREATE (e:Entity {{id: $id}}) \
             SET e.name = $name, \
                 e.normalized_name = $normalized, \
                 e.entity_type = $type, \
                 e.definition_snippet = $snippet, \
                 e.aliases = $aliases, \
                 e.embedding = {embedding}, \
                 e.mention_count = 1, \
                 e.first_seen = $now, \
                 e.last_seen = $now",
            embedding = lit::vecf32(embedding),
        );
        self.graph.run(&cypher, &params).await?;
        Ok(())
    }

    async fn touch_existing(
        &self,
        entity_id: &str,
        surface_form: &str,
    ) -> Result<(), GraphClientError> {
        let params = params! {
            "id" => lit::str(entity_id),
            "surface" => lit::str(surface_form),
            "now" => lit::int(now_millis()),
        };
        // Bump mention_count + last_seen, and append the new surface form to
        // aliases when unseen.  `aliases` may not exist on older nodes; coalesce.
        self.graph
            .run(
                "MATCH (e:Entity {id: $id}) \
                 SET e.mention_count = coalesce(e.mention_count, 0) + 1, \
                     e.last_seen = $now, \
                     e.aliases = CASE \
                         WHEN $surface IN coalesce(e.aliases, []) THEN e.aliases \
                         ELSE coalesce(e.aliases, []) + [$surface] \
                     END",
                &params,
            )
            .await?;
        Ok(())
    }

    /// One-shot YES/NEW probe against Ollama.  On any error returns `false`
    /// (treat as NEW) so ingest doesn't block.
    async fn llm_says_same(&self, a: &str, a_snip: &str, b: &str, b_snip: &str) -> bool {
        let url =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "phi:latest".to_string());
        let prompt = format!(
            "Are these two entities the same real-world entity? Reply with only YES or NEW.\n\
             A: {a}\nContext A: {a_snip}\n\
             B: {b}\nContext B: {b_snip}\n\
             Answer:",
            a = a,
            a_snip = a_snip.chars().take(300).collect::<String>(),
            b = b,
            b_snip = b_snip.chars().take(300).collect::<String>()
        );
        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "options": { "temperature": 0.0, "num_predict": 8 }
        });
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };
        let resp = match client
            .post(format!("{url}/api/generate"))
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "reconciler LLM call failed; treating as NEW");
                return false;
            }
        };
        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(_) => return false,
        };
        let answer = json
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_uppercase();
        answer.starts_with("YES")
    }
}

struct NeighbourRow {
    id: String,
    name: String,
    snippet: String,
    score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_snapshot_starts_zero() {
        let s = ReconcilerStats::default();
        let snap = s.snapshot();
        assert_eq!(snap.auto_merges, 0);
        assert_eq!(snap.llm_calls, 0);
    }

    #[test]
    fn stats_increment_independently() {
        let s = ReconcilerStats::default();
        s.auto_merges.fetch_add(3, Ordering::Relaxed);
        s.llm_calls.fetch_add(2, Ordering::Relaxed);
        s.llm_merges.fetch_add(1, Ordering::Relaxed);
        let snap = s.snapshot();
        assert_eq!(snap.auto_merges, 3);
        assert_eq!(snap.llm_calls, 2);
        assert_eq!(snap.llm_merges, 1);
        assert_eq!(snap.auto_news, 0);
    }

    #[test]
    #[ignore = "diagnostic probe: cargo test --features graph -- --nocapture --ignored probe_reconciler_cosines"]
    fn probe_reconciler_cosines() {
        use crate::embedder::{self, similarity::cosine};

        let pairs: &[(&str, &str, &str)] = &[
            ("identity",         "Hitachi Energy",       "Hitachi Energy"),
            ("bare names",       "Hitachi Energy",       "Hitachi Energy Ltd"),
            ("name + type",      "Hitachi Energy (ORG)", "Hitachi Energy Ltd (ORG)"),
            ("current qtext",
             "Hitachi Energy (ORG): Hitachi Energy reported strong Q3 results.",
             "Hitachi Energy Ltd (ORG): Hitachi Energy Ltd expanded operations in Tokyo."),
            ("Sony Corp/Corp",   "Sony Corp (ORG)",      "Sony Corporation (ORG)"),
            ("Sony Corp/SIE",    "Sony Corp (ORG)",      "Sony Interactive Entertainment (ORG)"),
            // Cross-format: new bare-name query lookup against old stored-with-snippet vectors.
            ("xfmt same name",   "Hitachi Energy (ORG)",
             "Hitachi Energy (ORG): Hitachi Energy reported strong Q3 results."),
            ("xfmt diff name",   "Hitachi Energy (ORG)",
             "Hitachi Energy Ltd (ORG): Hitachi Energy Ltd expanded operations in Tokyo."),
        ];
        for (label, a, b) in pairs {
            let va = embedder::embed(a);
            let vb = embedder::embed(b);
            println!("{:>20}  cosine={:.4}  a={:?}  b={:?}", label, cosine(&va, &vb), a, b);
        }
    }
}
