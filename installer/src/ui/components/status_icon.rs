//! Small status indicator used in detection tables, step lists, and summary.

use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Ok,
    Warn,
    Error,
    Pending,
    Active,
}

impl IconKind {
    fn glyph(self) -> &'static str {
        match self {
            IconKind::Ok => "✓",
            IconKind::Warn => "⚠",
            IconKind::Error => "✗",
            IconKind::Pending => "○",
            IconKind::Active => "●",
        }
    }
    fn css_class(self) -> &'static str {
        match self {
            IconKind::Ok => "icon icon-ok",
            IconKind::Warn => "icon icon-warn",
            IconKind::Error => "icon icon-error",
            IconKind::Pending => "icon icon-pending",
            IconKind::Active => "icon icon-active",
        }
    }
}

#[component]
pub fn StatusIcon(kind: IconKind) -> Element {
    rsx! { span { class: "{kind.css_class()}", "{kind.glyph()}" } }
}
