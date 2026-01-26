# Knowledge Graph Builder

The Knowledge Builder automatically extracts entities from uploaded documents and stores them in Neo4j, creating a queryable knowledge graph.

## How It Works

1. **Document Upload** → Document gets chunked and indexed in Tantivy (vector search)
2. **Graph Indexing** → Each chunk is processed through the knowledge graph:
   - Document node created in Neo4j
   - Chunk nodes created and linked to document via `HAS_CHUNK` relationship
   - Entities extracted (people, orgs, locations, dates, emails, URLs, etc.)
   - Entity nodes created and linked to chunks via `MENTIONS` relationship with confidence scores
   - Co-occurring entities linked with `co_occurs_with` relationships

## Configuration

Add these to your `.env` file:

```bash
# Neo4j Knowledge Graph
NEO4J_ENABLED=true
NEO4J_URI=bolt://localhost:7687
NEO4J_USER=neo4j
NEO4J_PASSWORD=agpassword123

# Entity Extraction - Automatically extract named entities from documents
# When enabled, the system identifies people, organizations, locations, dates,
# emails, URLs, money amounts, and percentages from uploaded documents.
# Extracted entities are stored in Neo4j for knowledge graph queries.
ENTITY_EXTRACTION_ENABLED=true

# Minimum confidence score (0.0-1.0) for an entity to be stored.
# Higher values = fewer but more accurate entities. Default: 0.7
ENTITY_EXTRACTION_CONFIDENCE_THRESHOLD=0.7

# Comma-separated list of entity types to extract.
# Available: person, organization, location, date, email, url, phone, money, percentage
# Default: all types
ENTITY_EXTRACTION_TYPES=person,organization,location,date,email,url,phone,money,percentage
```

## Starting the Services

```bash
# Start Neo4j container
docker compose up -d neo4j

# Start backend with Neo4j feature
cd backend && cargo run --features neo4j
```

## Graph Schema

### Nodes

| Label | Properties | Description |
|-------|------------|-------------|
| `Document` | id, title, source, content_hash, mime_type, chunk_count, indexed_at | Uploaded document |
| `Chunk` | id, content, embedding_id, position, token_count, created_at | Text chunk from document |
| `Entity` | id, name, normalized_name, entity_type, mention_count, first_seen, last_seen | Extracted entity |
| `Concept` | id, name, description, domain, updated_at | Abstract concept (optional) |

### Relationships

| Type | From | To | Properties | Description |
|------|------|-----|------------|-------------|
| `HAS_CHUNK` | Document | Chunk | position | Document contains chunk |
| `MENTIONS` | Chunk | Entity | confidence | Chunk mentions entity |
| `RELATED_TO` | Entity | Entity | relation_type, strength, evidence_count | Entity relationship |
| `co_occurs_with` | Entity | Entity | strength | Entities appear in same chunk |

### Entity Types

- `PERSON` - People names
- `ORG` - Organizations, companies
- `LOC` - Locations, places
- `DATE` - Dates
- `TIME` - Times
- `MONEY` - Currency amounts
- `PERCENT` - Percentages
- `EMAIL` - Email addresses
- `URL` - Web URLs
- `PHONE` - Phone numbers
- `PRODUCT` - Product names
- `EVENT` - Events
- `TECH` - Technology terms

## Querying the Graph

Access Neo4j Browser at http://localhost:7474 (login: neo4j / agpassword123)

### Example Queries

**See all entities:**
```cypher
MATCH (e:Entity) RETURN e LIMIT 25
```

**See document-chunk-entity relationships:**
```cypher
MATCH (d:Document)-[:HAS_CHUNK]->(c:Chunk)-[:MENTIONS]->(e:Entity)
RETURN d, c, e LIMIT 50
```

**Find entities by type:**
```cypher
MATCH (e:Entity {entity_type: 'PERSON'})
RETURN e.name, e.mention_count
ORDER BY e.mention_count DESC
LIMIT 10
```

**Find co-occurring entities:**
```cypher
MATCH (e1:Entity)-[r:co_occurs_with]->(e2:Entity)
RETURN e1.name, e2.name, r.strength
ORDER BY r.strength DESC
LIMIT 20
```

**Find all entities mentioned in a document:**
```cypher
MATCH (d:Document {title: 'my_document.txt'})-[:HAS_CHUNK]->(c:Chunk)-[:MENTIONS]->(e:Entity)
RETURN DISTINCT e.name, e.entity_type, count(*) as mentions
ORDER BY mentions DESC
```

**Find documents mentioning a specific entity:**
```cypher
MATCH (d:Document)-[:HAS_CHUNK]->(c:Chunk)-[:MENTIONS]->(e:Entity {name: 'Microsoft'})
RETURN DISTINCT d.title, count(*) as mentions
ORDER BY mentions DESC
```

**Vector similarity + graph (hybrid query):**
```cypher
// First get similar chunks via vector search (from your app)
// Then enrich with graph context:
MATCH (c:Chunk {id: $chunk_id})-[:MENTIONS]->(e:Entity)
OPTIONAL MATCH (e)-[:co_occurs_with]-(related:Entity)
RETURN c, collect(DISTINCT e) as entities, collect(DISTINCT related) as related_entities
```

## Architecture

```
        ┌──────────────┐
        │   ONNX Model  │
        │  (Embedder)   │
        └──────┬───────┘
               │
               ▼
      v ∈ ℝⁿ (embedding)
               │
       ┌───────┴───────┐
       │               │
       ▼               ▼
┌──────────────┐ ┌──────────────┐
│   Tantivy    │ │    Neo4j     │
│ (Vector+BM25)│ │ (Graph+Vectors)│
└──────────────┘ └──────────────┘
       │               │
       └───────┬───────┘
               │
               ▼
    Hybrid search + graph reasoning
```

## Files

| File | Purpose |
|------|---------|
| `backend/src/graph/knowledge_builder.rs` | KnowledgeBuilder struct and Neo4j operations |
| `backend/src/graph/config.rs` | GraphConfig and EntityExtractionSettings |
| `backend/src/tools/entity_extractor.rs` | EntityExtractorTool for NER |
| `backend/src/api/mod.rs` | `index_to_knowledge_graph()` integration |
| `backend/src/index.rs` | `index_content_with_graph()` function |
| `backend/src/main.rs` | KnowledgeBuilder initialization |

## Troubleshooting

**Entities not being extracted:**
- Check `ENTITY_EXTRACTION_ENABLED=true` in `.env`
- Check `ENTITY_EXTRACTION_CONFIDENCE_THRESHOLD` isn't too high
- Restart backend after changing `.env`

**Neo4j connection failed:**
- Ensure Neo4j container is running: `docker ps | grep neo4j`
- Check credentials match `.env`
- Check Neo4j logs: `docker logs ag-neo4j`

**Graph is empty after upload:**
- Check backend logs for errors during graph indexing
- Verify Neo4j is healthy: `curl http://localhost:7474`
- Run `MATCH (n) RETURN count(n)` in Neo4j Browser
