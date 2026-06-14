use crate::api;
use crate::app::RuntimeSuspended;
use crate::app::{BoardsHidden, ClearChat, PageErrors, Route, ShowHelpCommands, ShowRagInfo};
use crate::components::nav_dropdown::{DropdownActionItem, DropdownItem, NavDropdown};
use crate::pages::hardware::constants::INFO_ICON_SVG_CLASS;
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

#[component]
pub fn Header() -> Element {
    let mut menu_open = use_signal(|| false);
    let mut health_status = use_signal(|| "checking".to_string());
    let mut show_status_info = use_signal(|| false);
    let mut show_initial_status_details = use_signal(|| false);
    let mut show_green_details = use_signal(|| false);
    let mut show_yellow_details = use_signal(|| false);
    let mut show_red_details = use_signal(|| false);
    let mut show_orange_details = use_signal(|| false);
    let mut show_tantivy_details = use_signal(|| false);
    let mut status_hover_refcount = use_signal(|| 0i32);

    let mut show_inverted_index_details = use_signal(|| false);
    let mut show_busy_details = use_signal(|| false);
    let mut show_checking_details = use_signal(|| false);
    let mut show_indices_details = use_signal(|| false);

    // Log modal state
    let mut show_log_modal = use_signal(|| false);
    let mut log_status_type = use_signal(String::new);
    let mut log_content = use_signal(String::new);
    let mut log_loading = use_signal(|| false);
    let mut log_error = use_signal(|| Option::<String>::None);
    let mut log_total_lines = use_signal(|| 0usize);
    let current_route = use_route::<Route>();

    let mut show_help = use_context::<Signal<ShowHelpCommands>>();
    let mut show_rag_info = use_context::<Signal<ShowRagInfo>>();
    let mut clear_chat = use_context::<Signal<ClearChat>>();
    let mut boards_hidden = use_context::<Signal<BoardsHidden>>();
    let mut page_errors = use_context::<Signal<PageErrors>>();
    let runtime_suspended = use_context::<Signal<RuntimeSuspended>>();
    let runtime_ctx = use_context::<Signal<crate::app::RuntimeContext>>();

    let header_bg = "bg-gray-900";

    // Store last health response for load metrics display
    let mut last_health_response: Signal<Option<api::HealthResponse>> = use_signal(|| None);
    let mut last_upload_health_response: Signal<Option<api::HealthResponse>> = use_signal(|| None);
    // Track consecutive timeouts to show "checking" state
    let mut timeout_count: Signal<u32> = use_signal(|| 0);

    // Ollama thread-count drift. True between save of num_thread and the
    // moment the live runner reloads with the new value. Polled every 10s
    // by a background future; the backend clears the flag autonomously
    // once it sees a matching /proc cmdline for the ollama runner.
    let mut ollama_drift: Signal<Option<api::OllamaDrift>> = use_signal(|| None);
    use_future(move || async move {
        loop {
            if let Ok(snap) = api::fetch_ollama_drift().await {
                ollama_drift.set(Some(snap));
            }
            gloo_timers::future::TimeoutFuture::new(10_000).await;
        }
    });

    // Main health check loop
    use_future(move || async move {
        loop {
            match api::health_check().await {
                Ok(resp) => {
                    health_status.set(resp.status.clone());

                    // Check Redis status from cache endpoint
                    match api::fetch_cache_info().await {
                        Ok(cache_info) => {
                            if cache_info.redis.enabled && !cache_info.redis.connected {
                                page_errors
                                    .with_mut(|e| e.set_error("redis", "Redis not connected"));
                            } else {
                                page_errors.with_mut(|e| e.clear_error("redis"));
                            }
                        }
                        Err(_) => {
                            // Cache endpoint failed - can't determine Redis status, clear any stale error
                            page_errors.with_mut(|e| e.clear_error("redis"));
                        }
                    }

                    // Check Docker status
                    match api::fetch_docker_status().await {
                        Ok(docker_info) => {
                            if docker_info.docker_available {
                                let running = docker_info
                                    .containers
                                    .iter()
                                    .filter(|c| c.state == "running")
                                    .count();
                                let total = docker_info.containers.len();

                                if total > 0 && running < total {
                                    page_errors.with_mut(|e| {
                                        e.set_error(
                                            "docker",
                                            &format!(
                                                "Docker degraded: {}/{} containers",
                                                running, total
                                            ),
                                        )
                                    });
                                } else if total > 0 && running == 0 {
                                    page_errors.with_mut(|e| {
                                        e.set_error(
                                            "docker",
                                            "Docker unhealthy: no containers running",
                                        )
                                    });
                                } else {
                                    page_errors.with_mut(|e| e.clear_error("docker"));
                                }
                            } else {
                                page_errors
                                    .with_mut(|e| e.set_error("docker", "Docker not available"));
                            }
                        }
                        Err(_) => {
                            // Docker endpoint failed - clear any stale error
                            page_errors.with_mut(|e| e.clear_error("docker"));
                        }
                    }

                    // Check LLM runtime status based on configured backend.
                    // If the runtime is intentionally suspended (e.g., during bulk uploads),
                    // do not mark it as an error in the global status light.
                    if runtime_suspended().0 {
                        page_errors.with_mut(|e| e.clear_error("ollama"));
                        page_errors.with_mut(|e| e.clear_error("llama_cpp"));
                    } else {
                        let backend = runtime_ctx().configured_backend.clone();
                        match backend.as_str() {
                            "llama_cpp" => {
                                // Clear ollama/cloud errors since they're not the active backend
                                page_errors.with_mut(|e| e.clear_error("ollama"));
                                page_errors.with_mut(|e| e.clear_error("cloud_backend"));
                                match api::fetch_runtime_health().await {
                                    Ok(health) => {
                                        if health.llama_cpp_available {
                                            page_errors.with_mut(|e| e.clear_error("llama_cpp"));
                                        } else {
                                            page_errors.with_mut(|e| {
                                                e.set_error(
                                                    "llama_cpp",
                                                    "llama-server not reachable",
                                                )
                                            });
                                        }
                                    }
                                    Err(_) => {
                                        page_errors.with_mut(|e| {
                                            e.set_error("llama_cpp", "Runtime health check failed")
                                        });
                                    }
                                }
                            }
                            "openai" | "anthropic" | "openrouter" => {
                                // Cloud backends — check key is configured, no local process to ping
                                page_errors.with_mut(|e| e.clear_error("ollama"));
                                page_errors.with_mut(|e| e.clear_error("llama_cpp"));
                                match api::fetch_api_keys().await {
                                    Ok(keys) => {
                                        let has_key = match backend.as_str() {
                                            "openai" => keys.has_openai_key,
                                            "anthropic" => keys.has_anthropic_key,
                                            "openrouter" => keys.has_openrouter_key,
                                            _ => false,
                                        };
                                        if has_key {
                                            page_errors
                                                .with_mut(|e| e.clear_error("cloud_backend"));
                                        } else {
                                            page_errors.with_mut(|e| {
                                                e.set_error(
                                                    "cloud_backend",
                                                    &format!(
                                                        "No API key configured for {}",
                                                        backend
                                                    ),
                                                )
                                            });
                                        }
                                    }
                                    Err(_) => {
                                        page_errors.with_mut(|e| e.clear_error("cloud_backend"));
                                    }
                                }
                            }
                            _ => {
                                // Ollama or other backends
                                page_errors.with_mut(|e| e.clear_error("llama_cpp"));
                                page_errors.with_mut(|e| e.clear_error("cloud_backend"));
                                match api::fetch_models(&backend).await {
                                    Ok(models) => {
                                        if models.is_empty() {
                                            page_errors.with_mut(|e| {
                                                e.set_error(
                                                    "ollama",
                                                    &format!("{} online, but no models", backend),
                                                )
                                            });
                                        } else {
                                            page_errors.with_mut(|e| e.clear_error("ollama"));
                                        }
                                    }
                                    Err(_) => {
                                        page_errors.with_mut(|e| {
                                            e.set_error(
                                                "ollama",
                                                &format!("{} not reachable", backend),
                                            )
                                        });
                                    }
                                }
                            }
                        }
                    }

                    last_health_response.set(Some(resp));
                    timeout_count.set(0); // Reset timeout counter on success
                }
                Err(e) => {
                    // Check if it's likely a timeout (request failed)
                    if e.contains("timeout") || e.contains("Timeout") {
                        let count = timeout_count() + 1;
                        timeout_count.set(count);
                        if count >= 2 {
                            // After 2 consecutive timeouts, show checking
                            health_status.set("checking".to_string());
                        }
                    } else {
                        health_status.set("offline".to_string());
                        last_health_response.set(None);
                        timeout_count.set(0);
                    }
                }
            }

            gloo_timers::future::TimeoutFuture::new(5000).await;
        }
    });

    // Upload server health loop — runs independently of the search server loop
    use_future(move || async move {
        loop {
            match api::upload_health_check().await {
                Ok(resp) => {
                    let s = resp.status.clone();
                    last_upload_health_response.set(Some(resp));
                    match s.as_str() {
                        "offline" | "unhealthy" => {
                            page_errors.with_mut(|e| {
                                e.set_error("upload_server", "Upload server unhealthy")
                            });
                        }
                        "degraded" => {
                            page_errors.with_mut(|e| {
                                e.set_error("upload_server", "Upload server degraded")
                            });
                        }
                        _ => {
                            page_errors.with_mut(|e| e.clear_error("upload_server"));
                        }
                    }
                }
                Err(_) => {
                    last_upload_health_response.set(None);
                    page_errors.with_mut(|e| e.set_error("upload_server", "Upload server offline"));
                }
            }
            gloo_timers::future::TimeoutFuture::new(5000).await;
        }
    });

    let status = health_status();
    let errors = page_errors();
    let has_page_errors = errors.has_errors();

    let (bg_color, style_override, extra_class) = if has_page_errors {
        ("bg-red-500", Some("background-color: #ef4444;"), "")
    } else {
        match status.as_str() {
            "healthy" | "ok" => ("bg-green-500", Some("background-color: #22c55e;"), ""),
            "busy" => ("bg-pink-500", Some("background-color: #ec4899;"), ""),
            "degraded" => ("bg-yellow-500", Some("background-color: #eab308;"), ""),
            "checking" => (
                "bg-purple-400",
                Some("background-color: #c084fc;"),
                "animate-pulse",
            ),
            "offline" | "unhealthy" => ("bg-red-500", Some("background-color: #ef4444;"), ""),
            "unknown" => ("bg-blue-400", Some("background-color: #60a5fa;"), ""),
            _ => ("bg-orange-500", Some("background-color: #f97316;"), ""),
        }
    };

    let _modal_target = if has_page_errors {
        "page_errors".to_string()
    } else {
        status.clone()
    };

    let tooltip_text = if has_page_errors {
        let all_errors = errors.get_all_errors();
        let error_lines: Vec<String> = all_errors
            .iter()
            .map(|(page, err)| format!("[{}] {}", page, err))
            .collect();
        format!("Page errors:\n{}", error_lines.join("\n"))
    } else {
        let mut lines: Vec<String> = Vec::new();
        if let Some(resp) = last_health_response() {
            let mut line = format!("Search (3010): {}", resp.status);
            if let Some(d) = resp.documents {
                line = format!("{} | {} docs", line, d);
            }
            if let Some(v) = resp.vectors {
                line = format!("{}, {} vecs", line, v);
            }
            lines.push(line);
        } else {
            lines.push(format!("Search (3010): {}", status));
        }
        if let Some(resp) = last_upload_health_response() {
            let mut line = format!("Upload (3011): {}", resp.status);
            if let Some(m) = resp.message {
                line = format!("{} | {}", line, m);
            }
            if let Some(l) = resp.load {
                if l.indexing {
                    line = format!("{} | indexing", line);
                }
                line = format!("{} | queue:{}", line, l.queue_depth);
            }
            lines.push(line);
        } else {
            lines.push("Upload (3011): offline".to_string());
        }
        lines.join("\n")
    };

    let status_for_click = status.clone();
    let show_status_tooltip = status_hover_refcount() > 0;

    rsx! {
        header {
            class: "sticky top-0 shadow-md py-0 px-0.5 transition-colors {header_bg} flex items-center relative",
            style: "z-index: 60;",

            // Rust icon - ml-4 aligns with Panel p-4 padding where llama image sits
            div { class: "ml-4",
                img {
                    src: asset!("/assets/rusticon_black_bg.png"),
                    alt: "Rust Icon",
                    class: "h-8 w-8",
                }
            }

            // "Show Boards" — only on Home, only when the boards are hidden
            // (i.e. the user has already sent at least one message).
            if matches!(current_route, Route::Home {}) && boards_hidden().0 {
                button {
                    class: "ml-3 text-sm font-medium cursor-pointer hover:text-white transition-colors",
                    style: "color: #026B7C;",
                    onclick: move |_| boards_hidden.set(BoardsHidden(false)),
                    title: "Restore the Runtime / Mode / Corpus / RAG / KV boards",
                    "Show Boards"
                }
            }

            // Title and status — flex-1 center column, truncates on small screens
            div { class: "flex-1 min-w-0 flex justify-center items-center gap-2",
                h1 {
                    class: "font-medium truncate",
                    style: "font-family: ui-sans-serif, system-ui, sans-serif; font-size: 0.975rem; color: #026B7C;",
                    "Rust RAG Learning Platform"
                }
                div { class: "flex items-center gap-1 flex-shrink-0",
                    div {
                        class: "w-4 h-4 rounded-full border-2 border-gray-900 {bg_color} {extra_class} cursor-pointer hover:ring-2 hover:ring-white hover:ring-opacity-50 transition-all",
                        style: style_override.unwrap_or(""),
                        title: format!("Status: {} (click for details)", tooltip_text),
                        onmouseenter: move |_| status_hover_refcount.with_mut(|count| *count += 1),
                        onmouseleave: move |_| {
                            status_hover_refcount
                                .with_mut(|count| {
                                    if *count > 0 {
                                        *count -= 1;
                                    }
                                })
                        },
                        onclick: move |_| {
                            match status_for_click.as_str() {
                                "healthy" | "ok" => show_green_details.set(true),
                                "degraded" => show_yellow_details.set(true),
                                "offline" | "unhealthy" => show_red_details.set(true),
                                "busy" => show_busy_details.set(true),
                                "checking" => show_checking_details.set(true),
                                _ => show_orange_details.set(true),
                            }
                        },
                    }
                    button {
                        class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                        style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: transparent; border: 1.5px solid #026B7C;",
                        onclick: move |_| show_status_info.set(true),
                        title: "Status info",
                        svg {
                            class: INFO_ICON_SVG_CLASS,
                            view_box: "0 0 20 20",
                            fill: "none",
                            stroke: "#026B7C",
                            circle {
                                cx: "10",
                                cy: "10",
                                r: "9",
                                stroke_width: "1.5",
                            }
                            line {
                                x1: "10",
                                y1: "8",
                                x2: "10",
                                y2: "14",
                                stroke_width: "1.5",
                            }
                            circle {
                                cx: "10",
                                cy: "6.3",
                                r: "1",
                                fill: "#026B7C",
                                stroke: "none",
                            }
                        }
                    }
                }
            }

            if show_status_tooltip {
                div {
                    class: "absolute top-12 left-1/2 -translate-x-1/2 z-[65]",
                    onmouseenter: move |_| status_hover_refcount.with_mut(|count| *count += 1),
                    onmouseleave: move |_| {
                        status_hover_refcount
                            .with_mut(|count| {
                                if *count > 0 {
                                    *count -= 1;
                                }
                            })
                    },
                    div { class: "bg-gray-800 border border-gray-600 rounded-lg shadow-lg p-3 w-[98vw] select-text",
                        pre { class: "whitespace-pre-wrap text-xs text-gray-100", "{tooltip_text}" }
                    }
                }
            }

            div { class: "flex-shrink-0 flex justify-end items-center",

                nav {
                    class: "hidden md:flex items-center gap-[0.1875rem] text-sm",
                    style: "font-family: ui-sans-serif, system-ui, sans-serif;",
                    {
                        let home_color = if matches!(current_route, Route::Home {}) {
                            "#7C2A02"
                        } else {
                            "white"
                        };
                        rsx! {
                            Link {
                                to: Route::Home {},
                                class: "py-2 px-3 rounded-lg transition-colors font-medium",
                                style: format!("color: {};", home_color),
                                onclick: move |_| {
                                    // Clear chat and scroll to top when clicking Home
                                    clear_chat.set(ClearChat(true));
                                    if let Some(window) = web_sys::window() {
                                        window.scroll_to_with_x_and_y(0.0, 0.0);
                                    }
                                },
                                "Home"
                            }
                        }
                    }
                    {
                        let monitor_color = if matches!(
                            current_route,
                            Route::MonitorTip {}
                            | Route::MonitorAgentic {}
                            | Route::MonitorRequests {}
                            | Route::MonitorCache {}
                            | Route::MonitorDatastores {}
                            | Route::MonitorIndex {}
                            | Route::MonitorRateLimits {}
                            | Route::MonitorLogs {}
                            | Route::MonitorDocker {}
                            | Route::MonitorKnowledgeGraph {}
                            | Route::MonitorAgSystemd {}
                        ) {
                            "#7C2A02"
                        } else {
                            "white"
                        };
                        rsx! {
                            Link {
                                to: Route::MonitorTip {},
                                class: "py-2 px-3 rounded-lg transition-colors font-medium",
                                style: format!("color: {};", monitor_color),
                                "Monitor"
                            }
                        }
                    }
                    {
                        let config_color = if matches!(
                            current_route,
                            Route::ConfigRuntime {}
                            | Route::ConfigHardware {}
                            | Route::ConfigHardware {}
                            | Route::ConfigOther {}
                            | Route::Parameters {}
                        ) {
                            "#7C2A02"
                        } else {
                            "white"
                        };
                        rsx! {
                            Link {
                                to: Route::ConfigRuntime {},
                                class: "py-2 px-3 rounded-lg transition-colors font-medium",
                                style: format!("color: {};", config_color),
                                "Config"
                            }
                        }
                    }
                    {
                        let train_color = if matches!(current_route, Route::Train {}) {
                            "#7C2A02"
                        } else {
                            "white"
                        };
                        rsx! {
                            Link {
                                to: Route::Train {},
                                class: "py-2 px-3 rounded-lg transition-colors font-medium",
                                style: format!("color: {};", train_color),
                                "Train"
                            }
                        }
                    }
                    {
                        let docu_color = if matches!(current_route, Route::DocuIndex {}) {
                            "#7C2A02"
                        } else {
                            "white"
                        };
                        rsx! {
                            Link {
                                to: Route::DocuAgPipeline {},
                                class: "py-2 px-3 rounded-lg transition-colors font-medium",
                                style: format!("color: {};", docu_color),
                                "Pipe"
                            }
                        }
                    }
                    NavDropdown { title: "Help".to_string(),
                        DropdownActionItem { onclick: move |_| show_help.set(ShowHelpCommands(true)),
                            "/help commands"
                        }
                        DropdownItem { to: Route::Home {}, "Design" }
                        DropdownItem { to: Route::Home {}, "Consulting" }
                    }
                    NavDropdown { title: "About".to_string(),
                        DropdownActionItem { onclick: move |_| show_rag_info.set(ShowRagInfo(true)),
                            "RAG"
                        }
                        DropdownItem { to: Route::About {}, "Company" }
                        DropdownItem { to: Route::About {}, "Contact" }
                    }
                }
                button {
                    class: "md:hidden p-2 text-2xl",
                    onclick: move |_| menu_open.set(!menu_open()),
                    "☰"
                }
            }

            if menu_open() {
                div { class: "md:hidden mt-4 pb-4 flex flex-col gap-4",
                    Link {
                        to: Route::Home {},
                        class: "text-teal-200 hover:text-white transition-colors",
                        onclick: move |_| {
                            clear_chat.set(ClearChat(true));
                            menu_open.set(false);
                        },
                        "Home"
                    }
                    Link {
                        to: Route::MonitorTip {},
                        class: "text-teal-100 hover:text-white transition-colors",
                        onclick: move |_| menu_open.set(false),
                        "Monitor"
                    }
                    Link {
                        to: Route::ConfigRuntime {},
                        class: "text-teal-100 hover:text-white transition-colors",
                        onclick: move |_| menu_open.set(false),
                        "Config"
                    }
                    Link {
                        to: Route::Train {},
                        class: "text-teal-100 hover:text-white transition-colors",
                        onclick: move |_| menu_open.set(false),
                        "Train"
                    }
                    Link {
                        to: Route::DocuAgPipeline {},
                        class: "text-teal-100 hover:text-white transition-colors",
                        onclick: move |_| menu_open.set(false),
                        "Pipe"
                    }
                    Link {
                        to: Route::About {},
                        class: "hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors",
                        onclick: move |_| menu_open.set(false),
                        "About"
                    }
                    button {
                        class: "text-left text-teal-100 hover:text-white transition-colors",
                        onclick: move |_| {
                            show_help.set(ShowHelpCommands(true));
                            menu_open.set(false);
                        },
                        "/help commands"
                    }
                }
            }
        }

        if show_initial_status_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1110;",
                onclick: move |_| show_initial_status_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-2",
                        h3 { class: "text-lg font-semibold text-white", "Initial status details" }
                        a {
                            class: "text-blue-400 hover:text-blue-300 text-sm cursor-pointer",
                            href: "#log",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                log_status_type.set("initial".to_string());
                                log_loading.set(true);
                                log_error.set(None);
                                show_log_modal.set(true);
                                spawn(async move {
                                    match api::get_status_log("initial").await {
                                        Ok(resp) => {
                                            log_content.set(resp.content);
                                            log_total_lines.set(resp.total_lines);
                                            log_loading.set(false);
                                        }
                                        Err(e) => {
                                            log_error.set(Some(e));
                                            log_loading.set(false);
                                        }
                                    }
                                });
                            },
                            "Log"
                        }
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "An async loop immediately calls api::health_check() before awaiting any delay—only after the response (or error) does it sleep for 5 seconds and repeat. The “unknown (initial state before first check)” label only applies right after the component ‘mounts’ (in Dioxus this means the Rust Header component that, through the rsx! macro, renders the header HTML where the status indicator lives). The very first health check happens immediately on mount, before the first 5-second timer runs."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "health_check() is the frontend’s lightweight “ping the backend” call. When health_check() runs on the frontend, it first calls api_url(\"/monitoring/health\"). That helper, in turn, invokes resolve_api_base_url() to figure out the proper scheme/host/port (current browser origin if available, otherwise fallback to http://127.0.0.1:3010). It then appends the path, returning the full URL. Finally, health_check() performs the GET request against that assembled URL."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "In a browser, “origin” means the combination of scheme://host:port of the page you’re running. The resolve_api_base_url() helper in frontend/fro/src/api.rs looks at window.location.origin() to reuse that origin whenever possible. window.location.origin is part of the standard browser DOM API. When Dioxus runs in the browser, it can call into Web APIs via web_sys. In frontend/fro/src/api.rs, the helper function resolve_api_base_url() does exactly that."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "The DOM (Document Object Model) API is the browser’s programming interface for web pages. Your Dioxus frontend leans on the browser’s DOM API via the web_sys bindings that Dioxus/Rust expose. Classic DOM operations (window/document access, class toggling, scrolling) are wrapped in Rust code through Dioxus’s web_sys interface. Class toggling means programmatically adding or removing CSS class names so styling changes dynamically—for example, Layout grabs document.documentElement() and toggles the dark class depending on the theme signal."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_initial_status_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_green_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1110;",
                onclick: move |_| show_green_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-2",
                        h3 { class: "text-lg font-semibold text-green-400", "Healthy (Green)" }
                        a {
                            class: "text-blue-400 hover:text-blue-300 text-sm cursor-pointer",
                            href: "#log",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                log_status_type.set("healthy".to_string());
                                log_loading.set(true);
                                log_error.set(None);
                                show_log_modal.set(true);
                                spawn(async move {
                                    match api::get_status_log("healthy").await {
                                        Ok(resp) => {
                                            log_content.set(resp.content);
                                            log_total_lines.set(resp.total_lines);
                                            log_loading.set(false);
                                        }
                                        Err(e) => {
                                            log_error.set(Some(e));
                                            log_loading.set(false);
                                        }
                                    }
                                });
                            },
                            "Log"
                        }
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Green means the backend's /monitoring/health endpoint returned status: \"healthy\" (or \"ok\"). This indicates that retriever.health_check() passed all its internal validations."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "What retriever.health_check() validates: (1) The "
                        a {
                            class: "text-green-300 underline decoration-dotted cursor-pointer",
                            href: "#tantivy-info",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                show_tantivy_details.set(true);
                            },
                            "Tantivy"
                        }
                        " index directory exists and is readable. (2) An index reader and searcher can be created. (3) Vector storage is consistent—the number of vectors matches document mappings, "
                        a {
                            class: "text-green-300 underline decoration-dotted cursor-pointer",
                            href: "#indices-info",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                show_indices_details.set(true);
                            },
                            "all indices are in bounds"
                        }
                        ", and all vectors share the same dimension. (4) A basic search query executes successfully if documents exist. (5) The vector file (or its parent directory) is writable. (6) At least 100 MB of disk space remains."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "When all checks pass, the backend logs \"Health: OK\" with document and vector counts, and the frontend displays the green indicator."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_green_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_yellow_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1110;",
                onclick: move |_| show_yellow_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-2",
                        h3 { class: "text-lg font-semibold text-yellow-400", "Degraded (Yellow)" }
                        a {
                            class: "text-blue-400 hover:text-blue-300 text-sm cursor-pointer",
                            href: "#log",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                log_status_type.set("degraded".to_string());
                                log_loading.set(true);
                                log_error.set(None);
                                show_log_modal.set(true);
                                spawn(async move {
                                    match api::get_status_log("degraded").await {
                                        Ok(resp) => {
                                            log_content.set(resp.content);
                                            log_total_lines.set(resp.total_lines);
                                            log_loading.set(false);
                                        }
                                        Err(e) => {
                                            log_error.set(Some(e));
                                            log_loading.set(false);
                                        }
                                    }
                                });
                            },
                            "Log"
                        }
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Yellow means the backend returned status: \"degraded\". This is a middle ground—the system is operational but some non-critical component isn't performing optimally."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Possible causes include: high latency on certain operations, cache layers partially unavailable, background indexing still in progress, or optional services (like Redis L3 cache) being unreachable while core functionality remains intact."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "The system can still serve requests, but you may notice slower responses or reduced functionality in some areas. Check /monitoring/metrics or logs for specifics."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_yellow_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_red_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1110;",
                onclick: move |_| show_red_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4 max-h-[85vh] overflow-y-auto",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-2",
                        h3 { class: "text-lg font-semibold text-red-400", "Offline / Unhealthy (Red)" }
                        a {
                            class: "text-blue-400 hover:text-blue-300 text-sm cursor-pointer",
                            href: "#log",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                let current_status = health_status();
                                log_status_type.set(current_status.clone());
                                log_loading.set(true);
                                log_error.set(None);
                                show_log_modal.set(true);
                                spawn(async move {
                                    match api::get_status_log(&current_status).await {
                                        Ok(resp) => {
                                            log_content.set(resp.content);
                                            log_total_lines.set(resp.total_lines);
                                            log_loading.set(false);
                                        }
                                        Err(e) => {
                                            log_error.set(Some(e));
                                            log_loading.set(false);
                                        }
                                    }
                                });
                            },
                            "Log"
                        }
                    }

                    // Show active page errors if any
                    {
                        let current_errors = page_errors();
                        if current_errors.has_errors() {
                            let all_errors = current_errors.get_all_errors();
                            rsx! {
                                div { class: "bg-red-900/30 border border-red-700 rounded-lg p-4 space-y-2",
                                    h4 { class: "text-red-300 font-semibold mb-2", "Active Service Errors" }
                                    for (source , message) in all_errors.iter() {
                                        div { class: "flex items-start gap-2",
                                            span { class: "text-red-400 font-mono text-xs bg-red-900/50 px-2 py-0.5 rounded shrink-0",
                                                "{source}"
                                            }
                                            span { class: "text-gray-200 text-sm", "{message}" }
                                        }
                                    }
                                }
                                div { class: "text-xs text-gray-300",
                                    "Backend health status: "
                                    span { class: "font-mono text-gray-300", "{health_status()}" }
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Red appears when either: (1) The frontend's health_check() request failed entirely (network error, timeout, backend not running), setting status to \"offline\". (2) The backend responded with status: \"unhealthy\" because retriever.health_check() failed. (3) A monitored service (Redis, Docker, Ollama) reported an error."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Common failure reasons from retriever.health_check(): index directory missing or not a directory, unable to create index reader, vector/document mapping inconsistency, search test failure, vector file not writable, or insufficient disk space (< 100 MB)."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "When unhealthy, the backend returns HTTP 503 with an error message. Check that the backend process is running, the index directory exists and has correct permissions, and there's sufficient disk space."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_red_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_orange_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1110;",
                onclick: move |_| show_orange_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-2",
                        h3 { class: "text-lg font-semibold text-orange-400",
                            "Unexpected Status (Orange)"
                        }
                        a {
                            class: "text-blue-400 hover:text-blue-300 text-sm cursor-pointer",
                            href: "#log",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                log_status_type.set("unknown".to_string());
                                log_loading.set(true);
                                log_error.set(None);
                                show_log_modal.set(true);
                                spawn(async move {
                                    match api::get_status_log("unknown").await {
                                        Ok(resp) => {
                                            log_content.set(resp.content);
                                            log_total_lines.set(resp.total_lines);
                                            log_loading.set(false);
                                        }
                                        Err(e) => {
                                            log_error.set(Some(e));
                                            log_loading.set(false);
                                        }
                                    }
                                });
                            },
                            "Log"
                        }
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Orange is the fallback color when the backend returns a status string that doesn't match any known value (\"healthy\", \"ok\", \"degraded\", \"offline\", \"unhealthy\", or \"unknown\")."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "This could indicate: a newer backend version returning new status values the frontend doesn't recognize yet, a bug in the backend's health response, or corrupted/malformed JSON in the response."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Check the 'Current status' field at the bottom of the status info panel to see the exact value returned. If this persists, verify frontend and backend versions are compatible."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_orange_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_tantivy_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1120;",
                onclick: move |_| show_tantivy_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4",
                    onclick: move |evt| evt.stop_propagation(),
                    h3 { class: "text-lg font-semibold text-blue-300", "Tantivy" }
                    p { class: "text-gray-200 leading-relaxed",
                        "Think of Tantivy as a search engine that needs to be \"filled\" before an LLM can make use of it. Tantivy doesn't store raw data in a simple list. It transforms whatever you give it into an "
                        a {
                            class: "text-blue-300 underline decoration-dotted cursor-pointer",
                            href: "#inverted-index-info",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                show_inverted_index_details.set(true);
                            },
                            "inverted index"
                        }
                        " and other optimized structures. That transformation step is the \"filling.\""
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "You're not limited to long documents or traditional RAG material. You can index anything that can be expressed as structured or semi-structured information: facts, events, tool calls, code symbols, logs, tasks, or even the metadata an LLM generates about its own reasoning. Once this material is indexed, the LLM can treat Tantivy as a kind of symbolic memory or knowledge substrate. Instead of relying on embeddings or similarity search, the model formulates precise search queries—essentially instructions for what it wants to retrieve—and Tantivy responds with the matching items. The LLM then interprets those results, reasons over them, or uses them to decide what to do next."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "This creates a dynamic loop. The LLM can enrich the index by producing summaries, tags, classifications, or extracted entities, and those become part of the searchable space. Later, the model can query that same space to recall past decisions, find relevant code paths, identify patterns in logs, or select appropriate tools or workflows. Tantivy becomes a fast, structured retrieval layer that complements the LLM's reasoning rather than a repository of text chunks."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_tantivy_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_inverted_index_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1130;",
                onclick: move |_| show_inverted_index_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4",
                    onclick: move |evt| evt.stop_propagation(),
                    h3 { class: "text-lg font-semibold text-blue-300", "Inverted Index" }
                    p { class: "text-gray-200 leading-relaxed",
                        "An inverted index is simply a clever way of organizing information so that you can find things by looking up the words or terms first, rather than scanning every item one by one. It flips the usual structure \"inside out,\" which is why it's called inverted."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "A normal list stores items in their natural order—documents, messages, facts, logs, whatever you're indexing. If you want to know which items contain the word \"error,\" you would have to scan everything."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "An inverted index turns this around. Instead of starting from the items, it starts from the terms. For each term, it keeps a list of all the items in which that term appears. So \"error\" points directly to the items that contain it, \"user\" points to its own list, and so on. This structure makes searching extremely fast because the system no longer needs to read through all the data; it jumps straight to the relevant pieces."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "It allows answering queries in milliseconds even when the underlying dataset is large. It's the foundational trick that makes full-text search efficient."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_inverted_index_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_indices_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1140;",
                onclick: move |_| show_indices_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4 max-h-[85vh] overflow-y-auto",
                    onclick: move |evt| evt.stop_propagation(),
                    h3 { class: "text-lg font-semibold text-green-300", "All Indices Are In Bounds" }
                    p { class: "text-gray-200 leading-relaxed",
                        "When the health check says that all indices are in bounds, it is confirming that every document in the index points to a valid position inside the vector store. The vector store contains embeddings arranged in a fixed sequence, starting at index 0 and ending at the last stored vector. If the store contains N vectors, then the only valid indices are from 0 up to N\u{2011}1. Any reference to an index outside that range\u{2014}such as a negative number or a number equal to or greater than the total number of vectors\u{2014}would be considered out of bounds."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "The health check walks through each document's recorded vector index and verifies that it falls within this valid range. It also ensures that the referenced vector actually exists, is readable, and has the correct dimensionality. This prevents situations where a document claims to have an embedding at a position that doesn't exist in the vector file, which could happen due to partial writes, crashes, or corruption during indexing."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "This validation is essential because an out\u{2011}of\u{2011}bounds index would cause the retriever to attempt to read a vector that isn't there, which can lead to crashes, undefined behavior, or incorrect search results. Ensuring that all indices are in bounds guarantees that the mapping between documents and vectors is internally consistent and safe to use during retrieval."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_indices_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_busy_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1110;",
                onclick: move |_| show_busy_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-2",
                        h3 { class: "text-lg font-semibold text-pink-400", "Healthy but Busy (Pink)" }
                        a {
                            class: "text-blue-400 hover:text-blue-300 text-sm cursor-pointer",
                            href: "#log",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                log_status_type.set("busy".to_string());
                                log_loading.set(true);
                                log_error.set(None);
                                show_log_modal.set(true);
                                spawn(async move {
                                    match api::get_status_log("busy").await {
                                        Ok(resp) => {
                                            log_content.set(resp.content);
                                            log_total_lines.set(resp.total_lines);
                                            log_loading.set(false);
                                        }
                                        Err(e) => {
                                            log_error.set(Some(e));
                                            log_loading.set(false);
                                        }
                                    }
                                });
                            },
                            "Log"
                        }
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "The backend is healthy and responsive, but currently under heavy load. The system is processing resource-intensive tasks that may slow down response times."
                    }

                    // Show real-time load metrics
                    div { class: "bg-gray-900 rounded-lg p-4 space-y-2",
                        h4 { class: "text-pink-300 font-semibold mb-2", "Current Load Metrics" }
                        {
                            let load_opt = last_health_response().and_then(|r| r.load);
                            if let Some(load) = load_opt {
                                rsx! {
                                    div { class: "grid grid-cols-2 gap-2 text-sm",
                                        div { class: "text-gray-400", "CPU Usage:" }
                                        div { class: "text-white font-mono", "{load.cpu_percent:.1}%" }

                                        div { class: "text-gray-400", "Memory Usage:" }
                                        div { class: "text-white font-mono", "{load.memory_percent:.1}%" }

                                        div { class: "text-gray-400", "Active Tasks:" }
                                        div { class: "text-white font-mono", "{load.active_tasks}" }

                                        div { class: "text-gray-400", "Queue Depth:" }
                                        div { class: "text-white font-mono", "{load.queue_depth}" }

                                        div { class: "text-gray-400", "Indexing:" }
                                        div { class: if load.indexing { "text-pink-400 font-semibold" } else { "text-gray-300" },
                                            if load.indexing {
                                                "Yes ⚡"
                                            } else {
                                                "No"
                                            }
                                        }

                                        div { class: "text-gray-400", "LLM Active:" }
                                        div { class: if load.llm_active { "text-pink-400 font-semibold" } else { "text-gray-300" },
                                            if load.llm_active {
                                                "Yes 🤖"
                                            } else {
                                                "No"
                                            }
                                        }
                                    }
                                }
                            } else {
                                rsx! {
                                    p { class: "text-gray-400 italic", "Load metrics not available" }
                                }
                            }
                        }
                    }

                    // Show message if available
                    {
                        let msg_opt = last_health_response().and_then(|r| r.message);
                        if let Some(msg) = msg_opt {
                            rsx! {
                                div { class: "bg-pink-900/30 border border-pink-700 rounded p-3",
                                    span { class: "text-pink-300", "{msg}" }
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }

                    p { class: "text-gray-200 leading-relaxed",
                        "Common causes: initial document indexing on startup, large LLM generation requests, bulk document uploads, or reindexing operations."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_busy_details.set(false),
                        "Close"
                    }
                }
            }
        }

        if show_checking_details() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1110;",
                onclick: move |_| show_checking_details.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm space-y-4",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-2",
                        h3 { class: "text-lg font-semibold text-purple-400",
                            "Checking / Slow Response (Purple Pulsing)"
                        }
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Purple pulsing means the frontend's health check request is taking longer than expected (over 8 seconds). This is a transitional state indicating the backend might be very busy or experiencing issues."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Unlike \"offline\" (red), this state means we haven't given up yet—the request is still pending. The frontend will continue waiting and update the status once a response arrives or the next check cycle begins."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "Possible causes: backend is processing a very large request, system is under extreme load, network latency issues, the backend is starting up and loading models into memory, or the ONNX embedding thread pool is spinning at high CPU (saturates the tokio runtime and delays health check responses)."
                    }
                    p { class: "text-gray-200 leading-relaxed",
                        "If this persists, use the Monitor pages to diagnose:"
                    }
                    ul { class: "ml-4 space-y-2 list-disc list-outside text-gray-300 text-sm",
                        li {
                            Link {
                                to: Route::MonitorIndex {},
                                class: "font-semibold text-purple-300 underline decoration-dotted hover:text-purple-200",
                                onclick: move |_| show_checking_details.set(false),
                                "Monitor → Index"
                            }
                            " — the most common cause. Scroll to the "
                            span { class: "font-semibold", "Reindex Control" }
                            " section and check the jobs table for a running job. A large reindex saturates the backend thread pool and stalls the health endpoint. Wait for it to finish."
                        }
                        li {
                            Link {
                                to: Route::MonitorRequests {},
                                class: "font-semibold text-purple-300 underline decoration-dotted hover:text-purple-200",
                                onclick: move |_| show_checking_details.set(false),
                                "Monitor → Requests"
                            }
                            " — look at the "
                            span { class: "font-semibold", "Latency Breakdown" }
                            " table (p50 / p95 / p99). Values in the hundreds of ms confirm the backend is under load. If p95 is high but p50 is normal, a single slow operation is blocking some requests."
                        }
                        li {
                            span { class: "font-semibold", "Backend logs" }
                            " — in the terminal running "
                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "RUST_LOG=info cargo run" }
                            ", look for "
                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "WARN" }
                            "/"
                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "ERROR" }
                            " lines or mentions of "
                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "reindex" }
                            ", "
                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "embedding" }
                            ", or "
                            code { class: "text-gray-200 bg-gray-700 px-0.5 rounded", "Health:" }
                            "."
                        }
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_checking_details.set(false),
                        "Close"
                    }
                }
            }
        }

        // Status Info Modal
        if show_status_info() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/60",
                style: "z-index: 1100;",
                onclick: move |_| show_status_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[98vw] shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h2 { class: "text-lg font-semibold text-gray-100", "Backend Status Indicator" }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_status_info.set(false),
                            "×"
                        }
                    }
                    p { class: "text-sm text-gray-400 mb-4",
                        "The status light combines health from both backend servers (search :3010 and upload :3011):"
                    }
                    div { class: "space-y-3",
                        // Unknown
                        div { class: "flex items-center gap-3",
                            div {
                                class: "w-4 h-4 rounded-full bg-blue-400 border-2 border-gray-700",
                                style: "background-color: #60a5fa;",
                            }
                            div {
                                span { class: "text-blue-300 font-medium", "Blue" }
                                span { class: "text-gray-400 text-sm ml-2", "— " }
                                a {
                                    class: "text-gray-400 text-sm underline decoration-dotted",
                                    href: "#initial-status",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        show_initial_status_details.set(true);
                                    },
                                    "Initial state before first check"
                                }
                            }
                        }
                        // Healthy
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-green-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-green-400 font-medium", "Green" }
                                span { class: "text-gray-400 text-sm ml-2", "— " }
                                a {
                                    class: "text-gray-400 text-sm underline decoration-dotted",
                                    href: "#green-status",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        show_green_details.set(true);
                                    },
                                    "All components healthy"
                                }
                            }
                        }
                        // Busy
                        div { class: "flex items-center gap-3",
                            div {
                                class: "w-4 h-4 rounded-full bg-pink-500 border-2 border-gray-700",
                                style: "background-color: #ec4899;",
                            }
                            div {
                                span { class: "text-pink-400 font-medium", "Pink" }
                                span { class: "text-gray-400 text-sm ml-2", "— " }
                                a {
                                    class: "text-gray-400 text-sm underline decoration-dotted",
                                    href: "#busy-status",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        show_busy_details.set(true);
                                    },
                                    "Healthy but busy"
                                }
                            }
                        }
                        // Checking
                        div { class: "flex items-center gap-3",
                            div {
                                class: "w-4 h-4 rounded-full bg-purple-400 border-2 border-gray-700 animate-pulse",
                                style: "background-color: #c084fc;",
                            }
                            div {
                                span { class: "text-purple-400 font-medium", "Purple (pulsing)" }
                                span { class: "text-gray-400 text-sm ml-2", "— " }
                                a {
                                    class: "text-gray-400 text-sm underline decoration-dotted",
                                    href: "#checking-status",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        show_checking_details.set(true);
                                    },
                                    "Checking / slow response"
                                }
                            }
                        }
                        // Degraded
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-yellow-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-yellow-400 font-medium", "Yellow" }
                                span { class: "text-gray-400 text-sm ml-2", "— " }
                                a {
                                    class: "text-gray-400 text-sm underline decoration-dotted",
                                    href: "#yellow-status",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        show_yellow_details.set(true);
                                    },
                                    "Some component degraded"
                                }
                            }
                        }
                        // Offline/Unhealthy
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-red-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-red-400 font-medium", "Red" }
                                span { class: "text-gray-400 text-sm ml-2", "— " }
                                a {
                                    class: "text-gray-400 text-sm underline decoration-dotted",
                                    href: "#red-status",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        show_red_details.set(true);
                                    },
                                    "Backend down or unhealthy"
                                }
                            }
                        }
                        // Other
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-orange-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-orange-400 font-medium", "Orange" }
                                span { class: "text-gray-400 text-sm ml-2", "— " }
                                a {
                                    class: "text-gray-400 text-sm underline decoration-dotted",
                                    href: "#orange-status",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        show_orange_details.set(true);
                                    },
                                    "Unexpected status value"
                                }
                            }
                        }
                    }
                    div { class: "mt-4 pt-3 border-t border-gray-700 text-xs text-gray-300 space-y-1",
                        div {
                            "Search (3010): "
                            span { class: "font-mono text-gray-300", "{health_status()}" }
                        }
                        div {
                            "Upload (3011): "
                            span { class: "font-mono text-gray-300",
                                {
                                    last_upload_health_response()
                                        .map(|r| r.status)
                                        .unwrap_or_else(|| "offline".to_string())
                                }
                            }
                        }
                    }
                    button {
                        class: "btn btn-primary btn-sm mt-4 w-full",
                        onclick: move |_| show_status_info.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // Status Log Modal
        if show_log_modal() {
            div {
                class: "fixed inset-0 flex items-center justify-center bg-black/70",
                style: "z-index: 1200;",
                onclick: move |_| show_log_modal.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg w-[98vw] p-6 shadow-2xl text-sm max-h-[85vh] flex flex-col",
                    onclick: move |evt| evt.stop_propagation(),
                    div { class: "flex items-center justify-between mb-4",
                        h3 { class: "text-lg font-semibold text-blue-400",
                            "Status Log: {log_status_type()}"
                        }
                        button {
                            class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                            onclick: move |_| show_log_modal.set(false),
                            "×"
                        }
                    }
                    div { class: "text-xs text-gray-300 mb-2",
                        "Total entries: {log_total_lines()} (showing last 100)"
                    }
                    if log_loading() {
                        div { class: "flex items-center justify-center py-8",
                            div { class: "animate-spin rounded-full h-8 w-8 border-b-2 border-blue-400" }
                            span { class: "ml-3 text-gray-400", "Loading log..." }
                        }
                    } else if let Some(err) = log_error() {
                        div { class: "bg-red-900/30 border border-red-700 rounded p-4 text-red-300",
                            "Error loading log: {err}"
                        }
                    } else if log_content().is_empty() {
                        div { class: "text-gray-300 italic py-8 text-center",
                            "No log entries yet for this status."
                        }
                    } else {
                        div { class: "flex-1 overflow-auto bg-gray-900 rounded p-3 font-mono text-xs",
                            pre { class: "whitespace-pre-wrap text-gray-300", "{log_content()}" }
                        }
                    }
                    button {
                        class: "btn btn-primary btn-sm mt-4 w-full",
                        onclick: move |_| show_log_modal.set(false),
                        "Close"
                    }
                }
            }
        }

        // Ollama thread-count drift banner. Ollama bakes num_thread into
        // the runner at model-load time, so a config change made while the
        // model is resident is silently ignored until the runner reloads.
        // The backend flips this flag on save and clears it once the live
        // runner PID changes (i.e. Ollama actually reloaded the model).
        if let Some(snap) = ollama_drift() {
            if snap.drift {
                div {
                    class: "sticky top-10 z-50 bg-orange-900/40 border-b border-orange-700 text-orange-100 text-sm px-4 py-2 flex items-center gap-3",
                    span { class: "font-semibold", "Ollama thread drift:" }
                    span {
                        "configured for {snap.configured} thread(s) but the live runner was loaded before this change. Restart Ollama (`systemctl --user restart ollama.service`) so the new value takes effect."
                    }
                }
            }
        }
    }
}
