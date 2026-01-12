use crate::api;
use crate::api::{ManualObservationMetric, ManualObservationSummary, RagMemoryItem};
use crate::app::Route;
use crate::components::monitor::*;
use dioxus::prelude::*;

#[derive(Clone, Default)]
struct ObservationsState {
    loading: bool,
    error: Option<String>,
    metrics: Vec<ManualObservationMetric>,
    observations: Vec<ManualObservationSummary>,
    rag_memories: Vec<RagMemoryItem>,
}

#[component]
pub fn MonitorObservations() -> Element {
    let state = use_signal(ObservationsState::default);

    {
        let mut state = state.clone();
        use_future(move || async move {
            state.set(ObservationsState {
                loading: true,
                error: None,
                metrics: vec![],
                observations: vec![],
                rag_memories: vec![],
            });

            let metrics_result = api::fetch_manual_observation_metrics().await;
            let observations_result = api::fetch_recent_observations(20).await;
            let rag_result = api::fetch_rag_memories(20).await;

            match (metrics_result, observations_result, rag_result) {
                (Ok(m), Ok(o), Ok(r)) => {
                    state.set(ObservationsState {
                        loading: false,
                        error: None,
                        metrics: m.metrics,
                        observations: o.observations,
                        rag_memories: r.memories,
                    });
                }
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                    state.set(ObservationsState {
                        loading: false,
                        error: Some(e),
                        metrics: vec![],
                        observations: vec![],
                        rag_memories: vec![],
                    });
                }
            }
        });
    }

    let snapshot = state.read().clone();

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Observations", None::<Route>),
                ],
            }

            NavTabs { active: Route::MonitorObservations {} }

            if snapshot.loading {
                div { class: "text-gray-400 text-sm", "Loading…" }
            } else if let Some(err) = snapshot.error.clone() {
                div { class: "text-red-400 text-sm", "Failed to load: {err}" }
            } else {
                // RAG Memories Section (LLM Context)
                Panel { title: Some("RAG Memories (LLM Context)".into()), refresh: None::<String>,
                    if snapshot.rag_memories.is_empty() {
                        div { class: "text-gray-400 text-sm",
                            "No RAG memories stored yet. Use "
                            span { class: "font-mono bg-gray-800 px-1 rounded", "POST /memory/store_rag" }
                            " to add memories for LLM context."
                        }
                    } else {
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800",
                                tr {
                                    th { class: "py-2", "Type" }
                                    th { class: "py-2", "Content" }
                                    th { class: "py-2", "Agent" }
                                    th { class: "py-2", "Timestamp" }
                                }
                            }
                            tbody {
                                for mem in snapshot.rag_memories.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0",
                                        td { class: "py-2",
                                            span { class: "px-2 py-0.5 rounded text-xs bg-blue-900 text-blue-200", "{mem.memory_type}" }
                                        }
                                        td { class: "py-2 text-white max-w-md truncate", "{mem.content}" }
                                        td { class: "py-2 text-gray-400 text-xs", "{mem.agent_id}" }
                                        td { class: "py-2 text-gray-400 text-xs", "{mem.timestamp}" }
                                    }
                                }
                            }
                        }
                    }
                }

                // Manual Observations Section (Structured Work History)
                Panel { title: Some("Manual Observations (Work History)".into()), refresh: None::<String>,
                    if snapshot.observations.is_empty() {
                        div { class: "text-gray-400 text-sm",
                            "No observations stored yet. Use "
                            span { class: "font-mono bg-gray-800 px-1 rounded", "POST /memory/observations" }
                            " to create structured observations."
                        }
                    } else {
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800",
                                tr {
                                    th { class: "py-2", "Type" }
                                    th { class: "py-2", "Title" }
                                    th { class: "py-2", "Created" }
                                }
                            }
                            tbody {
                                for obs in snapshot.observations.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0",
                                        td { class: "py-2",
                                            span { class: "px-2 py-0.5 rounded text-xs bg-purple-900 text-purple-200", "{obs.entry_type}" }
                                        }
                                        td { class: "py-2 text-white", "{obs.title}" }
                                        td { class: "py-2 text-gray-400 text-xs", "{obs.created_at}" }
                                    }
                                }
                            }
                        }
                    }
                }

                // Endpoint Metrics Section
                Panel { title: Some("Endpoint Metrics".into()), refresh: None::<String>,
                    if snapshot.metrics.is_empty() {
                        div { class: "text-gray-400 text-sm", "No endpoint metrics yet. Hit the memory APIs to populate this view." }
                    } else {
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800",
                                tr {
                                    th { class: "py-2", "Endpoint" }
                                    th { class: "py-2", "Success" }
                                    th { class: "py-2", "Errors" }
                                    th { class: "py-2", "p50 (ms)" }
                                    th { class: "py-2", "p90 (ms)" }
                                }
                            }
                            tbody {
                                for metric in snapshot.metrics.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0",
                                        td { class: "py-2 text-white", "{metric.endpoint}" }
                                        td { class: "py-2 text-green-400", "{metric.ok}" }
                                        td { class: "py-2 text-red-400", "{metric.err}" }
                                        td { class: "py-2", "{metric.latency_p50:.1}" }
                                        td { class: "py-2", "{metric.latency_p90:.1}" }
                                    }
                                }
                            }
                        }
                    }
                }

                // Info Section
                Panel { title: Some("Memory Types".into()), refresh: None::<String>,
                    div { class: "grid grid-cols-1 md:grid-cols-2 gap-4 text-sm",
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-blue-300 font-semibold mb-2", "RAG Memories" }
                            div { class: "text-gray-400 mb-2", "Simple key-value memories for LLM context retrieval." }
                            ul { class: "text-gray-400 list-disc pl-4 space-y-1",
                                li { "conversation - Past exchanges" }
                                li { "note - User-added notes" }
                                li { "fact - Factual information" }
                                li { "preference - User preferences" }
                            }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-purple-300 font-semibold mb-2", "Manual Observations" }
                            div { class: "text-gray-400 mb-2", "Structured work history with rich metadata." }
                            ul { class: "text-gray-400 list-disc pl-4 space-y-1",
                                li { "bugfix - Bug resolutions" }
                                li { "feature - New features" }
                                li { "decision - Architectural choices" }
                                li { "discovery - Learnings" }
                            }
                        }
                    }
                }
            }
        }
    }
}
