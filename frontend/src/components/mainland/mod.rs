use dioxus::{document::eval, prelude::*};

use crate::components::nav::Route;

/// Home page
#[component]
pub fn Home() -> Element {
    rsx! {
        main { class: "p-6",
            div { class: "max-w-4xl mx-auto p-6 bg-white rounded-xl shadow-sm border border-gray-200",
                h1 { class: "text-2xl font-bold mb-2", "Main Content Area" }
                p { class: "text-gray-600", "Hello World!" }
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
        document::Title { "404 - Page Not Found" }
        div { style: "text-align: center; font-family: sans-serif; padding: 100px 20px; background-color: #f9fafb; min-height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center;",

            div { style: "max-width: 400px; background: white; padding: 40px; border-radius: 12px; box-shadow: 0 4px 12px rgba(0,0,0,0.05);",

                h1 { style: "color: #ef4444; font-size: 4em; margin: 0; font-weight: 800;",
                    "404"
                }
                h2 { style: "color: #111827; margin-top: 10px; font-size: 1.5em;",
                    "Page Not Found"
                }

                p { style: "color: #6b7280; font-size: 0.95em; margin: 15px 0 25px 0; line-height: 1.6;",
                    "We couldn't find anything matching "
                    code { style: "background: #f3f4f6; padding: 2px 6px; border-radius: 4px; font-family: monospace; color: #374151;",
                        "/{broken_path}"
                    }
                    ". It might have moved, or a parameter typed into the path was invalid."
                }

                Link {
                    to: Route::Home {},
                    style: "display: inline-block; padding: 12px 24px; background-color: #4f46e5; color: white; border-radius: 6px; text-decoration: none; font-weight: bold; font-size: 0.95em; transition: background 0.2s;",
                    "Return to Homepage"
                }
            }
        }
    }
}
