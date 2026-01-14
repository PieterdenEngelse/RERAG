use dioxus::prelude::*;
use std::borrow::Cow;

#[derive(Props, Clone, PartialEq)]
pub struct HealthCardProps {
    pub name: Cow<'static, str>,
    pub status: Cow<'static, str>,
    #[props(default)]
    pub detail: Option<Cow<'static, str>>,
}

#[component]
pub fn HealthCard(props: HealthCardProps) -> Element {
    let status_class = match props.status.as_ref() {
        "healthy" | "Healthy" => "text-green-400",
        "degraded" | "Degraded" => "text-yellow-400",
        "unhealthy" | "Unhealthy" => "text-red-400",
        _ => "text-gray-400",
    };

    rsx! {
        div { class: "rounded p-4 bg-gray-800 border border-gray-700",
            div { class: "text-xs text-gray-400", {props.name.as_ref()} }
            div { class: "text-xl font-semibold {status_class}", {props.status.as_ref()} }
            if let Some(detail) = &props.detail {
                div { class: "text-xs text-gray-500", {detail.as_ref()} }
            }
        }
    }
}
