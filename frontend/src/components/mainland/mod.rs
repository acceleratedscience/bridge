use std::ops::Deref;

use dioxus::{document::eval, prelude::*};

use crate::components::nav::Route;

/// Home page
#[component]
pub fn Home() -> Element {
    let d = use_resource(|| async move {
        let response =
            reqwest::get("https://open.accelerate.science/auth/.well-known/jwks.json").await;

        match response {
            Ok(res) => res
                .text()
                .await
                .unwrap_or_else(|e| format!("Failed to read text: {}", e)),
            Err(e) => format!("Network error: {:?}", e),
        }
    });

    rsx! {
        document::Title { "Bridge Portal" }
        main { class: "p-6",
            div { class: "max-w-7xl mx-auto p-6 bg-[#f4f4f4] dark:bg-[#121619] rounded-sm shadow-lg",
                h1 { class: "text-gray-300 text-2xl font-bold mb-2 text-center", "Bridge Portal" }
                p { class: "text-gray-300", "Hello World!" }
                p { class: "text-gray-300", {d.cloned()} }
            }
        }
    }
}
