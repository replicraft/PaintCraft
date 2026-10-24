use super::*;

fn studio() -> (App, egui::Context) {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.canvas_viewport = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
    app.canvas_rect = app.canvas_viewport;
    (app, egui::Context::default())
}
fn fit(app: &mut App, ctx: &egui::Context) {
    app.dispatch_view_action(ctx, ViewAction::FitWindow);
}

#[test]
fn fit_toggles_exact_fractional_zoom_and_centers_without_changing_document_data() {
    let (mut app, ctx) = studio();
    app.set_zoom(&ctx, 0.375, (40.0, 30.0));
    let ticket = app.editor.history_view().ticket;
    let bytes = paintcraft_formats::pcnd::write(app.editor.document()).unwrap();
    let (width, height) = app.editor.document().canvas;
    for _ in 0..3 {
        fit(&mut app, &ctx);
        let fitted = fit_scale(
            Vec2::new(776.0, 576.0),
            Vec2::new(width as f32, height as f32),
            1.0,
        );
        assert_eq!(app.zoom, fitted);
        assert_eq!(
            app.pending_center,
            Some((width as f32 / 2.0, height as f32 / 2.0))
        );
        fit(&mut app, &ctx);
        assert_eq!(app.zoom, 0.375);
        assert!(app.studio_fit.is_none());
        assert_eq!(
            app.pending_center,
            Some((width as f32 / 2.0, height as f32 / 2.0))
        );
    }
    assert_eq!(app.editor.history_view().ticket, ticket);
    assert_eq!(
        paintcraft_formats::pcnd::write(app.editor.document()).unwrap(),
        bytes
    );
    assert!(!app.editor.is_dirty());
}

#[test]
fn every_tab_keeps_its_own_fit_restore_state_and_new_documents_start_fresh() {
    let (mut app, ctx) = studio();
    let first = app.active_tab_id();
    app.set_zoom(&ctx, 0.375, (0.0, 0.0));
    fit(&mut app, &ctx);
    app.new_document();
    let second = app.active_tab_id();
    assert!(app.studio_fit.is_none());
    app.set_zoom(&ctx, 4.0, (0.0, 0.0));
    fit(&mut app, &ctx);
    app.activate_tab(first, &ctx);
    fit(&mut app, &ctx);
    assert_eq!(app.zoom, 0.375);
    app.activate_tab(second, &ctx);
    fit(&mut app, &ctx);
    assert_eq!(app.zoom, 4.0);
}

#[test]
fn other_zoom_actions_discard_the_fit_restore_target_but_painting_does_not() {
    let (mut app, ctx) = studio();
    for action in [
        ViewAction::ZoomIn,
        ViewAction::ZoomOut,
        ViewAction::ActualSize,
        ViewAction::SetZoom(3750),
    ] {
        app.set_zoom(&ctx, 8.0, (0.0, 0.0));
        fit(&mut app, &ctx);
        app.dispatch_view_action(&ctx, action);
        let manual = app.zoom;
        assert!(app.studio_fit.is_none());
        fit(&mut app, &ctx);
        assert_eq!(app.studio_fit.as_ref().unwrap().previous, manual);
        fit(&mut app, &ctx);
        assert_eq!(app.zoom, manual);
    }
    app.set_zoom(&ctx, 8.0, (0.0, 0.0));
    fit(&mut app, &ctx);
    app.editor.pointer_down((10, 10));
    app.editor.pointer_up();
    assert_eq!(app.editor.run_pending(), 1);
    let ticket = app.editor.history_view().ticket;
    fit(&mut app, &ctx);
    assert_eq!(app.zoom, 8.0);
    assert_eq!(app.editor.history_view().ticket, ticket);
}

#[test]
fn a_replaced_document_or_changed_canvas_cannot_restore_an_old_zoom() {
    let (mut app, ctx) = studio();
    app.set_zoom(&ctx, 8.0, (0.0, 0.0));
    fit(&mut app, &ctx);
    let old_fit = app.zoom;
    app.editor = Editor::new_document(200, 100).unwrap();
    fit(&mut app, &ctx);
    assert_ne!(app.zoom, 8.0);
    assert_eq!(app.studio_fit.as_ref().unwrap().previous, old_fit);
    let same_id = app.editor.document().document_id;
    app.editor.request(
        991,
        Action::ResizeCanvas {
            width: 300,
            height: 200,
        },
    );
    app.editor.run_pending();
    assert_eq!(app.editor.document().document_id, same_id);
    let old_fit = app.zoom;
    fit(&mut app, &ctx);
    assert_eq!(app.studio_fit.as_ref().unwrap().previous, old_fit);
    assert_eq!(app.studio_fit.as_ref().unwrap().size, (300, 200));
}

#[test]
fn classic_repeated_fit_remains_a_fit_and_text_drafts_stay_editable() {
    let (mut app, ctx) = studio();
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((20, 20));
    app.editor.type_text("Editable text");
    let ticket = app.editor.history_view().ticket;
    fit(&mut app, &ctx);
    fit(&mut app, &ctx);
    assert!(app.editor.is_editing_text());
    assert_eq!(app.editor.history_view().ticket, ticket);
    app.settings.experience = Experience::Classic;
    app.set_zoom(&ctx, 8.0, (0.0, 0.0));
    fit(&mut app, &ctx);
    let fitted = app.zoom;
    fit(&mut app, &ctx);
    assert_eq!(app.zoom, fitted);
    assert!(app.studio_fit.is_none());
}

#[test]
fn explicit_zoom_ends_navigation_drags_even_when_the_scale_is_unchanged() {
    use paintcraft_tools::navigation::PanButton;
    let (mut app, ctx) = studio();
    let scope = egui::Id::unique("source");
    app.studio
        .pan
        .begin(scope, PanButton::Primary, (10.0, 10.0));
    app.studio.zoom.begin(
        scope,
        paintcraft_input::PointerButton::Primary,
        (10.0, 10.0),
    );
    app.studio.auto_scroll.begin(scope, (0.0, 0.0), 0.0);
    let zoom = app.zoom;
    app.set_zoom(&ctx, zoom, (100.0, 100.0));
    assert!(!app.studio.pan.is_active());
    assert!(!app.studio.zoom.is_active());
    assert!(
        app.studio
            .auto_scroll
            .advance(
                &scope,
                (0.0, 0.0),
                (400.0, 150.0),
                ((0.0, 0.0), (400.0, 300.0)),
                0.1
            )
            .is_none()
    );
    assert_eq!(app.zoom, zoom);
}
