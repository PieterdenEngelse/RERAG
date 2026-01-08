use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::prelude::Link;

#[derive(Clone, PartialEq)]
pub enum ConfigTab {
    Home,
    Sampling,
    Prompt,
    Hardware,
    Other,
}

#[derive(Props, Clone, PartialEq)]
pub struct ConfigNavProps {
    pub active: ConfigTab,
}

#[component]
pub fn ConfigNav(props: ConfigNavProps) -> Element {
    let tabs = vec![
        ("Rag&Agent", Route::Config {}, ConfigTab::Home),
        ("Sampling", Route::ConfigSampling {}, ConfigTab::Sampling),
        ("Prompt", Route::ConfigPrompt {}, ConfigTab::Prompt),
        (
            "Hardware & performance",
            Route::ConfigHardware {},
            ConfigTab::Hardware,
        ),
        ("Other", Route::ConfigOther {}, ConfigTab::Other),
    ];

    rsx! {
        nav { class: "flex flex-wrap gap-4 text-sm text-gray-400",
            for (label, route, tab_id) in tabs {
                {
                    let is_active = props.active == tab_id;
                    rsx! {
                        Link {
                            to: route,
                            class: if is_active {
                                "text-white border-b-2 border-white pb-1"
                            } else {
                                "hover:text-white"
                            },
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}
