use super::*;
impl WebApp {
    pub(crate) fn pen_failure(&mut self, ctx: &egui::Context, diagnostic: impl std::fmt::Display) {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(&format!(
            "PaintCraft input: {diagnostic}"
        )));
        #[cfg(not(target_arch = "wasm32"))]
        let _ = diagnostic;
        self.message = t!(
            ctx,
            "Input was cancelled. Lift the pen or release the mouse buttons, then try again."
        );
    }
}
