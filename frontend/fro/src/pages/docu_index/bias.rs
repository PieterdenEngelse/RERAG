//! Bias documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuBias() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-900 p-3",
            div { class: "w-full",

                div { class: "flex items-center gap-3 mb-2",
                    Link { to: Route::DocuIndex {}, class: "text-primary hover:underline text-sm shrink-0", "← Index" }
                    h1 { class: "text-lg font-bold text-white", "Bias: Two Different Meanings" }
                    span { class: "text-xs text-gray-400", "Same word, completely different concepts depending on context." }
                }

                div { class: "grid grid-cols-3 gap-2",

                    // Social bias
                    div { class: "bg-red-900/30 border border-red-700/60 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-red-300 mb-1", "1. Social / Cognitive Bias" }
                        p { class: "text-xs text-gray-200 mb-1",
                            "What people usually mean in public discourse — the model favoring certain viewpoints, perpetuating stereotypes, having political leanings."
                        }
                        p { class: "text-xs text-gray-400 mb-1", "Sources:" }
                        ul { class: "text-xs text-gray-300 list-disc ml-3 space-y-0.5",
                            li { "Training data reflecting societal biases" }
                            li { "RLHF (Reinforcement Learning from Human Feedback) choices" }
                            li { "Imbalanced representation in datasets" }
                            li { "Human annotator biases" }
                        }
                    }

                    // Mathematical bias
                    div { class: "bg-blue-900/30 border border-blue-700/60 rounded-lg p-2",
                        h3 { class: "text-sm font-bold text-blue-300 mb-1", "2. Mathematical Bias (Weight Parameter)" }
                        p { class: "text-xs text-gray-200 mb-1",
                            "ML papers and code are full of \"bias terms\", \"bias vectors\", \"bias parameters\". This is a completely unrelated concept."
                        }
                        p { class: "text-xs text-gray-300 mb-1",
                            "In a neural network: "
                            code { class: "text-xs text-yellow-300", "y = σ(W·x + b)" }
                        }
                        p { class: "text-xs text-gray-300",
                            "The 'b' is the bias — a learnable offset that shifts the activation function. It allows neurons to activate even when inputs are zero."
                        }
                    }

                    // Table + summary
                    div { class: "space-y-2",
                        div { class: "bg-gray-800 border border-gray-700 rounded-lg p-2",
                            h3 { class: "text-sm font-bold text-white mb-1", "Quick Reference" }
                            table { class: "table table-xs w-full text-gray-300",
                                thead {
                                    tr {
                                        th { class: "text-gray-400 text-xs", "" }
                                        th { class: "text-red-300 text-xs", "Social" }
                                        th { class: "text-blue-300 text-xs", "Mathematical" }
                                    }
                                }
                                tbody {
                                    tr { td { class: "text-xs", "Meaning" } td { class: "text-xs", "Prejudice" } td { class: "text-xs", "Numeric offset" } }
                                    tr { td { class: "text-xs", "Source" } td { class: "text-xs", "Training data, RLHF" } td { class: "text-xs", "Learned parameter" } }
                                    tr { td { class: "text-xs", "Fix" } td { class: "text-xs", "Better data" } td { class: "text-xs", "N/A — intentional" } }
                                    tr { td { class: "text-xs", "In code" } td { class: "text-xs", "Not visible" } td { class: "text-xs", "model.bias" } }
                                    tr { td { class: "text-xs", "Origin" } td { class: "text-xs", "Fairness research" } td { class: "text-xs", "Statistics (1900s)" } }
                                }
                            }
                        }
                        div { class: "bg-gray-700 border border-gray-600 rounded-lg p-2 text-xs text-gray-200 space-y-0.5",
                            p { "• \"The model is biased\" → social/cognitive bias (prejudice)" }
                            p { "• Code says \"bias=True\" → mathematical offset parameter" }
                            p { class: "text-gray-400", "Context is everything — same word, completely different meanings." }
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
