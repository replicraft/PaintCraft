//! Browser-owned fullscreen, with DOM notifications and nonoptimistic menu state.
use eframe::egui;
use std::{cell::Cell, rc::Rc};

pub(crate) const UNAVAILABLE: &str =
    "Full screen is unavailable or the browser denied the request.";

#[derive(Debug, Default)]
pub(crate) struct State {
    active: bool,
    pending: Rc<Cell<bool>>,
    failed: Rc<Cell<bool>>,
    #[cfg(target_arch = "wasm32")]
    listener: Option<browser::Listener>,
}

impl State {
    pub fn active(&self) -> bool {
        self.active
    }

    pub fn exit(&mut self, ctx: &egui::Context) {
        // A native Escape may already have exited before egui sees the event.
        #[cfg(target_arch = "wasm32")]
        if !browser::fullscreen_active() {
            return;
        }
        if self.active {
            self.toggle(ctx);
        }
    }

    /// Returns an error notification once. External exits wake the UI through
    /// fullscreenchange rather than a permanent timer or animation loop.
    pub fn refresh(&mut self, ctx: &egui::Context) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            if self.listener.is_none() {
                self.listener = browser::Listener::new(ctx.clone(), Rc::clone(&self.failed));
            }
            self.active = browser::fullscreen_active();
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = ctx;
        self.failed.replace(false)
    }

    pub fn toggle(&mut self, ctx: &egui::Context) {
        if self.pending.replace(true) {
            return;
        }
        self.failed.set(false);
        #[cfg(target_arch = "wasm32")]
        {
            let promise = browser::toggle_fullscreen();
            let pending = Rc::clone(&self.pending);
            let failed = Rc::clone(&self.failed);
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if wasm_bindgen_futures::JsFuture::from(promise).await.is_err() {
                    failed.set(true);
                }
                pending.set(false);
                ctx.request_repaint();
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending.set(false);
            self.failed.set(true);
            ctx.request_repaint();
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use wasm_bindgen::{JsCast, closure::Closure, prelude::*};

    #[wasm_bindgen(module = "/src/lib/fullscreen.js")]
    unsafe extern "C" {
        #[wasm_bindgen(js_name = fullscreenActive)]
        pub fn fullscreen_active() -> bool;
        #[wasm_bindgen(js_name = toggleFullscreen)]
        pub fn toggle_fullscreen() -> js_sys::Promise;
    }

    pub struct Listener {
        document: web_sys::Document,
        callback: Closure<dyn FnMut(web_sys::Event)>,
    }
    impl std::fmt::Debug for Listener {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("FullscreenListener").finish_non_exhaustive()
        }
    }
    impl Listener {
        pub fn new(ctx: egui::Context, failed: Rc<Cell<bool>>) -> Option<Self> {
            let document = web_sys::window()?.document()?;
            let callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
                match event.type_().as_str() {
                    "fullscreenerror" => failed.set(true),
                    "keydown" => {
                        let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                            return;
                        };
                        if key.key() != "F11"
                            || key.alt_key()
                            || key.ctrl_key()
                            || key.meta_key()
                            || key.shift_key()
                        {
                            return;
                        }
                        // Let egui route F11, but suppress the browser's separate
                        // window-fullscreen toggle so it cannot also run.
                        event.prevent_default();
                    }
                    "fullscreenchange" => (),
                    _ => return,
                }
                ctx.request_repaint();
            }) as Box<dyn FnMut(web_sys::Event)>);
            let listener = Self { document, callback };
            for name in ["fullscreenchange", "fullscreenerror"] {
                listener
                    .document
                    .add_event_listener_with_callback(
                        name,
                        listener.callback.as_ref().unchecked_ref(),
                    )
                    .ok()?;
            }
            listener
                .document
                .add_event_listener_with_callback_and_bool(
                    "keydown",
                    listener.callback.as_ref().unchecked_ref(),
                    true,
                )
                .ok()?;
            Some(listener)
        }
    }
    impl Drop for Listener {
        fn drop(&mut self) {
            for name in ["fullscreenchange", "fullscreenerror"] {
                let _ = self.document.remove_event_listener_with_callback(
                    name,
                    self.callback.as_ref().unchecked_ref(),
                );
            }
            let _ = self.document.remove_event_listener_with_callback_and_bool(
                "keydown",
                self.callback.as_ref().unchecked_ref(),
                true,
            );
        }
    }
}
