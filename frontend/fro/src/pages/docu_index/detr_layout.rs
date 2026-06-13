//! DETR-style image-based layout model documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuDetrLayout() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-blue-300", "DETR-style image-based layout model" }
                    span { class: "text-xs text-gray-400", "What that phrase actually means, and why ag uses one in the PDF pipeline." }
                }

                div { class: "grid grid-cols-3 gap-2",

                    // Col 1: DETR architecture
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "DETR — DEtection TRansformer" }
                            p { class: "text-xs text-gray-200",
                                "Object-detection architecture from Meta AI (Carion et al., 2020). A CNN backbone — typically a ResNet — turns the input image into a feature map; a transformer encoder-decoder then predicts a fixed-size set of "
                                em { "(bounding box, class)" }
                                " tuples in a single forward pass."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Why \"transformer\"" }
                            p { class: "text-xs text-gray-200",
                                "Classical detectors (Faster R-CNN, YOLO) need region proposals, anchor boxes, and a non-maximum-suppression step at the end. DETR drops all three: the decoder's "
                                em { "object queries" }
                                " attend over the encoded feature map and emit detections directly, supervised by a bipartite-matching set loss. Cleaner pipeline, fewer hand-tuned heuristics."
                            }
                        }
                    }

                    // Col 2: image-based vs word-feature
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "\"Image-based\"" }
                            p { class: "text-xs text-gray-200",
                                "The model's input is a rendered page bitmap — the pixels of the PDF page after rasterization. It sees the visual layout directly: ruled tables, sidebars, graphical headers, multi-column splits."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "vs word-feature models" }
                            p { class: "text-xs text-gray-200",
                                "The other family (LayoutLMv3, LayoutXLM, ag's Tier 2 ORT model) consumes the word boxes and text extracted by a separate parser, never the pixels. Faster per page, but blind to anything purely visual — they miss tables drawn with rules instead of text, figures, and layouts where geometry matters more than wording."
                            }
                        }
                    }

                    // Col 3: layout models + ag usage
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "\"Layout model\"" }
                            p { class: "text-xs text-gray-200",
                                "Same DETR architecture, but fine-tuned on document datasets — PubLayNet (≈ scientific papers), DocLayNet (broader business documents), and similar. The \"objects\" being detected are no longer cats and cars; they are "
                                em { "titles, body text, tables, figures, captions, lists, headers, footers, page numbers." }
                                " One box per region, each tagged with its layout class."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Where ag uses it" }
                            p { class: "text-xs text-gray-200",
                                "Stage 2 of Native PDF Extraction. Stage 1 ("
                                code { class: "text-green-300", "lopdf" }
                                ") gives word boxes; Stage 2 takes the rendered page and the DETR model draws structural boxes around it, so the downstream chunker can split on real section boundaries — \"this paragraph belongs to that heading\", \"these rows are one table\" — instead of running everything together as flat text."
                            }
                        }
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-green-300 mb-1", "Tier 0 / Tier 1 in ag" }
                            p { class: "text-xs text-gray-200",
                                code { class: "text-green-300", "LAYOUT_ML_MODEL_ID" }
                                " (Tier 0) downloads a DETR layout model from HuggingFace Hub on first boot. "
                                code { class: "text-green-300", "LAYOUT_DETR_MODEL_PATH" }
                                " (Tier 1) points at a DETR ONNX already on disk. Both load the same architecture into the same Stage 2 slot — the tiers are about "
                                em { "where the file comes from" } ", not which one classifies better."
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
