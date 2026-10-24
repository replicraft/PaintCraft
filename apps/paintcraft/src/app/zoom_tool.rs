//! Studio view gestures remain separate from paint and document selection.
use super::*;
use paintcraft_tools::navigation::ZoomRequest;
use paintcraft_ui::actions::stepped_zoom;

impl App {
    fn studio_zoom_context(
        &mut self,
        ui: &egui::Ui,
        canvas: Rect,
        viewport: Rect,
    ) -> Option<(egui::Id, f32)> {
        let ppp = ui.ctx().pixels_per_point();
        let scope = egui::Id::unique((
            self.active_tab_id(),
            self.editor.render_key(),
            self.zoom.to_bits(),
            ppp.to_bits(),
            canvas.min.x.to_bits(),
            canvas.min.y.to_bits(),
            viewport.width().to_bits(),
            viewport.height().to_bits(),
        ));
        let enabled = self.experience() == Experience::Studio
            && self.editor.tool == Tool::Magnifier
            && ui.is_enabled()
            && !self.action_context().blocked
            && !egui::Popup::is_any_open(ui.ctx())
            && ui.input(|input| input.focused);
        self.studio.zoom.validate(&scope, enabled);
        let scale = self.zoom / ppp;
        if !enabled {
            return None;
        }
        Some((scope, scale))
    }

    pub(super) fn studio_zoom_event(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        canvas: Rect,
        viewport: Rect,
        event: &egui::Event,
    ) -> bool {
        let Some((scope, scale)) = self.studio_zoom_context(ui, canvas, viewport) else {
            return false;
        };
        let point = |pos: Pos2| {
            let doc = (pos - canvas.min) / scale;
            (doc.x, doc.y)
        };
        let ppp = ui.ctx().pixels_per_point();
        match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => {
                let button = match button {
                    egui::PointerButton::Primary => paintcraft_input::PointerButton::Primary,
                    egui::PointerButton::Secondary => paintcraft_input::PointerButton::Secondary,
                    _ => return true,
                };
                if *pressed {
                    if canvas.intersect(viewport).contains(*pos)
                        && ui.ctx().layer_id_at(*pos) == Some(painter.layer_id())
                    {
                        self.studio.zoom.begin(scope, button, point(*pos));
                    }
                } else if let Some(request) =
                    self.studio.zoom.release(&scope, button, point(*pos), scale)
                {
                    let (zoom, center) = match request {
                        ZoomRequest::Step { center, increase } => {
                            (stepped_zoom(self.zoom, increase), center)
                        }
                        ZoomRequest::Rectangle { from, to } => {
                            let size = Vec2::new(
                                (to.0 - from.0).abs().max(1.0),
                                (to.1 - from.1).abs().max(1.0),
                            );
                            let pixels = viewport.size() * ppp;
                            (
                                (pixels.x / size.x).min(pixels.y / size.y),
                                ((from.0 + to.0) * 0.5, (from.1 + to.1) * 0.5),
                            )
                        }
                    };
                    self.set_zoom(ui.ctx(), zoom, center);
                }
            }
            egui::Event::PointerMoved(pos) => self.studio.zoom.update(point(*pos)),
            egui::Event::PointerGone
            | egui::Event::WindowFocused(false)
            | egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => self.studio.zoom.cancel(),
            _ => {}
        }
        true
    }

    pub(super) fn draw_studio_zoom(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        canvas: Rect,
        viewport: Rect,
    ) {
        let Some((_, scale)) = self.studio_zoom_context(ui, canvas, viewport) else {
            return;
        };
        let ppp = ui.ctx().pixels_per_point();
        if let Some((from, to)) = self.studio.zoom.preview(scale) {
            let rect = Rect::from_two_pos(
                canvas.min + Vec2::new(from.0, from.1) * scale,
                canvas.min + Vec2::new(to.0, to.1) * scale,
            );
            let painter = painter.with_clip_rect(viewport.intersect(canvas));
            painter.rect_filled(rect, 0, Color32::from_rgba_unmultiplied(0, 120, 215, 24));
            for (width, ink) in [(3.0, Color32::WHITE), (1.0, Color32::from_rgb(0, 90, 180))] {
                painter.rect_stroke(
                    rect,
                    0,
                    egui::Stroke::new(width / ppp, ink),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}
