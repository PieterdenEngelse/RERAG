use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ChartPlaceholderProps {
    pub values: Vec<f64>,
    #[props(default = "Value".to_string())]
    pub label: String,
    #[props(default = "".to_string())]
    pub unit: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct ChartBarProps {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub unit: String,
}

#[component]
pub fn ChartPlaceholder(props: ChartPlaceholderProps) -> Element {
    let (mut min, mut max) = props
        .values
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));

    if min == f64::MAX {
        min = 0.0;
    }
    if (max - min).abs() < f64::EPSILON {
        max = min + 1.0;
    }

    rsx! {
        div { class: "p-4 bg-gray-900 rounded border border-gray-800 text-xs text-gray-400",
            div { class: "text-gray-200 mb-2 font-semibold", "{props.label}" }
            if props.values.is_empty() {
                div { "No data" }
            } else {
                div { class: "flex gap-2", role: "img", aria_label: "trend placeholder",
                    div { class: "flex flex-col justify-between text-[10px] text-gray-300 w-12 text-right pr-1",
                        span { "{max:.1}{props.unit}" }
                        span { "{((max + min) / 2.0):.1}{props.unit}" }
                        span { "{min:.1}{props.unit}" }
                    }
                    div { class: "flex-1 flex gap-1 items-end h-24",
                        for value in &props.values {
                            ChartBar { value: *value, min, max, unit: props.unit.clone() }
                        }
                    }
                }
                div { class: "text-[10px] text-gray-300 text-right italic mt-1", "X-axis: seconds (oldest → newest)" }
            }
        }
    }
}

#[component]
pub fn ChartBar(props: ChartBarProps) -> Element {
    let height = if (props.max - props.min).abs() < f64::EPSILON {
        1.0
    } else {
        (props.value - props.min) / (props.max - props.min)
    };

    rsx! {
        div {
            class: "flex-1 bg-teal-500/40",
            style: format!("height: {:.0}%", (height * 100.0).clamp(5.0, 100.0)),
            title: format!("{:.1}{}", props.value, props.unit)
        }
    }
}
