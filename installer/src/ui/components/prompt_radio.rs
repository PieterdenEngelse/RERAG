//! Radio group with label + description per option, used on the Prompts screen.

use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
pub struct RadioOption {
    pub key: String,
    pub label: String,
    pub description: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct PromptRadioProps {
    pub name: String,
    pub options: Vec<RadioOption>,
    pub selected: Signal<String>,
}

#[component]
pub fn PromptRadio(props: PromptRadioProps) -> Element {
    rsx! {
        fieldset { class: "prompt-radio",
            for opt in props.options.iter() {
                {
                    let key = opt.key.clone();
                    let label = opt.label.clone();
                    let description = opt.description.clone();
                    let mut selected = props.selected;
                    let name = props.name.clone();
                    let is_checked = *selected.read() == key;
                    let onclick_key = key.clone();
                    rsx! {
                        label { key: "{key}",
                            class: if is_checked { "prompt-radio-row prompt-radio-row-active" } else { "prompt-radio-row" },
                            input {
                                r#type: "radio",
                                name: "{name}",
                                value: "{key}",
                                checked: is_checked,
                                onchange: move |_| selected.set(onclick_key.clone()),
                            }
                            div { class: "prompt-radio-text",
                                div { class: "prompt-radio-label", "{label}" }
                                div { class: "prompt-radio-description", "{description}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
