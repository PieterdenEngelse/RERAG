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
        ("Tip", Route::MonitorTip {}),
        ("RAG", Route::MonitorRag {}),
        ("Index", Route::MonitorIndex {}),
        ("Chunks", Route::MonitorChunks {}),
        ("Datastores", Route::MonitorDatastores {}),
        ("Knowledge Graph", Route::MonitorKnowledgeGraph {}),
        ("Cache", Route::MonitorCache {}),
        ("Requests", Route::MonitorRequests {}),
        ("Rate Limits", Route::MonitorRateLimits {}),
        ("Logs", Route::MonitorLogs {}),
        ("Docker", Route::MonitorDocker {}),
        ("ONNX", Route::MonitorOnnx {}),
        ("Agentic", Route::MonitorAgentic {}),
        ("Tools", Route::MonitorTools {}),
        ("Agent", Route::MonitorObservations {}),
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
