use crate::api;
use crate::app::{ClearChat, Route, ShowHelpCommands, ShowRagInfo};
use crate::components::dark_mode_toggle::DarkModeToggle;
use crate::components::nav_dropdown::{DropdownActionItem, DropdownItem, NavDropdown};
use dioxus::prelude::*;

#[component]
pub fn Header() -> Element {
    let mut menu_open = use_signal(|| false);
    let mut health_status = use_signal(|| "unknown".to_string());
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
            div {
                class: "ml-2 w-4 h-4 rounded-full border-2 border-gray-900 bg-success",
                title: format!("Status: {}", health_status()),
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
    }
}
