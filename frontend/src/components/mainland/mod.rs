use dioxus::{document::eval, prelude::*};

use crate::components::nav::Route;

/// Home page
#[component]
pub fn Home() -> Element {
    rsx! {
        document::Title { "Bridge Portal" }
        main { class: "p-6",
            div { class: "max-w-7xl mx-auto p-6 bg-[#f4f4f4] dark:bg-[#121619] rounded-sm shadow-lg",
                h1 { class: "text-gray-300 text-2xl font-bold mb-2 text-center", "Bridge Portal" }
                p { class: "text-gray-300", "Hello World!" }
            }
        }
    }
}
