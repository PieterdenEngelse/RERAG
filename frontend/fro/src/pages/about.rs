use crate::app::Route;
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn About() -> Element {
    rsx! {
        div { class: "p-8 max-w-2xl mx-auto",
            h2 { class: "text-2xl font-semibold text-gray-800 dark:text-gray-200",
                "About This App"
            }
            p { class: "mt-2 text-gray-300 dark:text-gray-300",
                "Built with Dioxus and Tailwind CSS."
            }
            Link {
                to: Route::Home {},
                class: "mt-4 inline-block px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-700",
                "Back Home"
            }
        }
    }
}
