//! Browser Copy captures a private straight-RGBA image and waits for clipboard acknowledgment.
use crate::{app::WebApp, clipboard};
use eframe::egui;
use paintcraft_core::types::{Result, clipboard::Image, resources::SharedBudget};
use paintcraft_editor::clipboard::Capture;
use paintcraft_ui::t;

#[derive(Debug)]
pub(crate) struct State {
    budget: SharedBudget,
    job: Option<clipboard::Job>,
    capture: Option<Capture>,
    last: Option<Capture>,
    activation: Option<u64>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            budget: SharedBudget::new(64 << 20),
            job: None,
            capture: None,
            last: None,
            activation: None,
        }
    }
}
impl WebApp {
    pub(crate) fn copy_requested(&mut self, ctx: &egui::Context) {
        self.copy_with(ctx, |image| clipboard::submit(ctx, image));
    }
    fn copy_with(
        &mut self,
        ctx: &egui::Context,
        submit: impl FnOnce(Image) -> Result<clipboard::Job>,
    ) {
        if self.clipboard.activation == Some(ctx.cumulative_frame_nr()) {
            return;
        }
        self.clipboard.activation = Some(ctx.cumulative_frame_nr());
        if self.clipboard.job.is_some()
            || self.blocked()
            || self.editor.tools.gesture_in_progress()
            || self.editor.tools.is_editing_text()
            || !self.text.preedit.is_empty()
            || ctx.egui_wants_keyboard_input()
        {
            self.message = t!(ctx, "Finish the current edit before copying.");
            return;
        }
        let Some(target) = self.editor.target.filter(|id| {
            self.editor
                .session
                .document()
                .layers
                .get(id)
                .is_some_and(|layer| layer.visible)
        }) else {
            self.message = t!(ctx, "There are no selected pixels to copy.");
            return;
        };
        let source = self.editor.session.snapshot();
        let result = paintcraft_editor::clipboard::capture(
            &source,
            self.editor.session.selection(),
            target,
            &self.clipboard.budget,
            &|| Ok(()),
        );
        match result {
            Ok(Some(capture)) => match submit(capture.image.clone()) {
                Ok(job) => {
                    self.clipboard.job = Some(job);
                    self.clipboard.capture = Some(capture);
                    self.message = t!(ctx, "Copying to clipboard…");
                }
                Err(error) => {
                    self.message = format!("{}: {error}", t!(ctx, "Clipboard copy failed"))
                }
            },
            Ok(None) => self.message = t!(ctx, "There are no selected pixels to copy."),
            Err(error) => self.message = format!("{}: {error}", t!(ctx, "Clipboard copy failed")),
        }
    }
    pub(crate) fn poll_copy(&mut self, ctx: &egui::Context) {
        if let Some(result) = self
            .clipboard
            .job
            .as_mut()
            .and_then(clipboard::Job::try_result)
        {
            self.clipboard.job = None;
            match result {
                Ok(()) => {
                    self.clipboard.last = self.clipboard.capture.take();
                    self.message = t!(ctx, "Copied to clipboard.");
                }
                Err(error) => {
                    self.clipboard.capture = None;
                    self.message = format!("{}: {error}", t!(ctx, "Clipboard copy failed"));
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
