use crate::components::header::Header;
use crate::pages::{
    About, Config, ConfigHardware, ConfigOther, ConfigPrompt, ConfigSampling, Home, MonitorAgentic,
    MonitorCache, MonitorIndex, MonitorLogs, MonitorOverview, MonitorRateLimits, MonitorRequests,
    PageNotFound, Parameters,
};
use dioxus::prelude::*;

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Layout)]
        #[route("/")]
        Home {},
        #[route("/about")]
        About {},
        #[route("/monitor")]
        MonitorOverview {},
        #[route("/monitor/agentic")]
        MonitorAgentic {},
        #[route("/config")]
        Config {},
        #[route("/config/parameters")]
        Parameters {},
        #[route("/config/sampling")]
        ConfigSampling {},
        #[route("/config/prompt")]
        ConfigPrompt {},
        #[route("/config/hardware")]
        ConfigHardware {},
        #[route("/config/other")]
        ConfigOther {},
        #[route("/monitor/requests")]
        MonitorRequests {},
        #[route("/monitor/cache")]
        MonitorCache {},
        #[route("/monitor/index")]
        MonitorIndex {},
        #[route("/monitor/rate-limits")]
        MonitorRateLimits {},
        #[route("/monitor/logs")]
        MonitorLogs {},
    #[end_layout]
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },
}

#[component]
pub fn App() -> Element {
    use_context_provider(|| Signal::new(false));

    rsx! {
        document::Link { rel: "icon", href: asset!("/assets/favicon.ico") }
        document::Link { rel: "stylesheet", href: asset!("/assets/styling/output.css") }

        Router::<Route> {}
    }
}

#[component]
fn Layout() -> Element {
    let is_dark = use_context::<Signal<bool>>();

    // Apply dark class on mount and when toggled
    use_effect(use_reactive!(|is_dark| {
        let dark_mode = is_dark();
        web_sys::console::log_1(&format!("Dark mode effect running: {}", dark_mode).into());

        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(html) = document.document_element() {
                    let class_list = html.class_list();
                    if dark_mode {
                        web_sys::console::log_1(&"Adding dark class".into());
                        let _ = class_list.add_1("dark");
                    } else {
                        web_sys::console::log_1(&"Removing dark class".into());
                        let _ = class_list.remove_1("dark");
                    }
                    web_sys::console::log_1(&format!("HTML classes: {}", html.class_name()).into());
                }
            }
        }
    }));

    rsx! {
        div {
            class: "min-h-screen transition-colors bg-white dark:bg-gray-900 text-gray-900 dark:text-white",

            Header {},

            main {
                Outlet::<Route> {}
            }
        }
    }
}
