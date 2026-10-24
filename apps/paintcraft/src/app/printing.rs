//! Capture a committed document before handing a PNG to the native print panel.
use super::*;
use paintcraft_platform::printing::PrintOutcome;
use paintcraft_ui::t;

impl App {
    pub(super) fn print_requested(&mut self, ctx: &egui::Context) {
        if self.document_count() == 0 || self.printer.pending() || !self.editor.shape_file_ready() {
            return;
        }
        self.editor.cancel_gesture();
        self.editor.commit_text();
        self.editor.run_pending();
        if self.editing.is_some() {
            self.defer_file(editing::FileRequest::Print, 1027);
        } else {
            self.capture_print(ctx);
        }
    }

    pub(super) fn capture_print(&mut self, ctx: &egui::Context) {
        if self.printer.pending() || !self.editor.can_capture_file() {
            return;
        }
        let document = self.editor.document();
        let (width, height) = document.canvas;
        // Bound the full compositing/PNG capture before allocating it.
        if u64::from(width) * u64::from(height) > 16_777_216 {
            self.editor.set_status(t!(
                ctx,
                "Printing is limited to 16 megapixels per document."
            ));
            return;
        }
        let png = paintcraft_render::render_flat(document).and_then(|rgba| {
            paintcraft_formats::image::encode(ImageFormat::Png, width, height, &rgba, 90)
        });
        match png {
            Ok(png) => {
                let wake = ctx.clone();
                if let Err(error) =
                    self.printer
                        .request(png, self.document_window_title(), move || {
                            wake.request_repaint()
                        })
                {
                    self.editor.set_status(error);
                }
            }
            Err(error) => self.editor.set_status(error.to_string()),
        }
    }

    pub(super) fn poll_print(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.printer.poll() {
            match result {
                Ok(PrintOutcome::Printed) => {
                    self.editor.set_status(t!(ctx, "Print job submitted."))
                }
                Ok(PrintOutcome::Cancelled) => {
                    self.editor.set_status(t!(ctx, "Printing cancelled."))
                }
                Err(error) => self.editor.set_status(error),
            }
        }
        if self.printer.pending() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
