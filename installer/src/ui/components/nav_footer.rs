//! Shared bottom navigation: Back / Next / Cancel buttons.

use dioxus::prelude::*;

use crate::app::use_screen;

#[derive(Props, Clone, PartialEq)]
pub struct NavFooterProps {
    #[props(default = String::from("Next"))]
    pub next_label: String,
    #[props(default = true)]
    pub next_enabled: bool,
    #[props(default = false)]
    pub hide_back: bool,
    #[props(default = false)]
    pub hide_next: bool,
    #[props(default = false)]
    pub hide_cancel: bool,
}

#[component]
pub fn NavFooter(props: NavFooterProps) -> Element {
    let mut screen = use_screen();
    let can_back = screen.read().can_go_back();

    let on_back = move |_| {
        let s = *screen.read();
        screen.set(s.prev());
    };
    let on_next = move |_| {
        let s = *screen.read();
        screen.set(s.next());
    };
    let on_cancel = move |_| {
        // For Phase B, "Cancel" just resets to Welcome. In Phase D it'd be
        // wired to "abort install and close window".
        screen.set(crate::app::Screen::Welcome);
    };

    rsx! {
        div { class: "screen-footer",
            div { class: "screen-footer-left",
                if !props.hide_cancel {
                    button { class: "btn btn-ghost", onclick: on_cancel, "Cancel" }
                }
            }
            div { class: "screen-footer-right",
                if !props.hide_back && can_back {
                    button { class: "btn btn-secondary", onclick: on_back, "Back" }
                }
                if !props.hide_next {
                    button {
                        class: "btn btn-primary",
                        disabled: !props.next_enabled,
                        onclick: on_next,
                        "{props.next_label}"
                    }
                }
            }
        }
    }
}
