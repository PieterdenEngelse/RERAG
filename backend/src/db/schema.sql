-- ag/db/schema.sql v13.1.2
-- Embedded in schema_init.rs via include_str!

CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    source_type TEXT NOT NULL,
    source_path TEXT,
    file_hash TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    indexed_at TIMESTAMP,
    status TEXT DEFAULT 'active'
);

CREATE INDEX IF NOT EXISTS idx_documents_source ON documents(source_path);
CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(status);
CREATE INDEX IF NOT EXISTS idx_documents_created ON documents(created_at);

CREATE TABLE IF NOT EXISTS chunks (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    content TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    token_count INTEGER,
    metadata_json TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_chunks_created ON chunks(created_at);

CREATE TABLE IF NOT EXISTS embeddings (
    id TEXT PRIMARY KEY,
    chunk_id TEXT NOT NULL UNIQUE,
    model_name TEXT NOT NULL,
    model_version TEXT,
    vector_bytes BLOB NOT NULL,
    dimension INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(chunk_id) REFERENCES chunks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_embeddings_chunk ON embeddings(chunk_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_model ON embeddings(model_name);

CREATE TABLE IF NOT EXISTS agent_memory (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata_json TEXT,
    retrieved_count INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_agent_memory_agent_id ON agent_memory(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_memory_type ON agent_memory(memory_type);
CREATE INDEX IF NOT EXISTS idx_agent_memory_created ON agent_memory(created_at DESC);

CREATE TABLE IF NOT EXISTS agent_interactions (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    query TEXT NOT NULL,
    response TEXT,
    steps_json TEXT,
    retrieved_chunks INTEGER,
    confidence_score REAL,
    execution_time_ms INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_interactions_agent ON agent_interactions(agent_id);
CREATE INDEX IF NOT EXISTS idx_interactions_created ON agent_interactions(created_at DESC);

CREATE TABLE IF NOT EXISTS agent_goals (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    goal_text TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    priority INTEGER DEFAULT 1,
    result_json TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_goals_agent ON agent_goals(agent_id);
CREATE INDEX IF NOT EXISTS idx_goals_status ON agent_goals(status);

CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    value_type TEXT,
    description TEXT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO config (key, value, value_type, description) VALUES
    ('chunk_size', '512', 'int', 'Target tokens per chunk'),
    ('chunk_overlap', '75', 'int', 'Overlap tokens between chunks'),
    ('embedding_model', 'all-MiniLM-L6-v2', 'string', 'Embedding model name'),
    ('top_k_retrieval', '5', 'int', 'Default top-k for retrieval'),
    ('similarity_threshold', '0.5', 'float', 'Min similarity for results'),
    ('batch_size', '32', 'int', 'Batch size for embedding generation');

CREATE TABLE IF NOT EXISTS retriever_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    total_documents INTEGER DEFAULT 0,
    total_chunks INTEGER DEFAULT 0,
    total_embeddings INTEGER DEFAULT 0,
    last_index_time TIMESTAMP,
    avg_query_time_ms REAL,
    recorded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS vector_metadata (
    chunk_id TEXT PRIMARY KEY,
    vector_id TEXT,
    store_type TEXT,
    sync_status TEXT DEFAULT 'synced',
    last_sync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(chunk_id) REFERENCES chunks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vector_metadata_sync ON vector_metadata(sync_status);

CREATE TABLE IF NOT EXISTS schema_migrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version TEXT NOT NULL UNIQUE,
    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    description TEXT
);

INSERT OR IGNORE INTO schema_migrations (version, description) VALUES
    ('13.1.2', 'Agentic RAG with PathManager');

CREATE TABLE IF NOT EXISTS extraction_records (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    filename  TEXT NOT NULL,
    path      TEXT NOT NULL,
    format    TEXT NOT NULL,
    ok        INTEGER NOT NULL,
    chars     INTEGER NOT NULL,
    recorded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_extraction_records_recorded ON extraction_records(recorded_at DESC);

-- Golden corpus sample: a stable, seeded random subset of the user's actual
-- chunks captured under one tokenizer. Used as the baseline for tokenizer
-- diffs (Step 3) so a candidate tokenizer can be evaluated against the same
-- text the live system already chose to chunk that way.
CREATE TABLE IF NOT EXISTS golden_sample (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    chunk_text          TEXT NOT NULL,
    baseline_token_count INTEGER NOT NULL,
    baseline_token_ids  TEXT,            -- JSON array of u32, NULL if heuristic
    tokenizer_model     TEXT NOT NULL,
    captured_at         TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    position_in_corpus  INTEGER NOT NULL -- 0-indexed offset of this chunk in the offered stream
);

CREATE INDEX IF NOT EXISTS idx_golden_sample_position ON golden_sample(position_in_corpus);

-- Single-row meta table tracking the reservoir state across restarts.
CREATE TABLE IF NOT EXISTS golden_sample_meta (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    capacity            INTEGER NOT NULL,
    chunks_seen         INTEGER NOT NULL DEFAULT 0,
    seed                INTEGER NOT NULL,
    captured_at         TIMESTAMP,
    tokenizer_model     TEXT
);