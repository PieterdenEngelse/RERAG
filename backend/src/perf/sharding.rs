//! Sharding for Horizontal Scaling
//!
//! Distributes vectors across multiple shards for:
//! - Horizontal scaling beyond single machine
//! - Parallel search across shards
//! - Fault tolerance with replication
//!
//! # Sharding Strategies
//! - Hash-based: Consistent hashing for even distribution
//! - Range-based: For ordered data
//! - Directory-based: Lookup table for flexibility

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Shard identifier
pub type ShardId = u32;

/// Sharding strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ShardingStrategy {
    /// Consistent hashing
    Hash,
    /// Range-based partitioning
    Range,
    /// Directory-based lookup
    Directory,
}

/// Shard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    pub id: ShardId,
    pub address: String,
    pub port: u16,
    pub weight: u32,
    pub is_primary: bool,
    pub replicas: Vec<String>,
}

impl ShardConfig {
    pub fn new(id: ShardId, address: &str, port: u16) -> Self {
        Self {
            id,
            address: address.to_string(),
            port,
            weight: 1,
            is_primary: true,
            replicas: Vec::new(),
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

/// Consistent hash ring for shard selection
#[allow(dead_code)]
pub struct HashRing {
    ring: Vec<(u64, ShardId)>,
    virtual_nodes: u32,
}

impl HashRing {
    pub fn new(shards: &[ShardConfig], virtual_nodes: u32) -> Self {
        let mut ring = Vec::new();

        for shard in shards {
            for i in 0..virtual_nodes * shard.weight {
                let key = format!("{}:{}", shard.id, i);
                let hash = seahash::hash(key.as_bytes());
                ring.push((hash, shard.id));
            }
        }

        ring.sort_by_key(|(hash, _)| *hash);

        Self {
            ring,
            virtual_nodes,
        }
    }

    /// Get shard for a key
    pub fn get_shard(&self, key: &str) -> Option<ShardId> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = seahash::hash(key.as_bytes());

        // Binary search for the first node with hash >= key hash
        let idx = match self.ring.binary_search_by_key(&hash, |(h, _)| *h) {
            Ok(i) => i,
            Err(i) => i % self.ring.len(),
        };

        Some(self.ring[idx].1)
    }

    /// Get multiple shards for replication
    pub fn get_shards(&self, key: &str, count: usize) -> Vec<ShardId> {
        if self.ring.is_empty() {
            return Vec::new();
        }

        let hash = seahash::hash(key.as_bytes());
        let start_idx = match self.ring.binary_search_by_key(&hash, |(h, _)| *h) {
            Ok(i) => i,
            Err(i) => i % self.ring.len(),
        };

        let mut shards = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut idx = start_idx;

        while shards.len() < count && seen.len() < self.ring.len() {
            let shard_id = self.ring[idx].1;
            if seen.insert(shard_id) {
                shards.push(shard_id);
            }
            idx = (idx + 1) % self.ring.len();
        }

        shards
    }
}

/// Shard router
pub struct ShardRouter {
    shards: HashMap<ShardId, ShardConfig>,
    ring: HashRing,
    strategy: ShardingStrategy,
}

impl ShardRouter {
    pub fn new(shards: Vec<ShardConfig>, strategy: ShardingStrategy) -> Self {
        let ring = HashRing::new(&shards, 150); // 150 virtual nodes per shard
        let shard_map: HashMap<ShardId, ShardConfig> =
            shards.into_iter().map(|s| (s.id, s)).collect();

        Self {
            shards: shard_map,
            ring,
            strategy,
        }
    }

    /// Route a key to its shard
    pub fn route(&self, key: &str) -> Option<&ShardConfig> {
        match self.strategy {
            ShardingStrategy::Hash => self.ring.get_shard(key).and_then(|id| self.shards.get(&id)),
            ShardingStrategy::Range | ShardingStrategy::Directory => {
                // For simplicity, fall back to hash
                self.ring.get_shard(key).and_then(|id| self.shards.get(&id))
            }
        }
    }

    /// Route with replication
    pub fn route_with_replicas(&self, key: &str, replica_count: usize) -> Vec<&ShardConfig> {
        self.ring
            .get_shards(key, replica_count)
            .into_iter()
            .filter_map(|id| self.shards.get(&id))
            .collect()
    }

    /// Get all shards (for scatter-gather queries)
    pub fn all_shards(&self) -> Vec<&ShardConfig> {
        self.shards.values().collect()
    }

    /// Get shard by ID
    pub fn get_shard(&self, id: ShardId) -> Option<&ShardConfig> {
        self.shards.get(&id)
    }

    /// Number of shards
    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }
}

/// Sharded search coordinator
#[allow(dead_code)]
pub struct ShardedSearch {
    router: ShardRouter,
    timeout_ms: u64,
}

impl ShardedSearch {
    pub fn new(router: ShardRouter) -> Self {
        Self {
            router,
            timeout_ms: 5000,
        }
    }

    /// Plan a search query across shards
    pub fn plan_search(&self, query: &str) -> SearchPlan {
        // For vector search, we need to query all shards (scatter-gather)
        let shards: Vec<ShardId> = self.router.all_shards().iter().map(|s| s.id).collect();

        SearchPlan {
            query: query.to_string(),
            shards,
            strategy: SearchStrategy::ScatterGather,
        }
    }

    /// Plan a document lookup
    pub fn plan_lookup(&self, doc_id: &str) -> LookupPlan {
        let shard = self.router.route(doc_id).map(|s| s.id);

        LookupPlan {
            doc_id: doc_id.to_string(),
            shard,
        }
    }

    /// Merge results from multiple shards
    pub fn merge_results(&self, results: Vec<ShardResult>, top_k: usize) -> Vec<MergedResult> {
        let mut all_results: Vec<MergedResult> = results
            .into_iter()
            .flat_map(|sr| {
                sr.results.into_iter().map(move |r| MergedResult {
                    doc_id: r.doc_id,
                    score: r.score,
                    shard_id: sr.shard_id,
                })
            })
            .collect();

        // Sort by score descending
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        all_results.truncate(top_k);
        all_results
    }
}

/// Search plan
#[derive(Debug, Clone)]
pub struct SearchPlan {
    pub query: String,
    pub shards: Vec<ShardId>,
    pub strategy: SearchStrategy,
}

/// Search strategy
#[derive(Debug, Clone, Copy)]
pub enum SearchStrategy {
    /// Query all shards and merge
    ScatterGather,
    /// Query specific shard
    Targeted,
}

/// Lookup plan
#[derive(Debug, Clone)]
pub struct LookupPlan {
    pub doc_id: String,
    pub shard: Option<ShardId>,
}

/// Result from a single shard
#[derive(Debug, Clone)]
pub struct ShardResult {
    pub shard_id: ShardId,
    pub results: Vec<DocResult>,
    pub took_ms: u64,
}

/// Document result
#[derive(Debug, Clone)]
pub struct DocResult {
    pub doc_id: String,
    pub score: f32,
}

/// Merged result from multiple shards
#[derive(Debug, Clone)]
pub struct MergedResult {
    pub doc_id: String,
    pub score: f32,
    pub shard_id: ShardId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_ring() {
        let shards = vec![
            ShardConfig::new(0, "localhost", 5000),
            ShardConfig::new(1, "localhost", 5001),
            ShardConfig::new(2, "localhost", 5002),
        ];

        let ring = HashRing::new(&shards, 100);

        // Same key should always go to same shard
        let shard1 = ring.get_shard("test_key");
        let shard2 = ring.get_shard("test_key");
        assert_eq!(shard1, shard2);

        // Different keys should distribute across shards
        let mut distribution = HashMap::new();
        for i in 0..1000 {
            let key = format!("key_{}", i);
            if let Some(shard) = ring.get_shard(&key) {
                *distribution.entry(shard).or_insert(0) += 1;
            }
        }

        // Should have some distribution across all shards
        assert!(distribution.len() >= 2);
    }

    #[test]
    fn test_shard_router() {
        let shards = vec![
            ShardConfig::new(0, "localhost", 5000),
            ShardConfig::new(1, "localhost", 5001),
        ];

        let router = ShardRouter::new(shards, ShardingStrategy::Hash);

        assert_eq!(router.shard_count(), 2);
        assert!(router.route("test_doc").is_some());
    }

    #[test]
    fn test_merge_results() {
        let router = ShardRouter::new(
            vec![ShardConfig::new(0, "localhost", 5000)],
            ShardingStrategy::Hash,
        );
        let search = ShardedSearch::new(router);

        let results = vec![
            ShardResult {
                shard_id: 0,
                results: vec![
                    DocResult {
                        doc_id: "a".to_string(),
                        score: 0.9,
                    },
                    DocResult {
                        doc_id: "b".to_string(),
                        score: 0.7,
                    },
                ],
                took_ms: 10,
            },
            ShardResult {
                shard_id: 1,
                results: vec![DocResult {
                    doc_id: "c".to_string(),
                    score: 0.8,
                }],
                took_ms: 15,
            },
        ];

        let merged = search.merge_results(results, 2);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].doc_id, "a"); // Highest score
        assert_eq!(merged[1].doc_id, "c"); // Second highest
    }
}
