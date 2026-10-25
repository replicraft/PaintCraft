use super::*;

#[test]
fn rectangle_zoom_uses_logical_points_and_centers_the_selected_document_area() {
    for ppp in [1.0, 1.5, 2.0] {
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.editor.select_tool(Tool::Magnifier);
        app.zoom = 1.0;
        let ctx = egui::Context::default();
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(ppp);
        let mut output = ctx.run_ui(input, |ui| {
            let canvas = Rect::from_min_size(Pos2::new(20.0, 30.0), Vec2::new(1000.0, 486.0));
            let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
            let (scope, scale) = app.studio_zoom_context(ui, canvas, viewport).unwrap();
            assert_eq!(scale, 1.0);
            app.studio.zoom.begin(
                scope,
                paintcraft_input::PointerButton::Primary,
                (10.0, 20.0),
            );
            assert!(app.studio_zoom_event(
                ui,
                ui.painter(),
                canvas,
                viewport,
                &egui::Event::PointerButton {
                    pos: Pos2::new(130.0, 100.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default()
                }
            ));
        });
        output.textures_delta.clear();
        assert_eq!(app.zoom, 4.0);
        assert_eq!(app.pending_center, Some((60.0, 45.0)));
    }
}
