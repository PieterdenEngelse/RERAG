use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use crate::{
    api,
    app::PageErrors,
    components::{
        config_nav::{ConfigNav, ConfigTab},
        monitor::HealthCard,
    },
};
use dioxus::prelude::*;

// Styling constants matching hardware page
const PARAM_BLOCK_CLASS: &str = "flex flex-col gap-1 text-xs text-gray-200";
const PARAM_COLUMN_CLASS: &str = "param-column-spacing";
const PARAM_LABEL_CLASS: &str = "text-gray-400 whitespace-nowrap";
const PARAM_NUMBER_INPUT_CLASS: &str =
    "input input-xs input-bordered bg-gray-700 text-gray-200 !w-24";
const CHECKBOX_CLASS: &str = "checkbox checkbox-xs onnx-checkbox";

/// Info icon component
#[component]
fn InfoIcon() -> Element {
    rsx! {
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

/// Info modal component
fn info_modal(title: &str, toggle: Signal<bool>, paragraphs: Vec<&str>) -> Element {
    let mut toggle = toggle;
    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
            onclick: move |_| toggle.set(false),
            div {
                class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-lg max-h-[90vh] overflow-y-auto shadow-xl",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between mb-3",
                    h2 { class: "text-base font-semibold text-gray-100", "{title}" }
                    button {
                        class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                        onclick: move |_| toggle.set(false),
                        "×"
                    }
                }
                div { class: "text-sm text-gray-300 space-y-2",
                    for paragraph in paragraphs {
                        p { "{paragraph}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ConfigIoUring() -> Element {
    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 1: QUEUE & BUFFERS
    // ═══════════════════════════════════════════════════════════════
    let mut ring_size = use_signal(|| 256u32);
    let mut cq_size = use_signal(|| 0u32);
    let mut buffer_size = use_signal(|| 65536usize);
    let mut buffer_pool_size = use_signal(|| 64usize);
    let mut clamp = use_signal(|| false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 2: POLLING
    // ═══════════════════════════════════════════════════════════════
    let mut sqpoll = use_signal(|| false);
    let mut sqpoll_idle_ms = use_signal(|| 1000u32);
    let mut sqpoll_cpu = use_signal(|| -1i32);
    let mut iopoll = use_signal(|| false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 3: OPTIMIZATION
    // ═══════════════════════════════════════════════════════════════
    let mut single_issuer = use_signal(|| true);
    let mut coop_taskrun = use_signal(|| false);
    let mut defer_taskrun = use_signal(|| false);
    let mut submit_all = use_signal(|| false);
    let mut taskrun_flag = use_signal(|| false);

    // ═══════════════════════════════════════════════════════════════
    // CATEGORY 4: ADVANCED
    // ═══════════════════════════════════════════════════════════════
    let mut r_disabled = use_signal(|| false);
    let mut attach_wq_fd = use_signal(|| -1i32);
    let mut dontfork = use_signal(|| false);

    // Status state
    let available = use_signal(|| false);
    let feature_enabled = use_signal(|| false);
    let backend = use_signal(|| String::from("--"));
    let stats_reads = use_signal(|| 0u64);
    let stats_writes = use_signal(|| 0u64);
    let stats_bytes_read = use_signal(|| 0u64);
    let stats_bytes_written = use_signal(|| 0u64);
    let stats_total_errors = use_signal(|| 0u64);
    let stats_loaded = use_signal(|| false);

    let loading = use_signal(|| true);
    let error = use_signal(|| Option::<String>::None);

    // Save state
    let mut saving = use_signal(|| false);
    let mut save_status = use_signal(|| Option::<String>::None);
    let mut save_error = use_signal(|| Option::<String>::None);

    // Restart state
    let mut show_restart_confirm = use_signal(|| false);
    let mut restart_msg: Signal<Option<String>> = use_signal(|| None);

    // Reset to defaults handler
    let reset_to_defaults = move |_| {
        // Category 1: Queue & Buffers
        ring_size.set(256);
        cq_size.set(0);
        buffer_size.set(65536);
        buffer_pool_size.set(64);
        clamp.set(false);
        // Category 2: Polling
        sqpoll.set(false);
        sqpoll_idle_ms.set(1000);
        sqpoll_cpu.set(-1);
        iopoll.set(false);
        // Category 3: Optimization
        single_issuer.set(true);
        coop_taskrun.set(false);
        defer_taskrun.set(false);
        submit_all.set(false);
        taskrun_flag.set(false);
        // Category 4: Advanced
        r_disabled.set(false);
        attach_wq_fd.set(-1);
        dontfork.set(false);
        // Clear any save status
        save_status.set(Some("Reset to defaults (not saved yet)".to_string()));
        save_error.set(None);
    };

    // Info modal signals - Category 1
    let mut show_ring_size_info = use_signal(|| false);
    let mut show_cq_size_info = use_signal(|| false);
    let mut show_buffer_size_info = use_signal(|| false);
    let mut show_buffer_pool_info = use_signal(|| false);
    let mut show_clamp_info = use_signal(|| false);

    // Info modal signals - Category 2
    let mut show_sqpoll_info = use_signal(|| false);
    let mut show_sqpoll_idle_info = use_signal(|| false);
    let mut show_sqpoll_cpu_info = use_signal(|| false);
    let mut show_iopoll_info = use_signal(|| false);

    // Info modal signals - Category 3
    let mut show_single_issuer_info = use_signal(|| false);
    let mut show_coop_taskrun_info = use_signal(|| false);
    let mut show_defer_taskrun_info = use_signal(|| false);
    let mut show_submit_all_info = use_signal(|| false);
    let mut show_taskrun_flag_info = use_signal(|| false);

    // Info modal signals - Category 4
    let mut show_r_disabled_info = use_signal(|| false);
    let mut show_attach_wq_fd_info = use_signal(|| false);
    let mut show_dontfork_info = use_signal(|| false);
    // Info modal - main Configuration header
    let mut show_config_info = use_signal(|| false);

    // Info modal signals - Status board
    let mut show_available_info = use_signal(|| false);
    let mut show_feature_enabled_info = use_signal(|| false);
    let mut show_backend_info = use_signal(|| false);
    let mut show_reads_info = use_signal(|| false);
    let mut show_writes_info = use_signal(|| false);
    let mut show_bytes_read_info = use_signal(|| false);
    let mut show_bytes_written_info = use_signal(|| false);

    // Load io_uring config on mount
    {
        let mut ring_size = ring_size;
        let mut cq_size = cq_size;
        let mut buffer_size = buffer_size;
        let mut buffer_pool_size = buffer_pool_size;
        let mut clamp = clamp;
        let mut sqpoll = sqpoll;
        let mut sqpoll_idle_ms = sqpoll_idle_ms;
        let mut sqpoll_cpu = sqpoll_cpu;
        let mut iopoll = iopoll;
        let mut single_issuer = single_issuer;
        let mut coop_taskrun = coop_taskrun;
        let mut defer_taskrun = defer_taskrun;
        let mut submit_all = submit_all;
        let mut taskrun_flag = taskrun_flag;
        let mut r_disabled = r_disabled;
        let mut attach_wq_fd = attach_wq_fd;
        let mut dontfork = dontfork;
        let mut available = available;
        let mut feature_enabled = feature_enabled;
        let mut backend = backend;
        let mut stats_reads = stats_reads;
        let mut stats_writes = stats_writes;
        let mut stats_bytes_read = stats_bytes_read;
        let mut stats_bytes_written = stats_bytes_written;
        let mut stats_total_errors = stats_total_errors;
        let mut stats_loaded = stats_loaded;
        let mut loading = loading;
        let mut error = error;

        // Get global page errors context
        let mut page_errors = use_context::<Signal<PageErrors>>();

        use_future(move || async move {
            loading.set(true);
            error.set(None);
            // Clear any previous page error for this page
            page_errors.with_mut(|e| e.clear_error("io_uring"));

            match api::fetch_io_uring_stats().await {
                Ok(resp) => {
                    available.set(resp.io_uring.available);
                    feature_enabled.set(resp.io_uring.feature_enabled);
                    backend.set(resp.io_uring.backend);
                    // Category 1
                    ring_size.set(resp.io_uring.config.ring_size);
                    cq_size.set(resp.io_uring.config.cq_size);
                    buffer_size.set(resp.io_uring.config.buffer_size);
                    buffer_pool_size.set(resp.io_uring.config.buffer_pool_size);
                    clamp.set(resp.io_uring.config.clamp);
                    // Category 2
                    sqpoll.set(resp.io_uring.config.sqpoll);
                    sqpoll_idle_ms.set(resp.io_uring.config.sqpoll_idle_ms);
                    sqpoll_cpu.set(resp.io_uring.config.sqpoll_cpu);
                    iopoll.set(resp.io_uring.config.iopoll);
                    // Category 3
                    single_issuer.set(resp.io_uring.config.single_issuer);
                    coop_taskrun.set(resp.io_uring.config.coop_taskrun);
                    defer_taskrun.set(resp.io_uring.config.defer_taskrun);
                    submit_all.set(resp.io_uring.config.submit_all);
                    taskrun_flag.set(resp.io_uring.config.taskrun_flag);
                    // Category 4
                    r_disabled.set(resp.io_uring.config.r_disabled);
                    attach_wq_fd.set(resp.io_uring.config.attach_wq_fd);
                    dontfork.set(resp.io_uring.config.dontfork);
                    // Stats
                    stats_reads.set(resp.io_uring.stats.reads);
                    stats_writes.set(resp.io_uring.stats.writes);
                    stats_bytes_read.set(resp.io_uring.stats.bytes_read);
                    stats_bytes_written.set(resp.io_uring.stats.bytes_written);
                    stats_total_errors.set(resp.io_uring.stats.total_errors);
                    stats_loaded.set(true);
                    // Clear global error on success
                    page_errors.with_mut(|e| e.clear_error("io_uring"));
                }
                Err(e) => {
                    let err_msg = format!("Failed to load io_uring config: {}", e);
                    error.set(Some(err_msg.clone()));
                    // Set global page error so status light turns red
                    page_errors.with_mut(|errs| errs.set_error("io_uring", &err_msg));
                    // Log error to backend
                    let _ = api::log_frontend_error("io_uring", &err_msg).await;
                }
            }
            loading.set(false);
        });
    }

    // Format bytes helper
    let format_bytes = |bytes: u64| -> String {
        if bytes >= 1024 * 1024 * 1024 {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        } else if bytes >= 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else if bytes >= 1024 {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        } else {
            format!("{} B", bytes)
        }
    };

    // Save handler
    let on_save = {
        move |_| {
            let ring_size_val = ring_size();
            let cq_size_val = cq_size();
            let buffer_size_val = buffer_size();
            let buffer_pool_size_val = buffer_pool_size();
            let clamp_val = clamp();
            let sqpoll_val = sqpoll();
            let sqpoll_idle_ms_val = sqpoll_idle_ms();
            let sqpoll_cpu_val = sqpoll_cpu();
            let iopoll_val = iopoll();
            let single_issuer_val = single_issuer();
            let coop_taskrun_val = coop_taskrun();
            let defer_taskrun_val = defer_taskrun();
            let submit_all_val = submit_all();
            let taskrun_flag_val = taskrun_flag();
            let r_disabled_val = r_disabled();
            let attach_wq_fd_val = attach_wq_fd();
            let dontfork_val = dontfork();

            spawn(async move {
                saving.set(true);
                save_status.set(None);
                save_error.set(None);

                let config = api::IoUringConfig {
                    ring_size: ring_size_val,
                    cq_size: cq_size_val,
                    buffer_size: buffer_size_val,
                    buffer_pool_size: buffer_pool_size_val,
                    clamp: clamp_val,
                    sqpoll: sqpoll_val,
                    sqpoll_idle_ms: sqpoll_idle_ms_val,
                    sqpoll_cpu: sqpoll_cpu_val,
                    iopoll: iopoll_val,
                    single_issuer: single_issuer_val,
                    coop_taskrun: coop_taskrun_val,
                    defer_taskrun: defer_taskrun_val,
                    submit_all: submit_all_val,
                    taskrun_flag: taskrun_flag_val,
                    r_disabled: r_disabled_val,
                    attach_wq_fd: attach_wq_fd_val,
                    dontfork: dontfork_val,
                };

                match api::save_io_uring_config(&config).await {
                    Ok(_) => {
                        save_status.set(Some("Saved! Restart required.".to_string()));
                    }
                    Err(e) => {
                        save_error.set(Some(e));
                    }
                }

                saving.set(false);
            });
        }
    };

    // File I/O health card (mirrors the card that used to live on /monitor overview)
    let health_status: &'static str = if !stats_loaded() {
        "Unknown"
    } else if stats_total_errors() > 0 {
        "Unhealthy"
    } else if backend() == "io_uring" {
        "Healthy"
    } else {
        "Degraded"
    };
    let health_detail = if !stats_loaded() {
        "API unreachable".to_string()
    } else {
        format!("{} | {}", backend(), format_bytes(stats_bytes_read()))
    };

    rsx! {
        div { class: "p-6 space-y-6 w-full",
            // Navigation
            ConfigNav { active: ConfigTab::IoUring }

            // File I/O health summary
            div { class: "grid grid-cols-1 md:grid-cols-3 lg:grid-cols-4 gap-4",
                HealthCard {
                    name: "File I/O".to_string().into(),
                    status: health_status.to_string().into(),
                    detail: Some(health_detail.into()),
                    info: Some("Async file I/O backend. 'io_uring' (Linux 5.1+) provides 2-3x faster reads. Falls back to 'tokio::fs' on older systems. 'Unhealthy' means I/O errors have been recorded since startup.".to_string().into()),
                    link: Some("/docu/index/io-uring".to_string().into()),
                }
            }

            if loading() {
                div { class: "flex items-center justify-center py-8",
                    span { class: "loading loading-spinner loading-lg text-primary" }
                }
            } else if let Some(err) = error() {
                div { class: "alert alert-error",
                    span { "{err}" }
                }
            } else {
                // Configuration Panel with save button in header
                div { class: "bg-gray-800 border border-gray-700 rounded-lg p-4 shadow",
                    // Header with title on left, save button on right
                    div { class: "flex items-start justify-between mb-3",
                        div { class: "flex items-center gap-2",
                            h3 { class: "text-sm font-semibold text-gray-200", "Configuration" }
                            button {
                                class: PARAM_ICON_BUTTON_CLASS,
                                style: PARAM_ICON_BUTTON_STYLE,
                                onclick: move |_| show_config_info.set(true),
                                title: "About io_uring",
                                InfoIcon {}
                            }
                        }
                        div { class: "flex items-center gap-3",
                            if let Some(msg) = save_status() {
                                span { class: "text-green-400 text-xs", "{msg}" }
                            }
                            if let Some(err) = save_error() {
                                span { class: "text-red-400 text-xs", "{err}" }
                            }
                            div { class: "flex items-center gap-2",
                                button {
                                    class: "btn btn-sm btn-ghost text-gray-400 hover:text-gray-200",
                                    onclick: reset_to_defaults,
                                    "Reset"
                                }
                                button {
                                    class: "btn btn-sm",
                                    style: "background-color: #1D6B9A; border-color: #1D6B9A; color: white;",
                                    onclick: on_save,
                                    disabled: saving(),
                                    if saving() { "Saving…" } else { "Save" }
                                }
                                button {
                                    class: "btn btn-sm btn-ghost text-gray-300 border border-gray-600",
                                    onclick: move |_| show_restart_confirm.set(true),
                                    "Restart to apply"
                                }
                                if let Some(msg) = restart_msg() {
                                    span { class: "text-xs text-yellow-400", "{msg}" }
                                }
                            }
                        }
                    }
                    // Content - boards + restart note on the right stretching full height
                    div { class: "text-gray-100 text-xs",
                    div { class: "flex gap-4 items-stretch",
                        // Boards container
                        div { class: "flex flex-wrap gap-4 items-stretch",

                        // ═══════════════════════════════════════════════════════════════
                        // STATUS BOARD
                        // ═══════════════════════════════════════════════════════════════
                        div { class: "rounded border border-gray-600 p-4 w-fit",
                            div { class: "flex items-center gap-2 mb-3",
                                span { class: "text-sm text-gray-300 font-semibold", "io_uring Status" }
                            }
                            div { class: "flex flex-wrap gap-6 justify-start",
                                // Status column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs mb-1", "Status" }
                                    div {
                                        class: "grid gap-x-1 gap-y-1 items-center text-xs",
                                        style: "grid-template-columns: max-content min-content auto;",
                                        label { class: PARAM_LABEL_CLASS, "available" }
                                        button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: move |_| show_available_info.set(true), InfoIcon {} }
                                        span { class: if available() { "text-green-400" } else { "text-red-400" },
                                            if available() { "✓ Yes" } else { "✗ No" }
                                        }
                                        label { class: PARAM_LABEL_CLASS, "feature_enabled" }
                                        button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: move |_| show_feature_enabled_info.set(true), InfoIcon {} }
                                        span { class: if feature_enabled() { "text-green-400" } else { "text-yellow-400" },
                                            if feature_enabled() { "✓ Yes" } else { "○ No" }
                                        }
                                        label { class: PARAM_LABEL_CLASS, "backend" }
                                        button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: move |_| show_backend_info.set(true), InfoIcon {} }
                                        span { class: "text-blue-400 font-mono", "{backend}" }
                                    }
                                }
                                // I/O column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs mb-1", "I/O" }
                                    div {
                                        class: "grid gap-x-1 gap-y-1 items-center text-xs",
                                        style: "grid-template-columns: max-content min-content auto;",
                                        label { class: PARAM_LABEL_CLASS, "reads" }
                                        button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: move |_| show_reads_info.set(true), InfoIcon {} }
                                        span { class: "text-gray-200", "{stats_reads}" }
                                        label { class: PARAM_LABEL_CLASS, "writes" }
                                        button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: move |_| show_writes_info.set(true), InfoIcon {} }
                                        span { class: "text-gray-200", "{stats_writes}" }
                                        label { class: PARAM_LABEL_CLASS, "bytes_read" }
                                        button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: move |_| show_bytes_read_info.set(true), InfoIcon {} }
                                        span { class: "text-gray-200", "{format_bytes(stats_bytes_read())}" }
                                        label { class: PARAM_LABEL_CLASS, "bytes_written" }
                                        button { class: PARAM_ICON_BUTTON_CLASS, style: PARAM_ICON_BUTTON_STYLE, onclick: move |_| show_bytes_written_info.set(true), InfoIcon {} }
                                        span { class: "text-gray-200", "{format_bytes(stats_bytes_written())}" }
                                    }
                                }
                            }
                        }

                        // ═══════════════════════════════════════════════════════════════
                        // CATEGORY 1: QUEUE & BUFFERS
                        // ═══════════════════════════════════════════════════════════════
                        div { class: "rounded border border-gray-600 p-4 w-fit",
                            div { class: "flex items-center gap-2 mb-3",
                                span { class: "text-sm text-gray-300 font-semibold", "1. Queue & Buffers" }
                            }
                            div { class: "flex flex-wrap gap-6 justify-start",
                                // Queue sizes column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "Queue Sizes" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "ring_size" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "number",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: "{ring_size}",
                                                onchange: move |evt| {
                                                    if let Ok(v) = evt.value().parse::<u32>() {
                                                        ring_size.set(v);
                                                    }
                                                },
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_ring_size_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "cq_size" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "number",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: "{cq_size}",
                                                onchange: move |evt| {
                                                    if let Ok(v) = evt.value().parse::<u32>() {
                                                        cq_size.set(v);
                                                    }
                                                },
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_cq_size_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                                // Buffer sizes column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "Buffers" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "buffer_size" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "number",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: "{buffer_size}",
                                                onchange: move |evt| {
                                                    if let Ok(v) = evt.value().parse::<usize>() {
                                                        buffer_size.set(v);
                                                    }
                                                },
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_buffer_size_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "buffer_pool_size" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "number",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: "{buffer_pool_size}",
                                                onchange: move |evt| {
                                                    if let Ok(v) = evt.value().parse::<usize>() {
                                                        buffer_pool_size.set(v);
                                                    }
                                                },
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_buffer_pool_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                                // Options column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "Options" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "clamp" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: clamp(),
                                                onchange: move |_| clamp.set(!clamp()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_clamp_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ═══════════════════════════════════════════════════════════════
                        // CATEGORY 2: POLLING
                        // ═══════════════════════════════════════════════════════════════
                        div { class: "rounded border border-gray-600 p-4 w-fit",
                            div { class: "flex items-center gap-2 mb-3",
                                span { class: "text-sm text-gray-300 font-semibold", "2. Polling" }
                            }
                            div { class: "flex flex-wrap gap-6 justify-start",
                                // SQ Poll column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "SQ Poll" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "sqpoll" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: sqpoll(),
                                                onchange: move |_| sqpoll.set(!sqpoll()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_sqpoll_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "sqpoll_idle_ms" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "number",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: "{sqpoll_idle_ms}",
                                                onchange: move |evt| {
                                                    if let Ok(v) = evt.value().parse::<u32>() {
                                                        sqpoll_idle_ms.set(v);
                                                    }
                                                },
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_sqpoll_idle_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "sqpoll_cpu" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "number",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: "{sqpoll_cpu}",
                                                onchange: move |evt| {
                                                    if let Ok(v) = evt.value().parse::<i32>() {
                                                        sqpoll_cpu.set(v);
                                                    }
                                                },
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_sqpoll_cpu_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                                // IO Poll column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "IO Poll" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "iopoll" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: iopoll(),
                                                onchange: move |_| iopoll.set(!iopoll()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_iopoll_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ═══════════════════════════════════════════════════════════════
                        // CATEGORY 3: OPTIMIZATION
                        // ═══════════════════════════════════════════════════════════════
                        div { class: "rounded border border-gray-600 p-4 w-fit",
                            div { class: "flex items-center gap-2 mb-3",
                                span { class: "text-sm text-gray-300 font-semibold", "3. Optimization" }
                            }
                            div { class: "flex flex-wrap gap-6 justify-start",
                                // Task running column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "Task Running" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "single_issuer" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: single_issuer(),
                                                onchange: move |_| single_issuer.set(!single_issuer()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_single_issuer_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "coop_taskrun" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: coop_taskrun(),
                                                onchange: move |_| coop_taskrun.set(!coop_taskrun()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_coop_taskrun_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "defer_taskrun" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: defer_taskrun(),
                                                onchange: move |_| defer_taskrun.set(!defer_taskrun()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_defer_taskrun_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                                // Submission column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "Submission" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "submit_all" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: submit_all(),
                                                onchange: move |_| submit_all.set(!submit_all()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_submit_all_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "taskrun_flag" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: taskrun_flag(),
                                                onchange: move |_| taskrun_flag.set(!taskrun_flag()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_taskrun_flag_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // ═══════════════════════════════════════════════════════════════
                        // CATEGORY 4: ADVANCED
                        // ═══════════════════════════════════════════════════════════════
                        div { class: "rounded border border-gray-600 p-4 w-fit",
                            div { class: "flex items-center gap-2 mb-3",
                                span { class: "text-sm text-gray-300 font-semibold", "4. Advanced" }
                                span { class: "text-xs text-gray-300 italic", "(use with caution)" }
                            }
                            div { class: "flex flex-wrap gap-6 justify-start",
                                // Setup column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "Setup" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "r_disabled" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: r_disabled(),
                                                onchange: move |_| r_disabled.set(!r_disabled()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_r_disabled_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "dontfork" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "checkbox",
                                                class: CHECKBOX_CLASS,
                                                checked: dontfork(),
                                                onchange: move |_| dontfork.set(!dontfork()),
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_dontfork_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                                // Worker pool column
                                div { class: PARAM_COLUMN_CLASS,
                                    span { class: "text-gray-300 font-semibold text-xs", "Worker Pool" }
                                    div { class: PARAM_BLOCK_CLASS,
                                        label { class: PARAM_LABEL_CLASS, "attach_wq_fd" }
                                        div { class: "flex items-end gap-2",
                                            input {
                                                r#type: "number",
                                                class: PARAM_NUMBER_INPUT_CLASS,
                                                value: "{attach_wq_fd}",
                                                onchange: move |evt| {
                                                    if let Ok(v) = evt.value().parse::<i32>() {
                                                        attach_wq_fd.set(v);
                                                    }
                                                },
                                            }
                                            button {
                                                class: PARAM_ICON_BUTTON_CLASS,
                                                style: PARAM_ICON_BUTTON_STYLE,
                                                onclick: move |_| show_attach_wq_fd_info.set(true),
                                                InfoIcon {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        }
                    }
                    }
                }

                // Enable Instructions
                if !feature_enabled() {
                    div { class: "alert alert-warning mt-4",
                        svg { class: "w-6 h-6", fill: "none", view_box: "0 0 24 24", stroke: "currentColor",
                            path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" }
                        }
                        div {
                            h3 { class: "font-bold", "io_uring feature not enabled" }
                            p { class: "text-sm",
                                "Build with: "
                                code { class: "bg-gray-800 px-1 rounded", "cargo build --features io_uring" }
                            }
                        }
                    }
                }

                // Info modals - Category 1
                if show_ring_size_info() {
                    {info_modal("ring_size", show_ring_size_info, vec![
                        "Controls the size of the io_uring submission queue.",
                        "Valid range: 1-32768, must be a power of 2.",
                        "Larger values allow more concurrent I/O operations but use more memory.",
                        "Default: 256 entries."
                    ])}
                }
                if show_cq_size_info() {
                    {info_modal("cq_size", show_cq_size_info, vec![
                        "Controls the size of the completion queue separately from the submission queue.",
                        "Set to 0 for automatic sizing (2x ring_size).",
                        "Must be greater than ring_size if specified."
                    ])}
                }
                if show_buffer_size_info() {
                    {info_modal("buffer_size", show_buffer_size_info, vec![
                        "Size of the buffer used for each read/write operation.",
                        "Valid range: 4096 bytes (4KB) to 16MB.",
                        "Default: 65536 bytes (64KB)."
                    ])}
                }
                if show_buffer_pool_info() {
                    {info_modal("buffer_pool_size", show_buffer_pool_info, vec![
                        "Number of pre-allocated buffers in the buffer pool.",
                        "Valid range: 1-4096 buffers.",
                        "Default: 64 buffers."
                    ])}
                }
                if show_clamp_info() {
                    {info_modal("clamp", show_clamp_info, vec![
                        "Clamp queue sizes to their maximum values instead of returning an error.",
                        "When enabled, oversized queue requests are silently reduced to the maximum.",
                        "Default: false."
                    ])}
                }

                // Info modals - Category 2
                if show_sqpoll_info() {
                    {info_modal("sqpoll", show_sqpoll_info, vec![
                        "When enabled, a kernel thread continuously polls the submission queue.",
                        "Reduces syscall overhead but uses CPU even when idle.",
                        "Default: false."
                    ])}
                }
                if show_sqpoll_idle_info() {
                    {info_modal("sqpoll_idle_ms", show_sqpoll_idle_info, vec![
                        "How long the SQ poll kernel thread waits before going to sleep.",
                        "Only relevant when SQPOLL is enabled.",
                        "Default: 1000ms."
                    ])}
                }
                if show_sqpoll_cpu_info() {
                    {info_modal("sqpoll_cpu", show_sqpoll_cpu_info, vec![
                        "Pin the SQ poll kernel thread to a specific CPU core.",
                        "Set to -1 for no affinity (kernel decides).",
                        "Only relevant when SQPOLL is enabled."
                    ])}
                }
                if show_iopoll_info() {
                    {info_modal("iopoll", show_iopoll_info, vec![
                        "Enable busy-wait polling for I/O completion events.",
                        "Reduces latency but increases CPU usage significantly.",
                        "Only works with files opened with O_DIRECT.",
                        "Default: false."
                    ])}
                }

                // Info modals - Category 3
                if show_single_issuer_info() {
                    {info_modal("single_issuer", show_single_issuer_info, vec![
                        "Optimization hint that only one thread will submit to this ring.",
                        "Enables internal optimizations in the kernel (available since 6.0).",
                        "Default: true."
                    ])}
                }
                if show_coop_taskrun_info() {
                    {info_modal("coop_taskrun", show_coop_taskrun_info, vec![
                        "Reduces inter-processor interrupts when completions arrive.",
                        "Completions are processed at kernel/user transitions instead of immediately.",
                        "Available since kernel 5.19. Default: false."
                    ])}
                }
                if show_defer_taskrun_info() {
                    {info_modal("defer_taskrun", show_defer_taskrun_info, vec![
                        "Defer all work until an explicit io_uring_enter() call.",
                        "Requires SINGLE_ISSUER to be enabled.",
                        "Available since kernel 6.1. Default: false."
                    ])}
                }
                if show_submit_all_info() {
                    {info_modal("submit_all", show_submit_all_info, vec![
                        "Continue submitting requests even if one encounters an error.",
                        "Normally io_uring stops the batch on first error.",
                        "Available since kernel 5.18. Default: false."
                    ])}
                }
                if show_taskrun_flag_info() {
                    {info_modal("taskrun_flag", show_taskrun_flag_info, vec![
                        "Sets IORING_SQ_TASKRUN flag when completions are pending.",
                        "Used with COOP_TASKRUN for efficient peek-style completion checking.",
                        "Available since kernel 5.19. Default: false."
                    ])}
                }

                // Info modals - Category 4
                if show_r_disabled_info() {
                    {info_modal("r_disabled", show_r_disabled_info, vec![
                        "Start the io_uring instance with rings disabled.",
                        "Allows registering restrictions, buffers, and files before processing starts.",
                        "Available since kernel 5.10. Default: false."
                    ])}
                }
                if show_attach_wq_fd_info() {
                    {info_modal("attach_wq_fd", show_attach_wq_fd_info, vec![
                        "Share the async worker thread pool with another io_uring instance.",
                        "Set to the file descriptor of another ring, or -1 to disable.",
                        "Available since kernel 5.6. Default: -1."
                    ])}
                }
                if show_dontfork_info() {
                    {info_modal("dontfork", show_dontfork_info, vec![
                        "Prevent ring memory from being inherited by forked processes.",
                        "Useful for security isolation in applications that fork.",
                        "Note: Not directly supported by tokio_uring. Default: false."
                    ])}
                }
            }
        }
        // io_uring info modal
        if show_config_info() {
            div {
                class: "fixed inset-0 bg-gray-800",
                style: "top: 2.5rem; z-index: 50; overflow-y: auto; overscroll-behavior: contain;",

                div { class: "px-4 py-4 w-full pb-20",
                    div { class: "flex justify-between items-start mb-2",
                        h2 { class: "text-xl font-bold text-white",
                            "io_uring: A Unified Async I/O API for Linux"
                        }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_config_info.set(false),
                            "\u{2715}"
                        }
                    }

                    div { class: "grid grid-cols-1 lg:grid-cols-3 gap-2 mb-2",
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "What is io_uring?" }
                            p { class: "text-xs text-gray-300 mb-2",
                                "Linux kernel interface (5.1+) for async I/O:"
                            }
                            ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-0.5",
                                li { "One API for all I/O types" }
                                li { "Zero/minimal syscalls" }
                                li { "True async (not thread pools)" }
                                li { "Batching of operations" }
                            }
                            p { class: "text-xs text-yellow-300 mt-2",
                                "\u{2b50} File I/O (doc ingestion, index loading) is where io_uring helps most!"
                            }
                        }

                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Before (Fragmented)" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "Files:   AIO         - Limited\nSockets: epoll       - Different API\nTimers:  timerfd     - Yet another\nSignals: signalfd    - And another\n\n\u{274c} Each I/O = different API\n\u{274c} Can\u{2019}t batch mixed ops"
                            }
                        }

                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "With io_uring (Unified)" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "Files \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}\nSockets \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524} io_uring \u{2500}\u{25ba} CQ\nTimers \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524} (One API)\nSignals \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n\n\u{2705} One API for everything\n\u{2705} Batch N ops in 1 syscall\n\u{2705} True kernel-level async"
                            }
                        }
                    }

                    div { class: "grid grid-cols-1 lg:grid-cols-2 gap-2 mb-2",
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Architecture" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "USER SPACE              KERNEL SPACE\n\n\u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}        \u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}\n\u{2502} Submission Q  \u{2502}\u{25c4}\u{2500}shared\u{2500}\u{25ba}\u{2502}  io_uring   \u{2502}\n\u{2502}     (SQ)      \u{2502} memory  \u{2502}   kernel    \u{2502}\n\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}        \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n       \u{2502} submit                 \u{2502}\n       \u{25bc}                        \u{2502} complete\n\u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}               \u{2502}\n\u{2502} Completion Q  \u{2502}\u{25c4}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n\u{2502}     (CQ)      \u{2502} shared memory\n\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}"
                            }
                        }

                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Performance" }
                            div { class: "text-[10px] text-gray-300 space-y-0.5",
                                p {
                                    span { class: "text-gray-400", "Syscalls/IO: " }
                                    "epoll 1-2 \u{2192} io_uring 0-1 (batched)"
                                }
                                p {
                                    span { class: "text-gray-400", "File async: " }
                                    "epoll Fake \u{2192} io_uring True"
                                }
                                p {
                                    span { class: "text-gray-400", "Batching: " }
                                    "epoll No \u{2192} io_uring Yes"
                                }
                                p {
                                    span { class: "text-gray-400", "Zero-copy: " }
                                    "epoll Limited \u{2192} io_uring Yes"
                                }
                                p {
                                    span { class: "text-gray-400", "CPU: " }
                                    "io_uring 30-50% lower"
                                }
                            }
                            p { class: "text-[10px] text-green-400 mt-2",
                                "Benchmark: epoll ~400k ops/s \u{2192} io_uring ~800k ops/s (2x)"
                            }
                        }
                    }

                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_config_info.set(false),
                        "Got it!"
                    }
                }
            }
        }

        // ── Restart confirm modal ─────────────────────────────────────────────
        if show_restart_confirm() {
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_restart_confirm.set(false),
                div {
                    class: "bg-gray-900 border border-gray-700 rounded-lg p-6 w-80 shadow-xl",
                    onclick: move |evt| evt.stop_propagation(),
                    h2 { class: "text-base font-bold text-gray-100 mb-2", "Restart app?" }
                    p { class: "text-sm text-gray-300 mb-4",
                        "The app will restart to apply io_uring config changes. Active requests will be dropped."
                    }
                    div { class: "flex gap-2",
                        button {
                            class: "btn btn-sm flex-1",
                            style: "background-color:#7C2A02;border:1px solid #7C2A02;color:white;",
                            onclick: move |_| {
                                show_restart_confirm.set(false);
                                spawn(async move {
                                    match api::restart_service().await {
                                        Ok(()) => restart_msg.set(Some("Restarting…".into())),
                                        Err(e) => restart_msg.set(Some(format!("Error: {}", e))),
                                    }
                                });
                            },
                            "Yes, restart"
                        }
                        button {
                            class: "btn btn-sm flex-1 btn-ghost text-gray-300",
                            onclick: move |_| show_restart_confirm.set(false),
                            "Cancel"
                        }
                    }
                }
            }
        }

        // ── Status board info modals ──────────────────────────────────────────
        if show_available_info() {
            {info_modal("available", show_available_info, vec![
                "Whether the io_uring subsystem is present and usable on this machine.",
                "io_uring requires Linux 5.1 or later. The kernel must have been compiled with CONFIG_IO_URING=y. This flag is set at startup by probing the kernel — it cannot be changed at runtime.",
                "✓ Yes — io_uring syscalls are available and ag can use them for file I/O.",
                "✗ No — the kernel is too old or the feature was compiled out. ag falls back to standard tokio async I/O (tokio::fs). All functionality still works; only the I/O performance characteristic changes.",
            ])}
        }
        if show_feature_enabled_info() {
            {info_modal("feature_enabled", show_feature_enabled_info, vec![
                "Whether io_uring has been switched on in ag's configuration.",
                "Even when the kernel supports io_uring (available = Yes), ag only uses it when this flag is also true. This lets you disable io_uring without changing the kernel — useful for debugging I/O issues or comparing performance.",
                "○ No — ag is using tokio::fs for all file operations regardless of kernel support.",
                "✓ Yes — ag will use io_uring for file I/O when available = Yes. The active backend field confirms which path is actually running.",
            ])}
        }
        if show_backend_info() {
            {info_modal("backend", show_backend_info, vec![
                "The file I/O backend that is currently active.",
                "\"io_uring\" — ag is using the io_uring submission/completion ring for all file reads and writes. This is the fast path: fewer syscalls, lower CPU overhead, and higher throughput on Linux 5.1+.",
                "\"tokio::fs\" — ag is using the standard tokio async file I/O, which dispatches blocking calls onto a thread pool. This is the fallback when io_uring is unavailable or disabled.",
                "The backend is determined at startup from the combination of available and feature_enabled. Changing feature_enabled in config and restarting will switch the backend.",
            ])}
        }
        if show_reads_info() {
            {info_modal("reads", show_reads_info, vec![
                "Total number of read operations completed since ag started.",
                "Each time ag reads a chunk, index file, or document from disk, this counter increments by one regardless of how many bytes were transferred.",
                "A high read count relative to writes is normal for a search system — documents are written once during indexing but may be read many times during retrieval.",
            ])}
        }
        if show_writes_info() {
            {info_modal("writes", show_writes_info, vec![
                "Total number of write operations completed since ag started.",
                "Writes occur when ag persists index updates, flushes Tantivy segments, or saves vector data to disk. Each write call increments this counter once.",
                "Writes are typically much less frequent than reads. A spike in writes usually corresponds to a reindex or a batch upload completing.",
            ])}
        }
        if show_bytes_read_info() {
            {info_modal("bytes_read", show_bytes_read_info, vec![
                "Total bytes transferred from disk to memory by the I/O backend since ag started.",
                "This accumulates across all read operations: index lookups, chunk retrievals, and any file reads performed during search or reindex.",
                "High bytes_read with low read count means ag is reading large chunks per call — generally efficient. High read count with low bytes_read means many small reads, which io_uring handles better than the thread-pool fallback.",
            ])}
        }
        if show_bytes_written_info() {
            {info_modal("bytes_written", show_bytes_written_info, vec![
                "Total bytes flushed from memory to disk by the I/O backend since ag started.",
                "This accumulates across all write operations: Tantivy segment flushes, vector store saves, and any other persistence writes.",
                "Bytes written grows in steps rather than smoothly — Tantivy batches changes and flushes periodically, so you will see flat stretches followed by jumps during or after heavy indexing.",
            ])}
        }

    }
}
