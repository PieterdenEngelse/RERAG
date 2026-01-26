use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[derive(Props, Clone, PartialEq)]
pub struct NavTabsProps {
    pub active: Route,
}

#[component]
pub fn NavTabs(props: NavTabsProps) -> Element {
    let tabs = vec![
        ("Overview", Route::MonitorOverview {}),
        ("Agentic", Route::MonitorAgentic {}),
        ("Tools", Route::MonitorTools {}),
        ("Requests", Route::MonitorRequests {}),
        ("Cache", Route::MonitorCache {}),
        ("Index", Route::MonitorIndex {}),
        ("RAG", Route::MonitorRag {}),
        ("Agent", Route::MonitorObservations {}),
        ("Rate Limits", Route::MonitorRateLimits {}),
        ("Logs", Route::MonitorLogs {}),
        ("Docker", Route::MonitorDocker {}),
        ("Knowledge Graph", Route::MonitorKnowledgeGraph {}),
    ];

    rsx! {
        nav { class: "flex flex-wrap gap-4 text-sm text-gray-400",
            for (label, route) in tabs {
                Link {
                    to: route.clone(),
                    class: if route == props.active {
                        "text-white border-b-2 border-white pb-1"
                    } else {
                        "hover:text-white"
                    },
                    {label}
                }
            }
        }
    }
}
