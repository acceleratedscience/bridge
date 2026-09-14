use leptos::{logging::log, prelude::*};
use leptos_router::{components::Outlet, hooks::use_location};

use crate::components::Theme;

#[component]
pub fn Nav() -> impl IntoView {
    let is_open = RwSignal::new(false);
    let theme = expect_context::<RwSignal<Theme>>();
    let location = use_location();

    let theme_btn_class = move |btn_theme: Theme| {
        let base = "flex-1 py-1.5 text-center rounded-sm transition-all focus:outline-none text-sm";
        if theme.get() == btn_theme {
            let light = "bg-white dark:bg-[#4d5358] shadow-sm text-black dark:text-white";
            format!("{base} {light}")
        } else {
            let dark = "text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200";
            format!(
                "{base} {dark}"
            )
        }
    };

    let nav_link_class = move |target_prefix: &'static str| {
        let path = location.pathname.get();
        let is_active = if target_prefix == "/" {
            path == "/"
        } else {
            path.starts_with(target_prefix)
        };

        let base = "flex items-center px-6 py-2.5 dark:hover:bg-[#4d5358] hover:bg-[#c6c6c6] transition-colors border-l-4";
        if is_active {
            format!("{base} border-[#4589ff]")
        } else {
            format!("{base} border-transparent")
        }
    };

    view! {
        // TOP BAR
        <header class="bg-[#f4f4f4] dark:bg-[#121619] flex items-center justify-between h-14 px-4 shadow-lg">
            <div class="flex items-center space-x-4">
                <button
                    class="p-2 text-gray-600 rounded-md hover:bg-gray-100 focus:outline-none"
                    on:click=move |_| is_open.set(true)
                >
                    <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M4 6h16M4 12h16M4 18h16"
                        />
                    </svg>
                </button>
                <span class="text-lg font-semibold">"IBM Research"</span>
            </div>
            <div class="flex items-center">
                <button
                    class="flex items-center space-x-2 px-3 py-2 text-sm border-gray-200 dark:border-zinc-800 bg-gray-100 dark:bg-[#161a1d] transition-colors focus:outline-none"
                    on:click=move |_| log!("Logging out...")
                >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"
                        />
                    </svg>
                    <span>"Logout"</span>
                </button>
            </div>
        </header>

        // DIMMED BACKGROUND OVERLAY
        {move || {
            is_open.get().then(|| {
                view! {
                    <div
                        class="fixed inset-0 z-40 bg-black/40 backdrop-blur-sm transition-opacity"
                        on:click=move |_| is_open.set(false)
                    />
                }
            })
        }}

        // SIDEBAR PANEL
        <aside class=move || {
            let transform = if is_open.get() { "translate-x-0" } else { "-translate-x-full" };
            format!("bg-[#f4f4f4] dark:bg-[#343a3f] fixed inset-y-0 left-0 z-50 flex flex-col w-64 shadow-xl transition-transform {transform}")
        }>
            <div class="flex items-center justify-between h-14 px-4 border-b border-gray-200">
                <span class="text-sm font-bold tracking-wide">"Accelerated Discovery"</span>
                <button
                    class="p-2 rounded-md dark:hover:bg-[#4d5358] hover:bg-[#c6c6c6] focus:outline-none"
                    on:click=move |_| is_open.set(false)
                >
                    <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M6 18L18 6M6 6l12 12"
                        />
                    </svg>
                </button>
            </div>

            <nav class="flex-1 py-4 space-y-1 overflow-y-auto">
                <a
                    href="/"
                    class=move || nav_link_class("/")
                    on:click=move |_| is_open.set(false)
                >
                    <span>"Dashboard"</span>
                </a>
                <a
                    href="/projects"
                    class=move || nav_link_class("/projects")
                    on:click=move |_| is_open.set(false)
                >
                    <span>"Projects"</span>
                </a>
                <a
                    href="/settings"
                    class=move || nav_link_class("/settings")
                    on:click=move |_| is_open.set(false)
                >
                    <span>"Settings"</span>
                </a>
            </nav>

            <div class="p-3 border-t border-gray-200 dark:border-zinc-800 bg-gray-100 dark:bg-[#161a1d]">
                <div class="flex p-1 bg-gray-200 dark:bg-zinc-800 rounded-sm text-xs font-medium">
                    <button
                        class=move || theme_btn_class(Theme::Light)
                        on:click=move |_| theme.set(Theme::Light)
                    >
                        "Light"
                    </button>
                    <button
                        class=move || theme_btn_class(Theme::Dark)
                        on:click=move |_| theme.set(Theme::Dark)
                    >
                        "Dark"
                    </button>
                    <button
                        class=move || theme_btn_class(Theme::System)
                        on:click=move |_| theme.set(Theme::System)
                    >
                        "System"
                    </button>
                </div>
            </div>
        </aside>

        <Outlet />
    }
}
