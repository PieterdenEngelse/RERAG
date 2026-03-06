use dioxus::prelude::*;
use std::borrow::Cow;

#[derive(Props, Clone, PartialEq)]
pub struct RowHeaderProps {
    pub title: Cow<'static, str>,
    #[props(optional)]
    pub description: Option<Cow<'static, str>>,
    #[props(optional)]
    pub trailing: Option<Element>,
    #[props(optional)]
    pub leading: Option<Element>,
}

#[component]
pub fn RowHeader(props: RowHeaderProps) -> Element {
    rsx! {
        div { class: "flex items-center justify-between text-sm mb-2",
            div {
                div { class: "flex items-center gap-2",
                    h4 { class: "text-gray-200 font-semibold", {props.title.as_ref()} }
                    if let Some(ref leading) = props.leading {
                        {leading.clone()}
                    }
                }
                if let Some(ref desc) = props.description {
                    p { class: "text-xs text-gray-500", {desc.as_ref()} }
                }
            }
            if let Some(ref trailing) = props.trailing {
                div { {trailing.clone()} }
            }
        }
    }
}
