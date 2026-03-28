use crate::pages::hardware::constants::{
    INFO_ICON_SVG_CLASS, PARAM_ICON_BUTTON_CLASS, PARAM_ICON_BUTTON_STYLE,
};
use dioxus::prelude::*;
use std::borrow::Cow;

#[derive(Props, Clone, PartialEq)]
pub struct HealthCardProps {
    pub name: Cow<'static, str>,
    pub status: Cow<'static, str>,
    #[props(default)]
    pub detail: Option<Cow<'static, str>>,
    #[props(default)]
    pub info: Option<Cow<'static, str>>,
    #[props(default)]
    pub link: Option<Cow<'static, str>>,
}

#[component]
pub fn HealthCard(props: HealthCardProps) -> Element {
    let mut show_info = use_signal(|| false);

    let status_class = match props.status.as_ref() {
        "healthy" | "Healthy" => "text-green-400",
        "degraded" | "Degraded" | "Disabled" => "text-yellow-400",
        "unhealthy" | "Unhealthy" | "Unavailable" => "text-red-400",
        _ => "text-gray-400",
    };

    rsx! {
        div { class: "rounded p-4 bg-gray-800 border border-gray-700 relative",
            // Info button: link navigates, otherwise toggles modal
            if let Some(ref link) = props.link {
                a {
                    class: "absolute top-2 right-2 {PARAM_ICON_BUTTON_CLASS}",
                    style: PARAM_ICON_BUTTON_STYLE,
                    title: "More info",
                    href: "{link}",
                    InfoIcon {}
                }
            } else if props.info.is_some() {
                button {
                    class: "absolute top-2 right-2 {PARAM_ICON_BUTTON_CLASS}",
                    style: PARAM_ICON_BUTTON_STYLE,
                    title: "More info",
                    onclick: move |e| {
                        e.stop_propagation();
                        show_info.set(!show_info());
                    },
                    InfoIcon {}
                }
            }

            div { class: "text-xs text-gray-400", {props.name.as_ref()} }
            div { class: "text-xl font-semibold {status_class}", {props.status.as_ref()} }
            if let Some(detail) = &props.detail {
                div { class: "text-xs text-gray-500", {detail.as_ref()} }
            }

            // Info modal
            if show_info() {
                if let Some(info) = &props.info {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/60",
                        onclick: move |_| show_info.set(false),
                        div {
                            class: "bg-gray-800 border border-gray-600 rounded-lg p-5 w-[90vw] max-w-md max-h-[90vh] overflow-y-auto shadow-xl",
                            onclick: move |evt| evt.stop_propagation(),
                            div { class: "flex items-center justify-between mb-3",
                                h2 { class: "text-base font-semibold text-gray-100", {props.name.as_ref()} }
                                button {
                                    class: "text-gray-400 hover:text-gray-200 text-xl font-bold",
                                    onclick: move |_| show_info.set(false),
                                    "×"
                                }
                            }
                            div {
                                class: "text-sm text-gray-300 leading-relaxed",
                                {info.as_ref()}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Standard info icon from AGENTS.md
#[component]
fn InfoIcon() -> Element {
    rsx! {
        svg {
            class: INFO_ICON_SVG_CLASS,
            view_box: "0 0 20 20",
            fill: "none",
            stroke: "currentColor",
            circle { cx: "10", cy: "10", r: "9", stroke_width: "1" }
            line { x1: "10", y1: "8", x2: "10", y2: "14", stroke_width: "1.5" }
            circle { cx: "10", cy: "6.3", r: "1", fill: "currentColor", stroke: "none" }
        }
    }
}
