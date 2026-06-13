//! AG Pipeline documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuAgPipeline() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                // ── Header ────────────────────────────────────────────
                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-blue-300", "AG Pipeline" }
                    span { class: "text-xs text-gray-400", "Documents flow from upload to agentic retrieval — fanning out into three stores along the way, with caching, observability, and rate-limiting wrapped around the whole thing." }
                }

                // ── Flow diagram ──────────────────────────────────────
                div { class: "grid grid-cols-3 gap-2 mb-3",
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 col-span-1",
                        h3 { class: "text-sm font-bold text-white mb-1", "Flow" }
                        div { class: "font-mono text-xs text-blue-300 whitespace-pre leading-tight",
"  Document Upload          ⟵ /upload | file watcher
        │
        ▼
     Parsing                ⟵ mime_detect → pdf | text
        │
        ▼
    Chunking                ⟵ fixed | lightweight | semantic | sentence | pipeline
        │
        ▼
   Embedding                ⟵ ONNX (FastEmbed)
        │
        ▼
    Indexing
   ┌────┼────┐
   ▼    ▼    ▼
Tantivy Vec Graph          ⟵ BM25  | rkyv+HNSW | FalkorDB→Petgraph
   └────┼────┘
        ▼
   Retrieval                ⟵ RRF + cache + (optional) graph expand
        │
        ▼
      Agent                 ⟵ Rag | Llm | Hybrid | RagStrict | Agentic"
                        }
                    }

                    // Sidebar: cargo features + entry points
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 col-span-1",
                        h3 { class: "text-sm font-bold text-white mb-1", "Cargo features" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Defaults: " code { class: "text-green-300", "[\"onnx\", \"io_uring\", \"graph\"]" } "."
                        }
                        ul { class: "text-xs text-gray-300 list-disc pl-4 space-y-0.5",
                            li { code { class: "text-green-300", "onnx" } " — FastEmbed embeddings + ONNX layout models" }
                            li { code { class: "text-green-300", "io_uring" } " — async I/O on Linux ≥ 5.1" }
                            li { code { class: "text-green-300", "graph" } " — FalkorDB + petgraph runtime" }
                            li { code { class: "text-green-300", "layout_ml" } " — Native PDF (lopdf, DETR, TableFormer); " em { "off" } " by default" }
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 col-span-1",
                        h3 { class: "text-sm font-bold text-white mb-1", "Entry points" }
                        ul { class: "text-xs text-gray-300 list-disc pl-4 space-y-0.5",
                            li { code { class: "text-green-300", "/upload" } " — multipart POST, per-corpus" }
                            li { code { class: "text-green-300", "/search" } " — GET, hybrid query" }
                            li { code { class: "text-green-300", "/agent/chat" } " — agentic loop" }
                            li { code { class: "text-green-300", "/index/info" } " — store stats (used by /config/chunker)" }
                            li { "File watcher — drop files into the watched dir" }
                        }
                        p { class: "text-xs text-gray-400 mt-1",
                            "Full registry: " code { class: "text-green-300", "backend/src/api/mod.rs" } "."
                        }
                    }
                }

                // ── 8 stages ──────────────────────────────────────────
                h2 { class: "text-sm font-bold text-white mb-1 mt-2", "Pipeline stages" }
                div { class: "grid grid-cols-4 gap-2 mb-3",

                    // 1. Document Upload
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "1. Document Upload" }
                        p { class: "text-xs text-gray-300",
                            "Two ingress paths: a multipart " code { class: "text-green-300", "/upload" } " POST, or the "
                            Link { to: Route::DocuFileWatcher {}, class: "text-blue-400 hover:text-blue-300 underline", "file watcher" }
                            " picking up new files under the watched directory."
                        }
                        p { class: "text-xs text-gray-400",
                            "Files: " code { class: "text-green-300", "api/upload_search.rs" } " · "
                            code { class: "text-green-300", "file_watcher.rs" }
                        }
                        p { class: "text-xs text-gray-400",
                            "Knobs: " code { class: "text-green-300", "MAX_UPLOAD_MB" } " · "
                            code { class: "text-green-300", "FILE_WATCHER_ENABLED" } " · "
                            code { class: "text-green-300", "FILE_WATCHER_DEBOUNCE_MS" } " · per-corpus "
                            code { class: "text-green-300", "watch_dir" } " on " code { class: "text-green-300", "/config/corpus" }
                        }
                    }

                    // 2. Parsing
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "2. Parsing" }
                        p { class: "text-xs text-gray-300",
                            code { class: "text-green-300", "mime_detect.rs" } " dispatches by content type. Plain text/MD → "
                            code { class: "text-green-300", "parser.rs::clean_text" } " for whitespace + line normalization. PDFs branch."
                        }
                        p { class: "text-xs text-gray-300",
                            "PDF paths (priority): "
                            ol { class: "list-decimal pl-4 mt-0.5 space-y-0.5",
                                li { strong { class: "text-gray-200", "Native " } "(" code { class: "text-green-300", "layout_ml" } " feature + " code { class: "text-green-300", "LAYOUT_ML_ENABLED=true" } "): lopdf word boxes → "
                                    Link { to: Route::DocuDetrLayout {}, class: "text-blue-400 hover:text-blue-300 underline", "DETR layout" }
                                    " → TableFormer → DocIR with block tags." }
                                li { strong { class: "text-gray-200", "Extractous " } "fallback (sidecar JVM)." }
                                li { strong { class: "text-gray-200", "pdftotext " } "last-resort: flat text, no structure." }
                            }
                        }
                        p { class: "text-xs text-gray-400",
                            "Knobs: " code { class: "text-green-300", "LAYOUT_ML_ENABLED" } " · "
                            code { class: "text-green-300", "LAYOUT_ML_MODEL_ID" } " (HF Hub auto-download to " code { class: "text-green-300", "~/.cache/huggingface/hub/" } ") · per-corpus override on " code { class: "text-green-300", "/config/corpus" }
                        }
                    }

                    // 3. Chunking
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "3. Chunking" }
                        p { class: "text-xs text-gray-300",
                            code { class: "text-green-300", "memory/chunker.rs" } " + " code { class: "text-green-300", "memory/chunker_factory.rs" } " split text into chunks. "
                            code { class: "text-green-300", "CHUNKER_MODE" } " is hot-reloaded — change takes effect on next upload or reindex."
                        }
                        p { class: "text-xs text-gray-300",
                            "Modes: " code { class: "text-green-300", "fixed" } " (size + sentence snap) · "
                            code { class: "text-green-300", "lightweight" } " (in-text heading detection) · "
                            code { class: "text-green-300", "semantic" } " (embedding-similarity boundaries, one inference per split) · "
                            code { class: "text-green-300", "sentence" } " (sentence-first + overlap) · "
                            code { class: "text-green-300", "pipeline" } " (composes lightweight + semantic, most expensive)."
                        }
                        p { class: "text-xs text-gray-400",
                            "Knobs: " code { class: "text-green-300", "CHUNK_TARGET_SIZE" } " · " code { class: "text-green-300", "CHUNK_MIN_SIZE" } " · " code { class: "text-green-300", "CHUNK_MAX_SIZE" } " · " code { class: "text-green-300", "CHUNK_OVERLAP" } " · " code { class: "text-green-300", "CHUNK_CONTEXT_PREFIX" } " (Anthropic contextual retrieval) · " code { class: "text-green-300", "CHUNK_SEMANTIC_SIMILARITY_THRESHOLD" }
                        }
                        p { class: "text-xs text-gray-400",
                            "Configure at " code { class: "text-green-300", "/config/chunker" } "; per-corpus overrides at " code { class: "text-green-300", "/config/corpus" } "."
                        }
                    }

                    // 4. Embedding
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "4. Embedding" }
                        p { class: "text-xs text-gray-300",
                            code { class: "text-green-300", "embedder.rs" } " runs each chunk through an ONNX model via the "
                            code { class: "text-green-300", "ort" } " crate (FastEmbed wrapper). One fixed-length dense vector per chunk capturing semantic meaning."
                        }
                        p { class: "text-xs text-gray-400",
                            "Knobs: " code { class: "text-green-300", "EMBEDDING_MODEL" } " · ONNX SessionOptions on " code { class: "text-green-300", "/config/onnx" } " (threads, graph optimization level, mem-pattern, execution provider) — all restart-required since the Session is built once at startup."
                        }
                        p { class: "text-xs text-gray-400",
                            "Background: "
                            a { href: "/docu/index/embeddings", class: "text-blue-400 hover:text-blue-300 underline", "Embeddings" }
                            " · "
                            a { href: "/docu/index/onnx", class: "text-blue-400 hover:text-blue-300 underline", "ONNX" }
                            " · "
                            a { href: "/docu/index/tokenizers-general", class: "text-blue-400 hover:text-blue-300 underline", "Tokenizers" }
                        }
                    }

                    // 5. Indexing — fan-out
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "5. Indexing — fan-out" }
                        p { class: "text-xs text-gray-300",
                            code { class: "text-green-300", "index.rs" } " fans each chunk into three stores:"
                        }
                        ul { class: "text-xs text-gray-300 list-disc pl-4 space-y-0.5",
                            li {
                                strong { class: "text-gray-200", "Tantivy" } " on disk (" code { class: "text-gray-400", "~/.local/share/ag/index/tantivy/" } ") — full-text BM25 keyword search. "
                                a { href: "/docu/index/tantivy", class: "text-blue-400 hover:text-blue-300 underline", "Tantivy" } " · "
                                a { href: "/docu/index/bm25", class: "text-blue-400 hover:text-blue-300 underline", "BM25" } "."
                            }
                            li {
                                strong { class: "text-gray-200", "In-memory vector index" } " — rkyv-serialized dense vectors, optional HNSW + product quantization for approximate nearest-neighbour. "
                                a { href: "/docu/index/rkyv", class: "text-blue-400 hover:text-blue-300 underline", "rkyv" } "."
                            }
                            li { strong { class: "text-gray-200", "SQLite " } "(" code { class: "text-gray-400", "documents.db" } ") — durable chunk + embedding backup, metadata, corpus membership." }
                        }
                        p { class: "text-xs text-gray-400",
                            "Knobs: " code { class: "text-green-300", "INDEX_IN_RAM" } " (heap-allocate Tantivy segments) · " code { class: "text-green-300", "HNSW_EF_CONSTRUCTION" } " · " code { class: "text-green-300", "HNSW_EF_SEARCH" } " · " code { class: "text-green-300", "PQ_SUBVECTORS" } "."
                        }
                    }

                    // 6. Graph Building
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "6. Graph Building" }
                        p { class: "text-xs text-gray-300",
                            code { class: "text-green-300", "tools/entity_extractor.rs" } " runs NER on each chunk; "
                            code { class: "text-green-300", "graph/knowledge_builder.rs" } " writes "
                            em { "Document · Chunk · Entity" }
                            " nodes with "
                            code { class: "text-green-300", "HAS_CHUNK" } " / " code { class: "text-green-300", "MENTIONS" }
                            " edges into FalkorDB."
                        }
                        p { class: "text-xs text-gray-300",
                            code { class: "text-green-300", "graph/entity_reconciler.rs" } " merges duplicate entities across documents; "
                            code { class: "text-green-300", "graph/petgraph_runtime.rs" } " loads a snapshot of the durable FalkorDB graph into an in-memory petgraph used at query time — FalkorDB is never on the read path."
                        }
                        p { class: "text-xs text-gray-400",
                            "Knobs: " code { class: "text-green-300", "FALKOR_ENABLED" } " · NER model on " code { class: "text-green-300", "/config/ner" } " · graph URI + auth on " code { class: "text-green-300", "/config/falkordb" } ". Cargo feature: " code { class: "text-green-300", "graph" } "."
                        }
                        p { class: "text-xs text-gray-400",
                            "Background: "
                            a { href: "/docu/index/knowledge-graphs", class: "text-blue-400 hover:text-blue-300 underline", "Knowledge Graphs" }
                            " · "
                            a { href: "/docu/index/entities-production", class: "text-blue-400 hover:text-blue-300 underline", "Entities Production" }
                        }
                    }

                    // 7. Retrieval
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "7. Retrieval" }
                        p { class: "text-xs text-gray-300",
                            code { class: "text-green-300", "retriever.rs" } " runs Tantivy BM25 and vector cosine in parallel, fuses them with "
                            strong { class: "text-gray-200", "Reciprocal Rank Fusion" }
                            ", then optionally expands the result set by walking the petgraph runtime ("
                            code { class: "text-green-300", "graph/graph_retriever.rs" }
                            ")."
                        }
                        p { class: "text-xs text-gray-300",
                            "PointerRag / Section reassembly: when within-doc fragmentation is high, Auto routes to a pointer-based hydration path that returns whole sections instead of disjoint chunks. Knob: " code { class: "text-green-300", "POINTERRAG_AUTO_GAP_THRESHOLD" } "."
                        }
                        p { class: "text-xs text-gray-400",
                            "Caches: L1 in-process LRU · L2 DashMap · L3 Redis (" code { class: "text-green-300", "REDIS_ENABLED" } "). "
                            "Knobs: " code { class: "text-green-300", "SEARCH_TOP_K" } " · " code { class: "text-green-300", "RAG_HYBRID_WEIGHT" } "."
                        }
                    }

                    // 8. Agent
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "8. Agent" }
                        p { class: "text-xs text-gray-300",
                            code { class: "text-green-300", "agent.rs" } " orchestrates the LLM. Five modes — "
                            code { class: "text-green-300", "Rag" } " (retrieval only) · "
                            code { class: "text-green-300", "Llm" } " (no retrieval) · "
                            code { class: "text-green-300", "Hybrid" } " (default — search + LLM fallback) · "
                            code { class: "text-green-300", "RagStrict" } " (grounded answers only) · "
                            code { class: "text-green-300", "Agentic" } "."
                        }
                        p { class: "text-xs text-gray-300",
                            "In " code { class: "text-green-300", "Agentic" }
                            " mode a "
                            a { href: "/docu/index/rig", class: "text-blue-400 hover:text-blue-300 underline", "Rig" }
                            " tool-calling loop in " code { class: "text-green-300", "rig_tools/" }
                            " lets the LLM call retrieval, graph search, and memory tools across multiple turns. Working / episodic / semantic memory lives in "
                            code { class: "text-green-300", "agent_memory.rs" } "."
                        }
                        p { class: "text-xs text-gray-400",
                            "Knobs: " code { class: "text-green-300", "OLLAMA_URL" } " · " code { class: "text-green-300", "OLLAMA_MODEL" } " · " code { class: "text-green-300", "AGENT_MODE" } " · prompt cache on " code { class: "text-green-300", "/config/runtime" } "."
                        }
                    }
                }

                // ── Cross-cutting concerns ────────────────────────────
                h2 { class: "text-sm font-bold text-white mb-1 mt-2", "Cross-cutting concerns" }
                div { class: "grid grid-cols-4 gap-2 mb-3",

                    // Caching
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "Caching tiers" }
                        ul { class: "text-xs text-gray-300 list-disc pl-4 space-y-0.5",
                            li { strong { class: "text-gray-200", "L1 " } "— in-process LRU. Sub-millisecond. Per-process." }
                            li { strong { class: "text-gray-200", "L2 " } "— DashMap. Concurrent, in-process, larger." }
                            li { strong { class: "text-gray-200", "L3 " } "— Redis. Cross-process, optional. Toggle: "
                                code { class: "text-green-300", "REDIS_ENABLED" }
                                ". Hot-reloaded — the " code { class: "text-green-300", "RedisCache" } " is rebuilt in place when any " code { class: "text-green-300", "REDIS_*" } " setting changes."
                            }
                        }
                        p { class: "text-xs text-gray-400",
                            "Search results, embedding vectors, and entity-extraction outputs each have dedicated caches."
                        }
                    }

                    // Observability
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "Observability" }
                        ul { class: "text-xs text-gray-300 list-disc pl-4 space-y-0.5",
                            li {
                                "OpenTelemetry spans → Tempo. Toggle: " code { class: "text-green-300", "OTEL_TRACES_ENABLED" } " · endpoint " code { class: "text-green-300", "OTEL_EXPORTER_OTLP_ENDPOINT" } "."
                            }
                            li {
                                "Prometheus gauges + histograms in " code { class: "text-green-300", "monitoring/metrics.rs" }
                                " → Grafana. Tunable histogram buckets: " code { class: "text-green-300", "SEARCH_HISTO_BUCKETS" } " · " code { class: "text-green-300", "REINDEX_HISTO_BUCKETS" } "."
                            }
                            li {
                                "Structured logs (" code { class: "text-green-300", "RUST_LOG" } ", hot-reloaded) → Loki via Vector. UI tail at " code { class: "text-green-300", "/monitor/logs" } "."
                            }
                            li { code { class: "text-green-300", "/monitoring/health" } " + " code { class: "text-green-300", "/monitoring/ready" } " for Docker / k8s probes." }
                        }
                    }

                    // Rate limiting
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "Rate limiting" }
                        p { class: "text-xs text-gray-300",
                            "Token-bucket middleware in " code { class: "text-green-300", "monitoring/rate_limit_middleware.rs" }
                            " enforces per-endpoint budgets — separate for search and upload — and emits 429 with a structured body when a bucket runs out."
                        }
                        p { class: "text-xs text-gray-400",
                            "Knobs: " code { class: "text-green-300", "TRUST_PROXY" } " (honour " code { class: "text-green-300", "X-Forwarded-For" } " — only behind a trusted reverse proxy) · per-bucket capacity + refill rate. UI at " code { class: "text-green-300", "/monitor/rate-limits" } "."
                        }
                    }

                    // Multi-corpus
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "Multi-corpus isolation" }
                        p { class: "text-xs text-gray-300",
                            "Each named corpus has its own Tantivy index ("
                            code { class: "text-gray-400", "index/{{slug}}/" }
                            "), upload directory ("
                            code { class: "text-gray-400", "data/corpora/{{slug}}/documents/" }
                            "), vector store, and watched directory. The "
                            code { class: "text-green-300", "default" }
                            " corpus reuses the legacy paths for zero-migration."
                        }
                        p { class: "text-xs text-gray-300",
                            "Per-corpus settings (chunker mode, distance metric, HNSW params, watch dir, PQ subvectors, native-PDF override) layer on top of the global config — managed at "
                            code { class: "text-green-300", "/config/corpus" } "."
                        }
                    }
                }

                // ── Settings layering ─────────────────────────────────
                h2 { class: "text-sm font-bold text-white mb-1 mt-2", "Settings layering" }
                div { class: "grid grid-cols-3 gap-2 mb-3",
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "Precedence" }
                        p { class: "text-xs text-gray-300",
                            "Per-corpus override → runtime override → env var → hard-coded default."
                        }
                        p { class: "text-xs text-gray-400",
                            "Env vars live in " code { class: "text-green-300", "~/.config/ag/ag.env" } " (install-time). Runtime overrides land in " code { class: "text-green-300", "~/.local/share/ag/overrides.json" } " (UI-editable, written atomically; env file is never modified by ag)."
                        }
                    }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "Hot-reload vs restart" }
                        p { class: "text-xs text-gray-300",
                            "13 keys hot-reload via subscribers in " code { class: "text-green-300", "main.rs" }
                            ": " code { class: "text-green-300", "REDIS_*" } " · " code { class: "text-green-300", "RUST_LOG" }
                            " · " code { class: "text-green-300", "CHUNK_*" } " · " code { class: "text-green-300", "OTEL_*" }
                            " · " code { class: "text-green-300", "FILE_WATCHER_*" } " · " code { class: "text-green-300", "AUTO_EXPORT_ON_UPLOAD" }
                            ". The rest surface a banner that drives " code { class: "text-green-300", "/runtime/actions/restart-self" }
                            " (universal " code { class: "text-green-300", "execve" } "-based self-restart — works on bin, exe, systemd, or container)."
                        }
                    }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-white", "Boot-failure recovery" }
                        p { class: "text-xs text-gray-300",
                            "If a bad override prevents ag from reaching healthy, on the next start ag moves "
                            code { class: "text-green-300", "overrides.json" } " aside as "
                            code { class: "text-green-300", "overrides.json.bad-<ts>" }
                            " and boots with no overrides — so a UI-applied bad value can't permanently brick the install."
                        }
                    }
                }

                // ── Storage map ───────────────────────────────────────
                h2 { class: "text-sm font-bold text-white mb-1 mt-2", "Storage map" }
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 mb-3",
                    div { class: "font-mono text-xs text-gray-300 whitespace-pre leading-tight",
"~/.local/share/ag/                          ← AG_HOME (defaults to platform data dir)
├── data/
│   ├── documents.db                         ← SQLite: chunks, embeddings, metadata, corpora
│   ├── vectors.json                         ← legacy default-corpus vector dump
│   ├── overrides.json                       ← UI-editable runtime overrides
│   ├── canon_stats.json / chunking_stats.json / preprocess_stats.json
│   └── corpora/{{slug}}/documents/            ← per-corpus uploaded files
├── index/
│   ├── tantivy/                             ← default corpus Tantivy segments
│   └── {{slug}}/                              ← per-corpus Tantivy segments
├── db/                                      ← SQLite WAL/lock files
├── logs/
└── cache/

~/.cache/huggingface/hub/                    ← HF Hub auto-downloads (LAYOUT_ML_MODEL_ID etc.)
~/.config/ag/ag.env                          ← install-time env defaults (loaded by systemd EnvironmentFile=)"
                    }
                }

                // ── Failure modes ─────────────────────────────────────
                h2 { class: "text-sm font-bold text-white mb-1 mt-2", "Failure modes & graceful degradation" }
                div { class: "grid grid-cols-4 gap-2 mb-3",
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-yellow-300", "Embedding unavailable" }
                        p { class: "text-xs text-gray-300",
                            "ONNX session fails to build → embedder returns error; vector index path skipped, BM25 alone serves search. Surfaced on " code { class: "text-green-300", "/config/onnx" } " via the Layout-model / embedder chips."
                        }
                    }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-yellow-300", "Layout model missing" }
                        p { class: "text-xs text-gray-300",
                            "Tier 0 (HF Hub) → Tier 1 (local DETR) → Tier 2 (word-feature ONNX) → heuristic. Each fallthrough emits a warn and updates the active-tier chip; PDF ingestion never blocks."
                        }
                    }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-yellow-300", "FalkorDB offline" }
                        p { class: "text-xs text-gray-300",
                            "Graph building skipped, no entities written; retrieval falls back to vector + BM25 fusion without graph expansion. The petgraph runtime keeps serving its last loaded snapshot."
                        }
                    }
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 space-y-1",
                        h3 { class: "text-sm font-bold text-yellow-300", "Redis offline" }
                        p { class: "text-xs text-gray-300",
                            "L3 read attempts return None; misses cascade to L2/L1. No errors surfaced to the user — caching just degrades to per-process."
                        }
                    }
                }

                // ── Source map ────────────────────────────────────────
                h2 { class: "text-sm font-bold text-white mb-1 mt-2", "Source map" }
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2 mb-2",
                    div { class: "grid grid-cols-2 gap-x-6 gap-y-0.5 text-xs",
                        // Ingest
                        div { class: "text-gray-400 font-semibold col-span-2 mt-0.5", "Ingestion" }
                        div { code { class: "text-green-300", "api/upload_search.rs" } }
                        div { class: "text-gray-300", "multipart upload + search handlers" }
                        div { code { class: "text-green-300", "file_watcher.rs" } }
                        div { class: "text-gray-300", "notify-based per-corpus watchers + registry" }
                        div { code { class: "text-green-300", "mime_detect.rs" } }
                        div { class: "text-gray-300", "content-type dispatch" }
                        div { code { class: "text-green-300", "parser.rs" } " · " code { class: "text-green-300", "pdf/" } }
                        div { class: "text-gray-300", "text + PDF extractors (Native, Extractous, pdftotext)" }
                        // Chunk + embed
                        div { class: "text-gray-400 font-semibold col-span-2 mt-1", "Chunking + embedding" }
                        div { code { class: "text-green-300", "memory/chunker.rs" } " · " code { class: "text-green-300", "memory/chunker_factory.rs" } }
                        div { class: "text-gray-300", "5 modes + per-corpus config" }
                        div { code { class: "text-green-300", "embedder.rs" } " · " code { class: "text-green-300", "inference_gateway.rs" } }
                        div { class: "text-gray-300", "ONNX/FastEmbed; concurrency-limited inference" }
                        // Index
                        div { class: "text-gray-400 font-semibold col-span-2 mt-1", "Index + storage" }
                        div { code { class: "text-green-300", "index.rs" } }
                        div { class: "text-gray-300", "fan-out orchestrator" }
                        div { code { class: "text-green-300", "retriever.rs" } }
                        div { class: "text-gray-300", "Tantivy mgmt, HNSW/PQ builds, L1/L2 caches" }
                        div { code { class: "text-green-300", "db/" } }
                        div { class: "text-gray-300", "SQLite schema + per-corpus tables" }
                        // Graph
                        div { class: "text-gray-400 font-semibold col-span-2 mt-1", "Graph" }
                        div { code { class: "text-green-300", "tools/entity_extractor.rs" } }
                        div { class: "text-gray-300", "NER pipeline (ONNX)" }
                        div { code { class: "text-green-300", "graph/knowledge_builder.rs" } " · " code { class: "text-green-300", "graph/entity_reconciler.rs" } }
                        div { class: "text-gray-300", "FalkorDB writes + cross-document entity merging" }
                        div { code { class: "text-green-300", "graph/petgraph_runtime.rs" } " · " code { class: "text-green-300", "graph/graph_retriever.rs" } }
                        div { class: "text-gray-300", "in-memory query graph + retrieval expansion" }
                        // Agent
                        div { class: "text-gray-400 font-semibold col-span-2 mt-1", "Agent" }
                        div { code { class: "text-green-300", "agent.rs" } " · " code { class: "text-green-300", "agent_memory.rs" } }
                        div { class: "text-gray-300", "mode switching, prompt construction, memory tiers" }
                        div { code { class: "text-green-300", "rig_tools/" } }
                        div { class: "text-gray-300", "Rig tool implementations for Agentic mode" }
                        // Ops
                        div { class: "text-gray-400 font-semibold col-span-2 mt-1", "Operations" }
                        div { code { class: "text-green-300", "monitoring/" } }
                        div { class: "text-gray-300", "metrics, OTel, rate-limit middleware, histograms" }
                        div { code { class: "text-green-300", "settings/" } }
                        div { class: "text-gray-300", "registry + store + hot-reload subscribers" }
                        div { code { class: "text-green-300", "main.rs" } }
                        div { class: "text-gray-300", "boot phases, pre-warms, watcher startup, self-restart" }
                    }
                }

                // ── Footer ────────────────────────────────────────────
                div { class: "mt-2 pt-2 border-t border-gray-700 flex items-center gap-2",
                    Link { to: Route::DocuIndex {}, class: "btn btn-primary btn-xs", "← Back to Index" }
                    span { class: "text-xs text-gray-400", "AG_HOME defaults to " code { class: "text-gray-300", "~/.local/share/ag/" } " on Linux; override via the " code { class: "text-gray-300", "AG_HOME" } " env var to relocate the entire data directory." }
                }
            }
        }
    }
}
