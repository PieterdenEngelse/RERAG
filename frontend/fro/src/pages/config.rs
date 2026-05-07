use crate::{
    api,
    app::Route,
    components::config_nav::{ConfigNav, ConfigTab},
    components::monitor::*,
};
use dioxus::prelude::*;

#[component]
pub fn Config() -> Element {
    let mut doc_count = use_signal(|| "--".to_string());

    use_future(move || async move {
        if let Ok(info) = api::fetch_index_info().await {
            doc_count.set(info.total_documents.to_string());
        }
    });

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Config", Some(Route::Config {})),
                ],
            }

            ConfigNav { active: ConfigTab::Home }

            Panel { title: Some("Config sections".into()), refresh: None,
                div { class: "text-sm text-gray-300", "Use these tabs to open dedicated views for Sampling, Prompt, Hardware & performance, Chunker, or Other settings." }
            }

            RowHeader {
                title: "RAG".into(),
                description: Some("RAG subsystem status.".into()),
            }
            Panel { title: Some("RAG".into()), refresh: None,
                div { class: "flex flex-col gap-4",
                    div { class: "rounded p-4 bg-gray-800 border border-gray-700 flex flex-col gap-2",
                        span { class: "text-sm text-gray-200 font-semibold", "Chunker" }
                        span { class: "text-xs text-gray-400",
                            "Chunker configuration has moved to the dedicated "
                            dioxus_router::Link {
                                to: Route::ConfigChunker {},
                                class: "text-cyan-400 hover:underline",
                                "Chunker"
                            }
                            " tab. Configure mode, token sizes, semantic threshold, preprocessing, and context prefix there."
                        }
                    }
                    div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                        HealthCard { name: "Chunk-Size Overlapping".into(), status: "Healthy".into(), detail: Some("Ready".into()) }
                        HealthCard { name: "Chunker".into(), status: "Ready".into(), detail: Some("See Chunker tab".into()) }
                        HealthCard { name: "Documents".into(), status: doc_count().into(), detail: Some("Uploaded".into()) }
                    }
                }
            }

            RowHeader {
                title: "Agent".into(),
                description: Some("Agent runtime status.".into()),
            }
            Panel { title: Some("Agent".into()), refresh: None,
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    HealthCard { name: "Memory".into(), status: "Active".into(), detail: Some("SQLite".into()) }
                    HealthCard { name: "Tools".into(), status: "3".into(), detail: Some("Enabled".into()) }
                    HealthCard { name: "LLM".into(), status: "phi".into(), detail: Some("Local".into()) }
                    HealthCard { name: "Usage".into(), status: "--".into(), detail: Some("Recent".into()) }
                }
            }
        }
    }
}
