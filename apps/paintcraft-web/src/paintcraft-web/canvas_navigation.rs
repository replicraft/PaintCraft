//! Temporary navigation leaves the selected drawing tool and document intact.
use crate::WebApp;
use eframe::egui::{self, PointerButton, Vec2};
use paintcraft_ui::actions::Frontend;

impl WebApp {
    pub(crate) fn navigate_canvas(&mut self, ui: &egui::Ui, response: &egui::Response) -> bool {
        let temporary = response.drag_stopped_by(PointerButton::Middle)
            || ui.input(|input| {
                input.pointer.button_down(PointerButton::Middle)
                    || (input.key_down(egui::Key::Space)
                        && !self.text.widget_owned_input
                        && self.controls.tool != paintcraft_tools::Tool::Text
                        && !self.editor.has_text())
            });
        let on_canvas = response.hovered() || response.dragged();
        if !(temporary && on_canvas) {
            return false;
        }
        self.editor.cancel_floating();
        self.editor.tools.cancel_gesture();
        let dragging = response.dragged_by(PointerButton::Middle)
            || response.drag_stopped_by(PointerButton::Middle)
            || response.dragged_by(PointerButton::Primary)
            || response.drag_stopped_by(PointerButton::Primary);
        let delta = if dragging {
            ui.input(|input| input.pointer.delta())
        } else {
            Vec2::ZERO
        };
        if self.frontend == Frontend::Studio {
            paintcraft_studio::canvas::pan_by(
                ui.ctx(),
                egui::Id::unique("web-studio-canvas"),
                self.studio_canvas_scope(),
                delta,
            );
        } else {
            paintcraft_classic::canvas::pan_by(
                ui.ctx(),
                egui::Id::unique("web-editor-canvas"),
                delta,
            );
        }
        ui.ctx().set_cursor_icon(if delta == Vec2::ZERO {
            egui::CursorIcon::Grab
        } else {
            egui::CursorIcon::Grabbing
        });
        true
    }
}
