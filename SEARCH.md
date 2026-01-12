# 3-Layer Memory Search Architecture

## Status: ✅ IMPLEMENTED

This document describes the 3-layer memory search architecture for efficient observation retrieval.

## 1. Data Model & Storage

### Observation Table (SQLite + FTS5)

**Fields:**
- `id` (TEXT PRIMARY KEY)
- `entry_type` (TEXT) - bugfix, feature, refactor, etc.
- `title` (TEXT)
- `narrative` (TEXT) - full description
- `facts` (JSON array)
- `concepts` (JSON array)
- `files_read` (JSON array)
- `files_modified` (JSON array)
- `author` (TEXT, optional)
- `project` (TEXT, optional) - for multi-project filtering
- `created_at` (TEXT)
- `updated_at` (TEXT)

**FTS5 Virtual Table:** `manual_observations_fts` indexes title, narrative, facts, concepts, files for full-text search.

## 2. Backend API Design (Actix Web)

### Layer 1 – Search (Index)

**Route:** `POST /memory/observations/search`

**Request Body:**
```json
{
  "query": "search terms",
  "entry_type": "bugfix",       // optional
  "project": "my-project",      // optional
  "date_start": "2024-01-01",   // optional
  "date_end": "2024-12-31",     // optional
  "order": "relevance",         // relevance|newest|oldest
  "limit": 10,                  // default: 10
  "offset": 0                   // for pagination
}
```

**Response:**
```json
{
  "results": [
    {
      "summary": {
        "id": "uuid",
        "entry_type": "bugfix",
        "title": "Fixed memory leak",
        "project": "backend",
        "created_at": "2024-01-15T10:30:00Z"
      },
      "score": 0.95,
      "snippet": "...matched <b>text</b>..."
    }
  ],
  "offset": 0,
  "limit": 10,
  "request_id": "abc123"
}
```

### Layer 2 – Timeline (Context)

**Route:** `POST /memory/observations/timeline`

**Request Body:**
```json
{
  "anchor_id": "uuid",          // optional - specific observation
  "query": "search terms",      // optional - find anchor by search
  "entry_type": "feature",      // optional
  "project": "frontend",        // optional
  "depth_before": 3,            // default: 10
  "depth_after": 3              // default: 10
}
```

**Response:**
```json
{
  "timeline": [
    { "id": "...", "entry_type": "...", "title": "...", "project": "...", "created_at": "..." },
    { "id": "anchor", "entry_type": "...", "title": "ANCHOR", "project": "...", "created_at": "..." },
    { "id": "...", "entry_type": "...", "title": "...", "project": "...", "created_at": "..." }
  ],
  "request_id": "abc123"
}
```

### Layer 3 – Full Details (Fetch)

**Route:** `POST /memory/observations/fetch`

**Request Body:**
```json
{
  "ids": ["uuid1", "uuid2", "uuid3"]
}
```

**Constraints:** Max 20 IDs per request (enforced at API level).

**Response:**
```json
{
  "observations": [
    {
      "id": "uuid1",
      "entry_type": "bugfix",
      "title": "Fixed memory leak",
      "narrative": "Full description...",
      "facts": ["fact1", "fact2"],
      "concepts": ["memory", "performance"],
      "files_read": ["src/main.rs"],
      "files_modified": ["src/cache.rs"],
      "author": "developer",
      "project": "backend",
      "created_at": "2024-01-15T10:30:00Z",
      "updated_at": "2024-01-15T10:30:00Z"
    }
  ],
  "request_id": "abc123"
}
```

### CRUD Operations

| Route | Method | Description |
|-------|--------|-------------|
| `/memory/observations` | POST | Create new observation |
| `/memory/observations` | GET | List observations (with `entry_type`, `project`, `limit` query params) |
| `/memory/observations/{id}` | GET | Get single observation |
| `/memory/observations/{id}` | PUT | Update observation |
| `/memory/observations/{id}` | DELETE | Delete observation |

## 3. Monitoring & Metrics

### Prometheus Metrics

**Layer-specific counters:**
- `memory_search_requests_total{layer="search|timeline|fetch", status="ok|err"}`
- `memory_search_latency_ms{layer="search|timeline|fetch"}` (histogram)
- `memory_search_tokens_saved_total` (estimated tokens saved)

**Monitoring Endpoint:** `GET /monitoring/memory/search/stats`

```json
{
  "layers": [
    { "layer": "search", "requests_ok": 100, "requests_err": 2, "latency_p50_ms": 5.2, "latency_p99_ms": 10.4 },
    { "layer": "timeline", "requests_ok": 50, "requests_err": 0, "latency_p50_ms": 8.1, "latency_p99_ms": 16.2 },
    { "layer": "fetch", "requests_ok": 30, "requests_err": 1, "latency_p50_ms": 12.5, "latency_p99_ms": 25.0 }
  ],
  "tokens_saved_total": 15000,
  "request_id": "abc123"
}
```

### Tracing

All 3-layer endpoints emit spans with:
- `memory_search_layer` span name
- `layer` attribute (search|timeline|fetch)

## 4. Frontend / Client Updates

### UI Flow (Dioxus frontend under frontend/fro/src/):
1. Add a "Memory Search" page or panel with three panes: search results, timeline context, details view.
2. Search form triggers `/memory/observations/search`, listing results (IDs clickable).
3. Selecting an ID loads timeline (call `/memory/observations/timeline`).
4. "Get details" button batches selected IDs and calls `/memory/observations/fetch`.

### CLI / scripts (tools/, scripts/):
Mirror same flow with commands (e.g., `ag memory search ...`).

## 5. Token & Performance Controls

- **Limit defaults:** limit=10, depth_before=10, depth_after=10
- **Batch enforcement:** Max 20 IDs per fetch request
- **Caching:** Optional LRU for search responses or timeline contexts

## 6. Implementation Checklist

- [x] Define observation schema & migrations (SQLite + FTS5)
- [x] Add `project` field to all observation types
- [x] Implement `/memory/observations/search` with offset, project
- [x] Implement `/memory/observations/timeline` with project filter
- [x] Implement `/memory/observations/fetch` with batch limit (20)
- [x] Add layer-specific Prometheus metrics
- [x] Add tracing spans for observability
- [x] Add `/monitoring/memory/search/stats` endpoint
- [ ] Update frontend to use new endpoints
- [ ] Add CLI commands for memory search

## 7. Alternatives or Enhancements

- Vector embeddings (Chroma/Qdrant) for fuzzy search; wrap inside Layer 1 but still return compact results.
- Git/issue importers: scripts under tools/ to populate observation DB from git history or Linear/Jira tickets.
- Automated summarization: tasks to generate narrative/facts per observation using LLMs offline.
