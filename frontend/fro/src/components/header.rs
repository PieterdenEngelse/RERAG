use crate::api;
use crate::app::{ClearChat, Route, ShowHelpCommands, ShowRagInfo};
use crate::components::dark_mode_toggle::DarkModeToggle;
use crate::components::nav_dropdown::{DropdownActionItem, DropdownItem, NavDropdown};
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

#[component]
pub fn Header() -> Element {
    let mut menu_open = use_signal(|| false);
    let mut health_status = use_signal(|| "unknown".to_string());
    let mut show_status_info = use_signal(|| false);
    let current_route = use_route::<Route>();

    let is_dark = use_context::<Signal<bool>>();
    let mut show_help = use_context::<Signal<ShowHelpCommands>>();
    let mut show_rag_info = use_context::<Signal<ShowRagInfo>>();
    let mut clear_chat = use_context::<Signal<ClearChat>>();

    let header_bg = "bg-gray-900";

    use_future(move || async move {
        loop {
            match api::health_check().await {
                Ok(resp) => health_status.set(resp.status),
                Err(_) => health_status.set("offline".to_string()),
            }
            gloo_timers::future::TimeoutFuture::new(5000).await;
        }
    });

    rsx! {
        header { class: "sticky top-0 shadow-md py-0 px-0.5 z-50 transition-colors {header_bg} flex items-center relative",

            // Rust icon
            div {
                class: "ml-2",
                img {
                    src: asset!("/assets/rusticon_1.png"),
                    alt: "Rust Icon",
                    class: "h-8 w-8",
                }
            }

            // Status light - 0.5cm to the right, centered vertically
            {
                let status = health_status();
                let bg_color = match status.as_str() {
                    "healthy" | "ok" => "bg-green-500",
                    "degraded" => "bg-yellow-500",
                    "offline" | "unhealthy" => "bg-red-500",
                    "unknown" => "bg-gray-500",
                    _ => "bg-orange-500",
                };
                rsx! {
                    div { class: "flex items-center gap-1",
                        div {
                            class: "ml-2 w-4 h-4 rounded-full border-2 border-gray-900 {bg_color}",
                            title: format!("Status: {}", status),
                        }
                        // Info button for status explanation
                        button {
                            class: "shrink-0 rounded flex items-center justify-center cursor-pointer",
                            style: "width: 1.5rem; height: 1.5rem; min-width: 1.5rem; min-height: 1.5rem; background-color: #1D6B9A; border: 1px solid #1D6B9A;",
                            onclick: move |_| show_status_info.set(true),
                            title: "Status info",
                            svg {
                                class: "w-4 h-4 text-white",
                                view_box: "0 0 20 20",
                                fill: "none",
                                stroke: "currentColor",
                                circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
                                line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
                                circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                            }
                        }
                    }
                }
            }

            // Title - absolutely positioned to center on full viewport width
            {
                rsx! {
                    div {
                        class: "absolute inset-x-0 flex justify-center pointer-events-none",
                        h1 {
                            class: "font-medium text-center text-white pointer-events-auto",
                            style: "font-family: ui-sans-serif, system-ui, sans-serif; font-size: 0.975rem;",
                            "Rust Agentic Retrieval Augumented Generation"
                        }
                    }
                }
            }

            div { class: "flex-1" }

            div { class: "flex justify-end items-center",

                nav {
                    class: "hidden md:flex items-center gap-3 text-sm",
                    style: "font-family: ui-sans-serif, system-ui, sans-serif;",
                    {
                        let home_color = if matches!(current_route, Route::Home {}) {
                            "#1D6B9A"
                        } else if is_dark() {
                            "white"
                        } else {
                            "#111827"
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
                        let monitor_color = if matches!(current_route, Route::MonitorOverview {} | Route::MonitorAgentic {} | Route::MonitorRequests {} | Route::MonitorCache {} | Route::MonitorIndex {} | Route::MonitorRateLimits {} | Route::MonitorLogs {}) {
                            "#1D6B9A"
                        } else if is_dark() {
                            "white"
                        } else {
                            "#111827"
                        };
                        rsx! {
                            Link {
                                to: Route::MonitorOverview {},
                                class: "py-2 px-3 rounded-lg transition-colors font-medium",
                                style: format!("color: {};", monitor_color),
                                "Monitor"
                            }
                        }
                    }
                    {
                        let config_color = if matches!(current_route, Route::Config {} | Route::ConfigHardware {} | Route::ConfigSampling {} | Route::ConfigPrompt {} | Route::ConfigOther {} | Route::Parameters {}) {
                            "#1D6B9A"
                        } else if is_dark() {
                            "white"
                        } else {
                            "#111827"
                        };
                        rsx! {
                            Link {
                                to: Route::Config {},
                                class: "py-2 px-3 rounded-lg transition-colors font-medium",
                                style: format!("color: {};", config_color),
                                "Config"
                            }
                        }
                    }
                    {
                        let train_color = if matches!(current_route, Route::Train {}) {
                            "#1D6B9A"
                        } else if is_dark() {
                            "white"
                        } else {
                            "#111827"
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
                    NavDropdown { title: "Help".to_string(),
                        DropdownActionItem { onclick: move |_| show_help.set(ShowHelpCommands(true)), "/help commands" }
                        DropdownItem { to: Route::Home {}, "Design" }
                        DropdownItem { to: Route::Home {}, "Consulting" }
                    }
                    NavDropdown { title: "About".to_string(),
                        DropdownActionItem { onclick: move |_| show_rag_info.set(ShowRagInfo(true)), "RAG" }
                        DropdownItem { to: Route::About {}, "Company" }
                        DropdownItem { to: Route::About {}, "Contact" }
                    }
                }
                DarkModeToggle {}
                button {
                    class: "md:hidden p-2 text-2xl",
                    onclick: move |_| menu_open.set(!menu_open()),
                    "☰"
                }
            }

            if menu_open() {
                div { class: "md:hidden mt-4 pb-4 flex flex-col gap-4",
                    Link { to: Route::Home {}, class: "text-teal-200 hover:text-white transition-colors", onclick: move |_| { clear_chat.set(ClearChat(true)); menu_open.set(false); }, "Home" }
                    Link { to: Route::MonitorOverview {}, class: "text-teal-100 hover:text-white transition-colors", onclick: move |_| menu_open.set(false), "Monitor" }
                    Link { to: Route::Config {}, class: "text-teal-100 hover:text-white transition-colors", onclick: move |_| menu_open.set(false), "Config" }
                    Link { to: Route::Train {}, class: "text-teal-100 hover:text-white transition-colors", onclick: move |_| menu_open.set(false), "Train" }
                    Link { to: Route::About {}, class: "hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors", onclick: move |_| menu_open.set(false), "About" }
                    button {
                        class: "text-left text-teal-100 hover:text-white transition-colors",
                        onclick: move |_| {
                            show_help.set(ShowHelpCommands(true));
                            menu_open.set(false);
                        },
                        "/help commands"
                    }
                    DarkModeToggle {}
                }
            }
        }

        // Status Info Modal
        if show_status_info() {
            div {
                class: "fixed inset-0 z-[100] flex items-center justify-center bg-black/60",
                onclick: move |_| show_status_info.set(false),
                div {
                    class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-md shadow-xl",
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
                        "The status light shows the health of the backend server:"
                    }
                    div { class: "space-y-3",
                        // Healthy
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-green-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-green-400 font-medium", "Green" }
                                span { class: "text-gray-400 text-sm ml-2", "— All components healthy" }
                            }
                        }
                        // Degraded
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-yellow-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-yellow-400 font-medium", "Yellow" }
                                span { class: "text-gray-400 text-sm ml-2", "— Some component degraded" }
                            }
                        }
                        // Offline/Unhealthy
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-red-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-red-400 font-medium", "Red" }
                                span { class: "text-gray-400 text-sm ml-2", "— Backend down or unhealthy" }
                            }
                        }
                        // Unknown
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-gray-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-gray-300 font-medium", "Gray" }
                                span { class: "text-gray-400 text-sm ml-2", "— Initial state before first check" }
                            }
                        }
                        // Other
                        div { class: "flex items-center gap-3",
                            div { class: "w-4 h-4 rounded-full bg-orange-500 border-2 border-gray-700" }
                            div {
                                span { class: "text-orange-400 font-medium", "Orange" }
                                span { class: "text-gray-400 text-sm ml-2", "— Unexpected status value" }
                            }
                        }
                    }
                    div { class: "mt-4 pt-3 border-t border-gray-700 text-xs text-gray-500",
                        "Current status: "
                        span { class: "font-mono text-gray-300", "{health_status()}" }
                    }
                    button {
                        class: "btn btn-primary btn-sm mt-4 w-full",
                        onclick: move |_| show_status_info.set(false),
                        "Got it!"
                    }
                }
            }
        }
    }
}
