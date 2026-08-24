use dioxus::{document::eval, prelude::*};

use crate::components::{Theme, mainland::Home, notfound::NotFound};

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[layout(Nav)]
    #[route("/")]
    Home {},
    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

#[component]
pub fn Nav() -> Element {
    let mut is_open = use_signal(|| false);

    // Consume the global theme state instead of creating a local one
    let mut theme = use_context::<Signal<Theme>>();

    let current_route = use_route::<Route>();
    let sidebar_transform = if is_open() {
        "translate-x-0"
    } else {
        "-translate-x-full"
    };

    // The theme button logic remains identical!
    let theme_btn_class = |btn_theme: Theme| {
        let base = "flex-1 py-1.5 text-center rounded-sm transition-all focus:outline-none text-sm";
        if theme() == btn_theme {
            format!(
                "{} bg-white dark:bg-[#4d5358] shadow-sm text-black dark:text-white",
                base
            )
        } else {
            format!(
                "{} text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200",
                base
            )
        }
    };

    let nav_link_class = |target_route: Route| {
        let base = "flex items-center px-6 py-2.5 dark:hover:bg-[#4d5358] hover:bg-[#c6c6c6] transition-colors border-l-4";
        if current_route == target_route {
            format!("{} border-[#4589ff]", base)
        } else {
            format!("{} border-transparent", base)
        }
    };

    let class_home = nav_link_class(Route::Home {});
    let class_projects = nav_link_class(Route::NotFound {
        segments: vec!["projects".to_string()],
    });
    let class_settings = nav_link_class(Route::NotFound {
        segments: vec!["settings".to_string()],
    });

    rsx! {

        // TOP BAR
        header { class: "bg-[#f4f4f4] dark:bg-[#121619] flex items-center justify-between h-14 px-4 shadow-lg",

            div { class: "flex items-center space-x-4",
                button {
                    class: "p-2 text-gray-600 rounded-md hover:bg-gray-100 focus:outline-none",
                    onclick: move |_| is_open.set(true),
                    svg {
                        class: "w-6 h-6",
                        fill: "none",
                        stroke: "currentColor",
                        view_box: "0 0 24 24",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            stroke_width: "2",
                            d: "M4 6h16M4 12h16M4 18h16",
                        }
                    }
                }
                span { class: "text-lg font-semibold", "IBM Research" }
            }

            div { class: "flex items-center",
                button {
                    class: "flex items-center space-x-2 px-3 py-2 text-sm  border-gray-200 dark:border-zinc-800 bg-gray-100 dark:bg-[#161a1d] transition-colors focus:outline-none",
                    onclick: move |_| {
                        // Handle your logout logic here
                        info!("Logging out...");
                    },
                    svg {
                        class: "w-4 h-4",
                        fill: "none",
                        stroke: "currentColor",
                        view_box: "0 0 24 24",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            stroke_width: "2",
                            d: "M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1",
                        }
                    }
                    span { "Logout" }
                }
            }
        }

        // DIMMED BACKGROUND OVERLAY
        if is_open() {
            div {
                class: "fixed inset-0 z-40 bg-black/40 backdrop-blur-sm transition-opacity",
                onclick: move |_| is_open.set(false),
            }
        }

        // SIDEBAR PANEL
        aside { class: "bg-[#f4f4f4] dark:bg-[#343a3f] fixed inset-y-0 left-0 z-50 flex flex-col w-64 shadow-xl {sidebar_transform}",
            div { class: "flex items-center justify-between h-14 px-4 border-b border-gray-200",
                span { class: "text-sm font-bold tracking-wide", "Accelerated Discovery" }
                button {
                    class: "p-2 rounded-md dark:hover:bg-[#4d5358] hover:bg-[#c6c6c6] focus:outline-none",
                    onclick: move |_| is_open.set(false),
                    svg {
                        class: "w-6 h-6",
                        fill: "none",
                        stroke: "currentColor",
                        view_box: "0 0 24 24",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            stroke_width: "2",
                            d: "M6 18L18 6M6 6l12 12",
                        }
                    }
                }
            }

            nav { class: "flex-1 py-4 space-y-1 overflow-y-auto",
                // Pass the evaluated variables directly
                Link {
                    to: Route::Home {},
                    onclick: move |_| is_open.set(false),
                    class: "{class_home}",
                    span { "Dashboard" }
                }
                Link {
                    to: Route::NotFound {
                        segments: vec!["projects".to_string()],
                    },
                    onclick: move |_| is_open.set(false),
                    class: "{class_projects}",
                    span { "Projects" }
                }
                Link {
                    to: Route::NotFound {
                        segments: vec!["settings".to_string()],
                    },
                    onclick: move |_| is_open.set(false),
                    class: "{class_settings}",
                    span { "Settings" }
                }
            }

            div { class: "p-3 border-t border-gray-200 dark:border-zinc-800 bg-gray-100 dark:bg-[#161a1d]",
                div { class: "flex p-1 bg-gray-200 dark:bg-zinc-800 rounded-sm text-xs font-medium",

                    // Light Button
                    button {
                        class: "{theme_btn_class(Theme::Light)}",
                        onclick: move |_| theme.set(Theme::Light),
                        "Light"
                    }

                    // Dark Button
                    button {
                        class: "{theme_btn_class(Theme::Dark)}",
                        onclick: move |_| theme.set(Theme::Dark),
                        "Dark"
                    }

                    // System Button
                    button {
                        class: "{theme_btn_class(Theme::System)}",
                        onclick: move |_| theme.set(Theme::System),
                        "System"
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}
