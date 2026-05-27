use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ProgressBarProps {
    pub value: f64,
    #[props(default)]
    pub label: Option<String>,
}

#[component]
pub fn ProgressBar(props: ProgressBarProps) -> Element {
    let pct = props.value.clamp(0.0, 100.0);
    rsx! {
        div { class: "w-full",
            if let Some(label) = &props.label {
                div { class: "text-xs text-gray-400 mb-1", {label.clone()} }
            }
            div { class: "h-2 bg-gray-700 rounded",
                div {
                    class: "h-2 rounded bg-teal-400",
                    style: "width: {pct}%;",
                }
            }
            div { class: "text-right text-[10px] text-gray-300 mt-1", "{pct as usize}%" }
        }
    }
}
