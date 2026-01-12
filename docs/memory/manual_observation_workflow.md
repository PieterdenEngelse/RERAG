# Manual Observation Workflow

This document explains how to use the internal 3-layer manual memory workflow.

## Overview

The backend exposes the following routes (all require the `ADMIN_API_TOKEN` via the `Authorization: Bearer <token>` header):

- `POST /memory/observations` – create manual observation entries.
- `GET /memory/observations` – list observation summaries.
- `POST /memory/observations/search` – layer 1 index search.
- `POST /memory/observations/timeline` – layer 2 context around an anchor.
- `POST /memory/observations/fetch` – layer 3 full details by IDs.
- `GET|PUT|DELETE /memory/observations/{id}` – retrieve/update/delete a single entry.

All responses include a `request_id` for tracing.

## Authentication

Set an `ADMIN_API_TOKEN` in the environment (for example in `.env`):

```
ADMIN_API_TOKEN=super-secret-token
```

Clients must send `Authorization: Bearer super-secret-token` for every endpoint above. Requests without the correct token receive HTTP 401.

## Creating Observations

Request payload (`POST /memory/observations`):

```json
{
  "entry_type": "incident",
  "title": "Prometheus scrape failures",
  "narrative": "What happened and how it was fixed...",
  "facts": ["Impacted scrape targets: 12", "Root cause: expired cert"],
  "concepts": ["observability", "incident-response"],
  "files_read": ["prometheus/prometheus.yml"],
  "files_modified": ["prometheus/prometheus.yml"],
  "author": "oncall-2025-11-16"
}
```

Validation rules:

- `title`: 1–200 chars
- `entry_type`: 1–100 chars (free-form taxonomy)
- `narrative`: 1–10 000 chars
- `facts`, `concepts`, `files_read`, `files_modified`: max 32 items each

Successful responses:

```json
{
  "id": "d1a566d1-...",
  "request_id": "req_..."
}
```

## Listing Summaries

```
GET /memory/observations?entry_type=incident&limit=10
```

Returns compact `ManualObservationSummary` entries (id, entry_type, title, created_at).

## Layer 1 – Search Index

`POST /memory/observations/search`

```json
{
  "query": "prometheus AND incident",
  "entry_type": "incident",
  "date_start": "2025-01-01",
  "date_end": "2025-12-31",
  "order": "relevance",
  "limit": 5
}
```

`order` accepts `relevance` (default), `newest`, or `oldest`.

This returns `results[]` containing summary + snippet + score, ideal for scanning what exists.

## Layer 2 – Timeline

`POST /memory/observations/timeline`

```json
{
  "anchor_id": "d1a566d1-...",
  "depth_before": 3,
  "depth_after": 3
}
```

Alternative: supply `query` with no `anchor_id`; the server will run a search and anchor on the best match. The response contains chronological summaries before/after the anchor to reconstruct context.

## Layer 3 – Fetch Details

`POST /memory/observations/fetch`

```json
{
  "ids": ["d1a566d1-...", "43b2..."]
}
```

Fetches full observation records (narrative, facts, concepts, files, timestamps). Max 20 IDs per call.

## Update & Delete

- `PUT /memory/observations/{id}` – same payload as create (subject to the same validation rules).
- `DELETE /memory/observations/{id}` – removes the entry.

## Example Curl Sequence

```bash
# create observation
curl -X POST http://127.0.0.1:3010/memory/observations \
  -H "Authorization: Bearer $ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
        "entry_type": "deployment",
        "title": "Hotfix 2025-11-16",
        "narrative": "Patched the memory search module...",
        "facts": ["deployed at 15:22 UTC"],
        "concepts": ["deployment"],
        "files_modified": ["backend/src/api/mod.rs"]
      }'

# layer 1 search
curl -X POST http://127.0.0.1:3010/memory/observations/search \
  -H "Authorization: Bearer $ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query":"deployment", "limit":5}'

# layer 2 timeline around anchor
curl -X POST http://127.0.0.1:3010/memory/observations/timeline \
  -H "Authorization: Bearer $ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"anchor_id":"<id-from-search>", "depth_before":2, "depth_after":2}'

# layer 3 fetch full details
curl -X POST http://127.0.0.1:3010/memory/observations/fetch \
  -H "Authorization: Bearer $ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"ids":["<id1>", "<id2>"]}'
```

## Notes
- All manual observation APIs require an admin token.
- Rate limiting applies (see logs if requests are dropped).
- Use the 3-layer workflow to minimize payload sizes and keep queries efficient.
- Consider scripting ingestion from CI/CD pipelines or incident tooling using the CLI described below (coming next).
