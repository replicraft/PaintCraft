//! Private printable copies never publish edits or mark a project saved.
use crate::WebApp;
use eframe::egui;
use paintcraft_core::types::Result;
use paintcraft_document::Document;
use paintcraft_formats::image::{ImageFormat, encode};
use paintcraft_ui::t;
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Default)]
pub(crate) struct State {
    pending: bool,
    outcome: Rc<RefCell<Option<&'static str>>>,
    #[cfg(test)]
    pub captured: Option<(Vec<u8>, String)>,
}

pub(crate) fn capture(document: &Document) -> Result<Vec<u8>> {
    if u64::from(document.canvas.0) * u64::from(document.canvas.1) > crate::editor::MAX_PIXELS {
        return Err(paintcraft_core::types::Error::new(
            paintcraft_core::types::ErrorCode::ResourceLimit,
            "Browser canvas limit is 4 megapixels",
        ));
    }
    crate::spatial_host::with_document(document, || paintcraft_render::render_flat(document))
        .and_then(|rgba| {
            encode(
                ImageFormat::Png,
                document.canvas.0,
                document.canvas.1,
                &rgba,
                90,
            )
        })
}

impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl State {
    fn request(&mut self, ctx: &egui::Context, bytes: Vec<u8>, title: String) {
        if self.pending {
            return;
        }
        self.pending = true;
        #[cfg(target_arch = "wasm32")]
        {
            let promise = browser::print_image(&bytes, &title);
            let target = Rc::clone(&self.outcome);
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = wasm_bindgen_futures::JsFuture::from(promise).await;
                *target.borrow_mut() = Some(match result {
                    Ok(value) if value.as_string().as_deref() == Some("closed") => {
                        "Print dialog closed."
                    }
                    Ok(_) => "Printing cancelled.",
                    Err(_) => "Printing is unavailable or the browser denied the request.",
                });
                ctx.request_repaint();
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ctx;
            #[cfg(test)]
            {
                self.captured = Some((bytes, title));
            }
            #[cfg(not(test))]
            {
                let _ = (bytes, title);
                *self.outcome.borrow_mut() =
                    Some("Printing is unavailable or the browser denied the request.");
            }
        }
    }
    fn cancel(&mut self) {
        if !self.pending {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        browser::cancel_print();
        #[cfg(not(target_arch = "wasm32"))]
        {
            *self.outcome.borrow_mut() = Some("Printing cancelled.");
        }
    }
}

impl WebApp {
    pub(crate) fn print_requested(&mut self, ctx: &egui::Context) {
        if self.blocked() || !self.floating_file_ready() {
            return;
        }
        if self.editor.tools.is_constructing_shape() {
            self.message = paintcraft_tools::FINISH_SHAPE_BEFORE_FILE.into();
            return;
        }
        if self.text.is_composing() {
            self.message = t!(ctx, "Finish text composition before printing.");
            return;
        }
        self.editor.tools.cancel_gesture();
        self.sync_text();
        let snapshot = self.editor.copy_snapshot();
        match snapshot.and_then(|document| capture(&document)) {
            Ok(bytes) => self.printer.request(ctx, bytes, self.document_title()),
            Err(error) => self.message = error.to_string(),
        }
    }

    pub(crate) fn printing_pending(&self) -> bool {
        self.printer.pending
    }

    pub(crate) fn poll_print(&mut self) {
        if let Some(message) = self.printer.outcome.borrow_mut().take() {
            self.printer.pending = false;
            self.message = message.into();
        }
    }

    pub(crate) fn print_progress(&mut self, ctx: &egui::Context) {
        if !self.printer.pending {
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("web-print-progress")).show(ctx, |ui| {
            ui.heading(t!(ui.ctx(), "Printing"));
            ui.label(t!(
                ui.ctx(),
                "Preparing the document or waiting for the print dialog."
            ));
            ui.button(t!(ui.ctx(), "Cancel")).clicked()
        });
        if response.inner || response.should_close() {
            self.printer.cancel();
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(module = "/src/lib/printing.js")]
    extern "C" {
        #[wasm_bindgen(js_name = printImage)]
        pub fn print_image(bytes: &[u8], title: &str) -> js_sys::Promise;
        #[wasm_bindgen(js_name = cancelPrint)]
        pub fn cancel_print();
    }
}

#[cfg(test)]
mod tests;
