#!/usr/bin/env python3
"""
Batch 5: Move index_to_knowledge_graph out of api/mod.rs
  - Add the function to graph/mod.rs (takes &KnowledgeBuilder param)
  - Delete both cfg variants from api/mod.rs (~130 lines)
  - Update call site to pass kb from global
  - Re-export from graph module

Expected savings: ~120 lines (net)
"""
import sys, os, shutil
from datetime import datetime

MOD_RS = os.path.expanduser("~/ag/backend/src/api/mod.rs")
GRAPH_MOD = os.path.expanduser("~/ag/backend/src/graph/mod.rs")
ts = datetime.now().strftime("%Y%m%d_%H%M%S")
errors = []
changes = []

for f in [MOD_RS, GRAPH_MOD]:
    if not os.path.exists(f):
        print(f"FATAL: {f} not found")
        sys.exit(1)
    shutil.copy2(f, f"{f}.bak.{ts}")
    print(f"[OK] Backed up {os.path.basename(f)}")

# ═══════════════════════════════════════════════════════════════
# FILE 1: graph/mod.rs — add index_to_knowledge_graph
# ═══════════════════════════════════════════════════════════════

with open(GRAPH_MOD, 'r') as f:
    graph_content = f.read()

# Add the function before the #[cfg(test)] line at the end
test_anchor = '#[cfg(test)]\nmod petgraph_runtime_test;'

new_function = '''/// Process a document and its chunks through the knowledge graph.
/// Extracts entities and stores them in Neo4j.
#[cfg(feature = "neo4j")]
pub async fn index_to_knowledge_graph(
    kb: &KnowledgeBuilder,
    doc_id: &str,
    title: &str,
    source: &str,
    chunks: &[(String, String)], // (chunk_id, chunk_content)
) {
    use crate::tools::entity_extractor::EntityExtractorTool;
    use tracing::{debug, warn};

    // Check if entity extraction is enabled
    let graph_config = config::GraphConfig::from_env();
    if !graph_config.entity_extraction.enabled {
        debug!("Entity extraction disabled, skipping knowledge graph indexing");
        return;
    }

    // Add document to graph
    let doc_meta = knowledge_builder::DocumentMeta {
        id: doc_id.to_string(),
        title: title.to_string(),
        source: source.to_string(),
        content_hash: {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            title.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        },
        mime_type: "text/plain".to_string(),
        chunk_count: chunks.len(),
    };

    if let Err(e) = kb.add_document(&doc_meta).await {
        warn!(error = %e, doc_id = %doc_id, "Failed to add document to knowledge graph");
        return;
    }

    // Process each chunk
    let extractor = EntityExtractorTool::new();
    let confidence_threshold = graph_config.entity_extraction.confidence_threshold;

    for (chunk_id, chunk_content) in chunks {
        // Yield between chunks to prevent CPU starvation
        tokio::task::yield_now().await;

        // Add chunk to graph
        let chunk_meta = knowledge_builder::ChunkMeta {
            id: chunk_id.clone(),
            document_id: doc_id.to_string(),
            content: chunk_content.clone(),
            embedding_id: chunk_id.clone(),
            position: chunk_id
                .split('#')
                .last()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            token_count: chunk_content.split_whitespace().count(),
        };

        if let Err(e) = kb.add_chunk(&chunk_meta).await {
            warn!(error = %e, chunk_id = %chunk_id, "Failed to add chunk to knowledge graph");
            continue;
        }

        // Extract entities - try ONNX NER first, fall back to regex
        let ner_entities = crate::tools::ner_extractor::extract_entities(chunk_content);
        let use_ner = !ner_entities.is_empty();
        if use_ner {
            for ner_entity in &ner_entities {
                if let Err(e) = kb.add_entity_mention(
                    chunk_id, &ner_entity.text, &ner_entity.label, ner_entity.score,
                ).await {
                    debug!(error = %e, entity = %ner_entity.text, "Failed to add NER entity");
                }
            }
        }
        // Fallback regex extraction
        let extraction = extractor.extract(chunk_content);

        for entity in &extraction.entities {
            if !use_ner && entity.confidence >= confidence_threshold {
                if let Err(e) = kb
                    .add_entity_mention(
                        chunk_id,
                        &entity.text,
                        entity.entity_type.label(),
                        entity.confidence,
                    )
                    .await
                {
                    debug!(error = %e, entity = %entity.text, "Failed to add entity mention");
                }
            }
        }

        // Link co-occurring entities (entities in the same chunk are related)
        let high_confidence_entities: Vec<_> = extraction
            .entities
            .iter()
            .filter(|e| e.confidence >= confidence_threshold)
            .collect();

        for i in 0..high_confidence_entities.len() {
            for j in (i + 1)..high_confidence_entities.len() {
                let e1 = &high_confidence_entities[i];
                let e2 = &high_confidence_entities[j];
                let _ = kb
                    .link_entities(
                        &e1.text,
                        &e2.text,
                        "co_occurs_with",
                        (e1.confidence + e2.confidence) / 2.0,
                    )
                    .await;
            }
        }
    }

    debug!(
        doc_id = %doc_id,
        chunks = chunks.len(),
        "Indexed document to knowledge graph"
    );
}

/// No-op when neo4j feature is disabled
#[cfg(not(feature = "neo4j"))]
pub async fn index_to_knowledge_graph(
    _doc_id: &str,
    _title: &str,
    _source: &str,
    _chunks: &[(String, String)],
) {
    // No-op
}

#[cfg(test)]
mod petgraph_runtime_test;'''

if test_anchor in graph_content:
    graph_content = graph_content.replace(test_anchor, new_function, 1)
    changes.append("Added index_to_knowledge_graph to graph/mod.rs")
else:
    errors.append("FATAL: Could not find #[cfg(test)] anchor in graph/mod.rs")

with open(GRAPH_MOD, 'w') as f:
    f.write(graph_content)
print("[OK] Updated graph/mod.rs")

# ═══════════════════════════════════════════════════════════════
# FILE 2: api/mod.rs — delete function + update call site
# ═══════════════════════════════════════════════════════════════

with open(MOD_RS, 'r') as f:
    content = f.read()

original_lines = content.count('\n')

# --- DELETE: Both cfg variants of index_to_knowledge_graph ---
delete_block = '''/// Process a document and its chunks through the knowledge graph
/// This extracts entities and stores them in Neo4j
#[cfg(feature = "neo4j")]
pub async fn index_to_knowledge_graph(
    doc_id: &str,
    title: &str,
    source: &str,
    chunks: &[(String, String)], // (chunk_id, chunk_content)
) {
    use crate::graph::knowledge_builder::{ChunkMeta, DocumentMeta};
    use crate::tools::entity_extractor::EntityExtractorTool;
    use tracing::{debug, warn};

    let Some(kb) = get_knowledge_builder() else {
        return;
    };

    // Check if entity extraction is enabled
    let config = crate::graph::config::GraphConfig::from_env();
    if !config.entity_extraction.enabled {
        debug!("Entity extraction disabled, skipping knowledge graph indexing");
        return;
    }

    // Add document to graph
    let doc_meta = DocumentMeta {
        id: doc_id.to_string(),
        title: title.to_string(),
        source: source.to_string(),
        content_hash: {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            title.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        },
        mime_type: "text/plain".to_string(),
        chunk_count: chunks.len(),
    };

    if let Err(e) = kb.add_document(&doc_meta).await {
        warn!(error = %e, doc_id = %doc_id, "Failed to add document to knowledge graph");
        return;
    }

    // Process each chunk
    let extractor = EntityExtractorTool::new();
    let confidence_threshold = config.entity_extraction.confidence_threshold;

    for (chunk_id, chunk_content) in chunks {
        // B1-v1: Yield between chunks to prevent CPU starvation
        tokio::task::yield_now().await;

        // Add chunk to graph
        let chunk_meta = ChunkMeta {
            id: chunk_id.clone(),
            document_id: doc_id.to_string(),
            content: chunk_content.clone(),
            embedding_id: chunk_id.clone(),
            position: chunk_id
                .split('#')
                .last()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            token_count: chunk_content.split_whitespace().count(),
        };

        if let Err(e) = kb.add_chunk(&chunk_meta).await {
            warn!(error = %e, chunk_id = %chunk_id, "Failed to add chunk to knowledge graph");
            continue;
        }

        // Extract entities - try ONNX NER first, fall back to regex
        let ner_entities = crate::tools::ner_extractor::extract_entities(chunk_content);
        let use_ner = !ner_entities.is_empty();
        if use_ner {
            for ner_entity in &ner_entities {
                if let Err(e) = kb.add_entity_mention(
                    chunk_id, &ner_entity.text, &ner_entity.label, ner_entity.score,
                ).await {
                    debug!(error = %e, entity = %ner_entity.text, "Failed to add NER entity");
                }
            }
        }
        // Fallback regex extraction
        let extraction = extractor.extract(chunk_content);

        for entity in &extraction.entities {
            if !use_ner && entity.confidence >= confidence_threshold {
                if let Err(e) = kb
                    .add_entity_mention(
                        chunk_id,
                        &entity.text,
                        entity.entity_type.label(),
                        entity.confidence,
                    )
                    .await
                {
                    debug!(error = %e, entity = %entity.text, "Failed to add entity mention");
                }
            }
        }

        // Link co-occurring entities (entities in the same chunk are related)
        let high_confidence_entities: Vec<_> = extraction
            .entities
            .iter()
            .filter(|e| e.confidence >= confidence_threshold)
            .collect();

        for i in 0..high_confidence_entities.len() {
            for j in (i + 1)..high_confidence_entities.len() {
                let e1 = &high_confidence_entities[i];
                let e2 = &high_confidence_entities[j];
                let _ = kb
                    .link_entities(
                        &e1.text,
                        &e2.text,
                        "co_occurs_with",
                        (e1.confidence + e2.confidence) / 2.0,
                    )
                    .await;
            }
        }
    }

    debug!(
        doc_id = %doc_id,
        chunks = chunks.len(),
        "Indexed document to knowledge graph"
    );
}

#[cfg(not(feature = "neo4j"))]
pub async fn index_to_knowledge_graph(
    _doc_id: &str,
    _title: &str,
    _source: &str,
    _chunks: &[(String, String)],
) {
    // No-op when neo4j feature is disabled
}

'''

if delete_block in content:
    content = content.replace(delete_block, '', 1)
    changes.append("Deleted both index_to_knowledge_graph variants from api/mod.rs")
else:
    errors.append("FATAL: index_to_knowledge_graph block not found in api/mod.rs")

# --- UPDATE call site: pass kb from global ---
# The non-neo4j version has no kb parameter, so we need cfg-gated call
old_call = '            for (filename, source, chunks) in graph_index_tasks {\n                index_to_knowledge_graph(&filename, &filename, &source, &chunks).await;\n            }'

new_call = '''            for (filename, source, chunks) in graph_index_tasks {
                #[cfg(feature = "neo4j")]
                if let Some(kb) = get_knowledge_builder() {
                    crate::graph::index_to_knowledge_graph(&kb, &filename, &filename, &source, &chunks).await;
                }
                #[cfg(not(feature = "neo4j"))]
                { let _ = (&filename, &source, &chunks); }
            }'''

if old_call in content:
    content = content.replace(old_call, new_call, 1)
    changes.append("Updated call site to pass kb and use crate::graph path")
else:
    errors.append("FATAL: Call site not found")

# ═══════════════════════════════════════════════════════════════
# WRITE RESULT
# ═══════════════════════════════════════════════════════════════

new_lines = content.count('\n')
saved = original_lines - new_lines

with open(MOD_RS, 'w') as f:
    f.write(content)

print(f"\n{'='*60}")
print(f"CHANGES APPLIED:")
for c in changes:
    print(f"  ✓ {c}")

if errors:
    print(f"\nWARNINGS/ERRORS:")
    for e in errors:
        print(f"  ⚠ {e}")

fatal = [e for e in errors if e.startswith("FATAL")]
if fatal:
    print("\nFATAL errors occurred — check files and restore from .bak")
    sys.exit(1)

print(f"\nmod.rs: {original_lines} → {new_lines} (saved {saved})")
print(f"\nNext: cd ~/ag && cargo check 2>&1 | head -30")
