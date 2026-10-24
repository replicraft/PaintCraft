//! Browser-owned UI state. Native filesystem, menus and workers stay in desktop.
mod app;
mod canvas;
mod clipboard;
mod clipboard_ui;
mod editor;
mod files;
mod floating_ui;
mod layers;
mod preferences;
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
    use wasm_bindgen::prelude::*;

    /// JavaScript retains this handle until the page is closed or destroyed.
    #[derive(Clone)]
    #[wasm_bindgen]
    pub struct WebHandle {
        runner: eframe::WebRunner,
    }

    impl Default for WebHandle {
        fn default() -> Self {
            Self {
                runner: eframe::WebRunner::new(),
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
            self.runner
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| Ok(Box::new(super::WebApp::new(&cc.egui_ctx)))),
                )
                .await
        }

        /// Used by the host page's unsaved-work guard; active gestures count too.
        pub fn is_dirty(&self) -> bool {
            self.runner
                .app_mut::<super::WebApp>()
                .is_some_and(|app| app.unsaved() || app.editor.tools.gesture_in_progress())
        }

        pub fn destroy(&self) {
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
