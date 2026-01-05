use dioxus::prelude::*;
use std::borrow::Cow;

/// Same styling as hardware config page info buttons
const INFO_BUTTON_CLASS: &str =
    "w-6 h-6 min-w-6 min-h-6 shrink-0 rounded border border-blue-500/40 bg-blue-500/10 flex items-center justify-center cursor-pointer hover:bg-blue-500/20";

#[derive(Props, Clone, PartialEq)]
pub struct StatCardProps {
    pub title: Cow<'static, str>,
    pub value: Cow<'static, str>,
    #[props(default)]
    pub unit: Option<Cow<'static, str>>,
    #[props(default)]
    pub trend: Option<Cow<'static, str>>,
    #[props(default)]
    pub sparkline: Option<Vec<f64>>,
    #[props(default)]
    pub footer: Option<VNode>,
    #[props(default)]
    pub description: Option<Cow<'static, str>>,
    #[props(default)]
    pub info_tooltip: Option<Cow<'static, str>>,
}

#[component]
pub fn StatCard(props: StatCardProps) -> Element {
    let has_description = props.description.is_some();
    let has_title = !props.title.is_empty();
    let has_tooltip = props.info_tooltip.is_some();
    let mut show_tooltip = use_signal(|| false);

    rsx! {
        div {
            class: "rounded p-4 bg-gray-800 border border-gray-700 relative",
            style: if has_description { "width: fit-content;" } else { "" },
            
            // Title row (only if there's a title)
            if has_title {
                div { class: "flex items-center gap-2 mb-1",
                    div { class: "text-xs text-gray-400", {props.title.clone()} }
                }
            }
            
            // Tooltip popup (modal style matching hardware page)
            if *show_tooltip.read() {
                if let Some(tooltip) = &props.info_tooltip {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| show_tooltip.set(false),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg p-6 w-[90vw] max-w-4xl max-h-[95vh] overflow-y-auto shadow-xl",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-center justify-between mb-4",
                                h2 { class: "text-lg font-semibold text-gray-100", "Info" }
                                button {
                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                    onclick: move |_| show_tooltip.set(false),
                                    "×"
                                }
                            }
                            div {
                                class: "text-sm text-gray-300 whitespace-pre-line leading-relaxed",
                                {tooltip.clone()}
                            }
                        }
                    }
                }
            }
            
            if has_description {
                div { class: "flex items-start gap-4",
                    div { class: "flex-shrink-0",
                        div { class: "flex items-center gap-2",
                            div { class: "text-2xl font-bold text-gray-100", {props.value.clone()} }
                            if let Some(unit) = &props.unit {
                                span { class: "text-sm text-gray-500", {unit.clone()} }
                            }
                            if has_tooltip {
                                button {
                                    class: INFO_BUTTON_CLASS,
                                    onclick: move |_| show_tooltip.set(!show_tooltip()),
                                    title: "Show info",
                                    svg {
                                        class: "w-3 h-3 text-blue-400",
                                        view_box: "0 0 20 20",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        circle { cx: "10", cy: "10", r: "9" }
                                        line { x1: "10", y1: "8", x2: "10", y2: "14" }
                                        circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(desc) = &props.description {
                        div {
                            class: "text-[10px] text-gray-400 leading-relaxed",
                            style: "white-space: pre-line;",
                            {desc.clone()}
                        }
                    }
                }
            } else if !has_title {
                // No title - value and info button inline on same row
                div { class: "flex items-center gap-2",
                    span { class: "text-sm font-semibold text-gray-200", {props.value.clone()} }
                    if let Some(unit) = &props.unit {
                        span { class: "text-sm text-gray-500", {unit.clone()} }
                    }
                    if has_tooltip {
                        button {
                            class: INFO_BUTTON_CLASS,
                            onclick: move |_| show_tooltip.set(!show_tooltip()),
                            title: "Show info",
                            svg {
                                class: "w-3 h-3 text-blue-400",
                                view_box: "0 0 20 20",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                circle { cx: "10", cy: "10", r: "9" }
                                line { x1: "10", y1: "8", x2: "10", y2: "14" }
                                circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                            }
                        }
                    }
                }
            } else {
                // Has title - show value below, with optional info button in title row
                div { class: "flex items-center gap-2",
                    div { class: "text-2xl font-bold text-gray-100", {props.value.clone()} }
                    if let Some(unit) = &props.unit {
                        span { class: "text-sm text-gray-500", {unit.clone()} }
                    }
                    if has_tooltip {
                        button {
                            class: INFO_BUTTON_CLASS,
                            onclick: move |_| show_tooltip.set(!show_tooltip()),
                            title: "Show info",
                            svg {
                                class: "w-3 h-3 text-blue-400",
                                view_box: "0 0 20 20",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                circle { cx: "10", cy: "10", r: "9" }
                                line { x1: "10", y1: "8", x2: "10", y2: "14" }
                                circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
                            }
                        }
                    }
                }
            }
            if let Some(trend) = &props.trend {
                div { class: "text-xs text-gray-500", {trend.clone()} }
            }
            if let Some(points) = &props.sparkline {
                div { class: "text-[10px] text-gray-600", "sparkline: {points.len()} pts" }
            }
            if let Some(footer) = &props.footer {
                div { class: "mt-2", {footer.clone()} }
            }
        }
    }
}
