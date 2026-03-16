//! Bias documentation page

use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn DocuBias() -> Element {
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

                    h2 { class: "text-2xl font-bold text-white mb-4", "Bias: Two Different Meanings" }
                    p { class: "text-lg text-gray-200 mb-6",
                        "The word 'bias' means completely different things in ML depending on context."
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "1. Social/Cognitive Bias (Training Data, Behavior)"
                    }
                    div { class: "bg-red-900/30 border border-red-700 rounded p-4 mb-4",
                        p { class: "text-sm text-gray-200 mb-3",
                            "What people usually mean in public discourse\u{2014}the model favoring certain viewpoints, perpetuating stereotypes, having political leanings, etc."
                        }
                        p { class: "text-sm text-gray-300 mb-2", "This comes from:" }
                        ul { class: "text-sm text-gray-300 list-disc ml-6 space-y-1",
                            li { "Training data reflecting societal biases" }
                            li { "RLHF (Reinforcement Learning from Human Feedback) choices" }
                            li { "Imbalanced representation in datasets" }
                            li { "Human annotator biases" }
                        }
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3",
                        "2. Mathematical Bias (Weight Parameter)"
                    }
                    div { class: "bg-blue-900/30 border border-blue-700 rounded p-4 mb-4",
                        p { class: "text-sm text-gray-200 mb-3",
                            "ML papers and code are full of \"bias terms\", \"bias vectors\", \"bias parameters\". This is a completely unrelated concept."
                        }
                        p { class: "text-sm text-gray-300 mb-2",
                            "In a neural network: y = \u{03c3}(W\u{00b7}x + b)"
                        }
                        p { class: "text-sm text-gray-300",
                            "The 'b' is the bias\u{2014}a learnable offset that shifts the activation function. It allows neurons to activate even when inputs are zero."
                        }
                    }

                    h3 { class: "text-xl font-bold text-white mt-6 mb-3", "Quick Reference" }
                    table { class: "table table-sm w-full text-gray-300",
                        thead {
                            tr {
                                th { class: "text-gray-200", "" }
                                th { class: "text-gray-200", "Social Bias" }
                                th { class: "text-gray-200", "Mathematical Bias" }
                            }
                        }
                        tbody {
                            tr {
                                td { "Meaning" }
                                td { "Prejudice, unfairness" }
                                td { "Numeric offset" }
                            }
                            tr {
                                td { "Source" }
                                td { "Training data, RLHF" }
                                td { "Learned parameter" }
                            }
                            tr {
                                td { "Fix" }
                                td { "Better data, alignment" }
                                td { "N/A (it's intentional)" }
                            }
                            tr {
                                td { "In code" }
                                td { "Not visible" }
                                td { "model.bias, b tensor" }
                            }
                            tr {
                                td { "Origin" }
                                td { "Fairness research" }
                                td { "Statistics (1900s)" }
                            }
                        }
                    }

                    div { class: "bg-gray-700 rounded p-4 mt-6 text-sm text-gray-200",
                        p { class: "mb-2",
                            "\u{2022} When someone says \"the model is biased\" \u{2192} they mean social/cognitive bias (prejudice)"
                        }
                        p { class: "mb-2",
                            "\u{2022} When code says \"bias=True\" or \"self.bias\" \u{2192} it means the mathematical offset parameter"
                        }
                        p { "\u{2022} Context is everything\u{2014}same word, completely different meanings" }
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
