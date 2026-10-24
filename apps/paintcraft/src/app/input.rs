//! Desktop input integration.

use super::*;

impl App {
    pub(super) fn request(&mut self, ctx: &egui::Context, code: u64, action: Action) {
        if self.document_count() > 0 {
            self.editor.request(token(ctx, code), action);
        }
    }

    /// First pass only. Match modifiers from the event, and leave focused widgets
    /// and active IME composition in control of their own keyboard input.
    pub(super) fn handle_global_input(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.handle_canvas_wheel(ui);
        let events = ui.input(|i| i.events.clone());
        let widget_focus = ctx.egui_wants_keyboard_input();
        let popup_open = egui::Popup::is_any_open(&ctx);
        let picker_open = self.picking.is_some();
        for event in events {
            let text_focus = self.document_count() > 0
                && self.editor.is_editing_text()
                && !widget_focus
                && !popup_open
                && !picker_open;
            match event {
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    self.ime_composing = !text.is_empty();
                }
                egui::Event::Ime(egui::ImeEvent::Commit(_)) => self.ime_composing = false,
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } if !widget_focus && !self.ime_composing => match key {
                    egui::Key::Enter
                        if self.editor.has_floating_selection()
                            && modifiers == egui::Modifiers::NONE
                            && !popup_open
                            && !picker_open =>
                    {
                        self.editor.finish_selection_move();
                    }
                    egui::Key::Enter if text_focus && !modifiers.command => {
                        self.editor.type_text("\n");
                    }
                    egui::Key::Backspace if text_focus => self.editor.backspace(),
                    _ if repeat => {}
                    egui::Key::Enter if modifiers.command && text_focus => {
                        self.editor.commit_text();
                    }
                    // Popup dismissal takes precedence over cancelling a canvas
                    // draft or leaving fullscreen, even if egui released focus.
                    egui::Key::Escape if popup_open => {}
                    egui::Key::Escape if picker_open => self.picking = None,
                    egui::Key::Escape if self.studio.zoom.is_active() => self.studio.zoom.cancel(),
                    egui::Key::Escape if self.studio.pan.is_active() => self.studio.pan.cancel(),
                    egui::Key::Z | egui::Key::H if text_focus && !modifiers.command => {}
                    egui::Key::Escape => {
                        if self.fullscreen && !text_focus && !self.editor.gesture_in_progress() {
                            self.dispatch_chrome(
                                &ctx,
                                vec![paintcraft_ui::actions::Action::View(
                                    paintcraft_ui::actions::ViewAction::FullScreen,
                                )],
                            );
                        }
                        self.editor.cancel_gesture();
                        self.editor.cancel_text();
                        self.editor.cancel_pending_edits();
                    }
                    _ => {
                        if let Some(action) = paintcraft_ui::actions::Action::from_shortcut_for(
                            self.action_context().frontend,
                            key,
                            modifiers,
                        ) {
                            self.dispatch_chrome(&ctx, vec![action]);
                        }
                    }
                },
                // Focus loss ends an unfinished gesture without publishing it.
                egui::Event::WindowFocused(false) => {
                    self.ime_composing = false;
                    self.editor.cancel_gesture();
                    self.studio.zoom.cancel();
                    self.studio.pan.cancel();
                    self.studio.space_pan = false;
                    self.studio.auto_scroll.cancel();
                    if !matches!(self.editor.tool, Tool::Magnifier | Tool::Pan) {
                        self.editor.commit_text();
                    }
                }
                egui::Event::Text(typed) if text_focus && !self.ime_composing => {
                    self.editor.type_text(&typed);
                }
                _ => {}
            }
        }
    }
}
