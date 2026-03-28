//! io_uring documentation page
use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuIoUring() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "max-w-7xl mx-auto",
                div { class: "flex items-center gap-4 mb-3",
                    Link {
                        to: Route::DocuIndex {},
                        class: "text-primary hover:underline text-sm",
                        "\u{2190} Back to Index"
                    }
                    h2 { class: "text-lg font-bold text-white",
                        "io_uring: A Unified Async I/O API for Linux"
                    }
                }

                // Row 1: What is io_uring (wide) + Before + With io_uring
                div { class: "grid grid-cols-1 lg:grid-cols-3 gap-2 mb-2",
                    div { class: "bg-gray-800 border border-gray-700 rounded p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "What is io_uring?" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Normal I/O: your program calls read(), a syscall. The CPU switches from "
                            "user mode to kernel mode, the kernel does the I/O, copies data to your buffer, "
                            "then switches back. Every read or write costs that context switch."
                        }
                        p { class: "text-xs text-gray-300 mb-1",
                            "io_uring eliminates that using two shared ring buffers:"
                        }
                        div { class: "text-xs text-gray-300 space-y-0.5 mb-1",
                            p {
                                span { class: "text-yellow-300 font-mono", "SQ " }
                                "\u{2014} your program writes I/O requests here. Queue dozens without a syscall."
                            }
                            p {
                                span { class: "text-yellow-300 font-mono", "CQ " }
                                "\u{2014} the kernel writes results here. Your program reads completions."
                            }
                        }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Both queues live in shared memory. No copying, no context switches per operation."
                        }
                        pre { class: "text-xs text-gray-400 font-mono leading-tight mb-1",
                            "1. Push 10 reads into SQ\n2. One io_uring_enter() syscall\n3. Kernel processes all 10\n4. Results appear in CQ\n5. Read 10 results from CQ"
                        }
                        p { class: "text-xs text-green-400",
                            "Traditional: 10 reads = 10 syscalls. io_uring: 10 reads = 1 syscall."
                        }
                        p { class: "text-xs text-yellow-300 mt-1",
                            "\u{2b50} In AG: matters most during doc ingestion and reindexing."
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Before (Fragmented)" }
                        pre { class: "text-xs text-gray-300 font-mono leading-tight",
                            "Files:   AIO         - Limited\nSockets: epoll       - Different API\nTimers:  timerfd     - Yet another\nSignals: signalfd    - And another\n\n\u{274c} Each I/O = different API\n\u{274c} Can\u{2019}t batch mixed ops"
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "With io_uring (Unified)" }
                        pre { class: "text-xs text-gray-300 font-mono leading-tight",
                            "Files \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}\nSockets \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524} io_uring \u{2500}\u{25ba} CQ\nTimers \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524} (One API)\nSignals \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n\n\u{2705} One API for everything\n\u{2705} Batch N ops in 1 syscall\n\u{2705} True kernel-level async"
                        }
                    }
                }

                // Row 2: Architecture + Performance
                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-2 mb-2",
                    div { class: "bg-gray-800 border border-gray-700 rounded p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Architecture" }
                        pre { class: "text-xs text-gray-300 font-mono leading-tight",
                            "USER SPACE              KERNEL SPACE\n\n\u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}        \u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}\n\u{2502} Submission Q \u{2502}\u{25c4}\u{2500}shared\u{2500}\u{25ba}\u{2502}  io_uring   \u{2502}\n\u{2502}     (SQ)     \u{2502} memory  \u{2502}   kernel    \u{2502}\n\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}        \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n       \u{2502} submit                  \u{2502}\n       \u{25bc}                         \u{2502} complete\n\u{250c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}                 \u{2502}\n\u{2502} Completion Q \u{2502}\u{25c4}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}\n\u{2502}     (CQ)     \u{2502} shared memory\n\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}"
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Performance" }
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
                                    td { class: "text-xs", "Syscalls/IO" }
                                    td { class: "text-xs", "1-2" }
                                    td { class: "text-xs text-green-400", "0-1" }
                                }
                                tr {
                                    td { class: "text-xs", "File async" }
                                    td { class: "text-xs text-red-400", "Fake" }
                                    td { class: "text-xs text-green-400", "True" }
                                }
                                tr {
                                    td { class: "text-xs", "Batching" }
                                    td { class: "text-xs text-red-400", "No" }
                                    td { class: "text-xs text-green-400", "Yes" }
                                }
                                tr {
                                    td { class: "text-xs", "Zero-copy" }
                                    td { class: "text-xs text-red-400", "Limited" }
                                    td { class: "text-xs text-green-400", "Yes" }
                                }
                                tr {
                                    td { class: "text-xs", "CPU" }
                                    td { class: "text-xs", "Higher" }
                                    td { class: "text-xs text-green-400", "30-50% lower" }
                                }
                            }
                        }
                        p { class: "text-xs text-gray-400 mt-1",
                            "Benchmark: epoll ~400k ops/s \u{2192} io_uring ~800k ops/s (2x)"
                        }
                    }
                }

                div { class: "mt-2 pt-2 border-t border-gray-700",
                    Link {
                        to: Route::DocuIndex {},
                        class: "btn btn-primary btn-xs",
                        "\u{2190} Back to Index"
                    }
                }
            }
        }
    }
}
