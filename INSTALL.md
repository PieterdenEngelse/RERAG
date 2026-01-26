# ag Installation Guide

**ag** is an Agentic RAG (Retrieval-Augmented Generation) platform with knowledge graph support, distributed tracing, and full observability.

## Quick Start

### Prerequisites

- **Docker** and **Docker Compose** (for infrastructure services)
- **Ollama** (for LLM inference) - https://ollama.ai

### 1. Start Infrastructure

```bash
# Start all services (Neo4j, Redis, Prometheus, Grafana, Loki, Tempo, OTel)
docker compose -f docker-compose.full.yml up -d

# Verify services are running
docker compose -f docker-compose.full.yml ps
```

### 2. Configure Environment

```bash
# Copy example config
cp .env.example .env

# Edit if needed (defaults work with docker-compose.full.yml)
nano .env
```

### 3. Install Ollama Models

```bash
# Install embedding model
ollama pull nomic-embed-text

# Install chat model
ollama pull phi
```

### 4. Run ag

```bash
./ag
```

### 5. Access Services

| Service | URL | Credentials |
|---------|-----|-------------|
| **ag Frontend** | http://localhost:1789 | - |
| **ag Backend API** | http://localhost:3010 | - |
| **Neo4j Browser** | http://localhost:7474 | neo4j / agpassword123 |
| **Grafana** | http://localhost:3000 | admin / admin |
| **Prometheus** | http://localhost:9090 | - |

---

## Minimal Installation (No Docker)

If you only want basic RAG without graph/observability features:

```bash
# Just run the binary - uses SQLite and in-memory cache
./ag
```

Features available without infrastructure:
- ✅ Document upload and indexing
- ✅ Basic semantic search
- ✅ Ollama LLM integration
- ❌ Knowledge graph (GraphRAG)
- ❌ Redis caching
- ❌ Distributed tracing
- ❌ Metrics dashboards

---

## Service Details

### Neo4j (Knowledge Graph)
- **Purpose**: GraphRAG - multi-hop reasoning, entity relationships
- **Ports**: 7474 (browser), 7687 (bolt)
- **Data**: Persisted in `neo4j-data` volume

### Redis (Cache)
- **Purpose**: L3 cache for fast repeated queries
- **Port**: 6379
- **Data**: Persisted in `redis-data` volume

### Prometheus (Metrics)
- **Purpose**: Collects metrics from ag backend
- **Port**: 9090
- **Scrapes**: http://host.docker.internal:3010/monitoring/metrics

### Grafana (Dashboards)
- **Purpose**: Visualize metrics, logs, traces
- **Port**: 3001
- **Data sources**: Prometheus, Loki, Tempo

### Loki (Logs)
- **Purpose**: Log aggregation and search
- **Port**: 3100

### Tempo (Traces)
- **Purpose**: Distributed tracing storage
- **Ports**: 3200 (API), 4317 (OTLP gRPC)

### OTel Collector (Telemetry)
- **Purpose**: Routes traces/logs to backends
- **Ports**: 4318 (OTLP HTTP), 8888 (metrics)

---

## Troubleshooting

### Check service health
```bash
docker compose -f docker-compose.full.yml ps
docker compose -f docker-compose.full.yml logs <service-name>
```

### Reset everything
```bash
docker compose -f docker-compose.full.yml down -v
docker compose -f docker-compose.full.yml up -d
```

### Neo4j connection issues
```bash
# Test connection
curl http://localhost:7474

# Check logs
docker logs ag-neo4j
```

### Redis connection issues
```bash
# Test connection
redis-cli -h localhost ping

# Check logs
docker logs ag-redis
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Your Machine                             │
│                                                                  │
│  ┌──────────────┐     ┌─────────────────────────────────────┐   │
│  │  ag Backend  │────▶│           Docker Services            │   │
│  │    :3010     │     │                                      │   │
│  └──────────────┘     │  Neo4j     :7474, :7687             │   │
│         │             │  Redis     :6379                     │   │
│         │             │  Prometheus:9090                     │   │
│  ┌──────────────┐     │  Grafana   :3001                     │   │
│  │  ag Frontend │     │  Loki      :3100                     │   │
│  │    :1789     │     │  Tempo     :3200, :4317              │   │
│  └──────────────┘     │  OTel      :4318                     │   │
│                       └─────────────────────────────────────┘   │
│  ┌──────────────┐                                               │
│  │   Ollama     │  (install separately)                         │
│  │   :11434     │                                               │
│  └──────────────┘                                               │
└─────────────────────────────────────────────────────────────────┘
```

---

## Updating

```bash
# Pull latest images
docker compose -f docker-compose.full.yml pull

# Restart with new images
docker compose -f docker-compose.full.yml up -d
```

---

## Uninstalling

```bash
# Stop and remove containers
docker compose -f docker-compose.full.yml down

# Also remove data volumes (WARNING: deletes all data)
docker compose -f docker-compose.full.yml down -v
```
