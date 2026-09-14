use leptos::prelude::*;
use leptos_meta::Title;

#[component]
pub fn Home() -> impl IntoView {
    let jwks_data = LocalResource::new(move || async move {
        let response =
            reqwest::get("https://api.mydummyapi.com/comments/1").await;
        match response {
            Ok(res) => res
                .json()
                .await
                .unwrap_or_else(|e| format!("Failed to read text: {e}")),
            Err(e) => format!("Network error: {e:?}"),
        }
    });

    view! {
        <Title text="Bridge Portal" />
        <main class="p-6">
            <div class="max-w-7xl mx-auto p-6 bg-[#f4f4f4] dark:bg-[#121619] rounded-sm shadow-lg">
                <h1 class="text-gray-300 text-2xl font-bold mb-2 text-center">"Bridge Portal"</h1>
                <p class="text-gray-300">"Hello World!"</p>
                <Suspense fallback=move || view! { <p class="text-gray-300">"Loading..."</p> }>
                    <p class="text-gray-300">{move || jwks_data.get()}</p>
                </Suspense>
            </div>
        </main>
    }
}
