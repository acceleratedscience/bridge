mod mainland;
mod nav;
mod notfound;
mod login;

use dioxus::document::eval;
use dioxus::prelude::*;

use self::nav::Route;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

// Theme is hoisted to the top-level App component and provided via context to all child components
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[component]
pub fn App() -> Element {
    let mut theme = use_context_provider(|| Signal::new(Theme::System));
    let mut os_is_dark = use_signal(|| false);

    // 3. Move the OS tracking use_effect here
    use_effect(move || {
        spawn(async move {
            let mut os_theme_eval = eval(
                r#"
                const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
                dioxus.send(mediaQuery.matches);
                mediaQuery.addEventListener('change', (e) => {
                    dioxus.send(e.matches);
                });
                "#,
            );
            while let Ok(is_dark) = os_theme_eval.recv::<bool>().await {
                os_is_dark.set(is_dark);
            }
        });
    });

    // 4. Move the class-toggling use_effect here
    use_effect(move || {
        let should_be_dark = match theme() {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => os_is_dark(),
        };
        if should_be_dark {
            let _ = eval("document.documentElement.classList.add('dark');");
        } else {
            let _ = eval("document.documentElement.classList.remove('dark');");
        }
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }

        div { class: "relative min-h-screen", Router::<Route> {} }
    }
}
