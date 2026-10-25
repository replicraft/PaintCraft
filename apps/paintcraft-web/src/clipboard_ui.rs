//! Browser Copy captures a private straight-RGBA image and waits for clipboard acknowledgment.
use crate::{app::WebApp, clipboard};
use eframe::egui;
use paintcraft_core::types::{Result, clipboard::Image, resources::SharedBudget};
use paintcraft_editor::clipboard::{Capture, CutSource};
use paintcraft_ui::t;

#[derive(Debug)]
pub(crate) struct State {
    pub(crate) budget: SharedBudget,
    job: Option<clipboard::Job>,
    capture: Option<Capture>,
    last: Option<Capture>,
    activation: Option<u64>,
    cut: Option<CutSource>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            budget: SharedBudget::new(64 << 20),
            job: None,
            capture: None,
            last: None,
            activation: None,
            cut: None,
        }
    }
}
impl State {
    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }
}
impl WebApp {
    pub(crate) fn copy_requested(&mut self, ctx: &egui::Context) {
        self.copy_with(ctx, |image| clipboard::submit(ctx, image));
    }
    pub(crate) fn copy_visible_requested(&mut self, ctx: &egui::Context) {
        self.copy_source_with(ctx, true, |image| clipboard::submit(ctx, image));
    }
    pub(crate) fn cut_requested(&mut self, ctx: &egui::Context) {
        self.cut_with(ctx, |image| clipboard::submit(ctx, image));
    }
    fn cut_with(
        &mut self,
        ctx: &egui::Context,
        submit: impl FnOnce(Image) -> Result<clipboard::Job>,
    ) {
        self.clipboard_with(ctx, false, true, submit);
    }
    fn copy_with(
        &mut self,
        ctx: &egui::Context,
        submit: impl FnOnce(Image) -> Result<clipboard::Job>,
    ) {
        self.copy_source_with(ctx, false, submit);
    }
    fn copy_source_with(
        &mut self,
        ctx: &egui::Context,
        visible: bool,
        submit: impl FnOnce(Image) -> Result<clipboard::Job>,
    ) {
        self.clipboard_with(ctx, visible, false, submit);
    }
    fn clipboard_with(
        &mut self,
        ctx: &egui::Context,
        visible: bool,
        cut: bool,
        submit: impl FnOnce(Image) -> Result<clipboard::Job>,
    ) {
        if self.clipboard.activation == Some(ctx.cumulative_frame_nr()) {
            return;
        }
        self.clipboard.activation = Some(ctx.cumulative_frame_nr());
        if self.paste.busy()
            || self.clipboard.job.is_some()
            || self.blocked()
            || self.editor.tools.gesture_in_progress()
            || self.editor.tools.floating_selection().is_some()
            || self.editor.tools.is_constructing_shape()
            || self.editor.tools.is_editing_text()
            || !self.text.preedit.is_empty()
            || ctx.egui_wants_keyboard_input()
        {
            self.message = t!(ctx, "Finish the current edit before copying.");
            return;
        }
        let target = self.editor.target.filter(|id| {
            self.editor
                .session
                .document()
                .layers
                .get(id)
                .is_some_and(|layer| layer.visible)
        });
        if !visible && target.is_none() {
            self.message = t!(ctx, "There are no selected pixels to copy.");
            return;
        }
        let cut = if cut {
            let clear = paintcraft_tools::commands::canvas_color(self.editor.session.document());
            match CutSource::new(
                &self.editor.session,
                target.expect("validated target"),
                clear,
            ) {
                Ok(cut) => Some(cut),
                Err(error) => {
                    self.message = format!("{}: {error}", t!(ctx, "Clipboard copy failed"));
                    return;
                }
            }
        } else {
            None
        };
        let source = self.editor.session.snapshot();
        let selection = self.editor.session.selection();
        let result = if visible {
            paintcraft_render::clipboard::capture_visible(
                &source,
                &selection,
                &self.clipboard.budget,
                &|| Ok(()),
            )
            .map(|image| {
                image.map(|(image, origin)| Capture {
                    image,
                    origin,
                    selection,
                })
            })
        } else {
            paintcraft_editor::clipboard::capture(
                &source,
                self.editor.session.selection(),
                target.expect("validated target"),
                &self.clipboard.budget,
                &|| Ok(()),
            )
        };
        match result {
            Ok(Some(capture)) => match submit(capture.image.clone()) {
                Ok(job) => {
                    self.clipboard.job = Some(job);
                    self.clipboard.capture = Some(capture);
                    self.clipboard.cut = cut;
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
                    if let Some(cut) = self.clipboard.cut.take() {
                        let safe = !self.blocked()
                            && !self.editor.tools.gesture_in_progress()
                            && self.editor.tools.floating_selection().is_none()
                            && !self.editor.tools.is_constructing_shape()
                            && !self.editor.tools.is_editing_text()
                            && self.text.preedit.is_empty();
                        match cut.removal_batch(&self.editor.session, self.editor.target) {
                            Ok(batch) if safe => {
                                self.message = match self.editor.session.apply(&batch) {
                                    Ok(_) => t!(ctx, "Cut"),
                                    Err(error) => {
                                        format!("{}: {error}", t!(ctx, "Clipboard cut failed"))
                                    }
                                };
                            }
                            _ => {
                                self.message = t!(
                                    ctx,
                                    "Copied, but Cut was cancelled because the source changed."
                                )
                            }
                        }
                    }
                }
                Err(error) => {
                    self.clipboard.cut = None;
                    self.clipboard.capture = None;
                    self.message = format!("{}: {error}", t!(ctx, "Clipboard copy failed"));
                }
            }
        }
    }
    pub(crate) fn cancel_clipboard_cut(&mut self, ctx: &egui::Context) {
        if self.clipboard.cut.take().is_some() {
            self.clipboard.job = None;
            self.clipboard.capture = None;
            self.message = t!(ctx, "Cut cancelled; the canvas was not changed.");
        }
    }
}
#[cfg(test)]
mod tests;
