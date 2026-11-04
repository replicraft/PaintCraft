//! Derived finite-mask feedback. Viewport changes never alter selection samples.
use super::*;
use paintcraft_ui::t;
#[path = "selection_ui/toolbar.rs"]
mod toolbar;
pub(super) struct FeatherDialog {
    tab: Uuid,
    source: (Uuid, u64),
    radius: u16,
}
impl App {
    pub(super) fn begin_selection_feather(&mut self) {
        if self.editor.selection() == Selection::Inactive || self.editor.has_pending_edits() {
            return;
        }
        self.editor.cancel_gesture();
        self.editor.select_tool(Tool::Select);
        self.selection_feather_dialog = Some(FeatherDialog {
            tab: self.active_tab_id(),
            source: self.editor.render_key(),
            radius: self.editor.options.selection.feather,
        });
    }
    pub(super) fn selection_feather_window(&mut self, ctx: &egui::Context, first_pass: bool) {
        let Some(mut draft) = self.selection_feather_dialog.take() else {
            return;
        };
        if draft.tab != self.active_tab_id() || draft.source != self.editor.render_key() {
            return;
        }
        let mut apply = false;
        let modal =
            egui::Modal::new(egui::Id::unique("feather-selection-dialog")).show(ctx, |ui| {
                ui.set_width(300.);
                ui.heading("Feather selection");
                ui.add_space(8.);
                let mut radius = f32::from(draft.radius) / 10.;
                if ui
                    .add(
                        egui::Slider::new(&mut radius, 0. ..=128.)
                            .text("Radius (px)")
                            .step_by(0.1),
                    )
                    .changed()
                {
                    draft.radius = (radius * 10.).round() as u16;
                }
                ui.label("Softens selection coverage. Original image pixels are preserved.");
                ui.add_space(12.);
                ui.horizontal(|ui| {
                    apply = ui.button("Apply").clicked();
                    ui.button("Cancel").clicked()
                })
                .inner
            });
        if first_pass {
            if apply {
                self.editor.options.selection.feather = draft.radius;
                self.request(
                    ctx,
                    0x5e1ec7,
                    Action::FeatherSelection {
                        radius: draft.radius,
                    },
                );
                self.update_selection_preview(ctx, None);
            } else if modal.inner || modal.should_close() {
                self.update_selection_preview(ctx, None);
            } else {
                self.update_selection_preview(ctx, Some(draft.radius));
                self.selection_feather_dialog = Some(draft);
            }
        } else {
            self.selection_feather_dialog = Some(draft);
        }
    }
    /// Context controls overlay the viewport without shifting document coordinates.
    pub(super) fn selection_toolbar(
        &mut self,
        host: &egui::Ui,
        viewport: egui::Rect,
        first_pass: bool,
    ) -> Option<egui::Rect> {
        use paintcraft_core::types::selection::{Combine, Shape};
        if self.editor.tool != Tool::Select || self.document_count() == 0 {
            return None;
        }
        let ctx = host.ctx();
        let id = egui::Id::unique((self.active_tab_id(), "selection-toolbar"));
        let enabled = !self.action_context().blocked
            && !self.editor.pointer_is_down()
            && !self.editor.is_constructing_shape()
            && !self.editor.has_floating_selection();
        let mut options = self.editor.options;
        let mut invert = false;
        let mut deselect = false;
        let mut feather_changed = false;
        let mut feather_active = false;
        let before = options.selection.feather;
        let left_controls = self.tool_controls_visible();
        let response = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .movable(false)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(egui::pos2(viewport.center().x, viewport.top() + 4.))
            .constrain_to(viewport)
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(if ui.visuals().dark_mode {
                    Color32::from_gray(230)
                } else {
                    Color32::from_gray(24)
                });
                egui::Frame::popup(ui.style())
                    .inner_margin(8)
                    .show(ui, |ui| {
                        let compact = viewport.width() < 620.;
                        ui.spacing_mut().item_spacing.x = if compact { 4. } else { 5. };
                        ui.add_enabled_ui(enabled, |ui| {
                            ui.horizontal(|ui| {
                                ui.set_min_height(28.);
                                let s = &mut options.selection;
                                if matches!(options.selection_mode, Shape::Wand | Shape::Brush) {
                                    ui.add_enabled_ui(
                                        options.selection_mode != Shape::Brush || s.edge_aware,
                                        |ui| {
                                            toolbar::number(
                                                ui,
                                                id,
                                                "Tolerance",
                                                &mut s.tolerance,
                                                100.,
                                                compact,
                                            );
                                        },
                                    );
                                    ui.separator();
                                }
                                let before_field = s.feather;
                                feather_active = toolbar::number(
                                    ui,
                                    id,
                                    "Feather",
                                    &mut s.feather,
                                    128.,
                                    compact,
                                );
                                feather_changed = before_field != s.feather;
                                ui.separator();
                                for (mode, hint) in [
                                    (Combine::Replace, "Replace selection"),
                                    (Combine::Add, "Add to selection (Shift)"),
                                    (Combine::Subtract, "Subtract from selection (Alt)"),
                                    (Combine::Intersect, "Intersect selection (Shift+Alt)"),
                                ] {
                                    if toolbar::button(ui, Some(mode), hint, s.combine == mode)
                                        .clicked()
                                    {
                                        s.combine = mode;
                                    }
                                }
                                ui.separator();
                                invert =
                                    toolbar::button(ui, None, "Invert selection", false).clicked();
                                ui.menu_button("...", |ui| {
                                    if options.selection_mode == Shape::Brush {
                                        ui.checkbox(
                                            &mut s.edge_aware,
                                            "Color matching / edge aware",
                                        );
                                        ui.add_enabled_ui(s.edge_aware, |ui| {
                                            ui.checkbox(
                                                &mut s.sample_merged,
                                                "Sample all visible layers",
                                            );
                                        });
                                        if left_controls {
                                            ui.label("Size, opacity and hardness: left sliders");
                                        } else {
                                            ui.add(
                                                egui::Slider::new(&mut s.diameter, 1..=256)
                                                    .text("Size"),
                                            );
                                            ui.add(
                                                egui::Slider::new(&mut s.strength, 0..=100)
                                                    .text("Opacity"),
                                            );
                                            ui.add(
                                                egui::Slider::new(&mut s.hardness, 0..=100)
                                                    .text("Hardness"),
                                            );
                                        }
                                    } else if options.selection_mode == Shape::Wand {
                                        ui.checkbox(&mut s.contiguous, "Contiguous");
                                        ui.checkbox(
                                            &mut s.sample_merged,
                                            "Sample all visible layers",
                                        );
                                        ui.checkbox(&mut s.antialias, "Antialias edges");
                                    }
                                    ui.separator();
                                    deselect = ui.button("Deselect").clicked();
                                })
                                .response
                                .on_hover_text("Selection tool options");
                            });
                        });
                    });
            });
        if first_pass {
            self.editor.options = options;
            let pending_id = id.with("feather-pending");
            if feather_changed {
                ctx.data_mut(|d| {
                    if d.get_temp::<u16>(pending_id).is_none() {
                        d.insert_temp(pending_id, before);
                    }
                });
            }
            let pending = ctx.data(|d| d.get_temp::<u16>(pending_id));
            if let Some(original) = pending {
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    ctx.stop_dragging();
                    self.editor.options.selection.feather = original;
                    ctx.data_mut(|d| d.remove::<u16>(pending_id));
                } else if !feather_active {
                    ctx.data_mut(|d| d.remove::<u16>(pending_id));
                    if self.editor.selection() != Selection::Inactive {
                        self.request(
                            ctx,
                            0x5e1ec7,
                            Action::FeatherSelection {
                                radius: options.selection.feather,
                            },
                        );
                    }
                }
            }
            if invert {
                self.request(ctx, 0x5e1ec8, Action::InvertSelection);
            }
            if deselect {
                self.request(ctx, 0x5e1ec9, Action::Select { corners: None });
            }
        }
        let pending_radius = self
            .selection_feather_dialog
            .as_ref()
            .map(|d| d.radius)
            .or_else(|| {
                ctx.data(|d| d.get_temp::<u16>(id.with("feather-pending")))
                    .map(|_| self.editor.options.selection.feather)
            });
        if first_pass {
            self.update_selection_preview(ctx, pending_radius);
        }
        Some(response.response.rect)
    }
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
        let displayed = self
            .selection_preview
            .selection()
            .cloned()
            .unwrap_or_else(|| self.editor.display_selection());
        let Selection::Mask(mask) = displayed else {
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
        let raw = mask.base().unwrap_or(&mask);
        let mut visible = false;
        for (field, alpha, cache) in [
            (raw, 255, &mut self.selection_outline),
            (&mask, 102, &mut self.selection_feather_outline),
        ] {
            if alpha == 102 && mask.base().is_none() {
                continue;
            }
            match cache.get_at(field, area, if alpha == 102 { 1 } else { 128 }) {
                Ok(outline) => {
                    let phase = selection_ants::phase(painter.ctx().input(|i| i.time));
                    for segment in outline.segments() {
                        let a = view.to_screen(segment.from);
                        let b = view.to_screen(segment.to);
                        let line = [Pos2::new(a.0, a.1), Pos2::new(b.0, b.1)];
                        visible |=
                            selection_ants::ants_alpha(painter, &line, phase - a.0 - a.1, alpha);
                    }
                }
                Err(_) => {
                    painter.text(
                        clip.min + Vec2::splat(8.),
                        egui::Align2::LEFT_TOP,
                        t!(
                            painter.ctx(),
                            "Selection outline is too complex; zoom in to view it"
                        ),
                        egui::FontId::proportional(13.),
                        Color32::GRAY,
                    );
                }
            }
        }
        if visible {
            painter
                .ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        let _ = ppp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_ui::actions::{ViewAction, ZOOM_MAX, ZOOM_MIN};
    #[test]
    fn brush_selection_updates_while_held_commits_one_undo_and_cancels_the_next_draft() {
        use egui_kittest::Harness;
        use paintcraft_core::types::selection::{Combine, Shape};
        let mut app = App::new(None);
        app.install(Editor::new_document(240, 180).unwrap())
            .unwrap();
        app.editor.tool = Tool::Select;
        app.editor.options.selection_mode = Shape::Brush;
        app.editor.options.selection.diameter = 24;
        app.editor.options.selection.hardness = 50;
        app.editor.options.selection.strength = 60;
        app.editor.options.selection.feather = 25;
        let mut h = Harness::builder()
            .with_size(Vec2::new(1100., 760.))
            .with_pixels_per_point(1.5)
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app.with_initial_zoom(1.25));
        h.run();
        let canvas = h.state().canvas_rect.unwrap();
        let scale = canvas.width() / 240.;
        let position = |x, y| canvas.min + Vec2::new(x, y) * scale;
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        let wait = |h: &mut Harness<App>, x, y| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                h.run();
                if h.state().selection_preview.selection().is_some_and(
                    |selection| matches!(selection,Selection::Mask(mask) if mask.coverage(x,y)>0),
                ) {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "live brush preview did not reach {x},{y}"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        };
        let before = h.state().editor.document_snapshot();
        let ticket = h.state().editor.history_view().ticket;
        let depth = h.state().editor.undo_depth();
        let start = position(30.5, 40.5);
        h.event(egui::Event::PointerMoved(start));
        h.event(button(start, true));
        wait(&mut h, 30, 40);
        assert!(h.state().editor.pointer_is_down());
        assert!(
            h.state().editor.preview().is_none(),
            "no lasso preview at any point"
        );
        h.event(egui::Event::PointerMoved(position(170.5, 40.5)));
        h.event(egui::Event::PointerMoved(position(170.5, 140.5)));
        wait(&mut h, 170, 135);
        let exact = h.state().selection_preview.selection().unwrap().clone();
        let Selection::Mask(mask) = &exact else {
            panic!()
        };
        assert_eq!(
            mask.coverage(110, 90),
            0,
            "brush path has no closing polygon interior"
        );
        assert!(
            mask.base().is_some(),
            "feathered preview retains its original coverage"
        );

        assert_eq!(h.state().editor.selection(), Selection::Inactive);
        assert_eq!(h.state().editor.history_view().ticket, ticket);
        assert!(h.state().editor.document().same_semantics(&before));
        if let Ok(path) = std::env::var("PAINTCRAFT_LIVE_BRUSH_SCREENSHOT") {
            h.render().unwrap().save(path).unwrap();
        }
        h.event(button(position(170.5, 140.5), false));
        h.run();
        assert!(
            h.state()
                .editor
                .selection()
                .same_field_checked(&exact, 100000, &|| Ok(()))
                .unwrap()
        );
        assert_eq!(h.state().editor.undo_depth(), depth + 1);
        assert_eq!(h.state().editor.options.selection.combine, Combine::Add);
        let committed = h.state().editor.selection();
        let next = position(80.5, 105.5);
        h.event(egui::Event::PointerMoved(next));
        h.event(button(next, true));
        wait(&mut h, 80, 105);
        let Selection::Mask(draft) = h.state().selection_preview.selection().unwrap() else {
            panic!()
        };
        assert!(
            draft.coverage(30, 40) > 0,
            "next stroke adds to the previous one"
        );
        h.key_press(egui::Key::Escape);
        h.run();
        assert!(!h.state().editor.pointer_is_down());
        assert_eq!(h.state().editor.selection(), committed);
        assert_eq!(h.state().editor.undo_depth(), depth + 1);
        h.state_mut().editor.request(9001, Action::Undo);
        h.state_mut().editor.run_pending();
        assert_eq!(h.state().editor.selection(), Selection::Inactive);
        assert!(h.state().editor.document().same_semantics(&before));
    }
    #[test]
    fn brush_sliders_stack_and_edit_preferences_without_blocking_the_canvas() {
        use crate::settings::Appearance;
        use egui_kittest::{Harness, kittest::Queryable};
        use paintcraft_tools::SelectionMode;
        for (ppp, appearance) in [(1., Appearance::Dark), (2., Appearance::Light)] {
            let mut app = App::new(None);
            app.install(Editor::new_document(1280, 720).unwrap())
                .unwrap();
            app.settings.appearance = appearance;
            app.editor.tool = Tool::Select;
            app.editor.options.selection_mode = SelectionMode::Brush;
            app.editor.options.selection.edge_aware = false;
            let mut h = Harness::builder()
                .with_size(Vec2::new(1389., 866.))
                .with_pixels_per_point(ppp)
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app.with_initial_zoom(1.));
            h.run();
            let snapshot = h.state().editor.document_snapshot();
            let ticket = h.state().editor.history_view().ticket;
            let canvas = h.state().canvas_rect.unwrap();
            let mut previous = None;
            for (label, value) in [
                ("Tool size", 40.),
                ("Brush opacity", 65.),
                ("Brush hardness", 80.),
            ] {
                let slider = h.get_by_role_and_label(egui::Role::Slider, label);
                let rect = slider.rect();
                let field_label = match label {
                    "Tool size" => "Size",
                    "Brush opacity" => "Opacity",
                    _ => "Hardness",
                };
                let field = h
                    .get_by_role_and_label(egui::Role::SpinButton, field_label)
                    .rect();
                let header_label = match label {
                    "Tool size" => "Brush size",
                    other => other,
                };
                let header = h
                    .get_by_role_and_label(egui::Role::Image, header_label)
                    .rect();
                assert!((rect.center().x - field.center().x).abs() < 0.1);
                assert!((rect.center().x - header.center().x).abs() < 0.1);
                let panel = h
                    .output()
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Rect(r)
                            if r.fill == Color32::from_rgb(46, 46, 45)
                                && r.rect.contains_rect(rect) =>
                        {
                            Some(r.rect)
                        }
                        _ => None,
                    })
                    .expect("slider panel");
                assert!(panel.width() <= 42., "compact panel: {panel:?}");
                assert!((panel.center().x - rect.center().x).abs() < 0.1);
                if let Some(old) = previous {
                    let old: Rect = old;
                    assert!((old.left() - rect.left()).abs() < 0.1);
                    assert!(rect.top() > old.bottom());
                }
                previous = Some(rect);
                slider.set_value_accesskit(value);
                h.run();
            }
            assert_eq!(h.state().editor.options.selection.strength, 65);
            assert_eq!(h.state().editor.options.selection.hardness, 80);
            assert_eq!(h.state().canvas_rect.unwrap(), canvas);
            assert!(h.state().editor.document().same_semantics(&snapshot));
            assert_eq!(h.state().editor.history_view().ticket, ticket);
            assert!(!h.state().editor.pointer_is_down());
            // The open canvas between the left stack and top toolbar must not
            // be fenced by the union of those two disjoint control bounds.
            let point = canvas.min + Vec2::new(150., 180.);
            h.event(egui::Event::PointerMoved(point));
            h.event(egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });
            h.run();
            assert!(h.state().editor.pointer_is_down());
            h.event(egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });
            h.run();
            assert_ne!(h.state().editor.selection(), Selection::Inactive);
            if let Ok(path) = std::env::var("PAINTCRAFT_BRUSH_TOOLBAR_SCREENSHOT") {
                h.state_mut().editor.options.selection.edge_aware = true;
                h.run();
                h.get_by_label("...").click();
                h.run();
                h.render()
                    .unwrap()
                    .save(format!("{path}-{ppp}.png"))
                    .unwrap();
            }
        }
    }
    #[test]
    fn selection_ribbon_exposes_all_five_tools_and_the_viewport_toolbar() {
        use egui_kittest::{Harness, kittest::Queryable};
        use paintcraft_tools::SelectionMode;
        let mut app = App::new(None);
        app.install(Editor::new_document(96, 64).unwrap()).unwrap();
        let mut h = Harness::builder()
            .with_size(Vec2::new(1100., 760.))
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app.with_initial_zoom(4.));
        h.run();
        for mode in SelectionMode::ALL {
            h.get_by_label("Selection options").click();
            h.run();
            h.get_by_label(mode.label()).click();
            h.run();
            assert_eq!(h.state().editor.options.selection_mode, mode);
            assert_eq!(h.state().editor.tool, Tool::Select);
            h.get_by_role_and_label(egui::Role::Slider, "Feather");
            h.get_by_label("Add to selection (Shift)");
            let viewport = h.state().canvas_viewport.unwrap();
            assert!(
                viewport.width() > 600.,
                "floating toolbar does not consume viewport space"
            );
        }
        let layer = h.state().editor.target().unwrap();
        h.state_mut().editor.request(
            800,
            Action::SelectionRegion {
                layer,
                points: vec![(24, 16), (70, 48)],
                settings: paintcraft_core::types::selection::Settings {
                    feather: 30,
                    ..Default::default()
                },
            },
        );
        h.state_mut().editor.run_pending();
        h.run();
        let before = h.state().editor.selection();
        h.get_by_label("Selection options").click();
        h.run();
        h.get_by_label("Feather selection…").click();
        h.run();
        assert!(h.state().selection_feather_dialog.is_some());
        h.state_mut()
            .selection_feather_dialog
            .as_mut()
            .unwrap()
            .radius = 60;
        h.run();
        assert_eq!(
            h.state().editor.selection(),
            before,
            "dialog previews do not publish selection/history"
        );
        h.get_by_label("Cancel").click();
        h.run();
        assert!(h.state().selection_feather_dialog.is_none());
        assert_eq!(h.state().editor.selection(), before);
        if let Ok(path) = std::env::var("PAINTCRAFT_SELECTION_SCREENSHOT") {
            h.render().unwrap().save(path).unwrap();
        }
    }
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
