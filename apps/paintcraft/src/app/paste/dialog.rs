use super::*;
use paintcraft_editor::clipboard::PasteCanvas;
use paintcraft_ui::i18n::import_loss_description;
impl App {
    pub(crate) fn paste_layer_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut ready) = self.paste.ready.take() else {
            self.confirm = None;
            return;
        };
        if !ready.owner.current(self) {
            self.confirm = None;
            self.clipboard_status(
                ready.owner.tab,
                t!(ctx, "The document changed; paste again."),
            );
            return;
        }
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("paste-image-layer")).show(ctx, |ui| {
            ui.set_width(400.0);
            let onto = ready.owner.target.is_some();
            ui.heading(t!(
                ctx,
                if onto {
                    "Paste"
                } else {
                    "Paste as new layer…"
                }
            ));
            ui.label(t!(
                ctx,
                if onto {
                    "Drag to position; Enter places, Escape cancels."
                } else {
                    "Pasting creates a new layer."
                }
            ));
            ui.horizontal(|ui| {
                ui.label(t!(ctx, "Image size"));
                ui.label(format!(
                    "{} × {} px",
                    ready.draft.image().width(),
                    ready.draft.image().height()
                ));
            });
            if !onto {
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
            if !ready.grow && !onto {
                ui.label(t!(ctx, "Pixels outside the canvas stay in the new layer."));
            }
            let placement = ready.draft.place(
                ready.origin,
                if ready.grow {
                    PasteCanvas::Grow
                } else {
                    PasteCanvas::Keep
                },
            );
            if placement.is_err() {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    t!(ctx, "Choose a valid image position and canvas size."),
                );
            }
            if !ready.losses.is_empty() {
                ui.separator();
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
            let valid = placement.is_ok()
                && ready.name.chars().take(1025).count() <= 1024
                && (ready.losses.is_empty() || ready.accept_losses);
            ui.horizontal(|ui| {
                accept = ui
                    .add_enabled(valid, egui::Button::new(t!(ctx, "Paste")))
                    .clicked();
                cancel = ui.button(t!(ctx, "Cancel")).clicked();
            });
        });
        if ctx.current_pass_index() == 0
            && (cancel || ctx.input(|input| input.key_pressed(egui::Key::Escape)))
        {
            self.confirm = None;
            self.editor.set_status(t!(ctx, "Paste cancelled."));
        } else if ctx.current_pass_index() == 0 && accept {
            self.commit_ready_paste(ctx, ready);
        } else {
            self.paste.ready = Some(ready);
        }
    }
    pub(super) fn commit_ready_paste(&mut self, ctx: &egui::Context, mut ready: Ready) {
        self.confirm = None;
        if !ready.owner.current(self) {
            self.clipboard_status(
                ready.owner.tab,
                t!(ctx, "The document changed; paste again."),
            );
            return;
        }
        if (!ready.losses.is_empty() && !ready.accept_losses)
            || ready.name.chars().take(1025).count() > 1024
            || ready
                .draft
                .place(
                    ready.origin,
                    if ready.grow {
                        PasteCanvas::Grow
                    } else {
                        PasteCanvas::Keep
                    },
                )
                .is_err()
        {
            self.confirm = Some(Confirm::PasteLayer);
            self.paste.ready = Some(ready);
            return;
        }
        let result = if ready.owner.target.is_some() {
            let key = self
                .editor
                .options
                .transparent_selection
                .then_some(self.editor.secondary);
            self.editor.begin_image_paste(ready.draft, key)
        } else {
            self.editor.start_paste(ready.draft, ready.name)
        };
        if let Err(error) = result {
            self.paste_error(ctx, &error.to_string());
        } else {
            self.remember_published_canvas_size();
        }
    }
}
