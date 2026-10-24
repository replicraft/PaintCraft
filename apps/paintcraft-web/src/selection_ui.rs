//! Browser lasso input admission and cached true selection boundaries.
use crate::{app::WebApp, editor::MAX_POINTS};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use paintcraft_ui::{t, view::View};
impl WebApp {
    pub(crate) fn move_selection_pointer(&mut self, point: (i64, i64)) {
        if self.editor.tools.gesture_is_lasso() {
            if let Err(error) = self
                .editor
                .tools
                .try_pointer_move_subpixel(point, MAX_POINTS, MAX_POINTS)
            {
                self.message = error.message;
            }
        } else {
            self.editor.tools.pointer_move_subpixel(point);
        }
    }
    pub(crate) fn draw_mask_overlay(&mut self, painter: &egui::Painter, rect: Rect) {
        let paintcraft_raster::Selection::Mask(mask) = self.editor.session.selection() else {
            self.canvas.outline.clear();
            return;
        };
        let clip = painter.clip_rect();
        // Browser canvas uses logical points per pixel; View models physical zoom.
        let ppp = painter.ctx().pixels_per_point();
        let view = View::with_scale((rect.min.x, rect.min.y), self.zoom * ppp, ppp);
        let Some(area) = view.visible_region(
            (clip.min.x, clip.min.y),
            (clip.max.x, clip.max.y),
            self.editor.session.document().canvas,
        ) else {
            return;
        };
        match self.canvas.outline.get(&mask, area) {
            Ok(outline) => {
                for segment in outline.segments() {
                    let a = view.to_screen(segment.from);
                    let b = view.to_screen(segment.to);
                    let line = [Pos2::new(a.0, a.1), Pos2::new(b.0, b.1)];
                    painter.line_segment(line, egui::Stroke::new(3.0 / ppp, Color32::WHITE));
                    painter.line_segment(line, egui::Stroke::new(1.0 / ppp, Color32::BLACK));
                }
            }
            Err(_) => {
                painter.text(
                    clip.min + Vec2::splat(8.0),
                    egui::Align2::LEFT_TOP,
                    t!(
                        painter.ctx(),
                        "Selection outline is too complex; zoom in to view it"
                    ),
                    egui::FontId::proportional(13.0),
                    painter
                        .ctx()
                        .style_of(painter.ctx().theme())
                        .visuals
                        .text_color(),
                );
            }
        }
    }
}
