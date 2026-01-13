use crate::api;
use crate::api::{ManualObservationMetric, ManualObservationSummary};
use crate::app::Route;
use crate::components::monitor::*;
use dioxus::prelude::*;

#[derive(Clone, Default)]
struct ObservationsState {
    loading: bool,
    error: Option<String>,
    metrics: Vec<ManualObservationMetric>,
    observations: Vec<ManualObservationSummary>,
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
            });

            let metrics_result = api::fetch_manual_observation_metrics().await;
            let observations_result = api::fetch_recent_observations(50).await;

            match (metrics_result, observations_result) {
                (Ok(m), Ok(o)) => {
                    state.set(ObservationsState {
                        loading: false,
                        error: None,
                        metrics: m.metrics,
                        observations: o.observations,
                    });
                }
                (Err(e), _) | (_, Err(e)) => {
                    state.set(ObservationsState {
                        loading: false,
                        error: Some(e),
                        metrics: vec![],
                        observations: vec![],
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
                    BreadcrumbItem::new("Agent", None::<Route>),
                ],
            }

            NavTabs { active: Route::MonitorObservations {} }

            // Page header
            Panel { title: Some("Agent Work Log".into()), refresh: None::<String>,
                div { class: "text-sm text-gray-300 space-y-2",
                    p { "The Agent is an autonomous system that reasons, plans, executes tools, and logs its work history. This page shows the agent's decisions, actions, and discoveries." }
                    div { class: "flex flex-wrap gap-4 mt-3 text-xs",
                        div { class: "flex items-center gap-2",
                            span { class: "w-2 h-2 rounded-full bg-purple-500" }
                            span { class: "text-gray-400", "Has autonomy — decides what to do" }
                        }
                        div { class: "flex items-center gap-2",
                            span { class: "w-2 h-2 rounded-full bg-purple-500" }
                            span { class: "text-gray-400", "Stateful — remembers past actions" }
                        }
                        div { class: "flex items-center gap-2",
                            span { class: "w-2 h-2 rounded-full bg-purple-500" }
                            span { class: "text-gray-400", "Action-centric — tools, decisions, outcomes" }
                        }
                    }
                }
            }

            if snapshot.loading {
                div { class: "text-gray-400 text-sm", "Loading…" }
            } else if let Some(err) = snapshot.error.clone() {
                div { class: "text-red-400 text-sm", "Failed to load: {err}" }
            } else {
                // Stats row
                div { class: "grid grid-cols-1 md:grid-cols-4 gap-4",
                    StatCard {
                        title: "Total Observations".into(),
                        value: snapshot.observations.len().to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Bugfixes".into(),
                        value: snapshot.observations.iter().filter(|o| o.entry_type == "bugfix").count().to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Features".into(),
                        value: snapshot.observations.iter().filter(|o| o.entry_type == "feature").count().to_string().into(),
                        unit: None,
                    }
                    StatCard {
                        title: "Decisions".into(),
                        value: snapshot.observations.iter().filter(|o| o.entry_type == "decision").count().to_string().into(),
                        unit: None,
                    }
                }

                // Main table
                Panel { title: Some("Work History".into()), refresh: None::<String>,
                    if snapshot.observations.is_empty() {
                        div { class: "text-gray-400 text-sm py-8 text-center",
                            div { class: "mb-2", "No agent observations yet." }
                            div { class: "text-xs",
                                "Use "
                                span { class: "font-mono bg-gray-800 px-1 rounded", "POST /memory/observations" }
                                " to log agent work."
                            }
                        }
                    } else {
                        table { class: "w-full text-sm text-left",
                            thead { class: "text-gray-400 uppercase tracking-wide border-b border-gray-800",
                                tr {
                                    th { class: "py-2", "Action Type" }
                                    th { class: "py-2", "Description" }
                                    th { class: "py-2", "Timestamp" }
                                }
                            }
                            tbody {
                                for obs in snapshot.observations.iter() {
                                    tr { class: "border-b border-gray-800 last:border-0 hover:bg-gray-800/50",
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

                // API Metrics
                Panel { title: Some("Agent API Metrics".into()), refresh: None::<String>,
                    if snapshot.metrics.is_empty() {
                        div { class: "text-gray-400 text-sm", "No API metrics yet. Hit the agent endpoints to populate this view." }
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

                // Observation types info
                Panel { title: Some("Observation Types".into()), refresh: None::<String>,
                    div { class: "grid grid-cols-2 md:grid-cols-4 gap-4 text-sm",
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-purple-300 font-semibold mb-1", "bugfix" }
                            div { class: "text-gray-400 text-xs", "Bug resolutions and fixes" }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-purple-300 font-semibold mb-1", "feature" }
                            div { class: "text-gray-400 text-xs", "New features implemented" }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-purple-300 font-semibold mb-1", "decision" }
                            div { class: "text-gray-400 text-xs", "Architectural choices made" }
                        }
                        div { class: "bg-gray-800/50 rounded p-3",
                            div { class: "text-purple-300 font-semibold mb-1", "discovery" }
                            div { class: "text-gray-400 text-xs", "Learnings and insights" }
                        }
                    }
                }
            }
        }
    }
}
