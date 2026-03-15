use crate::app::Route;
use crate::components::monitor::{Breadcrumb, BreadcrumbItem, NavTabs, Panel};
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn MonitorOnnx() -> Element {
    rsx! {
        div { class: "p-4 space-y-4",
            NavTabs { active: Route::MonitorOnnx {} }

            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("ONNX", None),
                ]
            }

            Panel { title: "ONNX Embedding Engine",
                div { class: "space-y-2",
                    div {
                        class: "bg-gray-800 rounded px-3 py-2 flex items-center gap-3 cursor-pointer hover:bg-gray-600 transition-colors",
                        style: "border-left: 2px solid #00BCD4;",
                        Link {
                            to: Route::MonitorOnnxStatus {},
                            class: "text-xs font-semibold cursor-pointer hover:underline",
                            style: "color:#00BCD4;",
                            "Status & Config →"
                        }
                    }
                }
            }
        }
    }
}
