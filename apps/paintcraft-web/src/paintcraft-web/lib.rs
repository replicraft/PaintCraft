//! Browser-owned UI state. Native filesystem, menus and workers stay in desktop.
mod app;
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
mod preferences;
mod printing;
mod selection_ui;
mod text_ui;
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
    use std::{cell::RefCell, rc::Rc};
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
    #[derive(Clone)]
    #[wasm_bindgen]
    pub struct WebHandle {
        runner: eframe::WebRunner,
        codec: Rc<RefCell<Option<CodecService>>>,
    }

    impl Default for WebHandle {
        fn default() -> Self {
            Self {
                runner: eframe::WebRunner::new(),
                codec: Rc::new(RefCell::new(None)),
            }
        }
    }

    impl std::fmt::Debug for WebHandle {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("WebHandle").finish_non_exhaustive()
        }
    }

    #[wasm_bindgen]
    impl WebHandle {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            Self::default()
        }

        pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
            if !paintcraft_ui::fonts::web_cjk_ready() {
                return Err(JsValue::from_str("Browser fonts have not been loaded"));
            }
            self.runner
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| Ok(Box::new(super::WebApp::new(&cc.egui_ctx)))),
                )
                .await
        }

        pub fn application_version(&self) -> String {
            env!("CARGO_PKG_VERSION").into()
        }
        /// Install the Vite-owned service; selected files cannot supply modules.
        pub fn install_codec_service(&self, value: JsValue) -> Result<(), JsValue> {
            let service: CodecService = value.unchecked_into();
            if service.application_version()? != env!("CARGO_PKG_VERSION")
                || service.protocol_version()?
                    != paintcraft_formats::codec_worker::PROTOCOL_VERSION as u32
            {
                return Err(JsValue::from_str("Codec service version/protocol mismatch"));
            }
            if self.codec.borrow().is_some() {
                return Err(JsValue::from_str("Codec service already installed"));
            }
            let value: &JsValue = service.as_ref();
            self.runner
                .app_mut::<super::WebApp>()
                .ok_or_else(|| JsValue::from_str("Editor not started"))?
                .jxl
                .attach(value.clone());
            *self.codec.borrow_mut() = Some(service);
            Ok(())
        }
        pub fn codec_service(&self) -> JsValue {
            self.codec
                .borrow()
                .as_ref()
                .map_or(JsValue::NULL, |service| {
                    let value: &JsValue = service.as_ref();
                    value.clone()
                })
        }
        pub fn codec_service_budget(&self) -> Result<JsValue, JsValue> {
            self.codec
                .borrow()
                .as_ref()
                .ok_or_else(|| JsValue::from_str("Codec service missing"))?
                .budget()
        }
        /// Used by the host page's unsaved-work guard; active gestures count too.
        pub fn is_dirty(&self) -> bool {
            self.runner
                .app_mut::<super::WebApp>()
                .is_some_and(|app| app.unsaved() || app.editor.tools.gesture_in_progress())
        }

        pub fn destroy(&self) {
            if let Some(service) = self.codec.borrow().as_ref() {
                let _ = service.cancel();
            }
            self.runner.destroy();
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
