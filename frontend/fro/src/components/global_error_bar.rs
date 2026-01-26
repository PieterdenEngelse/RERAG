use crate::api;
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

/// Global health errors that can be displayed
/// These match the conditions that trigger red status lights
#[derive(Clone, Default, PartialEq)]
pub struct GlobalHealthErrors {
    /// Backend health API returned unhealthy/offline or is unreachable
    pub backend_error: Option<String>,
    /// io_uring has I/O errors or API unreachable
    pub io_uring_error: Option<String>,
}

impl GlobalHealthErrors {
    pub fn has_errors(&self) -> bool {
        self.backend_error.is_some() || self.io_uring_error.is_some()
    }

    pub fn error_list(&self) -> Vec<(&'static str, &str)> {
        let mut errors = Vec::new();
        if let Some(ref e) = self.backend_error {
            errors.push(("Backend", e.as_str()));
        }
        if let Some(ref e) = self.io_uring_error {
            errors.push(("File I/O", e.as_str()));
        }
        errors
    }
}

/// Global error bar component that polls health APIs and shows errors
/// Shows the same errors that trigger red status lights anywhere in the app
#[component]
pub fn GlobalErrorBar() -> Element {
    let errors = use_signal(GlobalHealthErrors::default);
    let mut dismissed = use_signal(|| false);

    // Poll health APIs every 10 seconds
    {
        let mut errors = errors.clone();
        let mut dismissed = dismissed.clone();
        use_future(move || async move {
            loop {
                let mut new_errors = GlobalHealthErrors::default();

                // Check main health API - same as header status light
                match api::health_check().await {
                    Ok(health) => {
                        // Red status triggers: "offline" or "unhealthy"
                        match health.status.as_str() {
                            "offline" => {
                                new_errors.backend_error = Some("offline".to_string());
                            }
                            "unhealthy" => {
                                new_errors.backend_error = Some("unhealthy".to_string());
                            }
                            _ => {} // healthy, degraded, etc. are not errors
                        }
                    }
                    Err(e) => {
                        new_errors.backend_error = Some(format!("unreachable: {}", e));
                    }
                }

                // Check io_uring API - same as File I/O health card
                match api::fetch_io_uring_stats().await {
                    Ok(stats) => {
                        // Red status triggers: total_errors > 0
                        if stats.io_uring.stats.total_errors > 0 {
                            new_errors.io_uring_error = Some(format!(
                                "{} errors ({})",
                                stats.io_uring.stats.total_errors, stats.io_uring.backend
                            ));
                        }
                    }
                    Err(_) => {
                        new_errors.io_uring_error = Some("API unreachable".to_string());
                    }
                }

                // If errors changed, reset dismissed state so new errors show
                let current = errors.read().clone();
                if new_errors != current {
                    dismissed.set(false);
                }

                errors.set(new_errors);

                // Poll every 10 seconds
                TimeoutFuture::new(10_000).await;
            }
        });
    }

    let current_errors = errors.read();

    // Don't show if no errors or dismissed
    if !current_errors.has_errors() || dismissed() {
        return rsx! {};
    }

    let error_list = current_errors.error_list();

    rsx! {
        div {
            class: "fixed top-0 left-0 right-0 z-[100] bg-red-900/95 border-b border-red-700 px-4 py-2 flex items-center justify-between",
            div { class: "flex items-center gap-4 flex-wrap",
                // Error icon
                svg {
                    class: "w-5 h-5 text-red-400 shrink-0",
                    fill: "none",
                    view_box: "0 0 24 24",
                    stroke: "currentColor",
                    stroke_width: "2",
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        d: "M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                    }
                }
                // Error messages
                div { class: "flex items-center gap-3 text-sm",
                    for (name, msg) in error_list {
                        span { class: "flex items-center gap-1",
                            span { class: "font-semibold text-red-300", "{name}:" }
                            span { class: "text-red-200", "{msg}" }
                        }
                        span { class: "text-red-600 last:hidden", "|" }
                    }
                }
            }
            // Dismiss button
            button {
                class: "text-red-400 hover:text-red-200 p-1",
                onclick: move |_| dismissed.set(true),
                title: "Dismiss (errors will reappear on next poll if still present)",
                svg {
                    class: "w-4 h-4",
                    fill: "none",
                    view_box: "0 0 24 24",
                    stroke: "currentColor",
                    stroke_width: "2",
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        d: "M6 18L18 6M6 6l12 12"
                    }
                }
            }
        }
    }
}
