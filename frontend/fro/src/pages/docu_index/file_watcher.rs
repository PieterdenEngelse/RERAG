//! File Watcher documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuFileWatcher() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-blue-300", "File Watcher" }
                    span { class: "text-xs text-gray-400", "Drop a file in a folder, and the app ingests it — no clicks, no upload form." }
                }

                div { class: "grid grid-cols-3 gap-2",

                    // Col 1
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "What it does" }
                            p { class: "text-xs text-gray-200",
                                "Each corpus has one filesystem watcher (built on the "
                                code { class: "text-green-300", "notify" } " crate). When a file appears or changes in its watched directory, the watcher debounces, then runs the full pipeline on that file: parse → chunk → embed → index → graph. No UI interaction required — copy a PDF into the folder and it shows up in search."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Which events" }
                            p { class: "text-xs text-gray-200",
                                "Only " code { class: "text-green-300", "Create" } " and " code { class: "text-green-300", "Modify" } " kernel events are forwarded — deletes, renames, and attribute changes are ignored. Modifies bump the same chunk_id, so re-saving a watched file replaces its chunks rather than producing duplicates."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Why debounce" }
                            p { class: "text-xs text-gray-200",
                                "Editors and sync tools fire many filesystem events for a single \"save\" (write temp file → rename → fsync). The watcher coalesces events per path within "
                                code { class: "text-green-300", "FILE_WATCHER_DEBOUNCE_MS" } " (default 500 ms) so each save triggers exactly one re-ingest, not five."
                            }
                        }
                    }

                    // Col 2
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Where it watches" }
                            p { class: "text-xs text-gray-200 mb-1",
                                "Path resolution for the " span { class: "font-mono text-gray-100", "default" } " corpus has three layers, top wins:"
                            }
                            ol { class: "text-xs text-gray-200 list-decimal pl-4 space-y-0.5",
                                li { code { class: "text-green-300", "FILE_WATCHER_DIR" } " — env var or runtime override (restart-required)" }
                                li { "The corpus' own watched-directory setting (Config → Corpus)" }
                                li { "The default: " code { class: "text-gray-400", "~/.local/share/ag/data/corpora/default/documents/" } }
                            }
                            p { class: "text-xs text-gray-300 mt-1",
                                "Non-default corpora skip layer 1 — they use their own setting, then fall back to "
                                code { class: "text-gray-400", "{{data_dir}}/corpora/{{slug}}/documents/" } "."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "One watcher per corpus" }
                            p { class: "text-xs text-gray-200",
                                "At startup the app reads the corpora table and spawns one watcher task per corpus, each routed to its corpus' retriever. They share the same debounce and enabled-flag, but each owns its own watched directory. The registry lives in-process — there is no on-disk watcher state to recover."
                            }
                        }
                    }

                    // Col 3
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Knobs" }
                            ul { class: "text-xs text-gray-200 list-disc pl-4 space-y-0.5",
                                li {
                                    code { class: "text-green-300", "FILE_WATCHER_ENABLED" } " — bool, hot-reloaded. Off = watchers stop firing but the spec is kept, so flipping back on respawns them."
                                }
                                li {
                                    code { class: "text-green-300", "FILE_WATCHER_DEBOUNCE_MS" } " — u64, hot-reloaded. All watchers are aborted and respawned with the new value."
                                }
                                li {
                                    code { class: "text-green-300", "FILE_WATCHER_DIR" } " — path, restart-required. Overrides the default corpus' watched directory."
                                }
                                li {
                                    "Per-corpus " code { class: "text-green-300", "watch_dir" } " — restart-required. Stored in the " code { class: "text-gray-400", "corpora" } " table, editable from Config → Corpus."
                                }
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Limits & gotchas" }
                            ul { class: "text-xs text-gray-200 list-disc pl-4 space-y-0.5",
                                li { "No initial sweep — only files that arrive after the watcher starts are picked up. Existing files use the index built at upload or re-index time." }
                                li { "Deleting a file from the folder does " span { class: "font-semibold text-gray-100", "not" } " remove its chunks. Use the documents page or the upload-search delete route." }
                                li { "Symlinks across filesystems can confuse "
                                    code { class: "text-green-300", "notify" } " — keep the watched dir on a real local path." }
                                li { "On Linux, each watched directory consumes one inotify watch; the kernel default cap is " code { class: "text-gray-400", "fs.inotify.max_user_watches" } " — usually fine, but worth knowing for many-corpus setups." }
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Source" }
                            p { class: "text-xs text-gray-300",
                                code { class: "text-green-300", "backend/src/file_watcher.rs" } " — registry, spawn task, debounce loop." br {}
                                code { class: "text-green-300", "backend/src/main.rs" } " phase 7.5 — startup wiring + path precedence."
                            }
                        }
                    }
                }

                div { class: "mt-2 pt-2 border-t border-gray-700",
                    Link { to: Route::DocuIndex {}, class: "btn btn-primary btn-xs", "← Back to Index" }
                }
            }
        }
    }
}
