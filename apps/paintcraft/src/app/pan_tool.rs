//! Studio pan input owns pointer events before the paint tools see them.
use super::*;
use paintcraft_tools::navigation::PanButton;

impl App {
    pub(super) fn studio_pan_scope(&self, ctx: &egui::Context) -> egui::Id {
        egui::Id::unique((
            self.active_tab_id(),
            self.editor.render_key(),
            self.editor.tool.action_id(),
            self.zoom.to_bits(),
            self.settings.studio_overscroll,
            ctx.pixels_per_point().to_bits(),
            ctx.content_rect().width().to_bits(),
            ctx.content_rect().height().to_bits(),
        ))
    }

    pub(super) fn validate_studio_pan(
        &mut self,
        ui: &egui::Ui,
        viewport: Rect,
    ) -> Option<egui::Id> {
        let enabled = self.experience() == Experience::Studio
            && ui.is_enabled()
            && !self.action_context().blocked
            && !egui::Popup::is_any_open(ui.ctx())
            && ui.input(|input| input.focused);
        let scope = egui::Id::unique((
            self.studio_pan_scope(ui.ctx()),
            viewport.width().to_bits(),
            viewport.height().to_bits(),
        ));
        self.studio.pan.validate(&scope, enabled);
        if !enabled {
            self.studio.space_pan = false;
            return None;
        }
        Some(scope)
    }

    pub(super) fn studio_pan_event(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        viewport: Rect,
        event: &egui::Event,
    ) -> bool {
        let Some(scope) = self.validate_studio_pan(ui, viewport) else {
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
                self.studio.space_pan = *pressed
                    && !ui.ctx().egui_wants_keyboard_input()
                    && !self.editor.is_editing_text()
                    && !self.ime_composing;
                if self.studio.space_pan {
                    self.studio.zoom.cancel();
                }
                consumed = self.studio.space_pan || self.studio.pan.is_active();
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
                    let was_active = self.studio.pan.is_active();
                    if *pressed {
                        let mode = button == PanButton::Middle
                            || self.editor.tool == Tool::Pan
                            || self.studio.space_pan;
                        if mode
                            && !self.editor.gesture_in_progress()
                            && viewport.contains(*pos)
                            && ui.ctx().layer_id_at(*pos) == Some(painter.layer_id())
                        {
                            self.studio.pan.begin(scope, button, at(*pos));
                            self.studio.zoom.cancel();
                        }
                    } else {
                        delta = self.studio.pan.release(&scope, button, at(*pos));
                    }
                    consumed = was_active || self.studio.pan.is_active();
                } else {
                    consumed = self.studio.pan.is_active();
                }
            }
            egui::Event::PointerMoved(pos) => {
                consumed = self.studio.pan.is_active();
                delta = self.studio.pan.move_to(&scope, at(*pos));
            }
            egui::Event::PointerGone
            | egui::Event::WindowFocused(false)
            | egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => {
                consumed = self.studio.pan.is_active();
                self.studio.pan.cancel();
                self.studio.space_pan = false;
            }
            _ => {}
        }
        if let Some(delta) = delta {
            paintcraft_studio::canvas::pan_by(
                ui.ctx(),
                egui::Id::unique(self.active_tab_id()),
                self.studio_pan_scope(ui.ctx()),
                Vec2::new(delta.0, delta.1),
            );
        }
        consumed || self.editor.tool == Tool::Pan
    }
}
