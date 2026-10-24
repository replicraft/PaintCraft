//! Derived finite-mask feedback. Viewport changes never alter selection samples.
use super::*;
use paintcraft_ui::t;
impl App {
    pub(super) fn draw_mask_overlay(&mut self, painter: &egui::Painter, view: &View, ppp: f32) {
        let Selection::Mask(mask) = self.editor.selection() else {
            self.selection_outline.clear();
            return;
        };
        let clip = painter.clip_rect();
        let Some(area) = view.visible_region(
            (clip.min.x, clip.min.y),
            (clip.max.x, clip.max.y),
            self.editor.document().canvas,
        ) else {
            return;
        };
        match self.selection_outline.get(&mask, area) {
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
