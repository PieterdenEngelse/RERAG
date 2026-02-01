// Run ONCE to populate Neo4j with minimal test data
MATCH (n:TestChunk) DETACH DELETE n;

CREATE (:Chunk:TestChunk {
  id: 'test:apple',
  content: 'Apple Inc. founded by Steve Jobs in 1976',
  entities: ['Apple Inc', 'Steve Jobs'],
  source: 'wiki:test'
});

CREATE (:Chunk:TestChunk {
  id: 'test:jobs',
  content: 'Steve Jobs born in San Francisco',
  entities: ['Steve Jobs', 'San Francisco'],
  source: 'wiki:test'
});

MATCH (a:TestChunk {id: 'test:apple'}), (b:TestChunk {id: 'test:jobs'})
CREATE (a)-[:MENTIONS {
  type: 'founded_by',
  confidence: 0.95
}]->(b);