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
        ui.ctx().set_cursor_icon(match self.editor.tool {
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
