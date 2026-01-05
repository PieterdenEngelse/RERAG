use crate::app::Route;
use dioxus::prelude::*;

/// Global context to track which dropdown is currently open (by title)
#[derive(Clone, Default)]
pub struct ActiveDropdown(pub Option<String>);

#[component]
pub fn NavDropdown(title: String, children: Element) -> Element {
    let is_dark = use_context::<Signal<bool>>();
    let mut active_dropdown = use_context::<Signal<ActiveDropdown>>();
    
    let is_open = active_dropdown().0.as_ref() == Some(&title);
    let title_clone = title.clone();

    rsx! {
        div {
            class: "relative",

            button {
                class: "flex items-center gap-2 py-2 px-3 rounded-lg transition-colors font-medium",
                class: if is_dark() {
                    "text-white hover:text-indigo-400"
                } else {
                    "text-gray-900 hover:text-indigo-600"
                },
                onclick: move |_| {
                    if is_open {
                        active_dropdown.set(ActiveDropdown(None));
                    } else {
                        active_dropdown.set(ActiveDropdown(Some(title_clone.clone())));
                    }
                },

                "{title}"
                span { class: "text-xs", if is_open { "▲" } else { "▼" } }
            }

            if is_open {
                div {
                    class: "absolute z-10 rounded-lg shadow-lg w-44 mt-2",
                    class: if is_dark() { "bg-gray-700" } else { "bg-white" },
                    ul {
                        class: "py-2",
                        {children}
                    }
                }
            }
        }
    }
}

#[component]
pub fn DropdownItem(to: Route, children: Element) -> Element {
    let is_dark = use_context::<Signal<bool>>();
    let mut active_dropdown = use_context::<Signal<ActiveDropdown>>();

    rsx! {
        li {
            Link {
                to: to,
                class: if is_dark() {
                    "block px-4 py-2 transition-colors text-gray-200 hover:bg-gray-600"
                } else {
                    "block px-4 py-2 transition-colors text-gray-700 hover:bg-gray-100"
                },
                onclick: move |_| active_dropdown.set(ActiveDropdown(None)),  // Close dropdown
                {children}
            }
        }
    }
}

#[component]
pub fn DropdownActionItem(onclick: EventHandler<MouseEvent>, children: Element) -> Element {
    let is_dark = use_context::<Signal<bool>>();
    let mut active_dropdown = use_context::<Signal<ActiveDropdown>>();

    rsx! {
        li {
            button {
                class: if is_dark() {
                    "block w-full text-left px-4 py-2 transition-colors text-gray-200 hover:bg-gray-600"
                } else {
                    "block w-full text-left px-4 py-2 transition-colors text-gray-700 hover:bg-gray-100"
                },
                onclick: move |evt| {
                    active_dropdown.set(ActiveDropdown(None));  // Close dropdown
                    onclick.call(evt);
                },
                {children}
            }
        }
    }
}
