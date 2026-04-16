//! Documentation - rkyv (Rust serialization framework)

use dioxus::prelude::*;

#[component]
pub fn DocuRkyv() -> Element {
    let mut show_serialized_info = use_signal(|| false);

    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",
                a {
                    href: "/docu/index",
                    class: "text-primary hover:underline mb-4 inline-block",
                    "\u{2190} Back to Index"
                }
                h1 { class: "text-xl font-bold mb-3 text-white", "rkyv - Zero-Copy Deserialization" }

                div { class: "grid grid-cols-1 lg:grid-cols-3 gap-2 mb-2",
                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "What is rkyv?" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "rkyv (archive) is a zero-copy deserialization framework for Rust. "
                            "It lets you access "
                            span {
                                class: "text-primary underline cursor-pointer",
                                onclick: move |_| show_serialized_info.set(true),
                                "serialized data"
                            }
                            " directly from bytes without parsing, "
                            "making load times 10-50x faster than JSON or bincode."
                        }
                        ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                            li { "Zero-copy: access data directly from mmap'd files" }
                            li { "No deserialization step needed for reads" }
                            li { "Deterministic memory layout" }
                            li { "Derive macros: Archive, Serialize, Deserialize" }
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Usage in AG" }
                        div { class: "text-xs text-gray-300 space-y-0.5",
                            p {
                                span { class: "text-gray-400", "Vector storage: " }
                                "VectorStorageRkyv for embedding vectors"
                            }
                            p {
                                span { class: "text-gray-400", "Search cache: " }
                                "Cached query results persisted to disk"
                            }
                            p {
                                span { class: "text-gray-400", "Load path: " }
                                "load_vectors_rkyv_async() via io_uring"
                            }
                            p {
                                span { class: "text-gray-400", "Format: " }
                                ".rkyv files in vectors/ directory"
                            }
                        }
                        p { class: "text-xs text-green-400 mt-1",
                            "\u{2705} 10-50x faster load times vs JSON"
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Comparison" }
                        div { class: "text-xs text-gray-300 space-y-0.5",
                            p {
                                span { class: "text-gray-400", "JSON: " }
                                "Parse entire file \u{2192} allocate \u{2192} populate structs"
                            }
                            p {
                                span { class: "text-gray-400", "bincode: " }
                                "Read bytes \u{2192} deserialize \u{2192} allocate"
                            }
                            p {
                                span { class: "text-gray-400", "rkyv: " }
                                "Read bytes \u{2192} access directly (zero-copy)"
                            }
                        }
                        pre { class: "text-[10px] text-gray-300 font-mono mt-2 leading-tight",
                            "Load 10k vectors (384-dim):\n  JSON:   ~850ms\n  bincode: ~120ms\n  rkyv:    ~15ms  (zero-copy access)"
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "VectorStorageRkyv" }
                        pre { class: "text-[10px] text-gray-300 font-mono leading-tight",
                            "#[derive(rkyv::Archive,\n         rkyv::Serialize,\n         rkyv::Deserialize)]\nstruct VectorStorageRkyv {{\n    version: u32,\n    vectors: Vec<Vec<f32>>,\n    doc_id_to_idx: Vec<(String, u32)>,\n}}\n\n// Access archived data directly:\nlet archived = rkyv::access::<\n    ArchivedVectorStorageRkyv\n>(&bytes)?;\n// No deserialization needed!"
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Archive" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "The three derive macros each do one job:"
                        }
                        div { class: "text-xs text-gray-300 space-y-0.5 mb-1",
                            p {
                                span { class: "text-gray-400 font-mono", "Archive " }
                                "\u{2014} generates a second \u{201c}archived\u{201d} type that maps directly to the byte layout on disk. "
                                "For VectorStorageRkyv it creates ArchivedVectorStorageRkyv \u{2014} a mirror "
                                "where String becomes ArchivedString, Vec<f32> becomes ArchivedVec<f32>, "
                                "u32 stays u32. These archived types use relative pointers and fixed layouts "
                                "so they work directly on raw bytes."
                            }
                            p {
                                span { class: "text-gray-400 font-mono", "Serialize " }
                                "\u{2014} writes your type into that archived byte layout."
                            }
                            p {
                                span { class: "text-gray-400 font-mono", "Deserialize " }
                                "\u{2014} rebuilds the original type from the archived version "
                                "(only if you need an owned copy, which you usually don\u{2019}t)."
                            }
                        }
                        p { class: "text-xs text-green-400",
                            "The zero-copy magic is in Archive. rkyv::access::<ArchivedVectorStorageRkyv>(&bytes) "
                            "gives you a reference living directly in the bytes. No new memory allocated."
                        }
                    }

                    div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-white mb-1", "Why Rust-Only" }
                        p { class: "text-xs text-gray-300 mb-1",
                            "rkyv\u{2019}s zero-copy trick works because it writes bytes to disk in the exact layout "
                            "that Rust uses in memory for that type. When you load the file, you cast the raw bytes "
                            "to a pointer and Rust sees valid data. No rebuilding, no copying."
                        }
                        p { class: "text-xs text-gray-300 mb-1",
                            "This only works because Rust gives you precise control over how types are laid out "
                            "in memory \u{2014} known alignment, known field ordering, deterministic sizes. "
                            "The serialized bytes and the in-memory representation are identical by design."
                        }
                        p { class: "text-xs text-gray-300 mb-1",
                            "Other languages can\u{2019}t do this:"
                        }
                        div { class: "text-xs text-gray-300 space-y-0.5",
                            p {
                                span { class: "text-gray-400", "Python: " }
                                "hidden headers and reference counts"
                            }
                            p {
                                span { class: "text-gray-400", "Java: " }
                                "GC metadata in every object"
                            }
                            p {
                                span { class: "text-gray-400", "JavaScript: " }
                                "values wrapped in engine-internal representations"
                            }
                            p {
                                span { class: "text-gray-400", "C++: " }
                                "could theoretically do it, but lacks the derive macro ecosystem "
                                "and safety guarantees that make rkyv practical"
                            }
                        }
                    }
                }

                a { href: "/docu/index", class: "btn btn-primary btn-sm mt-4 inline-block", "\u{2190} Back to Index" }
            }
        }
        // Serialized data info modal
        if show_serialized_info() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                onclick: move |_| show_serialized_info.set(false),

                div {
                    class: "bg-gray-800 border border-gray-700 rounded-lg p-4 shadow-xl max-w-lg mx-4",
                    onclick: move |e| e.stop_propagation(),

                    div { class: "flex justify-between items-start mb-3",
                        h2 { class: "text-base font-semibold text-gray-100", "Serialized Data" }
                        button {
                            class: "text-white text-xl hover:text-gray-300",
                            onclick: move |_| show_serialized_info.set(false),
                            "\u{2715}"
                        }
                    }
                    p { class: "text-sm text-gray-300 mb-3",
                        "Data that has been converted from its in-memory structure into a sequence "
                        "of raw bytes, ready to be stored on disk, sent over a network, or passed "
                        "between processes."
                    }
                    p { class: "text-sm text-gray-300 mb-3",
                        "It\u{2019}s no longer \u{201c}alive\u{201d} \u{2014} you can\u{2019}t call methods on it, "
                        "can\u{2019}t access fields by name, can\u{2019}t iterate its collections. "
                        "It\u{2019}s just bytes sitting there, waiting to be deserialized back into "
                        "something usable."
                    }
                    p { class: "text-sm text-gray-300 mb-3",
                        "Think of it like a flatpack furniture box. The table exists in there, "
                        "but you can\u{2019}t eat dinner on it until you unpack and assemble it. "
                        "rkyv\u{2019}s trick is that you open the box and the table is already assembled."
                    }
                    button {
                        class: "btn btn-primary btn-sm w-full",
                        onclick: move |_| show_serialized_info.set(false),
                        "Got it!"
                    }
                }
            }
        }

    }
}
