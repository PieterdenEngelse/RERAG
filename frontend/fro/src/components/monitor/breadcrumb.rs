use dioxus::prelude::*;
use dioxus_router::Link;
use std::borrow::Cow;

use crate::app::Route;

#[derive(Clone, PartialEq)]
pub struct BreadcrumbItem {
    pub label: Cow<'static, str>,
    pub route: Option<Route>,
}

impl BreadcrumbItem {
    pub fn new(label: impl Into<Cow<'static, str>>, route: Option<Route>) -> Self {
        Self {
            label: label.into(),
            route,
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct BreadcrumbProps {
    pub items: Vec<BreadcrumbItem>,
}

#[component]
pub fn Breadcrumb(props: BreadcrumbProps) -> Element {
    if props.items.is_empty() {
        return rsx! {};
    }

    rsx! {
        nav { class: "text-base font-semibold text-white flex items-center flex-wrap gap-3 py-4 px-2", style: "margin-bottom: -0.75%;",
            for (idx, item) in props.items.iter().enumerate() {
                if let Some(route) = &item.route {
                    Link {
                        to: route.clone(),
                        class: "text-white/80 hover:text-white transition-colors",
                        {item.label.as_ref()}
                    }
                } else {
                    span { class: "text-white font-bold", {item.label.as_ref()} }
                }

                if idx < props.items.len() - 1 {
                    span { class: "text-white/50", "›" }
                }
            }
        }
    }
}
