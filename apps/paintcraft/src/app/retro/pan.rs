//! Retro pan owns middle/Space input independently of Studio navigation.
use super::*;
use paintcraft_tools::navigation::{PanButton, PanDirection, keyboard_pan_delta};

impl App {
    pub(crate) fn retro_pan_scope(&self, ctx: &egui::Context) -> egui::Id {
        egui::Id::unique((
            self.active_tab_id(),
            self.editor.render_key(),
            self.editor.tool.action_id(),
            self.zoom.to_bits(),
            ctx.pixels_per_point().to_bits(),
            ctx.content_rect().width().to_bits(),
            ctx.content_rect().height().to_bits(),
        ))
    }

    pub(crate) fn validate_retro_pan(&mut self, ui: &egui::Ui, viewport: Rect) -> Option<egui::Id> {
        let enabled = self.experience() == Experience::Retro
            && ui.is_enabled()
            && (!self.action_context().blocked || self.parameter_navigation_ready())
            && !egui::Popup::is_any_open(ui.ctx())
            && ui.input(|input| input.focused);
        let scope = egui::Id::unique((
            self.retro_pan_scope(ui.ctx()),
            viewport.width().to_bits(),
            viewport.height().to_bits(),
        ));
        self.retro_pan.validate(&scope, enabled);
        if !enabled {
            self.retro_space = false;
            return None;
        }
        Some(scope)
    }

    pub(crate) fn retro_pan_event(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        viewport: Rect,
        event: &egui::Event,
    ) -> bool {
        let Some(scope) = self.validate_retro_pan(ui, viewport) else {
            return false;
        };
        let at = |pos: Pos2| (pos.x, pos.y);
        let mut delta = None;
        let mut consumed = false;
        match event {
            egui::Event::Key {
                key: egui::Key::Space,
                pressed,
                ..
            } => {
                self.retro_space = *pressed
                    && !ui.ctx().egui_wants_keyboard_input()
                    && !self.editor.is_editing_text()
                    && !self.ime_composing;
                consumed = self.retro_space || self.retro_pan.is_active();
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if self.retro_space
                && !modifiers.alt
                && !modifiers.shift
                && !ui.ctx().egui_wants_keyboard_input()
                && !self.editor.is_editing_text()
                && !self.editor.gesture_in_progress()
                && !self.ime_composing =>
            {
                let direction = match key {
                    egui::Key::ArrowLeft => Some(PanDirection::Left),
                    egui::Key::ArrowRight => Some(PanDirection::Right),
                    egui::Key::ArrowUp => Some(PanDirection::Up),
                    egui::Key::ArrowDown => Some(PanDirection::Down),
                    _ => None,
                };
                if let Some(direction) = direction {
                    delta =
                        keyboard_pan_delta(direction, modifiers.ctrl, ui.ctx().pixels_per_point());
                    consumed = true;
                    ui.input_mut(|input| {
                        input.consume_key(*modifiers, *key);
                    });
                    ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                }
            }
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => {
                let button = match button {
                    egui::PointerButton::Primary => Some(PanButton::Primary),
                    egui::PointerButton::Middle => Some(PanButton::Middle),
                    _ => None,
                };
                if let Some(button) = button {
                    let was_active = self.retro_pan.is_active();
                    if *pressed {
                        let mode = button == PanButton::Middle
                            || self.editor.tool == Tool::Pan
                            || self.retro_space;
                        if mode
                            && !self.editor.gesture_in_progress()
                            && viewport.contains(*pos)
                            && ui.ctx().layer_id_at(*pos) == Some(painter.layer_id())
                        {
                            self.retro_pan.begin(scope, button, at(*pos));
                        }
                    } else {
                        delta = self.retro_pan.release(&scope, button, at(*pos));
                    }
                    consumed = was_active || self.retro_pan.is_active();
                } else {
                    consumed = self.retro_pan.is_active();
                }
            }
            egui::Event::PointerMoved(pos) => {
                consumed = self.retro_pan.is_active();
                delta = self.retro_pan.move_to(&scope, at(*pos));
            }
            egui::Event::PointerGone
            | egui::Event::WindowFocused(false)
            | egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => {
                consumed = self.retro_pan.is_active();
                self.retro_pan.cancel();
                self.retro_space = false;
            }
            _ => {}
        }
        if let Some(delta) = delta {
            paintcraft_retro::pan_by(
                ui.ctx(),
                egui::Id::unique(self.active_tab_id()),
                Vec2::new(delta.0, delta.1),
            );
        }
        consumed || self.editor.tool == Tool::Pan
    }
}
