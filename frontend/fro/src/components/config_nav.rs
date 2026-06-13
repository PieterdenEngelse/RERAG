use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[derive(Clone, PartialEq)]
pub enum ConfigTab {
    Memories,
    Hardware,
    IoUring,
    Chunker,
    Embedding,
    Ner,
    Onnx,
    FalkorDb,
    Redis,
    Terms,
    Other,
    Corpus,
    Runtime,
}

#[derive(Props, Clone, PartialEq)]
pub struct ConfigNavProps {
    pub active: ConfigTab,
}

#[component]
pub fn ConfigNav(props: ConfigNavProps) -> Element {
    let tabs = vec![
        ("Runtime", Route::ConfigRuntime {}, ConfigTab::Runtime),
        ("Memories", Route::ConfigMemories {}, ConfigTab::Memories),
        (
            "Hardware & performance",
            Route::ConfigHardware {},
            ConfigTab::Hardware,
        ),
        ("Io-uring", Route::ConfigIoUring {}, ConfigTab::IoUring),
        ("Chunker", Route::ConfigChunker {}, ConfigTab::Chunker),
        ("Embedding", Route::ConfigEmbedding {}, ConfigTab::Embedding),
        ("Ner", Route::ConfigNer {}, ConfigTab::Ner),
        ("ONNX", Route::ConfigOnnx {}, ConfigTab::Onnx),
        ("FalkorDB", Route::ConfigFalkorDb {}, ConfigTab::FalkorDb),
        ("Redis", Route::ConfigRedis {}, ConfigTab::Redis),
        ("Terms", Route::ConfigTerms {}, ConfigTab::Terms),
        ("Actix", Route::ConfigOther {}, ConfigTab::Other),
        ("Corpus", Route::ConfigCorpus {}, ConfigTab::Corpus),
    ];

    rsx! {
        nav { class: "flex flex-wrap gap-4 text-sm text-gray-400 px-2",
            for (label, route, tab_id) in tabs {
                {
                    let is_active = props.active == tab_id;
                    rsx! {
                        Link {
                            to: route,
                            class: if is_active {
                                "text-white border-b-2 border-white pb-1"
                            } else {
                                "hover:text-white"
                            },
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}
