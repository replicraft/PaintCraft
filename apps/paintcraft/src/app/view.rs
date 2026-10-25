//! View state is independent of document publication and native save state.
use super::*;
use paintcraft_ui::actions::{ViewAction, ZOOM_MAX, ZOOM_MIN};

#[derive(Clone, Copy, Debug)]
pub(super) struct StudioFit {
    document: Uuid,
    size: (u32, u32),
    previous: f32,
    fitted: f32,
}

fn fit_scale(available: Vec2, size: Vec2) -> f32 {
    (available.x.max(1.0) / size.x.max(1.0))
        .min(available.y.max(1.0) / size.y.max(1.0))
        .clamp(ZOOM_MIN, ZOOM_MAX)
}

impl App {
    /// Public zoom is logical UI points per document pixel. The shared View
    /// contract takes physical pixels, so convert only at this boundary.
    pub(super) fn canvas_view(&self, origin: (f32, f32), ppp: f32) -> View {
        View::with_scale(origin, self.zoom * ppp, ppp)
    }

    pub(super) fn canvas_mip_level(&self, ppp: f32) -> u8 {
        (-(self.zoom * ppp).log2()).floor().clamp(0.0, 20.0) as u8
    }

    fn visible_document_center(&self) -> (f32, f32) {
        if let (Some(viewport), Some(canvas)) = (self.canvas_viewport, self.canvas_rect) {
            let point = (viewport.center() - canvas.min) * (1.0 / self.zoom);
            (
                point.x.clamp(0.0, self.editor.document().canvas.0 as f32),
                point.y.clamp(0.0, self.editor.document().canvas.1 as f32),
            )
        } else {
            let (w, h) = self.editor.document().canvas;
            (w as f32 / 2.0, h as f32 / 2.0)
        }
    }

    pub(super) fn set_zoom(&mut self, ctx: &egui::Context, zoom: f32, center: (f32, f32)) {
        if !self.editor.has_floating_selection() {
            self.editor.cancel_gesture();
        }
        self.studio.pan.cancel();
        self.studio.zoom.cancel();
        self.studio.auto_scroll.cancel();
        self.studio_fit = None;
        self.zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        self.pending_center = Some(center);
        ctx.request_repaint();
    }

    pub(super) fn handle_canvas_wheel(&mut self, ui: &egui::Ui) {
        let ctx = ui.ctx();
        if self.document_count() == 0 || self.action_context().blocked {
            return;
        }
        let Some(viewport) = self.canvas_viewport else {
            return;
        };
        let Some(pointer) = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| viewport.contains(*p))
        else {
            return;
        };
        if ctx.layer_id_at(pointer) != Some(ui.layer_id()) {
            return;
        }
        let delta = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|event| {
                    if let egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } = event
                        && (modifiers.ctrl || modifiers.command || modifiers.mac_cmd)
                        && !modifiers.alt
                    {
                        let units = match unit {
                            egui::MouseWheelUnit::Point => delta.y / 40.0,
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Page => delta.y * 3.0,
                        };
                        return units.is_finite().then_some(units);
                    }
                    None
                })
                .sum::<f32>()
        });
        if delta != 0.0 {
            let center = self.visible_document_center();
            self.set_zoom(
                ctx,
                self.zoom * 1.1_f32.powf(delta.clamp(-32.0, 32.0)),
                center,
            );
        }
    }

    pub(super) fn dispatch_view_action(&mut self, ctx: &egui::Context, action: ViewAction) {
        match action {
            ViewAction::Thumbnail => {
                if self.experience() == Experience::Classic && self.document_count() > 0 {
                    self.classic.thumbnail = !self.classic.thumbnail;
                    ctx.request_repaint();
                }
                return;
            }
            ViewAction::AlwaysShowToolbar | ViewAction::AutoHideToolbar => {
                self.editor.cancel_gesture();
                self.settings.classic_toolbar_auto_hide = action == ViewAction::AutoHideToolbar;
                self.persist_settings();
                ctx.request_repaint();
                return;
            }
            ViewAction::Rulers => {
                self.editor.cancel_gesture();
                self.settings.classic_rulers = !self.settings.classic_rulers;
                self.persist_settings();
                ctx.request_repaint();
                return;
            }
            ViewAction::StatusBar => {
                self.settings.classic_status_bar = !self.settings.classic_status_bar;
                self.persist_settings();
                ctx.request_repaint();
                return;
            }
            ViewAction::FullScreen => {
                self.editor.cancel_gesture();
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                ctx.request_repaint();
                return;
            }
            _ => {}
        }
        if self.document_count() == 0 {
            return;
        }
        if action == ViewAction::PixelGrid {
            self.pixel_grid = !self.pixel_grid;
            ctx.request_repaint();
            return;
        }
        let center = self.visible_document_center();
        let zoom = match action {
            ViewAction::ZoomIn => paintcraft_ui::actions::stepped_zoom(self.zoom, true),
            ViewAction::ZoomOut => paintcraft_ui::actions::stepped_zoom(self.zoom, false),
            ViewAction::ActualSize => 1.0,
            ViewAction::Double => 2.0,
            ViewAction::Quadruple => 4.0,
            ViewAction::Octuple => 8.0,
            ViewAction::SetZoom(value) => value as f32 / 10000.0,
            ViewAction::FitWindow | ViewAction::FitSelection => {
                let Some(viewport) = self.canvas_viewport else {
                    return;
                };
                let area = if action == ViewAction::FitSelection {
                    let Ok(Some(rect)) = self.selected_bounds() else {
                        return;
                    };
                    rect
                } else {
                    self.editor.document().canvas_rect()
                };
                // Keep fitted content centered in the complete viewport while
                // leaving room for Classic's floating controls on its left.
                let clearance = if self.tool_controls_visible() {
                    2.0 * paintcraft_classic::tool_controls::CANVAS_CLEARANCE
                } else {
                    0.0
                };
                let studio_toggle =
                    action == ViewAction::FitWindow && self.experience() == Experience::Studio;
                let restore = if studio_toggle {
                    self.studio_fit.take().filter(|fit| {
                        fit.document == self.editor.document().document_id
                            && fit.size == self.editor.document().canvas
                            && fit.fitted.to_bits() == self.zoom.to_bits()
                    })
                } else {
                    None
                };
                let previous = self.zoom;
                let zoom = restore.map_or_else(
                    || {
                        fit_scale(
                            viewport.size() - Vec2::new(clearance + 24.0, 24.0),
                            Vec2::new((area.x1 - area.x0) as f32, (area.y1 - area.y0) as f32),
                        )
                    },
                    |fit| fit.previous,
                );
                self.set_zoom(
                    ctx,
                    zoom,
                    (
                        (area.x0 + area.x1) as f32 / 2.0,
                        (area.y0 + area.y1) as f32 / 2.0,
                    ),
                );
                if studio_toggle && restore.is_none() {
                    self.studio_fit = Some(StudioFit {
                        document: self.editor.document().document_id,
                        size: self.editor.document().canvas,
                        previous,
                        fitted: self.zoom,
                    });
                }
                return;
            }
            ViewAction::PixelGrid
            | ViewAction::Thumbnail
            | ViewAction::StatusBar
            | ViewAction::FullScreen
            | ViewAction::Rulers
            | ViewAction::AlwaysShowToolbar
            | ViewAction::AutoHideToolbar => {
                unreachable!()
            }
        };
        self.set_zoom(ctx, zoom, center);
    }

    pub(super) fn draw_pixel_grid(&self, painter: &egui::Painter, canvas: Rect, view: &View) {
        paintcraft_ui::view::pixel_grid(
            painter,
            canvas,
            self.editor.document().canvas,
            view.points_per_pixel(),
            view.pixels_per_point,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_is_bounded_in_logical_points() {
        assert_eq!(
            fit_scale(Vec2::new(200.0, 100.0), Vec2::new(400.0, 400.0)),
            0.25
        );
        assert_eq!(fit_scale(Vec2::splat(1000.0), Vec2::splat(1.0)), ZOOM_MAX);
        assert!(fit_scale(Vec2::splat(1.0), Vec2::splat(1048576.0)) >= ZOOM_MIN);
    }

    #[test]
    fn grid_visits_visible_boundaries_at_one_physical_pixel_without_reading_document_pixels() {
        let mut app = App::new(None);
        app.editor = Editor::new_document(1048576, 1048576).unwrap();
        let ctx = egui::Context::default();
        for ppp in [1.0, 1.5, 2.0] {
            app.zoom = 4.0;
            let mut full = ctx.run_pass(egui::RawInput::default(), |ctx| {
                let painter = ctx
                    .layer_painter(egui::LayerId::background())
                    .with_clip_rect(Rect::from_min_max(
                        Pos2::new(20.0, 20.0),
                        Pos2::new(60.0, 60.0),
                    ));
                app.draw_pixel_grid(
                    &painter,
                    Rect::from_min_size(Pos2::ZERO, Vec2::splat(4194304.0)),
                    &app.canvas_view((0.0, 0.0), ppp),
                );
            });
            let lines: Vec<_> = full
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::LineSegment { points, stroke } = &s.shape {
                        Some((points, stroke))
                    } else {
                        None
                    }
                })
                .collect();
            full.textures_delta.clear();
            assert_eq!(lines.len(), 22);
            for (points, stroke) in lines {
                assert_eq!(stroke.width, 1.0 / ppp);
                for p in points {
                    assert!((20.0..=60.0).contains(&p.x) && (20.0..=60.0).contains(&p.y));
                }
            }
        }
    }

    #[test]
    fn grid_and_zoom_belong_to_each_document_and_do_not_change_history_or_pcnd() {
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        let ctx = egui::Context::default();
        let first = app.active_tab_id();
        let before = paintcraft_formats::pcnd::write(app.editor.document()).unwrap();
        let ticket = app.editor.history_view().ticket;
        app.dispatch_view_action(&ctx, ViewAction::PixelGrid);
        app.dispatch_view_action(&ctx, ViewAction::ZoomOut);
        assert!(app.pixel_grid);
        assert_eq!(app.zoom, 0.5);
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert_eq!(
            paintcraft_formats::pcnd::write(app.editor.document()).unwrap(),
            before
        );
        assert!(!app.editor.is_dirty());
        app.new_document();
        assert!(!app.pixel_grid);
        assert_eq!(app.zoom, 1.0);
        app.activate_tab(first, &ctx);
        assert!(app.pixel_grid);
        assert_eq!(app.zoom, 0.5);
    }
}

#[cfg(test)]
#[path = "view/fit_tests.rs"]
mod fit_tests;

#[cfg(test)]
#[path = "view/scale_tests.rs"]
mod scale_tests;
