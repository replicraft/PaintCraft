use super::*;

fn input(ppp: f32) -> egui::RawInput {
    let mut input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1000.0))),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(ppp);
    input
}

#[test]
fn native_percentages_match_logical_canvas_pointer_and_visible_region_at_each_dpi() {
    let mut app = App::new(None);
    for ppp in [1.0, 1.5, 2.0] {
        for zoom in [0.25, 0.5, 1.0, 2.0] {
            app.zoom = zoom;
            let view = app.canvas_view((12.25, -8.5), ppp);
            assert_eq!(view.canvas_size(1000, 486), (1000.0 * zoom, 486.0 * zoom));
            assert_eq!(view.points_per_pixel(), zoom);
            assert_eq!(view.zoom, zoom * ppp);
            let at = (12.25 + 100.25 * zoom, -8.5 + 22.75 * zoom);
            assert_eq!(view.to_doc(at), Some((100, 22)));
            assert_eq!(
                view.to_doc_subpixel(at),
                Some((100 * 256 + 64, 22 * 256 + 192))
            );
            let region = view
                .visible_region(
                    (12.25, -8.5),
                    (12.25 + 20.0 * zoom, -8.5 + 10.0 * zoom),
                    (1000, 486),
                )
                .unwrap();
            assert_eq!(
                region,
                paintcraft_core::types::Rect::new(0, 0, 20, 10).unwrap()
            );
        }
    }
}

#[test]
fn canvas_layout_and_render_request_use_logical_zoom_and_physical_mips() {
    for ppp in [1.0, 1.5, 2.0] {
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.initial_fit = false;
        app.editor = Editor::new_document(1000, 486).unwrap();
        let ctx = egui::Context::default();
        for zoom in [0.25, 0.5, 1.0, 2.0] {
            app.zoom = zoom;
            let mut output = ctx.run_ui(input(ppp), |ui| app.canvas(ui, true));
            assert_eq!(ctx.pixels_per_point(), ppp);
            let rect = app.canvas_rect.unwrap();
            assert!((rect.width() - 1000.0 * zoom).abs() < 0.001);
            assert!((rect.height() - 486.0 * zoom).abs() < 0.001);
            let expected = match (ppp, zoom) {
                (1.0, 0.25) => 2,
                (1.0, 0.5) | (1.5 | 2.0, 0.25) => 1,
                _ => 0,
            };
            assert_eq!(app.render_level, expected);
            let view = app.canvas_view((rect.min.x, rect.min.y), ppp);
            let visible = rect.intersect(app.canvas_viewport.unwrap());
            let actual = view
                .visible_region(
                    (visible.min.x, visible.min.y),
                    (visible.max.x, visible.max.y),
                    (1000, 486),
                )
                .unwrap();
            let request = paintcraft_render::display::DisplayRequest::new(
                app.editor.document(),
                actual,
                expected,
            )
            .unwrap();
            assert_eq!(app.render_region, Some(request.region()));
            output.textures_delta.clear();
        }
    }
}

#[test]
fn actual_size_and_fit_report_true_logical_percentages_and_preserve_view_center() {
    for ppp in [1.0, 1.5, 2.0] {
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.editor = Editor::new_document(1000, 486).unwrap();
        let ctx = egui::Context::default();
        let mut output = ctx.run_pass(input(ppp), |_| {});
        output.textures_delta.clear();
        app.zoom = 2.0;
        app.canvas_rect = Some(Rect::from_min_size(
            Pos2::new(-200.0, -100.0),
            Vec2::new(2000.0, 972.0),
        ));
        app.canvas_viewport = Some(Rect::from_min_size(
            Pos2::new(20.0, 40.0),
            Vec2::new(400.0, 200.0),
        ));
        app.dispatch_view_action(&ctx, ViewAction::ActualSize);
        assert_eq!(app.zoom, 1.0);
        assert_eq!(app.pending_center, Some((210.0, 120.0)));
        app.dispatch_view_action(&ctx, ViewAction::FitWindow);
        let expected = (376.0_f32 / 1000.0).min(176.0 / 486.0);
        assert_eq!(app.zoom, expected);
        assert!(app.zoom < 1.0, "fit is not relabeled 100%");
        assert_eq!(app.pending_center, Some((500.0, 243.0)));
        app.dispatch_view_action(&ctx, ViewAction::FitWindow);
        assert_eq!(app.zoom, 1.0, "Studio restores the preceding logical zoom");
    }
}

#[test]
fn classic_startup_fits_at_the_true_percentage_instead_of_relabeling_it_actual_size() {
    for ppp in [1.0, 1.5, 2.0] {
        let mut app = App::new(None);
        app.settings.experience = Experience::Classic;
        app.settings.classic_variant = Some(ClassicVariant::V7);
        app.settings.classic_rulers = false;
        app.editor = Editor::new_document(2000, 1000).unwrap();
        app.initial_fit = true;
        let ctx = egui::Context::default();
        let mut expected = 0.0;
        let mut output = ctx.run_ui(input(ppp), |ui| {
            let clearance = if app.variant() == ClassicVariant::V11 {
                2.0 * paintcraft_classic::tool_controls::CANVAS_CLEARANCE
            } else {
                0.0
            };
            expected = fit_scale(
                ui.available_rect_before_wrap().size() - Vec2::new(clearance + 24.0, 24.0),
                Vec2::new(2000.0, 1000.0),
            );
            app.canvas(ui, true);
        });
        output.textures_delta.clear();
        assert_eq!(app.zoom, expected);
        assert!(app.zoom < 1.0);
        assert!(!app.initial_fit);
        assert!((app.canvas_rect.unwrap().width() - 2000.0 * expected).abs() < 0.001);
    }
}

#[test]
fn control_and_command_scroll_keep_the_logical_view_center_at_each_dpi() {
    for ppp in [1.0, 1.5, 2.0] {
        for modifiers in [
            egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
            egui::Modifiers {
                command: true,
                mac_cmd: true,
                ..Default::default()
            },
        ] {
            let mut app = App::new(None);
            app.settings.experience = Experience::Studio;
            app.editor = Editor::new_document(1000, 486).unwrap();
            app.zoom = 2.0;
            let viewport = Rect::from_min_size(Pos2::new(20.0, 40.0), Vec2::new(400.0, 200.0));
            app.canvas_viewport = Some(viewport);
            app.canvas_rect = Some(Rect::from_min_size(
                Pos2::new(-200.0, -100.0),
                Vec2::new(2000.0, 972.0),
            ));
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(input(ppp), |ui| {
                ui.allocate_rect(viewport, egui::Sense::hover());
            });
            output.textures_delta.clear();
            let mut raw = input(ppp);
            raw.events = vec![
                egui::Event::PointerMoved(viewport.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: Vec2::new(0.0, 1.0),
                    modifiers,
                    source: egui::MouseWheelSource::Unknown,
                    phase: egui::TouchPhase::Move,
                },
            ];
            let mut output = ctx.run_ui(raw, |ui| {
                ui.allocate_rect(viewport, egui::Sense::hover());
                app.handle_canvas_wheel(ui);
            });
            output.textures_delta.clear();
            assert!((app.zoom - 2.2).abs() < 0.00001);
            assert_eq!(app.pending_center, Some((210.0, 120.0)));
        }
    }
}
