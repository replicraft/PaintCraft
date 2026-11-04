//! Canvas tool cursor overlays, measured in document pixels.
use super::*;

impl App {
    pub(super) fn draw_tool_cursor(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        visible_canvas: Rect,
        view: &View,
        ppp: f32,
    ) {
        let pan = self.experience() == Experience::Studio
            && (self.editor.tool == Tool::Pan
                || (self.studio.space_pan && !self.editor.gesture_in_progress())
                || self.studio.pan.is_active());
        let cursor_area = if pan {
            self.canvas_viewport.unwrap_or(visible_canvas)
        } else {
            visible_canvas
        };
        let Some(pos) = ui.input(|i| i.pointer.hover_pos()).filter(|pos| {
            ui.is_enabled()
                && cursor_area.contains(*pos)
                && ui.ctx().layer_id_at(*pos) == Some(painter.layer_id())
        }) else {
            return;
        };
        if pan {
            ui.ctx().set_cursor_icon(if self.studio.pan.is_active() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
            return;
        }
        if self.editor.painting_blocked() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Progress);
            return;
        }
        let selection_brush = self.editor.tool == Tool::Select
            && self.editor.options.selection_mode == paintcraft_tools::SelectionMode::Brush;
        if selection_brush {
            let a = view.to_screen((0, 0));
            let b = view.to_screen((i64::from(self.editor.options.selection.diameter), 0));
            let radius = (b.0 - a.0).abs() * 0.5;
            let subtract = ui.input(|i| {
                if i.modifiers.shift {
                    false
                } else if i.modifiers.alt {
                    true
                } else {
                    self.editor.options.selection.combine
                        == paintcraft_core::types::selection::Combine::Subtract
                }
            });
            paintcraft_ui::widgets::selection_brush_cursor(painter, pos, radius, subtract);
        }
        let move_selection = self.editor.tool == Tool::Select
            && matches!(
                self.editor.options.selection_mode,
                paintcraft_tools::SelectionMode::Rectangle
                    | paintcraft_tools::SelectionMode::FreeForm
                    | paintcraft_tools::SelectionMode::Polygon
            )
            && self.editor.options.selection.combine
                == paintcraft_core::types::selection::Combine::Replace
            && view.to_doc((pos.x, pos.y)).is_some_and(|point| {
                paintcraft_tools::FloatingSelection::hit_test(
                    &self.editor.display_selection(),
                    point,
                )
            });
        ui.ctx().set_cursor_icon(match self.editor.tool {
            Tool::Select if move_selection => egui::CursorIcon::Move,
            Tool::Eraser => egui::CursorIcon::None,
            Tool::Text => egui::CursorIcon::Text,
            Tool::Magnifier => egui::CursorIcon::ZoomIn,
            _ => egui::CursorIcon::Crosshair,
        });
        if self.editor.tool != Tool::Eraser {
            return;
        }
        let Some((x, y)) = view.to_doc((pos.x, pos.y)) else {
            return;
        };
        // Match square raster stamps, including their even-size right/down bias.
        let size = i64::from(self.editor.options.size.clamp(1, 1024));
        let (left, top) = (x - (size - 1) / 2, y - (size - 1) / 2);
        let a = view.to_screen((left, top));
        let b = view.to_screen((left + size, top + size));
        let rect = Rect::from_min_max(Pos2::new(a.0, a.1), Pos2::new(b.0, b.1));
        let painter = painter.with_clip_rect(visible_canvas);
        // Two contrasting edges remain visible over both light and dark pixels.
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(3.0 / ppp, Color32::WHITE),
            egui::StrokeKind::Middle,
        );
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0 / ppp, Color32::BLACK),
            egui::StrokeKind::Middle,
        );
    }
}
