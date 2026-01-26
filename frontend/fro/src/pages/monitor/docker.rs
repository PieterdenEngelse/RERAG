//! Docker Container Monitoring Page
//! Shows status of Docker containers used by the ag infrastructure

use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

/// Docker monitoring state
#[derive(Clone, Default)]
struct DockerState {
    loading: bool,
    error: Option<String>,
    containers: Vec<api::DockerContainer>,
    stats: Vec<api::DockerStats>,
    docker_available: bool,
}

// Styling constants matching other monitor pages
const BOARD_CLASS: &str = "rounded border border-gray-600 p-4 bg-gray-800/50";
const LABEL_CLASS: &str = "text-gray-400 text-xs";

#[component]
pub fn MonitorDocker() -> Element {
    let mut state = use_signal(|| DockerState {
        loading: true,
        ..Default::default()
    });
    let mut page_errors = use_context::<Signal<PageErrors>>();
    let mut show_help = use_signal(|| false);

    // Fetch Docker status periodically
    use_future(move || async move {
        loop {
            match api::fetch_docker_status().await {
                Ok(resp) => {
                    state.set(DockerState {
                        loading: false,
                        error: None,
                        containers: resp.containers,
                        stats: resp.stats,
                        docker_available: resp.docker_available,
                    });
                    page_errors.with_mut(|e| e.clear_error("docker"));
                }
                Err(err) => {
                    state.with_mut(|s| {
                        s.loading = false;
                        s.error = Some(err.clone());
                    });
                    page_errors.with_mut(|errs| errs.set_error("docker", &err));
                }
            }
            TimeoutFuture::new(5_000).await; // Refresh every 5 seconds
        }
    });

    let snapshot = state.read().clone();

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Docker", None),
                ],
            }

            NavTabs { active: Route::MonitorDocker {} }

            RowHeader {
                title: "Docker Infrastructure".into(),
                description: Some("Monitor Docker containers running ag services".into()),
            }

            // Help modal
            if show_help() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                    onclick: move |_| show_help.set(false),
                    div {
                        class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-lg max-h-[90vh] overflow-y-auto shadow-xl",
                        onclick: move |evt| evt.stop_propagation(),
                        div { class: "flex items-center justify-between mb-3",
                            h2 { class: "text-base font-semibold text-gray-100", "Docker Monitoring" }
                            button {
                                class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                onclick: move |_| show_help.set(false),
                                "×"
                            }
                        }
                        div { class: "text-sm text-gray-300 space-y-2",
                            p { "This page monitors Docker containers that provide infrastructure services for ag:" }
                            ul { class: "list-disc ml-5 space-y-1",
                                li { strong { "Neo4j" } " - Knowledge graph database for GraphRAG" }
                                li { strong { "Redis" } " - L3 cache for fast query responses" }
                                li { strong { "Prometheus" } " - Metrics collection and storage" }
                                li { strong { "Grafana" } " - Dashboards and visualization" }
                                li { strong { "Loki" } " - Log aggregation" }
                                li { strong { "Tempo" } " - Distributed tracing" }
                                li { strong { "OTel Collector" } " - Telemetry pipeline" }
                            }
                            p { class: "mt-3", "Container states:" }
                            ul { class: "list-disc ml-5 space-y-1",
                                li { span { class: "text-green-400", "running" } " - Container is active and healthy" }
                                li { span { class: "text-yellow-400", "restarting" } " - Container is restarting" }
                                li { span { class: "text-red-400", "exited" } " - Container has stopped" }
                            }
                        }
                    }
                }
            }

            // Main content
            if snapshot.loading {
                Panel { title: Some("Loading...".into()),
                    div { class: "flex items-center justify-center py-8",
                        span { class: "loading loading-spinner loading-lg text-primary" }
                    }
                }
            } else if !snapshot.docker_available {
                Panel { title: Some("Docker Not Available".into()),
                    div { class: "text-center py-8",
                        div { class: "text-6xl mb-4", "🐳" }
                        p { class: "text-gray-300 mb-2", "Docker is not available or not accessible." }
                        p { class: "text-gray-400 text-sm mb-4", "This could be because Docker isn't running, or the backend doesn't have permission to access Docker." }

                        div { class: "text-left max-w-lg mx-auto",
                            h4 { class: "text-gray-200 font-semibold mb-2", "Option 1: Add user to docker group" }
                            pre { class: "bg-gray-900 p-3 rounded text-xs text-gray-300 mb-4",
                                "# Add user to docker group\n"
                                "sudo usermod -aG docker $USER\n\n"
                                "# Then log out and back in, or run:\n"
                                "newgrp docker"
                            }

                            h4 { class: "text-gray-200 font-semibold mb-2", "Option 2: Check Docker status" }
                            pre { class: "bg-gray-900 p-3 rounded text-xs text-gray-300",
                                "# Check Docker status\n"
                                "sudo systemctl status docker\n\n"
                                "# Start Docker if not running\n"
                                "sudo systemctl start docker"
                            }
                        }
                    }
                }
            } else if let Some(err) = &snapshot.error {
                Panel { title: Some("Error".into()),
                    div { class: "text-red-400 text-sm", "Failed to fetch Docker status: {err}" }
                }
            } else {
                // Container Status Grid
                Panel {
                    title: Some("Container Status".into()),
                    refresh: Some("5s".into()),

                    if snapshot.containers.is_empty() {
                        div { class: "text-gray-400 text-sm py-4 text-center",
                            "No ag containers found. Start them with: "
                            code { class: "bg-gray-900 px-2 py-1 rounded", "docker compose up -d" }
                        }
                    } else {
                        div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4",
                            for container in &snapshot.containers {
                                ContainerCard { container: container.clone() }
                            }
                        }
                    }
                }

                // Resource Usage
                if !snapshot.stats.is_empty() {
                    Panel { title: Some("Resource Usage".into()), refresh: Some("5s".into()),
                        div { class: "overflow-x-auto",
                            table { class: "w-full text-sm",
                                thead {
                                    tr { class: "text-gray-400 text-left border-b border-gray-700",
                                        th { class: "pb-2 pr-4", "Container" }
                                        th { class: "pb-2 pr-4", "CPU %" }
                                        th { class: "pb-2 pr-4", "Memory" }
                                        th { class: "pb-2 pr-4", "Mem %" }
                                        th { class: "pb-2 pr-4", "Net RX" }
                                        th { class: "pb-2", "Net TX" }
                                    }
                                }
                                tbody {
                                    for stat in &snapshot.stats {
                                        tr { class: "border-b border-gray-800 hover:bg-gray-800/50",
                                            td { class: "py-2 pr-4 font-mono text-gray-200", "{stat.name}" }
                                            td { class: "py-2 pr-4",
                                                span {
                                                    class: if stat.cpu_percent > 80.0 { "text-red-400" }
                                                           else if stat.cpu_percent > 50.0 { "text-yellow-400" }
                                                           else { "text-green-400" },
                                                    "{stat.cpu_percent:.1}%"
                                                }
                                            }
                                            td { class: "py-2 pr-4 font-mono text-gray-300", "{stat.memory_usage} / {stat.memory_limit}" }
                                            td { class: "py-2 pr-4",
                                                span {
                                                    class: if stat.memory_percent > 80.0 { "text-red-400" }
                                                           else if stat.memory_percent > 50.0 { "text-yellow-400" }
                                                           else { "text-green-400" },
                                                    "{stat.memory_percent:.1}%"
                                                }
                                            }
                                            td { class: "py-2 pr-4 font-mono text-gray-300", "{stat.network_rx}" }
                                            td { class: "py-2 font-mono text-gray-300", "{stat.network_tx}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Quick Actions
                Panel { title: Some("Quick Actions".into()),
                    div { class: "flex flex-wrap gap-3",
                        ActionButton {
                            label: "Refresh",
                            icon: "🔄",
                            command: "docker compose ps",
                        }
                        ActionButton {
                            label: "View Logs",
                            icon: "📋",
                            command: "docker compose logs -f --tail=100",
                        }
                        ActionButton {
                            label: "Restart All",
                            icon: "🔁",
                            command: "docker compose restart",
                        }
                        ActionButton {
                            label: "Stop All",
                            icon: "⏹️",
                            command: "docker compose down",
                        }
                        ActionButton {
                            label: "Start All",
                            icon: "▶️",
                            command: "docker compose up -d",
                        }
                    }
                }

                // Service URLs
                Panel { title: Some("Service URLs".into()),
                    div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 text-sm",
                        ServiceLink { name: "Neo4j Browser", url: "http://localhost:7474", port: "7474" }
                        ServiceLink { name: "Grafana", url: "http://localhost:3001", port: "3001" }
                        ServiceLink { name: "Prometheus", url: "http://localhost:9090", port: "9090" }
                        ServiceLink { name: "Loki", url: "http://localhost:3100", port: "3100" }
                        ServiceLink { name: "Tempo", url: "http://localhost:3200", port: "3200" }
                    }
                }
            }
        }
    }
}

/// Container status card component
#[component]
fn ContainerCard(container: api::DockerContainer) -> Element {
    let state_color = match container.state.as_str() {
        "running" => "text-green-400",
        "restarting" => "text-yellow-400",
        "exited" | "dead" => "text-red-400",
        _ => "text-gray-400",
    };

    let health_badge = match container.health.as_deref() {
        Some("healthy") => Some(("✓", "bg-green-500/20 text-green-400")),
        Some("unhealthy") => Some(("✗", "bg-red-500/20 text-red-400")),
        Some("starting") => Some(("…", "bg-yellow-500/20 text-yellow-400")),
        _ => None,
    };

    // Extract service name from container name (remove "ag-" prefix)
    let display_name = container
        .name
        .strip_prefix("ag-")
        .unwrap_or(&container.name);

    rsx! {
        div { class: BOARD_CLASS,
            div { class: "flex items-start justify-between mb-2",
                div {
                    h3 { class: "text-gray-200 font-semibold text-sm", "{display_name}" }
                    p { class: "text-gray-500 text-xs truncate max-w-[150px]", title: "{container.image}", "{container.image}" }
                }
                if let Some((icon, class)) = health_badge {
                    span { class: "px-2 py-0.5 rounded text-xs {class}", "{icon}" }
                }
            }

            div { class: "space-y-1",
                div { class: "flex justify-between",
                    span { class: LABEL_CLASS, "State" }
                    span { class: "{state_color} text-xs font-medium", "{container.state}" }
                }
                if !container.ports.is_empty() {
                    div { class: "flex justify-between",
                        span { class: LABEL_CLASS, "Ports" }
                        span { class: "text-gray-300 text-xs font-mono",
                            {container.ports.join(", ")}
                        }
                    }
                }
                div { class: "flex justify-between",
                    span { class: LABEL_CLASS, "Status" }
                    span { class: "text-gray-400 text-xs truncate max-w-[120px]", title: "{container.status}", "{container.status}" }
                }
            }
        }
    }
}

/// Action button component
#[component]
fn ActionButton(label: &'static str, icon: &'static str, command: &'static str) -> Element {
    rsx! {
        div { class: "group relative",
            button {
                class: "flex items-center gap-2 px-3 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm text-gray-200 transition-colors",
                title: command,
                span { "{icon}" }
                span { "{label}" }
            }
            // Tooltip with command
            div {
                class: "absolute bottom-full left-0 mb-2 px-2 py-1 bg-gray-900 text-xs text-gray-300 rounded opacity-0 group-hover:opacity-100 transition-opacity whitespace-nowrap font-mono",
                "{command}"
            }
        }
    }
}

/// Service link component
#[component]
fn ServiceLink(name: &'static str, url: &'static str, port: &'static str) -> Element {
    rsx! {
        a {
            href: url,
            target: "_blank",
            rel: "noopener noreferrer",
            class: "flex items-center justify-between p-3 bg-gray-700/50 hover:bg-gray-700 rounded transition-colors",
            div {
                span { class: "text-gray-200", "{name}" }
                span { class: "text-gray-500 text-xs ml-2", ":{port}" }
            }
            span { class: "text-gray-400", "↗" }
        }
    }
}
