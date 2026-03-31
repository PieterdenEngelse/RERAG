use crate::{
    api,
    app::Route,
    components::monitor::*,
};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[component]
pub fn MonitorChunks() -> Element {
    let mut tokenizer = use_signal(|| None::<api::TokenizerInfo>);
    let mut stats = use_signal(|| None::<Vec<api::ChunkingStatsSnapshot>>);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);

    use_future(move || async move {
        loop {
            // Fetch both in parallel
            let (tok_res, stats_res) = futures_util::join!(
                api::fetch_tokenizer_info(),
                api::fetch_chunking_stats(20),
            );

            if let Ok(tok) = tok_res {
                tokenizer.set(Some(tok));
            }
            match stats_res {
                Ok(resp) => {
                    stats.set(Some(resp.snapshots));
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
            TimeoutFuture::new(10_000).await;
        }
    });

    let tok = tokenizer();
    let tok_model = tok.as_ref().map(|t| t.model.clone()).unwrap_or_default();
    let tok_exact = tok.as_ref().map(|t| t.is_exact).unwrap_or(false);
    let tok_vocab = tok.as_ref().map(|t| t.vocab_size).unwrap_or(0);

    rsx! {
        div { class: "space-y-6",
            Breadcrumb {
                items: vec![
                    BreadcrumbItem::new("Home", Some(Route::Home {})),
                    BreadcrumbItem::new("Monitor", Some(Route::MonitorOverview {})),
                    BreadcrumbItem::new("Chunks", None),
                ],
            }

            NavTabs { active: Route::MonitorChunks {} }

            // Tokenizer status board
            Panel { title: Some("Token Counter".into()), refresh: None,
                div { class: "flex flex-wrap gap-6 text-sm",
                    div { class: "flex flex-col gap-1",
                        span { class: "text-gray-400 text-xs", "Model" }
                        span { class: "text-gray-200 font-medium", "{tok_model}" }
                    }
                    div { class: "flex flex-col gap-1",
                        span { class: "text-gray-400 text-xs", "Vocab size" }
                        span { class: "text-gray-200 font-medium",
                            if tok_vocab > 0 {
                                "{tok_vocab}"
                            } else {
                                "N/A"
                            }
                        }
                    }
                    div { class: "flex flex-col gap-1",
                        span { class: "text-gray-400 text-xs", "Counting method" }
                        span {
                            class: if tok_exact { "text-green-400 font-medium" } else { "text-yellow-400 font-medium" },
                            if tok_exact { "Exact (GGUF)" } else { "Heuristic (approx)" }
                        }
                    }
                }
            }

            // Chunking history
            Panel { title: Some("Recent Chunking Operations".into()), refresh: None,
                if loading() {
                    div { class: "text-sm text-gray-400", "Loading..." }
                } else if let Some(err) = error() {
                    div { class: "text-sm text-red-400", "{err}" }
                } else if let Some(snaps) = stats() {
                    if snaps.is_empty() {
                        div { class: "text-sm text-gray-400", "No chunking operations recorded yet. Upload a document to see stats." }
                    } else {
                        div { class: "overflow-x-auto",
                            table { class: "table table-xs w-full text-gray-300",
                                thead {
                                    tr {
                                        th { class: "text-gray-400", "Time" }
                                        th { class: "text-gray-400", "File" }
                                        th { class: "text-gray-400", "Mode" }
                                        th { class: "text-gray-400 text-right", "Chunks" }
                                        th { class: "text-gray-400 text-right", "Tokens" }
                                        th { class: "text-gray-400 text-right", "Duration" }
                                        th { class: "text-gray-400", "Format" }
                                        th { class: "text-gray-400", "Strategy" }
                                    }
                                }
                                tbody {
                                    for snap in snaps.iter() {
                                        {
                                            let time_short = if snap.recorded_at.len() > 19 {
                                                &snap.recorded_at[11..19]
                                            } else {
                                                &snap.recorded_at
                                            };
                                            let file_short = snap.file.rsplit('/').next().unwrap_or(&snap.file);
                                            let detected_fmt = snap.detection.as_ref()
                                                .map(|d| d.detected_format.clone())
                                                .unwrap_or_default();
                                            let strategy = snap.detection.as_ref()
                                                .map(|d| d.chosen_strategy.clone())
                                                .unwrap_or_default();
                                            rsx! {
                                                tr { class: "hover:bg-gray-800/50",
                                                    td { class: "font-mono text-xs", "{time_short}" }
                                                    td { class: "max-w-48 truncate", title: "{snap.file}", "{file_short}" }
                                                    td { "{snap.chunker_mode}" }
                                                    td { class: "text-right", "{snap.chunks}" }
                                                    td { class: "text-right", "{snap.tokens}" }
                                                    td { class: "text-right", "{snap.duration_ms}ms" }
                                                    td { class: "text-xs", "{detected_fmt}" }
                                                    td { class: "text-xs", "{strategy}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
