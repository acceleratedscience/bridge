use leptos::{logging::log, prelude::*};
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

#[component]
pub fn NotFound() -> impl IntoView {
    let params = use_params_map();
    let broken_path = move || params.read().get("any").unwrap_or_default();

    Effect::new(move |_| {
        log!("Hello from Leptos");
    });

    view! {
        <Title text="Page Not Found" />
        <Meta name="robots" content="noindex, nofollow" />
        <div class="flex h-screen">
            <div class="m-auto text-center p-5 rounded-b-sm shadow-lg bg-[#f4f4f4] dark:bg-[#121619] max-w-[400px]">
                <h1 style="color: #ef4444; font-size: 4em; margin: 0; font-weight: 800;">"404"</h1>
                <h2 style="margin-top: 10px; font-size: 1.5em;">"Page Not Found"</h2>
                <p style="color: #6b7280; font-size: 0.95em; margin: 15px 0 25px 0; line-height: 1.6;">
                    "We couldn't find anything matching "
                    <code style="background: #f3f4f6; padding: 2px 6px; border-radius: 4px; font-family: monospace; color: #374151;">
                        {move || format!("/{}", broken_path())}
                    </code>
                    ". It might have moved, or a parameter typed into the path was invalid."
                </p>
                <a href="/" class="bg-blue-700 p-3 rounded-b-sm text-white">"Return to Homepage"</a>
            </div>
        </div>
    }
}
