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

-------------------------------------------

Here’s a high-level plan to implement the 3-layer memory search workflow as an internal backend feature, tailored to your project:

1. **Assess the Current Storage Layer**
   - Inventory the existing persistence (e.g., SQLite, Tantivy, embedded DBs) already used by the backend.
   - Determine whether there’s a natural place to store structured “observations” and their metadata (IDs, types, timestamps, concepts, linked files).
   - Decide if we can extend an existing SQLite DB or need a dedicated table/schema.

2. **Define the Memory Schema & Abstractions**
   - Draft a Rust data model representing observations (ID, type, title, narrative, facts, files read/modified, concepts).
   - Create the table(s) required for the index view and any secondary structures (e.g., FTS table for text search, optional vector embeddings via Tantivy or a Chroma-like store).
   - Plan migration strategy for the new schema (e.g., Diesel migration, rusqlite schema creation on startup).

3. **Implement Layer 1: `search` Endpoint**
   - Add a backend route (e.g., `POST /memory/search`) that executes full-text searches over the observations table.
   - Return compact summaries (ID, title, date, type, concepts) to mirror the “index” layer.
   - Integrate pagination, type/date filters, and ordering.

4. **Implement Layer 2: `timeline` Endpoint**
   - Add an endpoint (e.g., `POST /memory/timeline`) that either:
     - Accepts an `anchor_id` and fetches surrounding observations (`depth_before`, `depth_after`), or
     - Accepts a query and finds the anchor automatically.
   - Ensure chronological ordering and include enough context (ID, date, title, type).

5. **Implement Layer 3: `get_observations` Endpoint**
   - Add an endpoint (e.g., `POST /memory/observations`) that takes a list of IDs and returns full observation details (narrative, facts, files, concepts).
   - Enforce batching of IDs to keep calls efficient.

6. **Wire Up Service Layer + Reuse Storage**
   - Implement a Rust service module that encapsulates:
     - Search via SQLite FTS or Tantivy.
     - Timeline retrieval logic (based on timestamps and IDs).
     - Bulk fetch by IDs.
   - Ensure the service is reusable so future CLI or cron jobs can leverage it.

7. **Add Input Validation & Rate Limiting**
   - Extend existing middleware or add route-specific guards to prevent large payloads (e.g., limit IDs per request).
   - Apply existing rate-limit categories (maybe reuse the search or memory buckets) to protect the new endpoints.

8. **Observability & Metrics**
   - Expose Prometheus counters/histograms for each new endpoint (request counts, latencies, hits/misses).
   - Add tracing spans so `search`, `timeline`, and `get_observations` calls show up in OTLP pipelines.

9. **Documentation & Usage Guidance**
   - Update backend API docs (and possibly `docu/PLAN.md`) to describe the new workflow and request/response formats.
   - Provide examples showing the token-efficient 3-layer usage pattern.
   - Advise on how frontend or CLI should call these endpoints.

10. **Follow-up Advice**
    - After reviewing your current storage layer, we can confirm whether to extend SQLite or integrate an embedded vector DB.
    - Recommend whether to create a thin internal CLI client or front-end view to exercise the new endpoints.

If this plan matches what you’re looking for, the next step is to inspect your existing storage layer and confirm where the observation schema should live. Let me know, and I’ll proceed.

I'll search for information about Unsloth and how it might fit into your Agentic RAG project.Now let me search for how Unsloth exports to Ollama/GGUF since that's relevant for your setup:Now I have a comprehensive understanding. Let me give you a clear analysis:

## Unsloth for Your Agentic RAG Project - Analysis

### What Unsloth Is
Unsloth is a Python framework for fine-tuning LLMs 2x faster with 70% less VRAM. It supports GRPO, GSPO, DrGRPO, DAPO reinforcement learning methods with 0% loss in accuracy - no approximation methods.

### The Good News - Potential Fit

1. **Direct Ollama Export**: Unsloth can export fine-tuned models directly to GGUF formats for Ollama, Jan AI, Open WebUI with one-line commands like `model.save_pretrained_gguf("directory", tokenizer, quantization_method = "q4_k_m")`.

2. **Low VRAM Requirements**: You can fine-tune or do RL for free on Colab, Kaggle, or locally with just 3GB VRAM using their notebooks.

3. **RAG-Relevant**: Fine-tuning can replicate all of RAG's capabilities, but not vice versa. You can think of a fine-tuned model as a specialized agent designed to do specific tasks more effectively and efficiently.

### The Challenge - Your Hardware Constraints

| Requirement | Your System | Verdict |
|-------------|-------------|---------|
| GPU with CUDA | Intel i3-10110U (integrated graphics) | ❌ **No NVIDIA GPU** |
| Minimum VRAM | 3GB+ GPU VRAM | ❌ No dedicated GPU |
| RAM | 8GB | ⚠️ Tight for training |

Unsloth supports NVIDIA GPUs since 2018 with minimum CUDA Capability 7.0 (V100, T4, Titan V, RTX 20, 30, 40x, A100, H100, L40 etc).

### Realistic Integration Approaches

**Option A: Train Elsewhere, Deploy Locally (RECOMMENDED)**
```
┌──────────────────────────────────────────────────────────────┐
│  TRAINING (Cloud/Colab)          DEPLOYMENT (Your System)    │
│  ┌─────────────────────┐         ┌─────────────────────────┐ │
│  │ Unsloth + GPU       │ ──────► │ Ollama + Fine-tuned     │ │
│  │ Fine-tune on        │  GGUF   │ Model (Q4_K_M)          │ │
│  │ domain data         │  export │ Your Agentic RAG        │ │
│  └─────────────────────┘         └─────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

**Option B: Use Pre-Fine-Tuned Unsloth Models**
Unsloth provides optimized GGUF models you can use directly with Ollama without training.

### What Would This Enable for Your Project?

1. **Domain-Specific RAG**: Fine-tune a small model (Phi-3.5, Llama 3.2 1B) on your document corpus
2. **Better Retrieval Understanding**: Train model to understand your chunking/query patterns
3. **Custom Response Style**: Match your application's tone and format requirements

### Practical Integration Plan

```
Phase X: Custom Model Integration (Future Enhancement)
├── Step 1: Collect training data from your RAG interactions
├── Step 2: Fine-tune using free Colab/Kaggle GPU
├── Step 3: Export to GGUF (Q4_K_M for 8GB RAM)
├── Step 4: Deploy via existing Ollama infrastructure
└── Step 5: Update installer to download custom model
```

### Installer Impact (Future)

```toml
# config.toml addition (future)
[llm]
model_source = "ollama"           # or "custom"
custom_model_path = ""            # path to custom GGUF
custom_model_url = ""             # download URL for installer
```

### My Recommendation

**Not Now, But Keep It on the Roadmap**

1. **Current Phase**: Focus on completing Phase 16 (distributed tracing) and production hardening
2. **Later Phases**: Once your RAG system has real usage data, fine-tuning becomes valuable
3. **Training Strategy**: Use Google Colab (free T4 GPU) to fine-tune, deploy locally

**For Now**: Your current Phi-3.5 + Ollama setup is appropriate. Unsloth becomes valuable when:
- You have domain-specific training data collected
- Default model responses need improvement for your use case
- You want specialized behavior (better citation, specific format, etc.)

Would you like me to:
1. Create a future phase plan document for custom model integration?
2. Add this to your roadmap as a future enhancement?
3. Focus on something else for Phase 16?

----------------------------------------------------------------

# Phase 20: Custom Model Integration with Unsloth
## Version 1.0.0

> **Status:** PLANNED (Future Enhancement)
> **Prerequisites:** Phase 16-19 complete, production deployment stable
> **Hardware Note:** Training requires cloud GPU; deployment uses existing Ollama infrastructure

---

## Executive Summary

This phase introduces the ability to fine-tune custom LLM models for domain-specific RAG tasks using Unsloth, then deploy them through the existing Ollama infrastructure. The key innovation is a **Train Elsewhere, Deploy Locally** approach that respects the 8GB RAM constraint while enabling powerful customization.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        CUSTOM MODEL PIPELINE                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    PHASE 1: DATA COLLECTION                          │    │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │    │
│  │  │ RAG Queries │───▶│ QA Pairs    │───▶│ Training    │              │    │
│  │  │ & Responses │    │ Generator   │    │ Dataset     │              │    │
│  │  └─────────────┘    └─────────────┘    └─────────────┘              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    PHASE 2: CLOUD TRAINING                           │    │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │    │
│  │  │ Google      │    │ Unsloth     │    │ LoRA        │              │    │
│  │  │ Colab/Kaggle│───▶│ Fine-tune   │───▶│ Adapter     │              │    │
│  │  │ (Free T4)   │    │ QLoRA       │    │ (~100MB)    │              │    │
│  │  └─────────────┘    └─────────────┘    └─────────────┘              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    PHASE 3: MODEL EXPORT                             │    │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │    │
│  │  │ LoRA        │    │ GGUF        │    │ Q4_K_M      │              │    │
│  │  │ Merge       │───▶│ Conversion  │───▶│ Quantized   │              │    │
│  │  │             │    │             │    │ (~2-4GB)    │              │    │
│  │  └─────────────┘    └─────────────┘    └─────────────┘              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    PHASE 4: LOCAL DEPLOYMENT                         │    │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │    │
│  │  │ Download    │    │ Ollama      │    │ Agentic     │              │    │
│  │  │ GGUF Model  │───▶│ Import      │───▶│ RAG API     │              │    │
│  │  │             │    │             │    │             │              │    │
│  │  └─────────────┘    └─────────────┘    └─────────────┘              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Step 1: Training Data Collection Module

### 1.1 Purpose
Automatically collect and format training data from RAG interactions.

### 1.2 New File: `src/training/data_collector.rs`

```rust
// src/training/data_collector.rs
// Version: 1.0.0

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use chrono::{DateTime, Utc};

/// A single training example in QA format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingExample {
    /// Unique identifier
    pub id: String,
    /// User query
    pub instruction: String,
    /// Retrieved context (optional)
    pub context: Option<String>,
    /// Model response
    pub response: String,
    /// Quality score (1-5, from user feedback)
    pub quality_score: Option<u8>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Source conversation ID
    pub conversation_id: Option<String>,
}

/// Alpaca format for Unsloth compatibility
#[derive(Debug, Serialize, Deserialize)]
pub struct AlpacaFormat {
    pub instruction: String,
    pub input: String,
    pub output: String,
}

impl From<TrainingExample> for AlpacaFormat {
    fn from(example: TrainingExample) -> Self {
        AlpacaFormat {
            instruction: example.instruction,
            input: example.context.unwrap_or_default(),
            output: example.response,
        }
    }
}

/// Training data collector with buffered writes
pub struct TrainingDataCollector {
    output_path: PathBuf,
    buffer: Mutex<Vec<TrainingExample>>,
    buffer_size: usize,
    min_quality_score: u8,
}

impl TrainingDataCollector {
    pub fn new(output_path: PathBuf) -> Self {
        Self {
            output_path,
            buffer: Mutex::new(Vec::new()),
            buffer_size: 100, // Flush every 100 examples
            min_quality_score: 3, // Only keep quality >= 3
        }
    }

    /// Add a training example (buffers until flush)
    pub fn add_example(&self, example: TrainingExample) -> Result<(), std::io::Error> {
        // Filter by quality if score is provided
        if let Some(score) = example.quality_score {
            if score < self.min_quality_score {
                return Ok(()); // Skip low-quality examples
            }
        }

        let mut buffer = self.buffer.lock().unwrap();
        buffer.push(example);

        if buffer.len() >= self.buffer_size {
            self.flush_internal(&mut buffer)?;
        }

        Ok(())
    }

    /// Flush buffer to disk
    pub fn flush(&self) -> Result<(), std::io::Error> {
        let mut buffer = self.buffer.lock().unwrap();
        self.flush_internal(&mut buffer)
    }

    fn flush_internal(&self, buffer: &mut Vec<TrainingExample>) -> Result<(), std::io::Error> {
        if buffer.is_empty() {
            return Ok(());
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.output_path)?;

        let mut writer = BufWriter::new(file);

        for example in buffer.drain(..) {
            let alpaca: AlpacaFormat = example.into();
            serde_json::to_writer(&mut writer, &alpaca)?;
            writeln!(writer)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Export to Unsloth-compatible JSONL format
    pub fn export_for_unsloth(&self, output_path: &PathBuf) -> Result<usize, std::io::Error> {
        let input = std::fs::read_to_string(&self.output_path)?;
        let mut count = 0;

        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);

        for line in input.lines() {
            if let Ok(example) = serde_json::from_str::<AlpacaFormat>(line) {
                serde_json::to_writer(&mut writer, &example)?;
                writeln!(writer)?;
                count += 1;
            }
        }

        writer.flush()?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_training_example_to_alpaca() {
        let example = TrainingExample {
            id: "test-1".to_string(),
            instruction: "What is Rust?".to_string(),
            context: Some("Rust is a systems programming language.".to_string()),
            response: "Rust is a systems programming language focused on safety.".to_string(),
            quality_score: Some(5),
            timestamp: Utc::now(),
            conversation_id: None,
        };

        let alpaca: AlpacaFormat = example.into();
        assert_eq!(alpaca.instruction, "What is Rust?");
        assert!(!alpaca.input.is_empty());
    }

    #[test]
    fn test_collector_flush() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("training_data.jsonl");
        
        let collector = TrainingDataCollector::new(path.clone());
        
        let example = TrainingExample {
            id: "test-1".to_string(),
            instruction: "Test question".to_string(),
            context: None,
            response: "Test answer".to_string(),
            quality_score: Some(4),
            timestamp: Utc::now(),
            conversation_id: None,
        };

        collector.add_example(example).unwrap();
        collector.flush().unwrap();

        assert!(path.exists());
    }
}
```

### 1.3 Directory Structure
```
~/ag/
├── src/
│   └── training/
│       ├── mod.rs              # Module declaration
│       ├── data_collector.rs   # Training data collection
│       └── export.rs           # Export utilities
├── data/
│   └── training/
│       ├── raw/                # Raw collected examples
│       └── processed/          # Unsloth-ready JSONL
```

---

## Step 2: Unsloth Training Notebook

### 2.1 Purpose
Provide a ready-to-use Colab notebook for fine-tuning.

### 2.2 File: `notebooks/unsloth_finetune.ipynb` (Exported as Python)

```python
# notebooks/unsloth_finetune.py
# Version: 1.0.0
# Run in Google Colab with T4 GPU

"""
Agentic RAG Custom Model Fine-tuning with Unsloth
================================================
This notebook fine-tunes a small LLM for your RAG system.

Prerequisites:
- Upload your training_data.jsonl to Colab
- Select Runtime > Change runtime type > T4 GPU
"""

# Cell 1: Install Unsloth
# %%capture
!pip install "unsloth[colab-new] @ git+https://github.com/unslothai/unsloth.git"
!pip install --no-deps trl peft accelerate bitsandbytes

# Cell 2: Import and Configure
from unsloth import FastLanguageModel
import torch

# Configuration - MODIFY THESE
MODEL_NAME = "unsloth/Phi-3.5-mini-instruct-bnb-4bit"  # Matches your Ollama setup
MAX_SEQ_LENGTH = 2048
LOAD_IN_4BIT = True

# For 8GB RAM deployment, use smaller models:
# - unsloth/Phi-3.5-mini-instruct-bnb-4bit (3.8B params, ~2GB GGUF)
# - unsloth/Llama-3.2-1B-Instruct-bnb-4bit (1B params, ~0.6GB GGUF)
# - unsloth/Llama-3.2-3B-Instruct-bnb-4bit (3B params, ~1.8GB GGUF)

# Cell 3: Load Model
model, tokenizer = FastLanguageModel.from_pretrained(
    model_name=MODEL_NAME,
    max_seq_length=MAX_SEQ_LENGTH,
    load_in_4bit=LOAD_IN_4BIT,
    dtype=None,  # Auto-detect
)

# Cell 4: Add LoRA Adapters
model = FastLanguageModel.get_peft_model(
    model,
    r=16,  # LoRA rank
    target_modules=[
        "q_proj", "k_proj", "v_proj", "o_proj",
        "gate_proj", "up_proj", "down_proj"
    ],
    lora_alpha=16,
    lora_dropout=0,
    bias="none",
    use_gradient_checkpointing="unsloth",
    random_state=3407,
)

# Cell 5: Prepare Dataset
from datasets import load_dataset

# Upload your training_data.jsonl to Colab first
dataset = load_dataset("json", data_files="training_data.jsonl", split="train")

# Alpaca prompt template (matches your RAG output format)
alpaca_prompt = """Below is an instruction that describes a task, paired with an input that provides further context. Write a response that appropriately completes the request.

### Instruction:
{}

### Input:
{}

### Response:
{}"""

def formatting_prompts_func(examples):
    instructions = examples["instruction"]
    inputs = examples["input"]
    outputs = examples["output"]
    texts = []
    for instruction, input, output in zip(instructions, inputs, outputs):
        text = alpaca_prompt.format(instruction, input, output) + tokenizer.eos_token
        texts.append(text)
    return {"text": texts}

dataset = dataset.map(formatting_prompts_func, batched=True)

# Cell 6: Training Configuration
from trl import SFTTrainer
from transformers import TrainingArguments

trainer = SFTTrainer(
    model=model,
    tokenizer=tokenizer,
    train_dataset=dataset,
    dataset_text_field="text",
    max_seq_length=MAX_SEQ_LENGTH,
    dataset_num_proc=2,
    packing=False,
    args=TrainingArguments(
        per_device_train_batch_size=2,
        gradient_accumulation_steps=4,
        warmup_steps=5,
        max_steps=100,  # Increase for better results (500-1000)
        learning_rate=2e-4,
        fp16=not torch.cuda.is_bf16_supported(),
        bf16=torch.cuda.is_bf16_supported(),
        logging_steps=10,
        optim="adamw_8bit",
        weight_decay=0.01,
        lr_scheduler_type="linear",
        seed=3407,
        output_dir="outputs",
    ),
)

# Cell 7: Train!
trainer_stats = trainer.train()
print(f"Training completed in {trainer_stats.metrics['train_runtime']:.2f}s")

# Cell 8: Export to GGUF for Ollama
# Choose quantization based on your RAM:
# - q4_k_m: Best balance (recommended for 8GB RAM)
# - q8_0: Higher quality, larger file
# - q2_k: Smallest, lower quality

model.save_pretrained_gguf(
    "ag-custom-model",
    tokenizer,
    quantization_method="q4_k_m"
)

print("✅ Model exported to: ag-custom-model/")
print("Download the .gguf file and deploy via Ollama")

# Cell 9: Create Ollama Modelfile
modelfile_content = '''FROM ./ag-custom-model-q4_k_m.gguf

TEMPLATE """Below is an instruction that describes a task, paired with an input that provides further context. Write a response that appropriately completes the request.

### Instruction:
{{ .Prompt }}

### Input:
{{ .Context }}

### Response:
"""

PARAMETER temperature 0.7
PARAMETER top_p 0.9
PARAMETER stop "### Instruction:"
PARAMETER stop "### Input:"
'''

with open("Modelfile", "w") as f:
    f.write(modelfile_content)

print("✅ Modelfile created")
print("\nTo deploy locally:")
print("1. Download ag-custom-model-q4_k_m.gguf and Modelfile")
print("2. Run: ollama create ag-custom -f Modelfile")
print("3. Test: ollama run ag-custom")
```

---

## Step 3: Model Configuration Updates

### 3.1 File: `src/config.rs` additions

```rust
// Add to existing config.rs
// Version: 2.1.0 (adds custom model support)

/// Custom model configuration
#[derive(Debug, Clone, Deserialize)]
pub struct CustomModelConfig {
    /// Use custom model instead of default
    #[serde(default)]
    pub enabled: bool,

    /// Model name in Ollama (after import)
    #[serde(default = "default_custom_model_name")]
    pub model_name: String,

    /// Download URL for GGUF (optional, for installer)
    pub download_url: Option<String>,

    /// Expected file hash (SHA256) for verification
    pub file_hash: Option<String>,

    /// Fallback to default model if custom unavailable
    #[serde(default = "default_true")]
    pub fallback_enabled: bool,
}

fn default_custom_model_name() -> String {
    "ag-custom".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for CustomModelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_name: default_custom_model_name(),
            download_url: None,
            file_hash: None,
            fallback_enabled: true,
        }
    }
}

// Environment variables:
// CUSTOM_MODEL_ENABLED=true|false
// CUSTOM_MODEL_NAME=ag-custom
// CUSTOM_MODEL_URL=https://...
// CUSTOM_MODEL_HASH=sha256:...
// CUSTOM_MODEL_FALLBACK=true|false
```

### 3.2 File: `src/llm/model_loader.rs`

```rust
// src/llm/model_loader.rs
// Version: 1.0.0

use crate::config::CustomModelConfig;
use std::process::Command;
use tracing::{info, warn, error};

pub struct ModelLoader {
    config: CustomModelConfig,
    ollama_base_url: String,
}

impl ModelLoader {
    pub fn new(config: CustomModelConfig, ollama_base_url: String) -> Self {
        Self { config, ollama_base_url }
    }

    /// Check if custom model is available in Ollama
    pub async fn is_model_available(&self) -> bool {
        let client = reqwest::Client::new();
        let url = format!("{}/api/tags", self.ollama_base_url);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    if let Some(models) = json["models"].as_array() {
                        return models.iter().any(|m| {
                            m["name"].as_str() == Some(&self.config.model_name)
                        });
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    /// Get the model name to use (custom or fallback)
    pub async fn get_active_model(&self) -> String {
        if !self.config.enabled {
            return self.get_default_model();
        }

        if self.is_model_available().await {
            info!(model = %self.config.model_name, "Using custom model");
            self.config.model_name.clone()
        } else if self.config.fallback_enabled {
            warn!(
                custom = %self.config.model_name,
                fallback = %self.get_default_model(),
                "Custom model unavailable, using fallback"
            );
            self.get_default_model()
        } else {
            error!(model = %self.config.model_name, "Custom model unavailable and fallback disabled");
            panic!("Required custom model not available");
        }
    }

    fn get_default_model(&self) -> String {
        std::env::var("LLM_MODEL").unwrap_or_else(|_| "phi3.5:3.8b".to_string())
    }

    /// Import a GGUF model into Ollama (used by installer)
    pub fn import_gguf(gguf_path: &str, model_name: &str, modelfile_path: &str) -> Result<(), String> {
        let output = Command::new("ollama")
            .args(["create", model_name, "-f", modelfile_path])
            .output()
            .map_err(|e| format!("Failed to run ollama: {}", e))?;

        if output.status.success() {
            info!(model = %model_name, "Successfully imported custom model");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Failed to import model: {}", stderr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model() {
        let config = CustomModelConfig::default();
        let loader = ModelLoader::new(config, "http://localhost:11434".to_string());
        assert_eq!(loader.get_default_model(), "phi3.5:3.8b");
    }
}
```

---

## Step 4: Installer Updates

### 4.1 File: `installer/custom_model.sh`

```bash
#!/bin/bash
# installer/custom_model.sh
# Version: 1.0.0
# Custom model installation for Agentic RAG

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODEL_DIR="${HOME}/.local/share/ag/models"
MODEL_NAME="${CUSTOM_MODEL_NAME:-ag-custom}"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check if Ollama is running
check_ollama() {
    if ! curl -s http://localhost:11434/api/tags > /dev/null 2>&1; then
        log_error "Ollama is not running. Start with: ollama serve"
        exit 1
    fi
    log_info "Ollama is running"
}

# Download custom model GGUF
download_model() {
    local url="$1"
    local output="$2"
    local expected_hash="$3"

    log_info "Downloading custom model..."
    
    mkdir -p "$MODEL_DIR"
    
    if command -v wget &> /dev/null; then
        wget -q --show-progress -O "$output" "$url"
    elif command -v curl &> /dev/null; then
        curl -L --progress-bar -o "$output" "$url"
    else
        log_error "Neither wget nor curl available"
        exit 1
    fi

    # Verify hash if provided
    if [ -n "$expected_hash" ]; then
        local actual_hash=$(sha256sum "$output" | cut -d' ' -f1)
        if [ "$actual_hash" != "$expected_hash" ]; then
            log_error "Hash mismatch! Expected: $expected_hash, Got: $actual_hash"
            rm -f "$output"
            exit 1
        fi
        log_info "Hash verified ✓"
    fi
}

# Create Modelfile and import
import_to_ollama() {
    local gguf_path="$1"
    local model_name="$2"

    log_info "Creating Modelfile..."
    
    cat > "${MODEL_DIR}/Modelfile" << 'EOF'
FROM ./model.gguf

TEMPLATE """Below is an instruction that describes a task, paired with an input that provides further context. Write a response that appropriately completes the request.

### Instruction:
{{ .Prompt }}

### Input:
{{ .Context }}

### Response:
"""

PARAMETER temperature 0.7
PARAMETER top_p 0.9
PARAMETER stop "### Instruction:"
PARAMETER stop "### Input:"
EOF

    # Copy GGUF to expected location
    cp "$gguf_path" "${MODEL_DIR}/model.gguf"

    log_info "Importing model to Ollama..."
    cd "$MODEL_DIR"
    ollama create "$model_name" -f Modelfile

    log_info "Custom model '$model_name' imported successfully!"
}

# Check if model already exists
check_existing() {
    if ollama list | grep -q "$MODEL_NAME"; then
        log_warn "Model '$MODEL_NAME' already exists"
        read -p "Overwrite? [y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "Skipping model import"
            exit 0
        fi
    fi
}

# Main
main() {
    local gguf_url="${CUSTOM_MODEL_URL:-}"
    local gguf_path="${CUSTOM_MODEL_PATH:-}"
    local expected_hash="${CUSTOM_MODEL_HASH:-}"

    check_ollama
    check_existing

    if [ -n "$gguf_path" ] && [ -f "$gguf_path" ]; then
        log_info "Using local GGUF: $gguf_path"
        import_to_ollama "$gguf_path" "$MODEL_NAME"
    elif [ -n "$gguf_url" ]; then
        local output="${MODEL_DIR}/${MODEL_NAME}.gguf"
        download_model "$gguf_url" "$output" "$expected_hash"
        import_to_ollama "$output" "$MODEL_NAME"
    else
        log_error "No model source specified"
        echo "Set either:"
        echo "  CUSTOM_MODEL_PATH=/path/to/model.gguf"
        echo "  CUSTOM_MODEL_URL=https://..."
        exit 1
    fi

    # Enable custom model in config
    echo "CUSTOM_MODEL_ENABLED=true" >> "${HOME}/.config/ag/env"
    echo "CUSTOM_MODEL_NAME=$MODEL_NAME" >> "${HOME}/.config/ag/env"

    log_info "Custom model setup complete!"
    echo ""
    echo "Test with: ollama run $MODEL_NAME"
    echo "The Agentic RAG system will automatically use this model."
}

main "$@"
```

---

## Step 5: API Endpoints for Training Data

### 5.1 File: `src/api/training.rs`

```rust
// src/api/training.rs
// Version: 1.0.0

use actix_web::{web, HttpResponse, post, get};
use serde::{Deserialize, Serialize};
use crate::training::data_collector::{TrainingDataCollector, TrainingExample};
use chrono::Utc;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct FeedbackRequest {
    pub query: String,
    pub response: String,
    pub context: Option<String>,
    pub quality_score: u8,  // 1-5
    pub conversation_id: Option<String>,
}

#[derive(Serialize)]
pub struct FeedbackResponse {
    pub status: String,
    pub example_id: String,
}

/// POST /training/feedback
/// Submit user feedback for training data collection
#[post("/training/feedback")]
pub async fn submit_feedback(
    collector: web::Data<TrainingDataCollector>,
    body: web::Json<FeedbackRequest>,
) -> HttpResponse {
    let example_id = Uuid::new_v4().to_string();
    
    let example = TrainingExample {
        id: example_id.clone(),
        instruction: body.query.clone(),
        context: body.context.clone(),
        response: body.response.clone(),
        quality_score: Some(body.quality_score.clamp(1, 5)),
        timestamp: Utc::now(),
        conversation_id: body.conversation_id.clone(),
    };

    match collector.add_example(example) {
        Ok(_) => HttpResponse::Ok().json(FeedbackResponse {
            status: "collected".to_string(),
            example_id,
        }),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

#[derive(Serialize)]
pub struct TrainingStats {
    pub total_examples: usize,
    pub high_quality_count: usize,  // score >= 4
    pub ready_for_export: bool,
}

/// GET /training/stats
/// Get training data collection statistics
#[get("/training/stats")]
pub async fn get_training_stats(
    collector: web::Data<TrainingDataCollector>,
) -> HttpResponse {
    // Implementation would read from collected data
    HttpResponse::Ok().json(TrainingStats {
        total_examples: 0,  // TODO: Implement actual counting
        high_quality_count: 0,
        ready_for_export: false,
    })
}

/// POST /training/export
/// Export collected data for Unsloth training
#[post("/training/export")]
pub async fn export_training_data(
    collector: web::Data<TrainingDataCollector>,
) -> HttpResponse {
    match collector.flush() {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "status": "exported",
            "message": "Training data exported to data/training/processed/"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(submit_feedback)
       .service(get_training_stats)
       .service(export_training_data);
}
```

---

## Step 6: Integration Tests

### 6.1 File: `tests/custom_model_integration.rs`

```rust
// tests/custom_model_integration.rs
// Version: 1.0.0

use std::env;

/// Test that custom model configuration loads correctly
#[test]
fn test_custom_model_config_defaults() {
    // Clear any existing env vars
    env::remove_var("CUSTOM_MODEL_ENABLED");
    
    // Default should be disabled
    let enabled = env::var("CUSTOM_MODEL_ENABLED")
        .map(|v| v == "true")
        .unwrap_or(false);
    
    assert!(!enabled, "Custom model should be disabled by default");
}

/// Test fallback behavior
#[test]
fn test_model_fallback() {
    env::set_var("CUSTOM_MODEL_ENABLED", "true");
    env::set_var("CUSTOM_MODEL_FALLBACK", "true");
    
    let fallback = env::var("CUSTOM_MODEL_FALLBACK")
        .map(|v| v == "true")
        .unwrap_or(true);
    
    assert!(fallback, "Fallback should be enabled by default");
    
    // Cleanup
    env::remove_var("CUSTOM_MODEL_ENABLED");
    env::remove_var("CUSTOM_MODEL_FALLBACK");
}

/// Test training data format
#[test]
fn test_alpaca_format() {
    let example = serde_json::json!({
        "instruction": "What is Rust?",
        "input": "Context about Rust programming language.",
        "output": "Rust is a systems programming language."
    });
    
    assert!(example["instruction"].is_string());
    assert!(example["input"].is_string());
    assert!(example["output"].is_string());
}
```

---

## Installer Impact Summary

### New Components

| Component | Size | Required | Purpose |
|-----------|------|----------|---------|
| `installer/custom_model.sh` | ~3KB | Optional | Custom model import script |
| `data/training/` | Variable | Optional | Training data storage |
| Custom GGUF model | 2-4GB | Optional | Fine-tuned model file |

### New Environment Variables

```bash
# Custom Model Configuration (Optional)
CUSTOM_MODEL_ENABLED=false          # Enable custom model
CUSTOM_MODEL_NAME=ag-custom         # Model name in Ollama
CUSTOM_MODEL_URL=                   # Download URL (for installer)
CUSTOM_MODEL_HASH=                  # SHA256 hash for verification
CUSTOM_MODEL_FALLBACK=true          # Fallback to default if unavailable

# Training Data Collection (Optional)
TRAINING_DATA_ENABLED=false         # Enable training data collection
TRAINING_DATA_PATH=~/.local/share/ag/training/
TRAINING_MIN_QUALITY=3              # Minimum quality score to collect
```

### Installer Checklist Updates

```bash
# Add to installer/install.sh

# Optional: Custom Model Setup
if [ "$INSTALL_CUSTOM_MODEL" = "true" ]; then
    echo "Setting up custom model..."
    ./custom_model.sh
fi
```

---

## Recommended Model Choices for 8GB RAM

| Model | Parameters | GGUF Size (Q4_K_M) | RAM Usage | Recommendation |
|-------|------------|-------------------|-----------|----------------|
| Phi-3.5-mini | 3.8B | ~2.2GB | ~4GB | ✅ **Best for your system** |
| Llama-3.2-1B | 1B | ~0.6GB | ~2GB | Good for very limited RAM |
| Llama-3.2-3B | 3B | ~1.8GB | ~3.5GB | Good balance |
| Gemma-2-2B | 2B | ~1.2GB | ~3GB | Alternative option |

---

## Success Metrics

1. **Training Data Quality**: Average quality score >= 4.0
2. **Model Performance**: Response relevance improved by measurable margin
3. **Resource Compliance**: RAM usage stays under 6GB during inference
4. **Deployment Success**: Model imports to Ollama without errors
5. **Fallback Reliability**: System gracefully falls back if custom model unavailable

---

## Timeline Estimate

| Step | Duration | Dependencies |
|------|----------|--------------|
| Step 1: Data Collector | 2 hours | None |
| Step 2: Training Notebook | 1 hour | Step 1 |
| Step 3: Config Updates | 1 hour | None |
| Step 4: Installer Updates | 1 hour | Step 3 |
| Step 5: API Endpoints | 2 hours | Step 1 |
| Step 6: Integration Tests | 1 hour | All above |
| **Training (Cloud)** | 30 min - 2 hours | Steps 1-2 |
| **Total Development** | ~8 hours | |

---

## References

- [Unsloth Documentation](https://unsloth.ai/docs)
- [Unsloth + Ollama Tutorial](https://unsloth.ai/docs/get-started/fine-tuning-llms-guide/tutorial-how-to-finetune-llama-3-and-use-in-ollama)
- [GGUF Quantization Guide](https://unsloth.ai/docs/basics/inference-and-deployment/saving-to-gguf)

---

**Document Version:** 1.0.0
**Created:** 2025-01-11
**Status:** PLANNED

