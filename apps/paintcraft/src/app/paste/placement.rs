//! Desktop private-image controls; release never publishes automatically.
use super::*;
impl App {
    fn place_image_paste(&mut self, ctx: &egui::Context) {
        if let Err(error) = self.editor.place_image_paste() {
            self.paste_error(ctx, &error.to_string());
        } else {
            self.remember_published_canvas_size();
        }
    }
    pub(crate) fn image_paste_bar(&mut self, ui: &mut egui::Ui) {
        if !self.editor.has_image_paste() {
            return;
        }
        egui::Panel::top("native-paste-placement").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(t!(
                    ui.ctx(),
                    "Drag to position; Enter places, Escape cancels."
                ));
                let enabled = !self.action_context().blocked;
                if ui
                    .add_enabled(enabled, egui::Button::new(t!(ui.ctx(), "Apply")))
                    .clicked()
                {
                    self.place_image_paste(ui.ctx());
                }
                if ui
                    .add_enabled(enabled, egui::Button::new(t!(ui.ctx(), "Cancel")))
                    .clicked()
                {
                    self.cancel_paste(ui.ctx());
                }
            });
        });
    }
    pub(crate) fn image_paste_keyboard(
        &mut self,
        ctx: &egui::Context,
        event: &egui::Event,
        widget_focus: bool,
    ) -> bool {
        if !self.editor.has_image_paste()
            || widget_focus
            || self.ime_composing
            || self.action_context().blocked
        {
            return false;
        }
        let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            return false;
        };
        match key {
            egui::Key::Escape => self.cancel_paste(ctx),
            egui::Key::Enter if !modifiers.command && !modifiers.alt => self.place_image_paste(ctx),
            egui::Key::Z if modifiers.command => self.cancel_paste(ctx),
            egui::Key::ArrowLeft
            | egui::Key::ArrowRight
            | egui::Key::ArrowUp
            | egui::Key::ArrowDown
                if !modifiers.command && !modifiers.alt =>
            {
                let step = if modifiers.shift { 10 } else { 1 };
                let delta = match key {
                    egui::Key::ArrowLeft => (-step, 0),
                    egui::Key::ArrowRight => (step, 0),
                    egui::Key::ArrowUp => (0, -step),
                    _ => (0, step),
                };
                if let Err(error) = self.editor.nudge_image_paste(delta) {
                    self.paste_error(ctx, &error.to_string());
                }
            }
            _ => self.editor.set_status(t!(
                ctx,
                "Finish placing the pasted image before editing or saving."
            )),
        }
        true
    }
}
