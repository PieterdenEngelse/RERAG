use dioxus::prelude::*;

use std::borrow::Cow;

#[derive(Props, Clone, PartialEq)]
pub struct RowHeaderProps {
    pub title: Cow<'static, str>,
    #[props(default)]
    pub description: Option<Cow<'static, str>>,
}

#[component]
pub fn RowHeader(props: RowHeaderProps) -> Element {
    rsx! {
        div { class: "flex items-center justify-between text-sm mb-2",
            div {
                h4 { class: "text-gray-200 font-semibold", {props.title.as_ref()} }
                if let Some(desc) = &props.description {
                    p { class: "text-xs text-gray-500", {desc.as_ref()} }
                }
            }
        }
    }
}
