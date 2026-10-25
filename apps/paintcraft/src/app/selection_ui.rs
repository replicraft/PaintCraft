//! Derived finite-mask feedback. Viewport changes never alter selection samples.
use super::*;
use paintcraft_ui::t;
impl App {
    /// Derived coverage bounds are feedback only, never editing authority.
    pub(super) fn selected_bounds(
        &self,
    ) -> paintcraft_core::types::Result<Option<paintcraft_core::types::Rect>> {
        if self.document_count() == 0 {
            self.selection_bounds.borrow_mut().clear();
            return Ok(None);
        }
        let clip = self.editor.document().canvas_rect();
        match self.editor.display_selection() {
            Selection::Mask(mask) => self.selection_bounds.borrow_mut().get(&mask, clip),
            selection => {
                self.selection_bounds.borrow_mut().clear();
                Ok(match selection {
                    Selection::Rect(rect) => rect.intersect(&clip),
                    _ => None,
                })
            }
        }
    }
    pub(super) fn selected_dimensions(&self) -> Option<(i64, i64)> {
        if self.document_count() == 0 || self.editor.display_selection() == Selection::Inactive {
            self.selection_bounds.borrow_mut().clear();
            return None;
        }
        self.selected_bounds()
            .ok()
            .map(|bounds| bounds.map_or((0, 0), |r| (r.width(), r.height())))
    }

    pub(super) fn draw_mask_overlay(&mut self, painter: &egui::Painter, view: &View, ppp: f32) {
        let Selection::Mask(mask) = self.editor.display_selection() else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_ui::actions::{ViewAction, ZOOM_MAX, ZOOM_MIN};
    #[test]
    fn mask_dimensions_and_fit_follow_samples_without_publishing_or_placing_text() {
        for points in [
            vec![(2, 2), (18, 2), (18, 7), (7, 7), (7, 18), (2, 18)],
            vec![(2, 2), (18, 2), (2, 18)],
            vec![(40, 40), (48, 40), (40, 48)],
        ] {
            for inverted in [false, true] {
                let mut app = App::new(None);
                app.install(Editor::new_document(32, 24).unwrap()).unwrap();
                assert_eq!(app.selected_dimensions(), None);
                app.editor.request(
                    1,
                    Action::SelectPolygon {
                        points: points.clone(),
                    },
                );
                if inverted {
                    app.editor.request(2, Action::InvertSelection);
                }
                app.editor.run_pending();
                let Selection::Mask(mask) = app.editor.selection() else {
                    panic!("mask");
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
                assert_eq!(app.action_context().can_fit_selection, bounds.is_some());
                assert_eq!(app.action_context().can_crop_selection, bounds.is_some());
                app.editor.tool = Tool::Text;
                app.editor.pointer_down((1, 1));
                app.editor.pointer_up();
                app.editor.type_text("Keep editable");
                assert!(app.editor.is_editing_text());
                let preview = app.editor.preview();
                let snapshot = app.editor.document_snapshot();
                let ticket = app.editor.history_view().ticket;
                let dirty = app.editor.is_dirty();
                app.canvas_viewport = Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(400.0, 120.0),
                ));
                let zoom = app.zoom;
                app.dispatch_view_action(&egui::Context::default(), ViewAction::FitSelection);
                if let Some(r) = bounds {
                    let clearance = if app.tool_controls_visible() {
                        2.0 * paintcraft_classic::tool_controls::CANVAS_CLEARANCE
                    } else {
                        0.0
                    };
                    let expected = ((400.0 - clearance - 24.0) / r.width() as f32)
                        .min(96.0 / r.height() as f32)
                        .clamp(ZOOM_MIN, ZOOM_MAX);
                    assert_eq!(app.zoom, expected);
                    assert_eq!(
                        app.pending_center,
                        Some(((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0))
                    );
                } else {
                    assert_eq!(app.zoom, zoom);
                }
                assert_eq!(app.editor.preview(), preview);
                assert!(app.editor.is_editing_text());
                assert!(app.editor.document().same_semantics(&snapshot));
                assert_eq!(app.editor.history_view().ticket, ticket);
                assert_eq!(app.editor.is_dirty(), dirty);
            }
        }
    }
    #[test]
    fn mask_rotate_flip_and_crop_menus_publish_exact_samples_and_undo() {
        use egui_kittest::{Harness, kittest::Queryable};
        for rotated in [true, false] {
            let mut app = App::new(None);
            app.install(Editor::new_document(32, 24).unwrap()).unwrap();
            let layer = app.editor.target().unwrap();
            for (i, point) in [(3, 3), (3, 7), (6, 7)].into_iter().enumerate() {
                app.editor.request(
                    i as u64 + 100,
                    Action::Stroke(paintcraft_tools::Stroke {
                        media: Default::default(),
                        layer,
                        points: vec![point],
                        point_scale: 1,
                        color: Rgba([17, 33, 55, 255]),
                        size: 1,
                        round: false,
                        mode: paintcraft_tools::BlendMode::Replace,
                    }),
                );
            }
            app.editor.request(
                200,
                Action::SelectPolygon {
                    points: vec![(2, 2), (10, 2), (10, 5), (5, 5), (5, 10), (2, 10)],
                },
            );
            app.editor.run_pending();
            let before = app.editor.document_snapshot();
            let selection = app.editor.selection();
            let Selection::Mask(mask) = &selection else {
                panic!("mask")
            };
            let mut expected = vec![Rgba([255; 4]); 32 * 24];
            let mut mapped = vec![false; 32 * 24];
            for y in 0..24 {
                for x in 0..32 {
                    let index = (y * 32 + x) as usize;
                    expected[index] = before.layers[&layer].raster.get(x, y);
                    if mask.coverage(x, y) != 0 {
                        expected[index] = Rgba([255; 4]);
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
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app.with_initial_zoom(1.0));
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
            let Selection::Mask(actual) = h.state().editor.selection() else {
                panic!("mapped mask")
            };
            let doc = h.state().editor.document();
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
            assert!(h.state().editor.document().same_semantics(&before));
            assert_eq!(h.state().editor.selection(), selection);
            if rotated {
                h.get_by_role_and_label(egui::Role::Button, "Crop").click();
                h.run();
                let cropped = h.state().editor.document();
                assert_eq!(cropped.canvas, (8, 8));
                assert_eq!(h.state().editor.selection(), Selection::Inactive);
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
                assert!(h.state().editor.document().same_semantics(&before));
                assert_eq!(h.state().editor.selection(), selection);
            }
        }
    }
}
