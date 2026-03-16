//! io_uring documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuIoUring() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-6",
            div { class: "max-w-4xl mx-auto",

                div { class: "flex items-center gap-4 mb-6",
                    Link {
                        to: Route::DocuIndex {},
                        class: "text-primary hover:underline",
                        "← Back to Index"
                    }
                }

                    h2 { class: "text-xl font-bold text-white mb-4",
                        "io_uring: A Unified Async I/O API for Linux"
                    }

                    div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4 mb-4",
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "What is io_uring?" }
                            p { class: "text-xs text-gray-300 mb-2",
                                "Linux kernel interface (5.1+) for async I/O:"
                            }
                            ul { class: "text-xs text-gray-300 list-disc ml-4 space-y-0.5",
                                li { "One API for all I/O types" }
                                li { "Zero/minimal syscalls" }
                                li { "True async (not thread pools)" }
                                li { "Batching of operations" }
                            }
                            p { class: "text-xs text-yellow-300 mt-2",
                                "\u{2b50} File I/O (doc ingestion, index loading) is where io_uring helps most!"
                            }
                        }
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Before (Fragmented)" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "Files:   AIO         - Limited\nSockets: epoll       - Different API\nTimers:  timerfd     - Yet another\nSignals: signalfd    - And another\n\n\u{274c} Each I/O = different API\n\u{274c} Can't batch mixed ops"
                            }
                        }
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "With io_uring (Unified)" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "Files \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}\nSockets \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524} io_uring \u{2500}\u{25ba} CQ\nTimers \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524} (One API)\nSignals \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n\n\u{2705} One API for everything\n\u{2705} Batch N ops in 1 syscall\n\u{2705} True kernel-level async"
                            }
                        }
                    }

                    div { class: "grid grid-cols-1 lg:grid-cols-2 gap-4 mb-4",
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Architecture" }
                            pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                                "USER SPACE              KERNEL SPACE\n\n\u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}        \u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}\n\u{2502} Submission Q \u{2502}\u{25c4}\u{2500}shared\u{2500}\u{25ba}\u{2502}  io_uring   \u{2502}\n\u{2502}     (SQ)     \u{2502} memory  \u{2502}   kernel    \u{2502}\n\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}        \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n       \u{2502} submit                  \u{2502}\n       \u{25bc}                         \u{2502} complete\n\u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}                 \u{2502}\n\u{2502} Completion Q \u{2502}\u{25c4}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n\u{2502}     (CQ)     \u{2502} shared memory\n\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}"
                            }
                        }
                        div { class: "bg-gray-900 rounded p-3",
                            h3 { class: "text-lg font-bold text-white mb-2", "Performance" }
                            table { class: "table table-xs text-gray-300 w-full",
                                thead {
                                    tr {
                                        th { class: "text-gray-200 text-xs", "Metric" }
                                        th { class: "text-gray-200 text-xs", "epoll" }
                                        th { class: "text-gray-200 text-xs", "io_uring" }
                                    }
                                }
                                tbody {
                                    tr {
                                        td { "Syscalls/IO" }
                                        td { "1-2" }
                                        td { class: "text-green-400", "0-1" }
                                    }
                                    tr {
                                        td { "File async" }
                                        td { class: "text-red-400", "Fake" }
                                        td { class: "text-green-400", "True" }
                                    }
                                    tr {
                                        td { "Batching" }
                                        td { class: "text-red-400", "No" }
                                        td { class: "text-green-400", "Yes" }
                                    }
                                    tr {
                                        td { "Zero-copy" }
                                        td { class: "text-red-400", "Limited" }
                                        td { class: "text-green-400", "Yes" }
                                    }
                                    tr {
                                        td { "CPU" }
                                        td { "Higher" }
                                        td { class: "text-green-400", "30-50% lower" }
                                    }
                                }
                            }
                            p { class: "text-xs text-gray-400 mt-2",
                                "Benchmark: epoll ~400k ops/s \u{2192} io_uring ~800k ops/s (2x)"
                            }
                        }
                    }

                div { class: "mt-8 pt-4 border-t border-gray-700",
                    Link {
                        to: Route::DocuIndex {},
                        class: "btn btn-primary btn-sm",
                        "← Back to Index"
                    }
                }
            }
        }
    }
}
