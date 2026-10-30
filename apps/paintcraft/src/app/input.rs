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
        let copy_key = events.iter().any(|event| {
            matches!(event,
            egui::Event::Key {key: egui::Key::C, pressed:true, modifiers, ..} if modifiers.command)
        });
        let cut_key = events.iter().any(|event| {
            matches!(event,
            egui::Event::Key {key: egui::Key::X, pressed: true, modifiers, ..} if modifiers.command)
        });
        let widget_focus = ctx.egui_wants_keyboard_input();
        let popup_open = egui::Popup::is_any_open(&ctx);
        let picker_open = self.picking.is_some();
        for event in events {
            if self.image_paste_keyboard(&ctx, &event, widget_focus || popup_open || picker_open) {
                continue;
            }
            let text_focus = self.document_count() > 0
                && self.editor.is_editing_text()
                && !widget_focus
                && !popup_open
                && !picker_open;
            match event {
                egui::Event::Copy
                    if !copy_key && !widget_focus && !self.ime_composing && !text_focus =>
                {
                    self.copy_requested(&ctx);
                }
                egui::Event::Cut
                    if !cut_key && !widget_focus && !self.ime_composing && !text_focus =>
                {
                    self.cut_requested(&ctx);
                }
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    // Receipt-time shortcut exclusion has no text target or
                    // publication authority; canvas binding happens in order.
                    self.ime_composing = !widget_focus || !text.is_empty();
                }
                egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                    self.ime_composing = false;
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } if !widget_focus && !self.ime_composing => match key {
                    // Canvas editing is replayed with pointer events in order.
                    egui::Key::Enter | egui::Key::Backspace | egui::Key::Delete | egui::Key::A
                        if text_focus => {}
                    egui::Key::Enter
                        if self.editor.has_floating_selection()
                            && modifiers == egui::Modifiers::NONE
                            && !popup_open
                            && !picker_open =>
                    {
                        self.editor.finish_selection_move();
                    }
                    egui::Key::C | egui::Key::X | egui::Key::V
                        if modifiers.command && text_focus => {}
                    _ if repeat => {}
                    // Popup dismissal takes precedence over cancelling a canvas
                    // draft or leaving fullscreen, even if egui released focus.
                    egui::Key::Escape if popup_open => {}
                    egui::Key::Escape if picker_open => self.picking = None,
                    egui::Key::Escape if self.studio.zoom.is_active() => self.studio.zoom.cancel(),
                    egui::Key::Escape if self.studio.pan.is_active() => self.studio.pan.cancel(),
                    egui::Key::Z | egui::Key::H if text_focus && !modifiers.command => {}
                    egui::Key::Escape => {
                        self.cancel_clipboard_cut(&ctx);
                        self.cancel_paste(&ctx);
                        if self.fullscreen && !text_focus && !self.editor.gesture_in_progress() {
                            self.dispatch_chrome(
                                &ctx,
                                vec![paintcraft_ui::actions::Action::View(
                                    paintcraft_ui::actions::ViewAction::FullScreen,
                                )],
                            );
                        }
                        self.editor.cancel_gesture();
                        let text = self.editor.is_editing_text();
                        self.editor.cancel_text();
                        if !text {
                            self.editor.cancel_pending_edits();
                        }
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
                    #[cfg(not(target_os = "windows"))]
                    self.editor.cancel_gesture();
                    #[cfg(target_os = "windows")]
                    if !self.editor.framework_force_active() {
                        self.editor.cancel_gesture();
                    }
                    self.studio.zoom.cancel();
                    self.studio.pan.cancel();
                    self.studio.space_pan = false;
                    self.studio.auto_scroll.cancel();
                    // Text finish is replayed by the canvas after any earlier
                    // result in this same event list.
                }
                _ => {}
            }
        }
    }
}
