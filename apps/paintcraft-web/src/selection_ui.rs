//! Browser lasso input admission and cached true selection boundaries.
use crate::{app::WebApp, editor::MAX_POINTS};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use paintcraft_ui::{t, view::View};
impl WebApp {
    pub(crate) fn selected_bounds(
        &self,
    ) -> paintcraft_core::types::Result<Option<paintcraft_core::types::Rect>> {
        use paintcraft_raster::Selection;
        let clip = self.editor.session.document().canvas_rect();
        match self.editor.session.selection() {
            Selection::Mask(mask) => self.canvas.bounds.borrow_mut().get(&mask, clip),
            selection => {
                self.canvas.bounds.borrow_mut().clear();
                Ok(match selection {
                    Selection::Rect(rect) => rect.intersect(&clip),
                    _ => None,
                })
            }
        }
    }
    pub(crate) fn selected_dimensions(&self) -> Option<(i64, i64)> {
        if self.editor.session.selection() == paintcraft_raster::Selection::Inactive {
            self.canvas.bounds.borrow_mut().clear();
            return None;
        }
        self.selected_bounds()
            .ok()
            .map(|bounds| bounds.map_or((0, 0), |r| (r.width(), r.height())))
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_tools::{Action, Tool};
    #[test]
    fn mask_dimensions_and_fit_follow_samples_without_publishing_or_placing_text() {
        for points in [
            vec![(2, 2), (18, 2), (18, 7), (7, 7), (7, 18), (2, 18)],
            vec![(2, 2), (18, 2), (2, 18)],
            vec![(40, 40), (48, 40), (40, 48)],
        ] {
            for inverted in [false, true] {
                let mut app = WebApp::new(&egui::Context::default());
                app.editor = crate::editor::Editor::new(32, 24).unwrap();
                assert_eq!(app.selected_dimensions(), None);
                app.editor
                    .apply(Action::SelectPolygon {
                        points: points.clone(),
                    })
                    .unwrap();
                if inverted {
                    app.editor.apply(Action::InvertSelection).unwrap();
                }
                let paintcraft_raster::Selection::Mask(mask) = app.editor.session.selection()
                else {
                    panic!("mask")
                };
                let mut pixels = Vec::new();
                for y in 0..24 {
                    for x in 0..32 {
                        if mask.coverage(x, y) != 0 {
                            pixels.push((x, y));
                        }
                    }
                }
                let bounds = (!pixels.is_empty()).then(|| paintcraft_core::types::Rect {
                    x0: pixels.iter().map(|p| p.0).min().unwrap(),
                    y0: pixels.iter().map(|p| p.1).min().unwrap(),
                    x1: pixels.iter().map(|p| p.0).max().unwrap() + 1,
                    y1: pixels.iter().map(|p| p.1).max().unwrap() + 1,
                });
                assert_eq!(app.selected_bounds().unwrap(), bounds);
                assert_eq!(
                    app.selected_dimensions(),
                    Some(bounds.map_or((0, 0), |r| (r.width(), r.height())))
                );
                app.editor.tools.tool = Tool::Text;
                app.editor
                    .tools
                    .begin_text(app.editor.target.unwrap(), (1, 1));
                app.editor.tools.type_text("Keep editable");
                let text = app.editor.tools.text_content().map(str::to_owned);
                assert!(text.is_some());
                let snapshot = app.editor.session.snapshot();
                let ticket = app.editor.session.ticket();
                app.canvas.viewport = Vec2::new(400.0, 120.0);
                let zoom = app.zoom;
                app.fit_selection();
                if let Some(r) = bounds {
                    let clearance = if app.variant == paintcraft_classic::ClassicVariant::V11 {
                        2.0 * paintcraft_classic::tool_controls::CANVAS_CLEARANCE
                    } else {
                        0.0
                    };
                    let expected = ((400.0 - clearance - 24.0) / r.width() as f32)
                        .min(96.0 / r.height() as f32)
                        .clamp(0.125, 10.0);
                    assert_eq!(app.zoom, expected);
                    assert_eq!(
                        app.canvas.pending_center,
                        Some(((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0))
                    );
                } else {
                    assert_eq!(app.zoom, zoom);
                }
                assert_eq!(app.editor.tools.text_content().map(str::to_owned), text);
                assert!(app.editor.session.document().same_semantics(&snapshot));
                assert_eq!(app.editor.session.ticket(), ticket);
            }
        }
    }
    #[test]
    fn mask_rotate_flip_and_crop_menus_publish_exact_samples_and_undo() {
        use egui_kittest::{Harness, kittest::Queryable};
        for rotated in [true, false] {
            let mut app = WebApp::new(&egui::Context::default());
            app.editor = crate::editor::Editor::new(32, 24).unwrap();
            let layer = app.editor.target.unwrap();
            for point in [(3, 3), (3, 7), (6, 7)] {
                app.editor
                    .apply(Action::Stroke(paintcraft_tools::Stroke {
                        media: Default::default(),
                        layer,
                        points: vec![point],
                        point_scale: 1,
                        color: paintcraft_core::types::Rgba([17, 33, 55, 255]),
                        size: 1,
                        round: false,
                        mode: paintcraft_tools::BlendMode::Replace,
                    }))
                    .unwrap();
            }
            app.editor
                .apply(Action::SelectPolygon {
                    points: vec![(2, 2), (10, 2), (10, 5), (5, 5), (5, 10), (2, 10)],
                })
                .unwrap();
            let before = app.editor.session.snapshot();
            let selection = app.editor.session.selection();
            let paintcraft_raster::Selection::Mask(mask) = &selection else {
                panic!("mask")
            };
            let mut expected = vec![paintcraft_core::types::Rgba([255; 4]); 32 * 24];
            let mut mapped = vec![false; 32 * 24];
            for y in 0..24 {
                for x in 0..32 {
                    let index = (y * 32 + x) as usize;
                    expected[index] = before.layers[&layer].raster.get(x, y);
                    if mask.coverage(x, y) != 0 {
                        expected[index] = paintcraft_core::types::Rgba([255; 4]);
                    }
                }
            }
            for y in 0..24 {
                for x in 0..32 {
                    if mask.coverage(x, y) != 0 {
                        let (dx, dy) = if rotated { (11 - y, x) } else { (11 - x, y) };
                        expected[(dy * 32 + dx) as usize] = before.layers[&layer].raster.get(x, y);
                        mapped[(dy * 32 + dx) as usize] = true;
                    }
                }
            }
            let mut h = Harness::builder()
                .with_size(Vec2::new(1360.0, 900.0))
                .build_eframe(move |cc| {
                    let mut host = WebApp::new(&cc.egui_ctx);
                    host.editor = app.editor;
                    host
                });
            h.run();
            let (menu, label) = if rotated {
                ("Rotate", "Rotate right 90°")
            } else {
                ("Flip", "Flip horizontal")
            };
            h.get_by_label(menu).click();
            h.run();
            h.get_by_label(label).click();
            h.run();
            let paintcraft_raster::Selection::Mask(actual) = h.state().editor.session.selection()
            else {
                panic!("mapped mask")
            };
            let doc = h.state().editor.session.document();
            let raster = &doc.layers[&layer];
            for y in 0..24 {
                for x in 0..32 {
                    assert_eq!(
                        raster.raster.get(x - raster.origin.0, y - raster.origin.1),
                        expected[(y * 32 + x) as usize]
                    );
                    assert_eq!(actual.coverage(x, y) != 0, mapped[(y * 32 + x) as usize]);
                }
            }
            h.get_by_label("Undo").click();
            h.run();
            assert!(h.state().editor.session.document().same_semantics(&before));
            assert_eq!(h.state().editor.session.selection(), selection);
            if rotated {
                h.get_by_role_and_label(egui::Role::Button, "Crop").click();
                h.run();
                let cropped = h.state().editor.session.document();
                assert_eq!(cropped.canvas, (8, 8));
                assert_eq!(
                    h.state().editor.session.selection(),
                    paintcraft_raster::Selection::Inactive
                );
                let raster = &cropped.layers[&layer];
                for y in 0..8 {
                    for x in 0..8 {
                        assert_eq!(
                            raster.raster.get(x - raster.origin.0, y - raster.origin.1),
                            before.layers[&layer].raster.get(x + 2, y + 2),
                            "Crop retains holes inside the tight bounding rectangle"
                        );
                    }
                }
                h.get_by_label("Undo").click();
                h.run();
                assert!(h.state().editor.session.document().same_semantics(&before));
                assert_eq!(h.state().editor.session.selection(), selection);
            }
        }
    }
}
