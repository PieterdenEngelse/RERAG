#!/usr/bin/env python3
"""
patch_docker_rich_modals.py  v1.0.0

Replaces the simple string-based container info modal with
rich structured content (paragraphs + bullet lists) per container.

Run:
    python3 patch_docker_rich_modals.py
"""

import sys
from pathlib import Path

TARGET = Path.home() / "ag/frontend/fro/src/pages/monitor/docker.rs"

if not TARGET.exists():
    print(f"ERROR: not found: {TARGET}")
    sys.exit(1)

text = TARGET.read_text()

# ── 1. Replace modal body to use ContainerInfoBody component ─────────────────
OLD_BODY = (
    '                                div { class: "text-sm text-gray-300",\n'
    '                                    p { { container_info(display_name).1 } }\n'
    '                                }'
)
NEW_BODY = (
    '                                div { class: "text-sm text-gray-300",\n'
    '                                    ContainerInfoBody { name: display_name.to_string() }\n'
    '                                }'
)

# ── 2. Replace container_info fn + add ContainerInfoBody component ────────────
OLD_INFO_FN = '''\
/// Per-container info descriptions
fn container_info(name: &str) -> (&'static str, &'static str) {
    match name {
        "grafana"    => ("Grafana", "Dashboard and visualization platform. Displays Prometheus metrics, Loki logs, and Tempo traces. Access at port 3001."),
        "redis"      => ("Redis", "Redis serves as L3 cache for search results. L1 (in-memory) and L2 (disk) caches are checked first. Redis provides shared caching across restarts. Runs on port 6379. Requires REDIS_ENABLED=true and REDIS_URL in .env to connect."),
        "prometheus" => ("Prometheus", "Metrics collection and storage. Scrapes backend metrics and feeds them to Grafana. Port 9090."),
        "loki"       => ("Loki", "Log aggregation service. Receives logs from Vector and makes them queryable in Grafana. Port 3100."),
        "tempo"      => ("Tempo", "Distributed tracing backend. Receives traces from the OTel Collector and displays them in Grafana. Port 4317."),
        "otel"       => ("OTel Collector", "OpenTelemetry (OTel) is an open standard for observability. It does distributed tracing — every HTTP request gets a trace_id assigned by the trace middleware. The backend sends trace data via OTLP HTTP to the OTel Collector on port 4318, which forwards to Tempo on port 4317 for storage. Grafana queries Tempo to display the traces. The trace_id in every log line links that log entry to its full trace in Tempo/Grafana. The backend sends traces in a fire-and-forget manner — it does not maintain a persistent connection to the OTel Collector. If the Collector is not reachable, traces are silently dropped and the backend continues normally. This is why there is no meaningful connected/disconnected status for OTel, unlike Redis which has a persistent connection pool. Main use cases: Debugging slow queries — a trace shows the full timeline of a single request: how long retrieval, LLM, embedding each took. Comparing backends — llama-server vs Ollama side by side in Grafana, actual latency per request not just averages. Finding bottlenecks — BM25 search, vector search, RRF fusion, embedding, or LLM call — the trace shows which is the culprit. Correlating logs with requests — jump from a log error directly to the full trace in Tempo. On constrained hardware knowing exactly where milliseconds are going is especially valuable."),
        "neo4j"      => ("Neo4j", "Knowledge graph database. Used only during document ingestion to extract entities. Not used at runtime — petgraph handles runtime graph queries. Ports 7474 (HTTP) and 7687 (Bolt)."),
        _            => ("Container", "Infrastructure container for the ag observability stack."),
    }
}'''

NEW_INFO_FN = '''\
/// Per-container info title
fn container_info_title(name: &str) -> &'static str {
    match name {
        "grafana"    => "Grafana",
        "redis"      => "Redis",
        "prometheus" => "Prometheus",
        "loki"       => "Loki",
        "tempo"      => "Tempo",
        "otel"       => "OTel Collector",
        "neo4j"      => "Neo4j",
        _            => "Container",
    }
}

/// Rich modal body per container
#[component]
fn ContainerInfoBody(name: String) -> Element {
    match name.as_str() {
        "grafana" => rsx! {
            p { "Dashboard and visualization platform. Displays Prometheus metrics, Loki logs, and Tempo traces." }
            p { class: "mt-2 text-gray-400", "Port: 3001" }
        },
        "redis" => rsx! {
            p { "Redis serves as the L3 cache for search results. Caches are checked in order:" }
            ul { class: "list-disc ml-5 mt-1 space-y-1",
                li { "L1 — in-memory (fastest)" }
                li { "L2 — disk" }
                li { "L3 — Redis (shared across restarts)" }
            }
            p { class: "mt-2 text-gray-400", "Port: 6379. Requires REDIS_ENABLED=true and REDIS_URL in .env to connect." }
        },
        "prometheus" => rsx! {
            p { "Metrics collection and storage. Scrapes backend metrics on a regular interval and makes them available to Grafana for visualization." }
            p { class: "mt-2 text-gray-400", "Port: 9090" }
        },
        "loki" => rsx! {
            p { "Log aggregation service. Receives logs from Vector and makes them queryable in Grafana." }
            p { class: "mt-2 text-gray-400", "Port: 3100" }
        },
        "tempo" => rsx! {
            p { "Distributed tracing storage backend. Receives traces from the OTel Collector and stores them for querying in Grafana." }
            p { class: "mt-2 text-gray-400", "Port: 4317" }
        },
        "otel" => rsx! {
            p { "OpenTelemetry (OTel) is an open standard for observability. It provides distributed tracing — every HTTP request gets a " code { "trace_id" } " assigned by the trace middleware." }
            p { class: "mt-2", "The backend sends trace data via OTLP HTTP to the OTel Collector on port 4318, which forwards to Tempo on port 4317. Grafana queries Tempo to display traces. The " code { "trace_id" } " in every log line links that entry to its full trace in Tempo/Grafana." }
            p { class: "mt-2", "The backend sends traces in a fire-and-forget manner — no persistent connection is maintained. If the Collector is unreachable, traces are silently dropped and the backend continues normally." }
            p { class: "mt-2 font-semibold text-gray-200", "Main use cases:" }
            ul { class: "list-disc ml-5 mt-1 space-y-1",
                li { strong { "Debugging slow queries" } " — full timeline of a request: retrieval, LLM, embedding latency." }
                li { strong { "Comparing backends" } " — llama-server vs Ollama side by side in Grafana." }
                li { strong { "Finding bottlenecks" } " — BM25, vector search, RRF, embedding, or LLM — the trace shows which." }
                li { strong { "Correlating logs with requests" } " — jump from a log error to the full trace in Tempo." }
            }
            p { class: "mt-2 text-gray-400", "Ports: 4318 (OTLP HTTP in), 4317 (Tempo out)" }
        },
        "neo4j" => rsx! {
            p { "Knowledge graph database used " strong { "only during document ingestion" } " to extract and store entities." }
            p { class: "mt-2", "Not used at runtime — all runtime graph queries use petgraph loaded from an exported JSON snapshot. Neo4j is stopped after ingestion to save ~800MB RAM." }
            p { class: "mt-2 text-gray-400", "Ports: 7474 (HTTP), 7687 (Bolt)" }
        },
        _ => rsx! {
            p { "Infrastructure container for the ag observability stack." }
        },
    }
}'''

# ── 3. Update modal title to use new function ─────────────────────────────────
OLD_TITLE = '                                    h2 { class: "text-base font-semibold text-gray-100",\n                                        { container_info(display_name).0 }\n                                    }'
NEW_TITLE = '                                    h2 { class: "text-base font-semibold text-gray-100",\n                                        { container_info_title(display_name) }\n                                    }'

PATCHES = [
    ("info fn",    OLD_INFO_FN, NEW_INFO_FN),
    ("modal body", OLD_BODY,    NEW_BODY),
    ("modal title",OLD_TITLE,   NEW_TITLE),
]

missing = [name for name, old, _ in PATCHES if old not in text]
if missing:
    print("ERROR: could not find:", ", ".join(missing))
    sys.exit(1)

TARGET.with_suffix(".rs.bak5").write_text(text)
for name, old, new in PATCHES:
    text = text.replace(old, new, 1)
    print(f"Patched: {name}")

TARGET.write_text(text)
print(f"\nDone: {TARGET}")
print("Now check: cargo check --target wasm32-unknown-unknown in frontend/fro")
