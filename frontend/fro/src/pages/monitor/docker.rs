//! Docker Container Monitoring Page
//! Shows status of Docker containers used by the ag infrastructure

use crate::{
    api,
    app::{PageErrors, Route},
    components::monitor::*,
    pages::hardware::constants::{
        INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
    },
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
const BOARD_CLASS: &str = "relative rounded border border-gray-600 p-4 pb-12 bg-gray-800/50";
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
                // Container Status Grid with Quick Actions
                Panel {
                    title: Some("Container Status".into()),
                    refresh: Some("5s".into()),

                    if snapshot.containers.is_empty() {
                        div { class: "text-gray-400 text-sm py-4 text-center",
                            "No ag containers found. Start them with: "
                            code { class: "bg-gray-900 px-2 py-1 rounded", "docker compose up -d" }
                        }
                    } else {
                        {
                            let mut containers = snapshot.containers.clone();
                            // Swap Neo4j and OTel container cards if both exist
                            let neo_idx = containers.iter().position(|c| c.name == "ag-neo4j");
                            let otel_idx = containers.iter().position(|c| c.name == "ag-otel");
                            if let (Some(i), Some(j)) = (neo_idx, otel_idx) {
                                containers.swap(i, j);
                            }

                            rsx! {
                                div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4",
                                    for (idx, container) in containers.iter().enumerate() {
                                        ContainerCard { container: container.clone(), grid_index: idx }
                                    }
                            // Quick Actions card - appears as last item in grid
                            div { class: "bg-gray-800/50 rounded-lg border border-gray-700 p-4",
                                h3 { class: "text-sm font-semibold text-gray-300 mb-3", "Quick Actions" }
                                div { class: "flex flex-col gap-2",
                                    ActionButton {
                                        label: "Restart All",
                                        icon: "🔁",
                                        action: "restart",
                                    }
                                    ActionButton {
                                        label: "Stop All",
                                        icon: "⏹️",
                                        action: "down",
                                    }
                                    ActionButton {
                                        label: "Start All",
                                        icon: "▶️",
                                        action: "up",
                                    }
                                }
                            }
                        }
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
fn ContainerCard(container: api::DockerContainer, grid_index: usize) -> Element {
    // State display - rust color for active, red for inactive
    let (state_text, state_style) = match container.state.as_str() {
        "running" => ("active process", "color: #7C2A02;"), // Rust color
        "restarting" => ("restarting", "color: #facc15;"),  // yellow
        "exited" | "dead" => ("no active process", "color: #ef4444;"), // red
        _ => ("unknown", "color: #9ca3af;"),                // gray
    };

    // Health badge - shows 'healthy' or 'not healthy' text
    let health_badge = match container.health.as_deref() {
        Some("healthy") => Some(("healthy", "bg-green-500/20 text-green-400")),
        Some("unhealthy") => Some(("not healthy", "bg-red-500/20 text-red-400")),
        Some("starting") => Some(("starting", "bg-yellow-500/20 text-yellow-400")),
        _ => None, // No health check configured
    };

    // Extract service name from container name (remove "ag-" prefix)
    let display_name = container
        .name
        .strip_prefix("ag-")
        .unwrap_or(&container.name);

    // Extract just the version from image (e.g., "grafana/grafana:10.4.2" -> "v10.4.2")
    let version = container
        .image
        .split(':')
        .last()
        .map(|v| {
            if v.starts_with('v') {
                v.to_string()
            } else {
                format!("v{}", v)
            }
        })
        .unwrap_or_else(|| "latest".to_string());

    // Extract ports for display.
    // We prefer host/external ports when present ("0.0.0.0:7474->7474/tcp"),
    // but fall back to container port specs ("7474/tcp") so ports remain visible even
    // when the container is stopped and there are no published mappings.
    let mut port_set = std::collections::BTreeSet::new();
    for p in &container.ports {
        if let Some(arrow) = p.find("->") {
            // host mapping: "...:7474->7474/tcp"
            let before_arrow = &p[..arrow];
            if let Some(colon) = before_arrow.rfind(':') {
                let port = &before_arrow[colon + 1..];
                if let Ok(num) = port.parse::<u16>() {
                    port_set.insert(num.to_string());
                }
            }
        } else {
            // container-only: "7474/tcp"
            let port_part = p.split('/').next().unwrap_or("");
            if !port_part.is_empty() {
                port_set.insert(port_part.to_string());
            }
        }
    }
    let mut ports_simple: Vec<String> = port_set.into_iter().collect();
    let mut ports_detail = container.ports.join("\n");
    let mut has_ports = !container.ports.is_empty();

    // When the container is stopped, docker sometimes reports an empty ports list.
    // For Neo4j we still want to show the well-known ports for operator clarity.
    if container.name.eq_ignore_ascii_case("ag-neo4j") || display_name.eq_ignore_ascii_case("neo4j")
    {
        if !has_ports {
            ports_simple = vec!["7474".into(), "7687".into()];
            ports_detail = "7474/tcp\n7687/tcp".to_string();
            has_ports = true;
        }
    }

    rsx! {
        div { class: BOARD_CLASS,
            div { class: "relative mb-2",
                // Header row (title left, badge right, Neo4j note centered on same top line)
                div { class: "relative",
                    h3 { class: "text-gray-200 font-semibold text-sm", "{display_name}" }

                    if display_name.eq_ignore_ascii_case("neo4j") {
                        p {
                            class: "text-cyan-400 text-xs text-center absolute top-0 left-1/2 -translate-x-1/2 w-full",
                            "Only for ingestion"
                        }
                    }

                    if let Some((icon, class)) = health_badge {
                        span { class: "absolute top-0 right-0 px-2 py-0.5 rounded text-xs {class}", "{icon}" }
                    }
                }

                // Neo4j quick link (positioned so it doesn't affect card layout)
                if display_name.eq_ignore_ascii_case("neo4j") {
                    a {
                        href: "/config/neo4j",
                        class: "btn btn-sm absolute left-1/2 -translate-x-1/2 z-10",
                        // ~1.5cm + 5mm lower than the top of the card (~76px at 96dpi)
                        style: "top: 76px; background-color: #1D6B9A; border-color: #1D6B9A; color: white;",
                        "Config"
                    }
                }

                p { class: "text-gray-500 text-xs",
                    "Docker image: "
                    span { "{version}" }
                }
            }

            // Start/Stop button (container-scoped)
            {
                let is_running = container.state == "running";
                let action_label = if is_running { "Stop" } else { "Start" };
                let action = if is_running { "stop" } else { "start" };
                let container_name = container.name.clone();

                let bottom_px = if grid_index < 4 {
                    // First visual row (4 cards)
                    64
                } else if grid_index < 7 {
                    // Second visual row (next 3 cards)
                    120
                } else {
                    // Remaining cards
                    92
                };

                rsx! {
                    div {
                        class: "absolute left-1/2 -translate-x-1/2 z-20",
                        style: "bottom: {bottom_px}px;",
                        button {
                            class: "btn btn-sm",
                            style: "background-color: #1D6B9A; border-color: #1D6B9A; color: white;",
                        onclick: move |_| {
                            let container_name = container_name.clone();
                            let action = action.to_string();
                            spawn(async move {
                                let _ = api::docker_action(&action, Some(&container_name)).await;
                            });
                        },
                            "{action_label}"
                        }
                    }
                }
            }

            div { class: "space-y-1",
                div { class: "flex justify-between",
                    span { class: LABEL_CLASS, "State:" }
                    span { class: "text-xs font-medium", style: "{state_style}", "{state_text}" }
                }
                if has_ports {
                    PortsRow { ports_simple: ports_simple.clone(), ports_detail: ports_detail.clone() }
                }
                div { class: "flex justify-between gap-2",
                    span { class: LABEL_CLASS, "Status:" }
                    span {
                        class: "text-gray-400 text-xs text-right whitespace-normal break-words",
                        title: "{container.status}",
                        "{container.status}"
                    }
                }
            }
        }
    }
}

/// Ports row with simple display and info button for details
#[component]
fn PortsRow(ports_simple: Vec<String>, ports_detail: String) -> Element {
    let mut show_detail = use_signal(|| false);

    rsx! {
        div { class: "flex justify-between items-center",
            // Label with info button
            div { class: "flex items-center gap-1",
                span { class: LABEL_CLASS, "Ports:" }
                // Small info button after "Ports"
                button {
                    class: PARAM_ICON_BUTTON_CLASS,
                    style: PARAM_ICON_BUTTON_STYLE,
                    onclick: move |_| show_detail.set(!show_detail()),
                    title: "Show port details",
                    svg {
                        class: INFO_ICON_SVG_CLASS,
                        view_box: "0 0 20 20",
                        fill: "none",
                        stroke: "currentColor",
                        circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                        line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                    }
                }
            }
            // Port numbers
            span { class: "text-gray-300 text-xs font-mono",
                {ports_simple.join(", ")}
            }
        }
        // Detail popup
        if show_detail() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_detail.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-4 max-w-md shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-3",
                        h3 { class: "text-sm font-semibold text-gray-100", "Port Mappings" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-lg font-bold",
                            onclick: move |_| show_detail.set(false),
                            "×"
                        }
                    }
                    pre { class: "bg-gray-900 text-green-400 p-3 rounded font-mono text-xs whitespace-pre-wrap",
                        "{ports_detail}"
                    }
                    p { class: "text-xs text-gray-500 mt-2", "Format: host:port->container:port/protocol" }
                }
            }
        }
    }
}

/// Action button component - executes docker actions with info button
#[component]
fn ActionButton(label: &'static str, icon: &'static str, action: &'static str) -> Element {
    let mut status = use_signal(|| "idle".to_string()); // idle, loading, success, error
    let mut error_msg = use_signal(|| None::<String>);
    let mut show_info = use_signal(|| false);

    let command = match action {
        "restart" => "docker compose restart",
        "down" => "docker compose down",
        "up" => "docker compose up -d",
        "stop" => "docker compose stop",
        "start" => "docker compose start",
        _ => action,
    };

    let execute_action = move |_| {
        let action_str = action.to_string();
        status.set("loading".to_string());
        error_msg.set(None);

        spawn(async move {
            match api::docker_action(&action_str, None).await {
                Ok(resp) => {
                    if resp.success.unwrap_or(false) {
                        status.set("success".to_string());
                    } else {
                        status.set("error".to_string());
                        error_msg.set(resp.stderr.or(resp.error));
                    }
                }
                Err(e) => {
                    status.set("error".to_string());
                    error_msg.set(Some(e));
                }
            }
            // Reset after 3 seconds
            TimeoutFuture::new(3000).await;
            status.set("idle".to_string());
        });
    };

    let btn_class = match status().as_str() {
        "loading" => "flex-1 flex items-center gap-2 px-3 py-2 bg-yellow-600 rounded-l text-sm text-white transition-colors cursor-wait",
        "success" => "flex-1 flex items-center gap-2 px-3 py-2 bg-green-600 rounded-l text-sm text-white transition-colors",
        "error" => "flex-1 flex items-center gap-2 px-3 py-2 bg-red-600 rounded-l text-sm text-white transition-colors",
        _ => "flex-1 flex items-center gap-2 px-3 py-2 bg-gray-700 hover:bg-gray-600 rounded-l text-sm text-gray-200 transition-colors",
    };

    let display_label = match status().as_str() {
        "loading" => "Running...",
        "success" => "Done!",
        "error" => "Failed",
        _ => label,
    };

    rsx! {
        div { class: "flex w-full",
            // Main action button
            button {
                class: btn_class,
                onclick: execute_action,
                disabled: status() == "loading",
                title: "Execute: {command}",
                span { class: "w-6 text-center", "{icon}" }
                span { "{display_label}" }
            }
            // Info button - explicit square size matching action button height
            // Default: 24px button with 20px icon (83.3% ratio)
            // Here: 36px button, so icon = 36 * 0.833 = 30px
            button {
                class: "shrink-0 rounded-r flex items-center justify-center cursor-pointer hover:opacity-80",
                style: "background-color: #7C2A02; border: 1px solid #7C2A02; width: 36px; height: 36px;",
                onclick: move |_| show_info.set(!show_info()),
                title: "Show command info",
                svg {
                    style: "width: 30px; height: 30px; color: #cccccc;", // 80% white
                    view_box: "0 0 20 20",
                    fill: "none",
                    stroke: "currentColor",
                    circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                    line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                    circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                }
            }
        }

        // Error tooltip
        if let Some(err) = error_msg() {
            div {
                class: "absolute top-full left-0 mt-1 p-2 bg-red-900 text-red-200 text-xs rounded max-w-xs z-10",
                "{err}"
            }
        }

        // Info modal
        if show_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 max-w-md shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "{icon} {label}" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_info.set(false),
                            "×"
                        }
                    }
                    div { class: "mb-4",
                        p { class: "text-sm text-gray-400 mb-2", "This button executes:" }
                        code { class: "block bg-gray-900 text-green-400 p-3 rounded font-mono text-sm", "{command}" }
                    }
                    p { class: "text-xs text-gray-500", "Runs in the ag project directory" }
                }
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
