# Agentic Elements of the AG Project

This project implements a **multi-layered agentic RAG (Retrieval-Augmented Generation) system** with sophisticated agent capabilities. Here's a breakdown of the agentic architecture:

---

## 🧠 1. Core Agent System (`backend/src/agent.rs`)

The foundational agent that orchestrates query processing:

```
┌─────────────────────────────────────────────────────────────┐
│                        Agent                                 │
├─────────────────────────────────────────────────────────────┤
│  • agent_id: Unique identifier                              │
│  • memory_db_path: SQLite persistence                       │
│  • retriever: Arc<Mutex<Retriever>>                         │
├─────────────────────────────────────────────────────────────┤
│  Execution Steps:                                           │
│  1. Recall recent memory (last 5 items)                     │
│  2. Retrieve relevant chunks (hybrid search)                │
│  3. Plan fallback if no chunks found                        │
│  4. Summarize retrieved content                             │
│  5. Store interaction in memory                             │
└─────────────────────────────────────────────────────────────┘
```

**Key Features:**
- **Memory recall** before answering (episodic memory)
- **Hybrid search** combining semantic + keyword retrieval
- **Step-by-step reasoning trace** (`AgentStep` with kind + message)
- **Automatic memory persistence** of Q&A pairs

---

## 🗄️ 2. Agent Memory Layer (`backend/src/memory/agent.rs`)

A sophisticated memory system with:

| Component | Purpose |
|-----------|---------|
| **Goals** | Track active objectives with status (Active/Completed/Failed) |
| **Tasks** | Sub-goals with status tracking (Pending/InProgress/Completed/Failed) |
| **Episodes** | Individual query-response interactions with success tracking |
| **Reflections** | Self-analysis of past performance (Success/Failure/Pattern/Improvement) |

**Memory Operations:**
```rust
// Goal management
set_goal(goal_text) → Goal
complete_goal(goal_id)
get_active_goals() → Vec<Goal>

// Episodic memory
record_episode(query, response, chunks_used, success) → Episode
recall_similar_episodes(query, top_k) → Vec<Episode>  // Semantic search!

// Self-reflection
reflect_on_episodes() → Reflection  // Analyzes 24h success rate
get_agent_context() → AgentContext  // Full memory snapshot
```

---

## 🤔 3. Decision Engine (`backend/src/memory/decision_engine.rs`)

**Multi-step reasoning with tool selection:**

```
Query → Assess Context → Check Similar Queries → Decide Strategy → Execute RAG → Record → Reflect
```

**Decision Tools:**
| Tool | When Used |
|------|-----------|
| `SemanticSearch` | Fresh search, no similar past queries |
| `ReflectOnHistory` | Analyze past episodes for patterns |
| `RefinedSearch` | Mixed past results, adjust parameters |
| `DirectAnswer` | High success rate on similar queries |

**Adaptive Behavior:**
- Adjusts `top_k` based on confidence (3-7 chunks)
- Learns from past query success rates
- Generates reasoning traces for transparency

---

## 👥 4. Multi-Agent Collaboration (`backend/src/memory/multi_agent.rs`)

**Team-based agent architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│                      AgentTeam                               │
├─────────────────────────────────────────────────────────────┤
│  Capabilities:                                               │
│  • Search      - Vector store search                        │
│  • Analyze     - Document analysis                          │
│  • Summarize   - Content summarization                      │
│  • Verify      - Information verification                   │
│  • Coordinate  - Orchestrate other agents                   │
├─────────────────────────────────────────────────────────────┤
│  Message Types:                                              │
│  • Query       - Request information                        │
│  • Share       - Share discovery                            │
│  • Delegate    - Delegate task                              │
│  • Response    - Respond to query                           │
│  • Reflection  - Share learning                             │
└─────────────────────────────────────────────────────────────┘
```

**Collaboration Features:**
- Agent registration with capabilities
- Message passing between agents
- Capability-based agent discovery
- Broadcast to agents with specific capabilities
- Shared knowledge base

---

## 🔧 5. Tool System (`backend/src/tools/`)

**Extensible tool registry for agent actions:**

| Tool Type | Description |
|-----------|-------------|
| `SemanticSearch` | Vector store search |
| `WebSearch` | External web search |
| `Calculator` | Math operations |
| `URLFetch` | Fetch web content |
| `DatabaseQuery` | SQL queries |
| `CodeExecution` | Run code |
| `ImageGeneration` | Generate images |

### Tool Selection (`tool_selector.rs`)
Automatic intent detection:
```rust
"Calculate 5 + 3"     → Math intent      → Calculator (95% confidence)
"Find latest papers"  → WebSearch intent → WebSearch (85% confidence)
"https://example.com" → UrlFetch intent  → URLFetch (80% confidence)
```

### Tool Composition (`tool_composer.rs`)
Multi-step query handling:
```rust
"Find papers and count" → Split into:
  1. WebSearch: "Find papers"
  2. Calculator: "count"
```

### Tool Execution (`tool_executor.rs`)
- Execute with fallback chains
- Result validation
- Data extraction from results

---

## 📊 6. RAG Query Pipeline (`backend/src/memory/query.rs`)

**Full retrieval-augmented generation flow:**

```
Query → Embed → Search Vector Store → Filter by Threshold → Assemble Context → Generate with LLM → Return Sources
```

**Configuration:**
- `top_k`: Number of chunks to retrieve (default: 5)
- `similarity_threshold`: Minimum score (default: 0.3)
- `max_context_length`: Token limit (default: 2000)

---

## 🗃️ 7. Vector Store with Memory Bounds (`backend/src/memory/vector_store.rs`)

**Intelligent storage with eviction policies:**

| Policy | Behavior |
|--------|----------|
| `LRU` | Evict least recently accessed |
| `FIFO` | Evict oldest first |
| `ByScore` | Evict lowest relevance |

**Metrics tracked:**
- Total insertions/evictions
- Lookup hits/misses
- Peak vector count
- Hit rate calculation

---

## 🔄 8. Agent Memory Persistence (`backend/src/agent_memory.rs`)

**Dual-mode memory storage:**

1. **Legacy Memory** - Simple append-only store
2. **RAG Memory** - Vector-embedded memory with semantic search

```rust
// Store with embedding
store_rag(agent_id, memory_type, content, timestamp)

// Semantic search over memories
search_rag(agent_id, query, top_k) → Vec<MemorySearchResult>

// Recall recent memories
recall_rag(agent_id, limit) → Vec<MemoryItem>
```

---

## 🌐 9. API Endpoints for Agents

| Endpoint | Purpose |
|----------|---------|
| `POST /agent` | Run agent query |
| `GET /agent/chat` | GET-based chat (CORS-friendly) |
| `POST /memory/store_rag` | Store agent memory |
| `POST /memory/search_rag` | Semantic memory search |
| `POST /memory/recall_rag` | Recall recent memories |

---

## 📈 Architecture Summary

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Frontend (Dioxus)                            │
│                    Chat UI with RAG toggle                           │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Actix Web API Layer                             │
│              /agent, /memory/*, /search, /config/*                   │
└─────────────────────────────────────────────────────────────────────┘
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        ▼                           ▼                           ▼
┌───────────────┐         ┌─────────────────┐         ┌─────────────────┐
│    Agent      │         │ Decision Engine │         │  Multi-Agent    │
│  (Core Loop)  │◄───────►│ (Tool Selection)│◄───────►│    Team         │
└───────────────┘         └─────────────────┘         └─────────────────┘
        │                           │                           │
        ▼                           ▼                           ▼
┌───────────────┐         ┌─────────────────┐         ┌─────────────────┐
│ Agent Memory  │         │  Tool Registry  │         │ Shared Knowledge│
│  (Episodes,   │         │  (Calculator,   │         │    Base         │
│   Goals,      │         │   WebSearch,    │         │                 │
│   Reflections)│         │   URLFetch...)  │         │                 │
└───────────────┘         └─────────────────┘         └─────────────────┘
        │                           │
        └───────────────┬───────────┘
                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      RAG Query Pipeline                              │
│         Embed → Search → Filter → Context → LLM Generate             │
└─────────────────────────────────────────────────────────────────────┘
                        │
        ┌───────────────┼───────────────┐
        ▼               ▼               ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│ Vector Store  │ │ Tantivy Index │ │   SQLite      │
│ (Lance/Memory)│ │ (Full-text)   │ │ (Persistence) │
└───────────────┘ └───────────────┘ └───────────────┘
```

---

## 🚀 Key Agentic Capabilities

1. **Autonomous Reasoning** - Multi-step planning with tool selection
2. **Episodic Memory** - Learn from past interactions
3. **Self-Reflection** - Analyze success patterns
4. **Goal Tracking** - Maintain objectives across sessions
5. **Tool Use** - Dynamic tool selection based on query intent
6. **Multi-Agent Collaboration** - Team-based problem solving
7. **Adaptive Retrieval** - Adjust search parameters based on history
8. **Memory Persistence** - SQLite + vector embeddings for long-term memory

---

## 📁 File Reference

| File | Purpose |
|------|---------|
| `backend/src/agent.rs` | Core agent loop |
| `backend/src/agent_memory.rs` | RAG memory persistence |
| `backend/src/memory/agent.rs` | Agent memory layer (goals, episodes, reflections) |
| `backend/src/memory/multi_agent.rs` | Multi-agent collaboration |
| `backend/src/memory/decision_engine.rs` | Decision engine with tool selection |
| `backend/src/memory/query.rs` | RAG query pipeline |
| `backend/src/memory/vector_store.rs` | Vector storage with eviction |
| `backend/src/tools/mod.rs` | Tool registry and interfaces |
| `backend/src/tools/tool_selector.rs` | Intent detection and tool selection |
| `backend/src/tools/tool_executor.rs` | Tool execution with fallbacks |
| `backend/src/tools/tool_composer.rs` | Multi-step query composition |
| `backend/src/tools/calculator.rs` | Math tool |
| `backend/src/tools/web_search.rs` | Web search tool |
| `backend/src/tools/url_fetch.rs` | URL fetch tool |

---

## 📊 10. Agentic Monitoring Recommendations

The current monitor pages track infrastructure metrics (cache, rate limits, requests, logs, index) but lack visibility into the **agentic layer**. Here are recommended additions:

### Current Monitor Pages

| Page | What It Tracks |
|------|----------------|
| Overview | Health, documents, vectors, request rate, latency, error rate |
| Cache | L1/L2/Redis hit rates, troubleshooting checklists |
| Rate Limits | Drops, active keys, configuration |
| Requests | Request rate, latency breakdown, status codes |
| Logs | Filtered log viewing |
| Index | Reindex status, chunking info, storage paths |

**Gap**: No visibility into agents, memory, decisions, tools, or goals.

---

### 10.1 Agent Activity Dashboard (New Page: `/monitor/agents`)

| Metric | Description |
|--------|-------------|
| **Active Agents** | Count of registered agents |
| **Episodes/hour** | Query-response interactions recorded |
| **Success Rate** | % of episodes marked successful |
| **Active Goals** | Goals currently being tracked |
| **Recent Reflections** | Latest self-analysis insights |

```
┌─────────────────────────────────────────────────────────────┐
│  Agent Activity                              Refresh: 5s    │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ Agents   │  │ Episodes │  │ Success  │  │ Goals    │    │
│  │    3     │  │   127/hr │  │   84.2%  │  │    5     │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
│                                                             │
│  Recent Episodes:                                           │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 12:34:56 │ agent-1 │ "What is Rust?" │ ✓ success   │   │
│  │ 12:34:52 │ agent-1 │ "Find papers"   │ ✓ success   │   │
│  │ 12:34:48 │ agent-2 │ "Calculate 5+3" │ ✓ success   │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

### 10.2 Decision Engine Monitor (New Page: `/monitor/decisions`)

Track the decision-making process:

| Metric | Description |
|--------|-------------|
| **Tool Selection Distribution** | Which tools are being chosen |
| **Confidence Scores** | Average decision confidence |
| **Reasoning Traces** | View step-by-step reasoning |
| **Fallback Rate** | How often secondary tools are used |

```
┌─────────────────────────────────────────────────────────────┐
│  Decision Engine                             Refresh: 10s   │
├─────────────────────────────────────────────────────────────┤
│  Tool Usage (last hour):                                    │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ SemanticSearch  ████████████████████  65%           │   │
│  │ Calculator      ████████             25%            │   │
│  │ WebSearch       ███                   8%            │   │
│  │ URLFetch        █                     2%            │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  Avg Confidence: 78.3%    Fallback Rate: 12.1%             │
└─────────────────────────────────────────────────────────────┘
```

---

### 10.3 Memory Health Monitor (New Page: `/monitor/memory`)

Track agent memory systems:

| Metric | Description |
|--------|-------------|
| **Episodic Memory Size** | Total episodes stored |
| **Vector Store Utilization** | % of max_vectors used |
| **Eviction Rate** | How often vectors are evicted |
| **Memory Recall Latency** | Time to recall similar episodes |
| **Goal Completion Rate** | % of goals completed vs failed |

```
┌─────────────────────────────────────────────────────────────┐
│  Agent Memory                                Refresh: 10s   │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Episodes     │  │ Vector Store │  │ Evictions    │      │
│  │   1,247      │  │  8,432/10K   │  │    23/hr     │      │
│  │              │  │    84.3%     │  │              │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                             │
│  Goal Status:                                               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Active: 5  │  Completed: 42  │  Failed: 3          │   │
│  │ Completion Rate: 93.3%                              │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  Recent Reflections:                                        │
│  • "Last 24h: 127 episodes, 107 successful (84.3%)"        │
│  • "Pattern detected: math queries have 98% success"       │
└─────────────────────────────────────────────────────────────┘
```

---

### 10.4 Tool Execution Monitor (New Page: `/monitor/tools`)

Track tool performance:

| Metric | Description |
|--------|-------------|
| **Tool Success Rates** | Per-tool success % |
| **Execution Times** | Avg latency per tool |
| **Error Distribution** | Which tools fail most |
| **Chain Completion** | Multi-step query success |

```
┌─────────────────────────────────────────────────────────────┐
│  Tool Performance                            Refresh: 5s    │
├─────────────────────────────────────────────────────────────┤
│  Tool             │ Success │ Avg Time │ Executions        │
│  ─────────────────┼─────────┼──────────┼──────────────     │
│  Calculator       │  99.2%  │   12ms   │    342            │
│  SemanticSearch   │  87.4%  │  145ms   │    891            │
│  WebSearch        │  72.1%  │  890ms   │    156            │
│  URLFetch         │  68.3%  │ 1,240ms  │     41            │
│                                                             │
│  Multi-Step Chains:                                         │
│  • Total: 89  │  Completed: 76  │  Success Rate: 85.4%     │
└─────────────────────────────────────────────────────────────┘
```

---

### 10.5 Multi-Agent Team Monitor (New Page: `/monitor/team`)

For multi-agent collaboration:

| Metric | Description |
|--------|-------------|
| **Team Size** | Registered agents |
| **Message Queue** | Pending inter-agent messages |
| **Capability Coverage** | Which capabilities are available |
| **Task Distribution** | How tasks are assigned |

```
┌─────────────────────────────────────────────────────────────┐
│  Agent Team                                  Refresh: 5s    │
├─────────────────────────────────────────────────────────────┤
│  Team Stats:                                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Agents       │  │ Pending Msgs │  │ Capabilities │      │
│  │      4       │  │      7       │  │      5       │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                             │
│  Agent Roster:                                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ search-agent  │ Search, Analyze    │ 45 tasks      │   │
│  │ verify-agent  │ Verify             │ 12 tasks      │   │
│  │ coord-agent   │ Coordinate         │  8 tasks      │   │
│  │ summary-agent │ Summarize          │ 31 tasks      │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

### 10.6 RAG Pipeline Monitor (Enhance existing or new page)

Track the RAG query pipeline:

| Metric | Description |
|--------|-------------|
| **Queries/min** | RAG pipeline throughput |
| **Avg Chunks Retrieved** | Context chunks per query |
| **Similarity Threshold Hits** | % passing threshold |
| **LLM Generation Time** | Time spent in LLM |
| **Context Assembly Time** | Time building context |

---

### 10.7 Backend API Endpoints Needed

To support these monitors, new API endpoints are required:

```rust
// Agent monitoring
GET /monitoring/agents/stats        → AgentStats
GET /monitoring/agents/episodes     → Vec<Episode>
GET /monitoring/agents/goals        → Vec<Goal>
GET /monitoring/agents/reflections  → Vec<Reflection>

// Decision engine
GET /monitoring/decisions/stats     → DecisionStats
GET /monitoring/decisions/recent    → Vec<Decision>

// Memory
GET /monitoring/memory/stats        → MemoryStats
GET /monitoring/memory/vector-store → VectorStoreStats

// Tools
GET /monitoring/tools/stats         → ToolStats
GET /monitoring/tools/executions    → Vec<ToolExecution>

// Multi-agent
GET /monitoring/team/stats          → TeamStats
GET /monitoring/team/messages       → Vec<AgentMessage>
```

---

### 10.8 Implementation Priority

| Priority | Feature | Rationale |
|----------|---------|----------|
| **High** | Agent Activity Dashboard | Core visibility into agent behavior |
| **High** | Memory Health Monitor | Critical for debugging retrieval issues |
| **Medium** | Tool Execution Monitor | Understand tool selection patterns |
| **Medium** | Decision Engine Monitor | Debug reasoning issues |
| **Low** | Multi-Agent Team Monitor | Only needed when using multi-agent |

---

### 10.9 Quick Wins (Add to Existing Pages)

1. **Overview page**: Add "Agent Episodes/hr" and "Agent Success Rate" cards
2. **Cache page**: Add "Memory Recall Cache" section for agent memory
3. **Requests page**: Add "Agent Queries" breakdown vs regular search
4. **Index page**: Add "Episodic Memory Size" alongside document chunks

---

## 📡 11. Using the Agent Endpoint to Populate Monitoring Data

The `/agent` endpoint now stores episodes in the database, which populates the Agentic Monitoring dashboard.

### 11.1 Start the Backend

```bash
cd /home/pde/ag/backend && cargo run
```

### 11.2 Make Agent Queries

Use the `/agent` endpoint to ask questions. Each query creates an episode that shows up in the monitoring dashboard:

```bash
# POST request
curl -X POST http://127.0.0.1:3010/agent \
  -H 'Content-Type: application/json' \
  -d '{"query": "What is Rust?", "top_k": 3}'

# Or GET request (simpler, no CORS preflight)
curl "http://127.0.0.1:3010/agent/chat?query=What%20is%20Rust&top_k=3"
```

### 11.3 What Happens Behind the Scenes

When you call `/agent`:

1. **Memory Recall**: The agent recalls recent memory from `agent_memory` table (last 5 items)
2. **Hybrid Search**: Retrieves relevant chunks from the knowledge base using hybrid search
3. **Planning**: If no chunks found, returns fallback response
4. **Summarization**: Generates a response (naive summarization of top chunks)
5. **Episode Storage**: **Stores the episode** in the `episodes` table for monitoring
6. **Memory Persistence**: Stores the Q&A in the `agent_memory` table

### 11.4 Episode Data Structure

Each episode stored contains:

| Field | Type | Description |
|-------|------|-------------|
| `id` | TEXT | UUID for the episode |
| `agent_id` | TEXT | Identifier of the agent (default: "default") |
| `query` | TEXT | The user's question |
| `response` | TEXT | The agent's answer |
| `context_chunks_used` | INTEGER | Number of chunks retrieved |
| `success` | INTEGER | 1 if chunks found, 0 if fallback |
| `created_at` | INTEGER | Unix timestamp |

### 11.5 View in Monitoring Dashboard

After making some agent queries, access the Agentic Monitoring page:

```
http://localhost:1789/monitor/agentic
```

You'll see:

| Metric | Description |
|--------|-------------|
| **Episodes Total** | Count of all agent interactions |
| **Episodes Last Hour** | Recent activity |
| **Success Rate** | Percentage of successful queries (ones that found relevant chunks) |
| **Active Goals** | Goals currently being tracked |
| **Recent Episodes** | List of recent queries with their responses |

### 11.6 Backend Implementation

The episode storage is implemented in `backend/src/agent.rs`:

```rust
/// Store episode for monitoring dashboard
fn store_episode(&self, query: &str, response: &str, chunks_used: usize, success: bool) {
    if let Ok(conn) = Connection::open(self.memory_db_path) {
        // Ensure episodes table exists
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS episodes (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                query TEXT NOT NULL,
                response TEXT NOT NULL,
                context_chunks_used INTEGER NOT NULL,
                success INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        );
        
        let episode_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp();
        let success_int = if success { 1 } else { 0 };
        
        let _ = conn.execute(
            "INSERT INTO episodes (id, agent_id, query, response, context_chunks_used, success, created_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![episode_id, self.agent_id, query, response, chunks_used, success_int, created_at],
        );
    }
}
```

### 11.7 Monitoring API Endpoints

The monitoring dashboard fetches data from these endpoints:

| Endpoint | Returns |
|----------|--------|
| `GET /monitoring/agents/stats` | Aggregate statistics (episodes, success rate, goals) |
| `GET /monitoring/agents/episodes?limit=N` | Recent episodes list |
| `GET /monitoring/agents/goals` | All goals with status breakdown |
| `GET /monitoring/agents/reflections?limit=N` | Recent reflections |
| `GET /monitoring/memory/stats` | Memory system statistics |
| `GET /monitoring/tools/stats` | Tool usage statistics |

### 11.8 Example Response from `/monitoring/agents/stats`

```json
{
  "active_agents": 1,
  "episodes_total": 42,
  "episodes_last_hour": 5,
  "success_rate": 85.7,
  "active_goals": 2,
  "completed_goals": 10,
  "failed_goals": 1,
  "total_reflections": 3,
  "timestamp": "2026-01-04T13:45:00Z"
}
```

---

## 12. Chat Commands

Based on the existing endpoints, here are the chat commands that can be wired up. The table now reflects both the currently wired commands and the newly requested ones so product/design can prioritize quickly.

### 12.1 Already Supported by the API

| Command | Endpoint | Example |
|---------|----------|--------|
| `/goal` | `POST /agent/goals` | `/goal Find articles about Rust async` |
| `/goals` | `GET /agent/goals` | `/goals` (list active) |
| `/search` | `GET /search` | `/search vector databases` |
| `/upload` | `POST /upload` | `/upload` (trigger file picker) |
| `/docs` | `GET /documents` | `/docs` (list all) |
| `/summarize` | `POST /summarize` | `/summarize <paste text>` |
| `/remember` | `POST /memory/store_rag` | `/remember The project uses Actix` |
| `/recall` | `POST /memory/recall_rag` | `/recall What framework do we use?` |
| `/plan` | `POST /api/composer/plan` | `/plan Research and summarize X` |
| `/tools` | `GET /api/tools/available` | `/tools` (list available) |
| `/status` | `GET /monitoring/health` | `/status` |

### 12.2 Planned Command Extensions (New)

#### Knowledge management
| Command | Purpose |
|---------|---------|
| `/forget <topic>` | Remove specific memories/history |
| `/history` | Show recent interactions/sources |
| `/sources` | Show where info came from |
| `/learn <url>` | Ingest a webpage |
| `/note <text>` | Quick note without RAG processing |

#### Goal & task management
| Command | Purpose |
|---------|---------|
| `/subgoal <text>` | Add sub-goal to current goal |
| `/pause` | Pause current goal |
| `/resume` | Resume paused goal |
| `/abandon` | Cancel current goal |
| `/reflect` | Agent reflects on progress |
| `/why` | Explain current reasoning/approach |

#### Context control
| Command | Purpose |
|---------|---------|
| `/focus <topic>` | Narrow context to topic |
| `/unfocus` | Return to broad context |
| `/persona <name>` | Switch agent behavior style |
| `/verbose` | More detailed responses |
| `/brief` | Shorter responses |

#### Tools & execution
| Command | Purpose |
|---------|---------|
| `/run <tool>` | Execute specific tool |
| `/chain <a> -> <b>` | Run tools in sequence |
| `/retry` | Re-run last action |
| `/undo` | Revert last change |
| `/dry-run <query>` | Show plan without executing |

#### System
| Command | Purpose |
|---------|---------|
| `/model <name>` | Switch LLM model |
| `/temperature <n>` | Adjust creativity |
| `/export` | Export conversation/memories |
| `/import` | Import data |
| `/debug` | Show internal state |
| `/tokens` | Show token usage |

##### 12.2.1 Step 1 – Command Inventory & Scope

| Category | Command(s) | Expected Behavior | Data / API Dependencies |
|----------|------------|-------------------|-------------------------|
| Knowledge | `/forget <topic>` | Delete matching memories or episodes for active agent | `rag_memory` delete helper, episode table access |
| Knowledge | `/history`, `/sources` | Show recent interactions and where context came from | `episodes` table + last retrieval metadata |
| Knowledge | `/learn <url>` | Fetch + ingest content from external URL | `URLFetchTool`, new ingestion helper |
| Knowledge | `/note <text>` | Store quick note without embeddings | `agent_memory` insert helper |
| Goal/Task | `/subgoal <text>` | Attach sub-task under current goal | `tasks` table CRUD |
| Goal/Task | `/pause`, `/resume`, `/abandon` | Change goal lifecycle state | extend goal status enum + update queries |
| Goal/Task | `/reflect` | Trigger reflection summary | reuse reflection logic (`AgentMemoryLayer::reflect_on_episodes`) |
| Goal/Task | `/why` | Explain current reasoning / plan | capture last `AgentResponse.steps` |
| Context | `/focus <topic>`, `/unfocus` | Narrow / reset context window | shared session state store |
| Context | `/persona <name>` | Switch agent style presets | persona registry + session state |
| Context | `/verbose`, `/brief` | Adjust response verbosity | agent config & summarizer tuning |
| Tools & Execution | `/run <tool>` | Execute named tool once | tool registry + async runner |
| Tools & Execution | `/chain <a> -> <b>` | Compose sequential tool calls | tool composer + planner |
| Tools & Execution | `/retry` | Re-run last non-command query | cached query payload |
| Tools & Execution | `/undo` | Revert last stateful command (note, focus, etc.) | action history stack |
| Tools & Execution | `/dry-run <query>` | Show plan/intent without execution | decision engine planning API |
| System | `/model <name>` | Switch backend model | hardware/config routes |
| System | `/temperature <n>` | Adjust LLM sampling params | config update helper |
| System | `/export`, `/import` | Export/import conversations or memories | file/archive service |
| System | `/debug`, `/tokens` | Show internal state + token usage snapshot | telemetry capture, request accounting |

This inventory locks scope for subsequent steps (state management, DB helpers, frontend parsing, etc.).

### 12.3 Implementation in Frontend

```rust
fn parse_command(input: &str) -> Option<(Command, &str)> {
    if let Some(rest) = input.strip_prefix("/goal ") {
        Some((Command::Goal, rest))
    } else if input == "/goals" {
        Some((Command::ListGoals, ""))
    } else if let Some(rest) = input.strip_prefix("/search ") {
        Some((Command::Search, rest))
    } else if let Some(rest) = input.strip_prefix("/remember ") {
        Some((Command::Remember, rest))
    } else {
        None // Regular chat message
    }
}
```

### 12.4 Currently Implemented

The `/goal` command is currently implemented in the backend (`run_agent` function in `backend/src/api/mod.rs`):

```rust
// Check for /goal command
if let Some(goal_text) = parse_goal_command(&req.query) {
    match create_goal_from_command(&goal_text) {
        Ok(goal) => {
            return Ok(HttpResponse::Ok().json(json!({
                "response": {
                    "answer": format!("✓ Goal created: {}", goal_text),
                    "chunks_used": 0,
                    "sources": []
                },
                "goal": goal,
                "request_id": request_id
            })));
        }
        // ...
    }
}
```

Users can type `/goal Find all Rust error handling patterns` in the chat and a goal will be created.

---

*Generated from codebase analysis*


-----------------------------------------------

Here are some commands that could extend `ag`'s capabilities:

## Knowledge management
| Command | Purpose |
|---------|---------|
| `/forget <topic>` | Remove specific memories |
| `/history` | Show recent interactions |
| `/sources` | Show where info came from |
| `/learn <url>` | Ingest a webpage |
| `/note <text>` | Quick note without RAG processing |

## Goal & task management
| Command | Purpose |
|---------|---------|
| `/subgoal <text>` | Add sub-goal to current goal |
| `/pause` | Pause current goal |
| `/resume` | Resume paused goal |
| `/abandon` | Cancel current goal |
| `/reflect` | Agent reflects on progress |
| `/why` | Explain current reasoning/approach |

## Context control
| Command | Purpose |
|---------|---------|
| `/focus <topic>` | Narrow context to topic |
| `/unfocus` | Return to broad context |
| `/persona <name>` | Switch agent behavior style |
| `/verbose` | More detailed responses |
| `/brief` | Shorter responses |

## Tools & execution
| Command | Purpose |
|---------|---------|
| `/run <tool>` | Execute specific tool |
| `/chain <a> -> <b>` | Run tools in sequence |
| `/retry` | Re-run last action |
| `/undo` | Revert last change |
| `/dry-run <query>` | Show plan without executing |

## System
| Command | Purpose |
|---------|---------|
| `/model <name>` | Switch LLM model |
| `/temperature <n>` | Adjust creativity |
| `/export` | Export conversation/memories |
| `/import` | Import data |
| `/debug` | Show internal state |
| `/tokens` | Show token usage |

## Which interest you?

Some are simple (frontend only), others need new endpoints. Which ones fit how you want to use `ag`?
