pub mod mainland;
pub mod nav;
pub mod notfound;

use leptos::prelude::*;
use leptos_meta::{MetaTags, provide_meta_context};
use leptos_router::{
    components::{ParentRoute, Route, Router, Routes},
    path,
};
use wasm_bindgen::{JsCast, closure::Closure};

use self::{mainland::Home, nav::Nav, notfound::NotFound};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let theme = RwSignal::new(Theme::System);
    provide_context(theme);

    // Read the initial OS preference synchronously
    let initial_os_dark = web_sys::window()
        .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok().flatten())
        .map(|m| m.matches())
        .unwrap_or(false);
    let os_is_dark = RwSignal::new(initial_os_dark);

    // Track OS dark mode preference changes
    Effect::new(move |_| {
        if let Some(win) = web_sys::window()
            && let Ok(Some(mql)) = win.match_media("(prefers-color-scheme: dark)")
        {
            let closure = Closure::<dyn FnMut(web_sys::MediaQueryListEvent)>::new(
                move |e: web_sys::MediaQueryListEvent| {
                    os_is_dark.set(e.matches());
                },
            );
            let _ =
                mql.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
            closure.forget();
        }
    });

    // Synchronize the "dark" class on <html>
    Effect::new(move |_| {
        let should_be_dark = match theme.get() {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => os_is_dark.get(),
        };

        if let Some(doc) = web_sys::window().and_then(|w| w.document())
            && let Some(el) = doc.document_element()
        {
            let list = el.class_list();
            if should_be_dark {
                let _ = list.add_1("dark");
            } else {
                let _ = list.remove_1("dark");
            }
        }
    });

    view! {
        <MetaTags />
        <div class="relative min-h-screen">
            <Router>
                <Routes fallback=|| view! { <NotFound /> }>
                    <ParentRoute path=path!("") view=Nav>
                        <Route path=path!("") view=Home />
                    </ParentRoute>
                    <Route path=path!("*any") view=NotFound />
                </Routes>
            </Router>
        </div>
    }
}
