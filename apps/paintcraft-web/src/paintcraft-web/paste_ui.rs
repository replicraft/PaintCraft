//! Browser-owned image paste; publication uses shared editor transactions.
use crate::{app::WebApp, clipboard};
use eframe::egui;
use paintcraft_classic::ClassicVariant;
use paintcraft_document::Document;
use paintcraft_editor::{
    clipboard::{PasteCanvas, PasteDraft},
    session::HistoryTicket,
};
use paintcraft_ui::{i18n::import_loss_description, t};
use std::sync::Arc;
mod placement;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Intent {
    #[default]
    NewLayer,
    CurrentLayer,
}

#[derive(Debug)]
struct Owner {
    document: Arc<Document>,
    ticket: HistoryTicket,
    variant: ClassicVariant,
    target: Option<uuid::Uuid>,
}
impl Owner {
    fn capture(app: &WebApp) -> Self {
        Self {
            document: app.editor.session.snapshot(),
            ticket: app.editor.session.history_ticket(),
            variant: app.variant,
            target: app.editor.target,
        }
    }
    fn current(&self, app: &WebApp) -> bool {
        Arc::ptr_eq(&self.document, &app.editor.session.snapshot())
            && self.ticket == app.editor.session.history_ticket()
            && self.variant == app.variant
            && self.target == app.editor.target
            && app.paste_safe()
    }
}
#[derive(Debug)]
struct Ready {
    intent: Intent,
    draft: PasteDraft,
    losses: Vec<String>,
    accept_losses: bool,
    origin: (i64, i64),
    grow: bool,
    name: String,
}
#[derive(Debug, Default)]
pub(crate) struct State {
    #[cfg(test)]
    next_read: Option<clipboard::Job<clipboard::Decoded>>,
    intent: Intent,
    placing: Option<placement::Placing>,
    owner: Option<Owner>,
    job: Option<clipboard::Job<clipboard::Decoded>>,
    ready: Option<Ready>,
    activation: Option<u64>,
}
impl State {
    pub(crate) fn can_read(&self) -> bool {
        #[cfg(test)]
        if self.next_read.is_some() {
            return true;
        }
        clipboard::read_supported()
    }
    pub(crate) fn busy(&self) -> bool {
        self.job.is_some() || self.ready.is_some() || self.placing.is_some()
    }
    pub(crate) fn is_placing(&self) -> bool {
        self.placing.is_some()
    }
    pub(crate) fn modal(&self) -> bool {
        self.ready.is_some()
    }
}
impl WebApp {
    fn paste_safe(&self) -> bool {
        !self.editor.tools.gesture_in_progress()
            && !self.editor.tools.is_constructing_shape()
            && !self.editor.tools.is_editing_text()
            && self.editor.tools.floating_selection().is_none()
            && !self.text.is_composing()
    }
    pub(crate) fn paste_requested(&mut self, ctx: &egui::Context) {
        self.paste_requested_kind(ctx, Intent::NewLayer);
    }
    pub(crate) fn paste_current_requested(&mut self, ctx: &egui::Context) {
        self.paste_requested_kind(ctx, Intent::CurrentLayer);
    }
    fn paste_requested_kind(&mut self, ctx: &egui::Context, intent: Intent) {
        if self.paste.busy() {
            return;
        }
        self.paste.intent = intent;
        let frontend = match self.variant {
            ClassicVariant::V7 => paintcraft_formats::registry::Frontend::ClassicV7,
            ClassicVariant::V10 => paintcraft_formats::registry::Frontend::ClassicV10,
            ClassicVariant::V11 => paintcraft_formats::registry::Frontend::ClassicV11,
        };
        let budget = self.clipboard.budget.clone();
        #[cfg(test)]
        let next = self.paste.next_read.take();
        self.paste_with(ctx, || {
            #[cfg(test)]
            if let Some(job) = next {
                return Ok(job);
            }
            clipboard::read(ctx, frontend, budget)
        });
    }
    fn paste_with(
        &mut self,
        ctx: &egui::Context,
        submit: impl FnOnce() -> paintcraft_core::types::Result<clipboard::Job<clipboard::Decoded>>,
    ) {
        if self.paste.activation == Some(ctx.cumulative_frame_nr()) {
            return;
        }
        self.paste.activation = Some(ctx.cumulative_frame_nr());
        if self.paste.busy()
            || self.clipboard.busy()
            || self.blocked()
            || !self.paste_safe()
            || ctx.egui_wants_keyboard_input()
        {
            self.message = t!(ctx, "Finish the current edit before pasting.");
            return;
        }
        if self.paste.intent == Intent::CurrentLayer
            && self.editor.target.is_none_or(|id| {
                self.editor
                    .session
                    .document()
                    .layers
                    .get(&id)
                    .is_none_or(|l| !l.visible)
            })
        {
            self.message = t!(ctx, "Finish the current edit before pasting.");
            return;
        }
        match submit() {
            Ok(job) => {
                self.paste.owner = Some(Owner::capture(self));
                self.paste.job = Some(job);
                self.message = t!(ctx, "Reading clipboard…");
            }
            Err(error) => self.message = format!("{}: {error}", t!(ctx, "Clipboard paste failed")),
        }
    }
    pub(crate) fn cancel_paste(&mut self, ctx: &egui::Context) {
        if self.paste.busy() {
            self.paste.job = None;
            self.paste.ready = None;
            self.paste.placing = None;
            self.canvas.invalidate();
            self.paste.owner = None;
            self.message = t!(ctx, "Paste cancelled.");
        }
    }
    pub(crate) fn poll_paste(&mut self, ctx: &egui::Context) {
        if self.paste.job.is_none() {
            return;
        }
        if self.blocked()
            || self
                .paste
                .owner
                .as_ref()
                .is_none_or(|owner| !owner.current(self))
        {
            self.cancel_paste(ctx);
            self.message = t!(ctx, "The document changed; paste again.");
            return;
        }
        let Some(result) = self.paste.job.as_mut().and_then(clipboard::Job::try_result) else {
            return;
        };
        self.paste.job = None;
        match result.and_then(|decoded| {
            if decoded
                .losses
                .iter()
                .any(|loss| import_loss_description(loss).is_none())
            {
                return Err(paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                    "Unrecognized clipboard import limitation",
                ));
            }
            Ok(Ready {
                intent: self.paste.intent,
                draft: PasteDraft::new(&self.editor.session, decoded.image)?,
                losses: decoded.losses,
                accept_losses: false,
                origin: (0, 0),
                grow: false,
                name: t!(ctx, "Pasted image"),
            })
        }) {
            Ok(ready) => self.paste.ready = Some(ready),
            Err(error) => {
                self.paste.owner = None;
                self.message = format!("{}: {error}", t!(ctx, "Clipboard paste failed"));
            }
        }
    }
    fn validate_ready_placement(ready: &mut Ready) -> bool {
        ready
            .draft
            .place(
                ready.origin,
                if ready.grow {
                    PasteCanvas::Grow
                } else {
                    PasteCanvas::Keep
                },
            )
            .is_ok()
            && {
                let (w, h) = ready.draft.canvas_size();
                u64::from(w) * u64::from(h) <= crate::editor::MAX_PIXELS
            }
    }
    fn accept_paste(&mut self, ctx: &egui::Context, mut ready: Ready) {
        if self
            .paste
            .owner
            .as_ref()
            .is_none_or(|owner| !owner.current(self))
        {
            self.paste.owner = None;
            self.message = t!(ctx, "The document changed; paste again.");
            return;
        }
        if !Self::validate_ready_placement(&mut ready)
            || ready.name.chars().take(1025).count() > 1024
            || (!ready.losses.is_empty() && !ready.accept_losses)
        {
            self.paste.ready = Some(ready);
            return;
        }
        if ready.intent == Intent::CurrentLayer {
            self.begin_paste_placement(ctx, ready);
            return;
        }
        self.paste.owner = None;
        let before = self.editor.session.document().canvas;
        match self.editor.session.paste_image(&ready.draft, &ready.name) {
            Ok(receipt) => {
                self.editor.target = receipt
                    .aliases
                    .get("pasted")
                    .copied()
                    .or(self.editor.target);
                let size = self.editor.session.document().canvas;
                if size != before {
                    self.remember_canvas_size(size);
                }
                self.message = t!(ctx, "Paste");
            }
            Err(error) => self.message = format!("{}: {error}", t!(ctx, "Clipboard paste failed")),
        }
    }
    pub(crate) fn paste_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut ready) = self.paste.ready.take() else {
            return;
        };
        if self
            .paste
            .owner
            .as_ref()
            .is_none_or(|owner| !owner.current(self))
        {
            self.paste.owner = None;
            self.message = t!(ctx, "The document changed; paste again.");
            return;
        }
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("web-paste-layer")).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.heading(t!(
                ctx,
                if ready.intent == Intent::NewLayer {
                    "Paste as new layer…"
                } else {
                    "Paste"
                }
            ));
            if ready.intent == Intent::NewLayer {
                ui.label(t!(ctx, "Pasting creates a new layer."));
            }
            ui.label(format!(
                "{} × {} px",
                ready.draft.image().width(),
                ready.draft.image().height()
            ));
            if ready.intent == Intent::NewLayer {
                ui.horizontal(|ui| {
                    let label = ui.label(t!(ctx, "Name"));
                    ui.text_edit_singleline(&mut ready.name)
                        .labelled_by(label.id);
                });
            }
            ui.horizontal(|ui| {
                let label = ui.label("X");
                ui.add(egui::DragValue::new(&mut ready.origin.0).range(-1_048_576..=1_048_576))
                    .labelled_by(label.id);
                let label = ui.label("Y");
                ui.add(egui::DragValue::new(&mut ready.origin.1).range(-1_048_576..=1_048_576))
                    .labelled_by(label.id);
            });
            ui.checkbox(&mut ready.grow, t!(ctx, "Expand canvas to fit image"));
            if !ready.grow && ready.intent == Intent::NewLayer {
                ui.label(t!(ctx, "Pixels outside the canvas stay in the new layer."));
            }
            let valid = Self::validate_ready_placement(&mut ready);
            if !valid {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    t!(ctx, "Choose a valid image position and canvas size."),
                );
            }
            if !ready.losses.is_empty() {
                ui.label(t!(ctx, "Paste with these import limitations?"));
                for loss in &ready.losses {
                    if let Some(description) = import_loss_description(loss) {
                        ui.label(t!(ctx, description));
                    }
                }
                ui.checkbox(
                    &mut ready.accept_losses,
                    t!(ctx, "Accept import limitations"),
                );
            }
            ui.horizontal(|ui| {
                accept = ui
                    .add_enabled(
                        valid
                            && ready.name.chars().take(1025).count() <= 1024
                            && (ready.losses.is_empty() || ready.accept_losses),
                        egui::Button::new(t!(ctx, "Paste")),
                    )
                    .clicked();
                cancel = ui.button(t!(ctx, "Cancel")).clicked();
            });
        });
        if ctx.current_pass_index() == 0
            && (cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)))
        {
            self.paste.owner = None;
            self.message = t!(ctx, "Paste cancelled.");
        } else if ctx.current_pass_index() == 0 && accept {
            self.accept_paste(ctx, ready);
        } else {
            self.paste.ready = Some(ready);
        }
    }
}
#[cfg(test)]
mod tests;
