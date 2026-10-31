//! Browser-owned UI state. Native filesystem, menus and workers stay in desktop.
mod app;
#[cfg(any(test, target_arch = "wasm32"))]
mod browser_canvas;
#[cfg(any(test, target_arch = "wasm32"))]
mod browser_lifecycle;
mod canvas;
mod canvas_navigation;
mod clipboard;
mod clipboard_ui;
mod editor;
mod files;
mod floating_ui;
mod fullscreen;
mod importing;
mod layers;
mod local_fonts;
mod paste_ui;
mod pen_input;
mod preferences;
mod printing;
mod selection_ui;
mod spatial_host;
mod text_ui;
mod touch_navigation;
pub use app::WebApp;

#[cfg(test)]
mod app_tests;
#[cfg(test)]
mod construction_tests;
#[cfg(test)]
mod controls_tests;
#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod forms_tests;
#[cfg(test)]
mod text_tests;

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::browser_canvas::{ClaimedStart, Error as CanvasError, dom};
    use super::browser_lifecycle::{Lifecycle, Resources, StartError};
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(typescript_type = "CodecService")]
        type CodecService;
        #[wasm_bindgen(method, catch, js_name = applicationVersion)]
        fn application_version(this: &CodecService) -> Result<String, JsValue>;
        #[wasm_bindgen(method, catch, js_name = protocolVersion)]
        fn protocol_version(this: &CodecService) -> Result<u32, JsValue>;
        #[wasm_bindgen(method, catch)]
        fn budget(this: &CodecService) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(method, catch)]
        fn cancel(this: &CodecService) -> Result<(), JsValue>;
    }

    #[wasm_bindgen]
    pub fn install_cjk_fonts(japanese: Vec<u8>, chinese: Vec<u8>) -> Result<(), JsValue> {
        paintcraft_ui::fonts::install_web_cjk_fonts(japanese, chinese).map_err(JsValue::from_str)
    }

    /// JavaScript retains this handle until the page is closed or destroyed.
    #[wasm_bindgen]
    pub struct WebHandle {
        lifecycle: Lifecycle<BrowserResources>,
        owner_abort: web_sys::AbortController,
    }

    struct BrowserResources {
        runner: eframe::WebRunner,
        codec: RefCell<Option<CodecService>>,
        pen: RefCell<Option<super::pen_input::dom::Guard>>,
        canvas: RefCell<Option<dom::CanvasLease>>,
        owner: JsValue,
    }
    impl Resources for BrowserResources {
        fn stop(&self) {
            let guard = self.pen.borrow_mut().take();
            drop(guard);
            // Remove the installed binding before invoking external JS; a
            // restarted editor needs an explicitly installed service of its own.
            // cancel keeps the service controller's pending Worker charges in
            // quarantine until terminal acknowledgement; no budget is reset here.
            let service = self.codec.borrow_mut().take();
            if let Some(service) = service {
                let _ = service.cancel();
            }
            self.runner.destroy();
            let canvas = self.canvas.borrow_mut().take();
            drop(canvas);
        }
    }

    impl std::fmt::Debug for WebHandle {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("WebHandle").finish_non_exhaustive()
        }
    }

    impl Drop for WebHandle {
        fn drop(&mut self) {
            // Host closures must stop referring to the public JS wrapper before
            // its internal runner/resources begin their own teardown.
            self.owner_abort.abort();
        }
    }

    #[wasm_bindgen]
    impl WebHandle {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Result<Self, JsValue> {
            let owner_abort = web_sys::AbortController::new()?;
            Ok(Self {
                lifecycle: Lifecycle::new(BrowserResources {
                    runner: eframe::WebRunner::new(),
                    codec: RefCell::new(None),
                    pen: RefCell::new(None),
                    canvas: RefCell::new(None),
                    owner: dom::owner(),
                }),
                owner_abort,
            })
        }

        /// Read-only lifetime for host callbacks. Destroy permits a restart;
        /// freeing this public owner aborts its host callbacks permanently.
        pub fn owner_lifetime(&self) -> web_sys::AbortSignal {
            self.owner_abort.signal()
        }

        /// Rejects concurrent initialization. After destroy, a pending GPU
        /// request must settle and clean up before another start can begin.
        /// A new editor requires an explicitly installed codec service.
        #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
        pub fn start(&self, canvas: web_sys::HtmlCanvasElement) -> js_sys::Promise {
            if !paintcraft_ui::fonts::web_cjk_ready() {
                return js_sys::Promise::reject(&JsValue::from_str(
                    "Browser fonts have not been loaded",
                ));
            }
            let claim = match dom::CanvasLease::acquire(
                dom::Map,
                canvas.clone(),
                self.lifecycle.resources().owner.clone(),
            ) {
                Ok(claim) => claim,
                Err(error) => {
                    return js_sys::Promise::reject(&JsValue::from_str(match error {
                        CanvasError::Busy => "Canvas is owned by another editor",
                        CanvasError::Capacity => "Canvas ownership capacity exhausted",
                        CanvasError::Storage => "Canvas ownership storage unavailable",
                    }));
                }
            };
            let start = match self.lifecycle.begin() {
                Ok(start) => ClaimedStart::new(start, claim),
                Err(error) => {
                    return js_sys::Promise::reject(&JsValue::from_str(match error {
                        StartError::Busy => "Editor startup is still in progress",
                        StartError::Exhausted => "Editor startup generation exhausted",
                    }));
                }
            };
            // The future owns its lease, not a borrowed public WebHandle. JS
            // free/drop can therefore retire it while GPU startup is awaiting.
            wasm_bindgen_futures::future_to_promise(async move {
                let cancelled = || JsValue::from_str("Editor startup was cancelled");
                if !start.current() {
                    return Err(cancelled());
                }
                let resources = start.resources();
                let cancel_target: web_sys::EventTarget = canvas.clone().unchecked_into();
                resources
                    .runner
                    .start(
                        canvas.clone(),
                        eframe::WebOptions::default(),
                        Box::new(|cc| Ok(Box::new(super::WebApp::new(&cc.egui_ctx)))),
                    )
                    .await?;
                if !start.current() {
                    return Err(cancelled());
                }
                // eframe 0.37 queues TouchCancel without requesting a repaint.
                resources.runner.add_event_listener(
                    &cancel_target,
                    "touchcancel",
                    |_: web_sys::Event, runner| runner.egui_ctx().request_repaint(),
                )?;
                *resources.pen.borrow_mut() =
                    Some(super::pen_input::dom::install(&resources.runner, canvas)?);
                start
                    .commit(|resources, claim| *resources.canvas.borrow_mut() = Some(claim))
                    .map_err(|()| cancelled())?;
                Ok(JsValue::UNDEFINED)
            })
        }

        pub fn application_version(&self) -> String {
            env!("CARGO_PKG_VERSION").into()
        }
        /// Install the Vite-owned service; selected files cannot supply modules.
        pub fn install_codec_service(&self, value: JsValue) -> Result<(), JsValue> {
            if !self.lifecycle.running() {
                return Err(JsValue::from_str("Editor not started"));
            }
            let resources = self.lifecycle.resources();
            let service: CodecService = value.unchecked_into();
            if service.application_version()? != env!("CARGO_PKG_VERSION")
                || service.protocol_version()?
                    != paintcraft_formats::codec_worker::PROTOCOL_VERSION as u32
            {
                return Err(JsValue::from_str("Codec service version/protocol mismatch"));
            }
            if resources.codec.borrow().is_some() {
                return Err(JsValue::from_str("Codec service already installed"));
            }
            let value: &JsValue = service.as_ref();
            resources
                .runner
                .app_mut::<super::WebApp>()
                .ok_or_else(|| JsValue::from_str("Editor not started"))?
                .tiff
                .attach(value.clone());
            resources
                .runner
                .app_mut::<super::WebApp>()
                .ok_or_else(|| JsValue::from_str("Editor not started"))?
                .jxl
                .attach(value.clone());
            resources
                .runner
                .app_mut::<super::WebApp>()
                .ok_or_else(|| JsValue::from_str("Editor not started"))?
                .dds
                .attach(value.clone());
            resources
                .runner
                .app_mut::<super::WebApp>()
                .ok_or_else(|| JsValue::from_str("Editor not started"))?
                .msp
                .attach(value.clone());
            *resources.codec.borrow_mut() = Some(service);
            Ok(())
        }
        pub fn codec_service(&self) -> JsValue {
            self.lifecycle
                .resources()
                .codec
                .borrow()
                .as_ref()
                .map_or(JsValue::NULL, |service| {
                    let value: &JsValue = service.as_ref();
                    value.clone()
                })
        }
        pub fn codec_service_budget(&self) -> Result<JsValue, JsValue> {
            let value = self
                .lifecycle
                .resources()
                .codec
                .borrow()
                .as_ref()
                .map(|service| {
                    let value: &JsValue = service.as_ref();
                    value.clone()
                })
                .ok_or_else(|| JsValue::from_str("Codec service missing"))?;
            value.unchecked_into::<CodecService>().budget()
        }
        /// Used by the host page's unsaved-work guard; active gestures count too.
        pub fn is_dirty(&self) -> bool {
            self.lifecycle
                .resources()
                .runner
                .app_mut::<super::WebApp>()
                .is_some_and(|app| app.unsaved() || app.editor.tools.gesture_in_progress())
        }

        /// Retire input and the installed service binding before the runner.
        /// Worker cancellation does not claim that its memory was reclaimed.
        pub fn destroy(&self) {
            self.lifecycle.destroy();
        }
    }
}

#[cfg(test)]
mod picker_tests;

#[cfg(test)]
mod media_tests;

#[cfg(test)]
mod language_tests;

#[cfg(test)]
mod canvas_anchor_tests;

#[cfg(test)]
mod responsive_tests;

#[cfg(test)]
mod touch_navigation_tests;
