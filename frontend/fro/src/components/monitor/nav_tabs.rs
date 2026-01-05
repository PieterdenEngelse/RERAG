use crate::app::Route;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct NavTabsProps {
    pub active: Route,
}

#[component]
pub fn NavTabs(props: NavTabsProps) -> Element {
    let tabs = vec![
        ("Overview", Route::MonitorOverview {}),
        ("Agentic", Route::MonitorAgentic {}),
        ("Requests", Route::MonitorRequests {}),
        ("Cache", Route::MonitorCache {}),
        ("Index", Route::MonitorIndex {}),
        ("Rate Limits", Route::MonitorRateLimits {}),
        ("Logs", Route::MonitorLogs {}),
    ];

    rsx! {
        nav { class: "flex flex-wrap gap-4 text-sm text-gray-400",
            for (label, route) in tabs {
                Link {
                    to: route.clone(),
                    class: if route == props.active {
                        "text-teal-400 border-b-2 border-teal-400 pb-1"
                    } else {
                        "hover:text-teal-300"
                    },
                    {label}
                }
            }
        }
    }
}
