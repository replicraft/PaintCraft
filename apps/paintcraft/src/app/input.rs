//! Desktop input integration.

use super::*;

impl App {
    pub(super) fn request(&mut self, ctx: &egui::Context, code: u64, action: Action) {
        self.editor.request(token(ctx, code), action);
    }

    /// Keyboard shortcuts and window events; first pass only. Each key press is
    /// judged with the modifiers it arrived with, not the pointer-time state.
    pub(super) fn handle_global_input(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let events = ui.input(|i| i.events.clone());
        let text_focus = self.editor.is_editing_text() && !ctx.egui_wants_keyboard_input();
        for event in events {
            match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } => match key {
                    // Holding Backspace or Enter repeats while typing; one-shot commands do not.
                    egui::Key::Enter if text_focus && !modifiers.command => {
                        self.editor.type_text("\n")
                    }
                    egui::Key::Backspace if text_focus => self.editor.backspace(),
                    _ if repeat => {}
                    egui::Key::Z if modifiers.command && modifiers.shift => {
                        self.request(&ctx, code::REDO, Action::Redo)
                    }
                    egui::Key::Z if modifiers.command => {
                        self.request(&ctx, code::UNDO, Action::Undo)
                    }
                    egui::Key::Y if modifiers.command => {
                        self.request(&ctx, code::REDO, Action::Redo)
                    }
                    egui::Key::S if modifiers.command => self.save_requested(&ctx),
                    egui::Key::Enter if modifiers.command => self.editor.commit_text(),
                    egui::Key::Escape => {
                        self.editor.cancel_gesture();
                        self.editor.cancel_text();
                    }
                    _ => {}
                },
                // Focus loss ends an unfinished gesture without publishing it.
                egui::Event::WindowFocused(false) => {
                    self.editor.cancel_gesture();
                    self.editor.commit_text();
                }
                // Typed characters go to the text being edited unless a text field has the keyboard.
                egui::Event::Text(typed) if text_focus => self.editor.type_text(&typed),
                _ => {}
            }
        }
    }
}
