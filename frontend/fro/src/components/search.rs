use crate::api::{self, SearchResult};
use dioxus::prelude::*;

#[component]
pub fn SearchBar() -> Element {
    let mut query = use_signal(String::new);
    let mut results = use_signal(Vec::<SearchResult>::new);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut backend_status = use_signal(|| String::from("Checking..."));

    // Check backend health on mount
    use_effect(move || {
        spawn(async move {
            match api::health_check().await {
                Ok(health) => {
                    backend_status.set(format!(
                        "✓ Connected ({} docs, {} vectors)",
                        health.documents.unwrap_or(0),
                        health.vectors.unwrap_or(0)
                    ));
                }
                Err(e) => {
                    backend_status.set(format!("✗ Backend offline: {}", e));
                }
            }
        });
    });

    let on_search = move |_evt: Event<MouseData>| {
        let query_text = query();
        if query_text.trim().is_empty() {
            return;
        }

        spawn(async move {
            loading.set(true);
            error.set(None);

            match api::search(&query_text).await {
                Ok(response) => {
                    let is_empty = response.results.is_empty();
                    results.set(response.results);
                    if is_empty {
                        error.set(Some("No results found".to_string()));
                    }
                }
                Err(e) => {
                    error.set(Some(format!("Search failed: {}", e)));
                    results.set(Vec::new());
                }
            }

            loading.set(false);
        });
    };

    let on_input = move |evt: Event<FormData>| {
        query.set(evt.value().clone());
    };

    let on_keypress = move |evt: Event<KeyboardData>| {
        if evt.key() == Key::Enter {
            let query_text = query();
            if query_text.trim().is_empty() {
                return;
            }

            spawn(async move {
                loading.set(true);
                error.set(None);

                match api::search(&query_text).await {
                    Ok(response) => {
                        let is_empty = response.results.is_empty();
                        results.set(response.results);
                        if is_empty {
                            error.set(Some("No results found".to_string()));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Search failed: {}", e)));
                        results.set(Vec::new());
                    }
                }

                loading.set(false);
            });
        }
    };

    rsx! {
        div {
            class: "w-full max-w-4xl mx-auto p-6",

            // Backend status indicator
            div {
                class: "mb-4 text-sm text-gray-300 dark:text-gray-400",
                "Backend: {backend_status}"
            }

            // Search input
            div {
                class: "flex gap-2 mb-6",

                input {
                    class: "flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg
                           bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100
                           focus:outline-none focus:ring-2 focus:ring-indigo-500",
                    r#type: "text",
                    placeholder: "Search documents...",
                    value: "{query}",
                    oninput: on_input,
                    onkeypress: on_keypress,
                    disabled: loading(),
                }

                button {
                    class: "px-6 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg
                           transition-colors disabled:opacity-50 disabled:cursor-not-allowed",
                    onclick: on_search,
                    disabled: loading() || query().trim().is_empty(),

                    if loading() {
                        "Searching..."
                    } else {
                        "Search"
                    }
                }
            }

            // Error message
            if let Some(err) = error() {
                div {
                    class: "mb-4 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200
                           dark:border-red-800 rounded-lg text-red-700 dark:text-red-400",
                    "{err}"
                }
            }

            // Results
            if !results().is_empty() {
                div {
                    class: "space-y-4",

                    div {
                        class: "text-sm text-gray-300 dark:text-gray-400 mb-4",
                        "Found {results().len()} result(s)"
                    }

                    for (idx, result) in results().iter().enumerate() {
                        div {
                            key: "{idx}",
                            class: "p-4 bg-white dark:bg-gray-800 border border-gray-200
                                   dark:border-gray-700 rounded-lg shadow-sm hover:shadow-md
                                   transition-shadow",

                            // Provenance row
                            div {
                                class: "flex items-center gap-2 mb-2 flex-wrap",

                                span {
                                    class: "text-xs font-medium px-2 py-0.5 rounded bg-indigo-900 text-indigo-200",
                                    "{result.block_type}"
                                }

                                if let Some(page) = result.page {
                                    span {
                                        class: "text-xs text-gray-400",
                                        "p.{page}"
                                    }
                                }

                                if result.extractor != "builtin" {
                                    span {
                                        class: "text-xs px-2 py-0.5 rounded bg-amber-900 text-amber-200",
                                        "{result.extractor}"
                                    }
                                }
                            }

                            // Text
                            p {
                                class: "text-gray-800 dark:text-gray-200 leading-relaxed text-sm",
                                "{result.text}"
                            }
                        }
                    }
                }
            } else if !loading() && query().trim().is_empty() {
                div {
                    class: "text-center py-12 text-gray-300 dark:text-gray-400",

                    div {
                        class: "text-4xl mb-4",
                        "🔍"
                    }

                    p {
                        class: "text-lg",
                        "Enter a search query to find relevant documents"
                    }
                }
            }

            // Loading state
            if loading() {
                div {
                    class: "text-center py-12",

                    div {
                        class: "inline-block animate-spin rounded-full h-12 w-12
                               border-b-2 border-indigo-600"
                    }

                    p {
                        class: "mt-4 text-gray-300 dark:text-gray-400",
                        "Searching..."
                    }
                }
            }
        }
    }
}
