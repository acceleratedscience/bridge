use dioxus::{document::eval, prelude::*};

use crate::components::nav::Route;

/// Home page
#[component]
pub fn Home() -> Element {
    rsx! {
        main { class: "p-6",
            div { class: "max-w-4xl mx-auto p-6 bg-[#f4f4f4] dark:bg-[#121619] rounded-sm shadow-lg",
                h1 { class: "text-gray-300 text-2xl font-bold mb-2", "Main Content Area" }
                p { class: "text-gray-300", "Hello World!" }
            }
        }
    }
}

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    // Reconstruct the broken URL path from the sliced segments
    let broken_path = segments.join("/");

    let _message = use_resource(move || async move {
        let mut e = eval(
            r#"
            // 1. Tell search crawlers to ignore this page completely
            let meta = document.createElement('meta');
            meta.name = 'robots';
            meta.content = 'noindex, nofollow';
            document.head.appendChild(meta);

            // 2. Clear the browser tab title
            dioxus.send("done!");
            console.log("Hello from Dioxus");
        "#,
        );
        e.recv::<String>().await.unwrap()
    });

    rsx! {
        document::Title { "Page Not Found" }
        div { class: "flex h-screen",

            div { class: "m-auto text-center p-5 rounded-b-sm shadow-lg bg-[#f4f4f4] dark:bg-[#121619] max-w-[400px]",

                h1 { style: "color: #ef4444; font-size: 4em; margin: 0; font-weight: 800;",
                    "404"
                }
                h2 { style: "margin-top: 10px; font-size: 1.5em;", "Page Not Found" }

                p { style: "color: #6b7280; font-size: 0.95em; margin: 15px 0 25px 0; line-height: 1.6;",
                    "We couldn't find anything matching "
                    code { style: "background: #f3f4f6; padding: 2px 6px; border-radius: 4px; font-family: monospace; color: #374151;",
                        "/{broken_path}"
                    }
                    ". It might have moved, or a parameter typed into the path was invalid."
                }

                Link {
                    to: Route::Home {},
                    class: "bg-blue-700 p-3 rounded-b-sm",
                    "Return to Homepage"
                }
            }
        }
    }
}
