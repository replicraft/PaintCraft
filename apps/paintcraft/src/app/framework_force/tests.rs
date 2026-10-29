use super::*;
fn app() -> App {
    let mut app = App::new(None);
    app.editor = Editor::new_document(48, 32).unwrap();
    app.editor.select_tool(Tool::Brush);
    app.editor.options.size = 14;
    app.initial_fit = false;
    app.zoom = 1.0;
    app
}
fn owner(app: &App) -> Owner {
    app.framework_force_owner(paintcraft_ui::view::View::new((0.0, 0.0), 1, 1.0))
}
fn touch(id: u64, phase: egui::TouchPhase, force: Option<f32>) -> egui::Event {
    egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos: egui::pos2(8.0, 8.0),
        force,
    }
}
fn point(force: f32) -> FrameworkPoint {
    FrameworkPoint {
        at: (8 * 256, 8 * 256),
        force,
    }
}
fn mouse(pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: egui::pos2(8.0, 8.0),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
#[test]
fn absent_force_linux_fallback_and_widget_starts_do_not_become_pressure_ink() {
    let app = app();
    let owner = owner(&app);
    let mut state = State::default();
    let event = touch(1, egui::TouchPhase::Start, None);
    let unchanged = event.clone();
    assert!(matches!(
        state.event(&event, owner, None, true),
        Decision::Ordinary
    ));
    assert_eq!(event, unchanged);
    assert!(state.owner.is_none());
    assert!(matches!(
        state.event(&mouse(true), owner, None, true),
        Decision::Ordinary
    ));
    assert!(matches!(
        state.event(&touch(1, egui::TouchPhase::End, None), owner, None, true),
        Decision::Ordinary
    ));
    assert!(matches!(
        state.event(&mouse(false), owner, None, true),
        Decision::Ordinary
    ));
    // A real force-bearing contact that started on a widget never acquires the
    // canvas later merely because its drag moves over the image.
    assert!(matches!(
        state.event(
            &touch(2, egui::TouchPhase::Start, Some(0.5)),
            owner,
            Some(point(0.5)),
            false
        ),
        Decision::Ordinary
    ));
    assert!(matches!(
        state.event(
            &touch(2, egui::TouchPhase::Move, Some(0.5)),
            owner,
            Some(point(0.5)),
            true
        ),
        Decision::Ordinary
    ));
    assert!(state.owner.is_none());
}
#[test]
fn contact_ownership_fences_all_pointer_painting_without_guessing_mirrors() {
    let app = app();
    let owner = owner(&app);
    let mut state = State::default();
    assert!(matches!(
        state.event(
            &touch(1, egui::TouchPhase::Start, Some(0.3)),
            owner,
            Some(point(0.3)),
            true
        ),
        Decision::Begin(_)
    ));
    assert!(matches!(
        state.event(&mouse(true), owner, None, true),
        Decision::Suppress
    ));
    assert!(matches!(
        state.event(
            &egui::Event::PointerMoved(egui::pos2(20.0, 8.0)),
            owner,
            None,
            true
        ),
        Decision::Suppress
    ));
    assert!(matches!(
        state.event(&touch(2, egui::TouchPhase::Start, None), owner, None, true),
        Decision::Cancel
    ));
    assert!(state.owner.is_none());
    assert!(matches!(
        state.event(
            &egui::Event::PointerMoved(egui::pos2(20.0, 8.0)),
            owner,
            None,
            true
        ),
        Decision::Suppress
    ));
    state.event(&touch(1, egui::TouchPhase::End, None), owner, None, true);
    assert!(state.fenced);
    state.event(&touch(2, egui::TouchPhase::Cancel, None), owner, None, true);
    assert!(!state.fenced);
    assert!(matches!(
        state.event(
            &touch(3, egui::TouchPhase::Start, Some(0.3)),
            owner,
            Some(point(0.3)),
            true
        ),
        Decision::Begin(_)
    ));
}
#[test]
fn competing_force_contact_cancels_mouse_canvas_ink_until_both_are_idle() {
    let app = app();
    let owner = owner(&app);
    let mut state = State::default();
    assert!(matches!(
        state.event(&mouse(true), owner, None, true),
        Decision::Ordinary
    ));
    assert!(matches!(
        state.event(
            &touch(1, egui::TouchPhase::Start, Some(0.5)),
            owner,
            Some(point(0.5)),
            true
        ),
        Decision::Cancel
    ));
    assert!(state.owner.is_none());
    state.event(&touch(1, egui::TouchPhase::End, None), owner, None, true);
    assert!(state.fenced);
    assert!(matches!(
        state.event(&mouse(false), owner, None, true),
        Decision::Suppress
    ));
    assert!(!state.fenced);
}
#[test]
fn view_focus_invalid_force_and_capacity_fail_closed_without_move_reacquisition() {
    let app = app();
    let mut owner = owner(&app);
    let mut state = State::default();
    state.event(
        &touch(1, egui::TouchPhase::Start, Some(0.5)),
        owner,
        Some(point(0.5)),
        true,
    );
    owner.view = paintcraft_ui::view::View::new((2.0, 0.0), 1, 1.0);
    assert!(matches!(
        state.event(
            &touch(1, egui::TouchPhase::Move, Some(0.5)),
            owner,
            Some(point(0.5)),
            true
        ),
        Decision::Cancel
    ));
    assert!(state.owner.is_none());
    state.event(&egui::Event::WindowFocused(false), owner, None, false);
    assert!(matches!(
        state.event(
            &touch(1, egui::TouchPhase::Move, Some(0.5)),
            owner,
            Some(point(0.5)),
            true
        ),
        Decision::Ordinary
    ));
    assert!(state.owner.is_none());
    assert!(matches!(
        state.event(
            &touch(1, egui::TouchPhase::Start, Some(f32::NAN)),
            owner,
            None,
            true
        ),
        Decision::Cancel
    ));
    state.event(&egui::Event::WindowFocused(false), owner, None, false);
    for id in 0..16 {
        state.event(
            &touch(id, egui::TouchPhase::Start, None),
            owner,
            None,
            false,
        );
    }
    assert!(matches!(
        state.event(
            &touch(17, egui::TouchPhase::Start, Some(0.5)),
            owner,
            Some(point(0.5)),
            true
        ),
        Decision::Cancel
    ));
    assert!(state.overflow);
    assert!(state.owner.is_none());
}
fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<bool> {
    let mut routed = Vec::new();
    let original = events.clone();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(100.0, 80.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            let view = app.canvas_view((0.0, 0.0), ctx.pixels_per_point());
            app.framework_force_validate(ui, view);
            for event in ui.input(|i| i.events.clone()) {
                routed.push(app.framework_force_event(
                    ui,
                    ui.layer_id(),
                    ui.max_rect(),
                    None,
                    view,
                    &event,
                ));
            }
            assert_eq!(ui.input(|i| i.events.clone()), original);
        },
    );
    output.textures_delta.clear();
    routed
}
#[test]
fn real_ui_contact_handoff_keeps_preview_private_then_publishes_once_and_undoes() {
    let mut app = app();
    let ctx = egui::Context::default();
    let before = app.editor.document_snapshot();
    let depth = app.editor.undo_depth();
    // Establish the actual root UI layer before checking visible hit testing.
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![touch(1, egui::TouchPhase::Start, Some(0.2)), mouse(true)],
    );
    assert!(app.editor.framework_force_active());
    assert!(app.editor.document().same_semantics(&before));
    frame(
        &mut app,
        &ctx,
        vec![touch(1, egui::TouchPhase::Move, Some(0.8))],
    );
    let preview = app.editor.preview_batch().unwrap().unwrap();
    let expected = paintcraft_editor::execute::execute(
        app.editor.document(),
        app.editor.selection(),
        &preview,
    )
    .unwrap()
    .doc;
    frame(
        &mut app,
        &ctx,
        vec![touch(1, egui::TouchPhase::End, None), mouse(false)],
    );
    app.editor.run_pending();
    assert_eq!(app.editor.undo_depth(), depth + 1);
    assert_eq!(
        paintcraft_render::render_flat(app.editor.document()).unwrap(),
        paintcraft_render::render_flat(&expected).unwrap()
    );
    app.editor.request(555, Action::Undo);
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&before));
}
#[test]
fn real_ui_cancel_view_change_and_disabled_terminal_never_publish_private_ink() {
    let mut app = app();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    let before = app.editor.document_snapshot();
    let depth = app.editor.undo_depth();
    frame(
        &mut app,
        &ctx,
        vec![touch(1, egui::TouchPhase::Start, Some(0.5))],
    );
    assert!(app.editor.framework_force_active());
    app.zoom = 2.0;
    frame(&mut app, &ctx, vec![]);
    assert!(!app.editor.framework_force_active());
    frame(
        &mut app,
        &ctx,
        vec![touch(1, egui::TouchPhase::Move, Some(0.5))],
    );
    assert!(!app.editor.framework_force_active());
    app.framework_force_inactive(&[touch(1, egui::TouchPhase::End, None)]);
    frame(
        &mut app,
        &ctx,
        vec![touch(2, egui::TouchPhase::Start, Some(0.5))],
    );
    assert!(app.editor.framework_force_active());
    app.framework_force_inactive(&[touch(2, egui::TouchPhase::Cancel, None)]);
    app.editor.run_pending();
    assert_eq!(app.editor.undo_depth(), depth);
    assert!(app.editor.document().same_semantics(&before));
}

#[test]
fn zoom_then_new_start_in_one_frame_waits_for_fresh_layout_and_new_contact() {
    let mut app = app();
    app.editor.options.size = 2;
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    let before = app.editor.document_snapshot();
    let depth = app.editor.undo_depth();
    let events = vec![
        egui::Event::Zoom(2.0),
        touch(1, egui::TouchPhase::Start, Some(1.0)),
        mouse(true),
        touch(1, egui::TouchPhase::End, None),
        mouse(false),
    ];
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(100.0, 80.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            let rendered = app.canvas_view((0.0, 0.0), ctx.pixels_per_point());
            for event in ui.input(|i| i.events.clone()) {
                let consumed = app.framework_force_event(
                    ui,
                    ui.layer_id(),
                    ui.max_rect(),
                    None,
                    rendered,
                    &event,
                );
                if let egui::Event::Zoom(scale) = event {
                    assert!(!consumed);
                    app.set_zoom(ui.ctx(), app.zoom * scale, (0.0, 0.0));
                }
            }
        },
    );
    output.textures_delta.clear();
    app.editor.run_pending();
    assert_eq!(app.zoom, 2.0);
    assert!(!app.editor.framework_force_active());
    assert_eq!(app.editor.undo_depth(), depth);
    assert!(app.editor.document().same_semantics(&before));
    frame(
        &mut app,
        &ctx,
        vec![
            touch(2, egui::TouchPhase::Start, Some(1.0)),
            touch(2, egui::TouchPhase::End, None),
        ],
    );
    app.editor.run_pending();
    assert_eq!(app.editor.undo_depth(), depth + 1);
    let raster = &app
        .editor
        .document()
        .layers_in_order()
        .next()
        .unwrap()
        .raster;
    let prior = &before.layers_in_order().next().unwrap().raster;
    assert_ne!(raster.get(4, 4), prior.get(4, 4));
    assert_eq!(raster.get(8, 8), prior.get(8, 8));
}

#[test]
fn unsupported_pencil_force_contact_cannot_cancel_the_existing_pointer_gesture() {
    let mut app = app();
    app.editor.select_tool(Tool::Pencil);
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    let before = app.editor.undo_depth();
    app.editor.pointer_down((4, 4));
    frame(
        &mut app,
        &ctx,
        vec![mouse(true), touch(1, egui::TouchPhase::Start, Some(0.5))],
    );
    assert!(app.editor.pointer_is_down());
    assert!(!app.editor.framework_force_active());
    frame(
        &mut app,
        &ctx,
        vec![touch(1, egui::TouchPhase::End, None), mouse(false)],
    );
    app.editor.pointer_up();
    app.editor.run_pending();
    assert_eq!(app.editor.undo_depth(), before + 1);
}
#[test]
fn stale_force_lifecycle_does_not_retire_a_new_private_text_draft() {
    let mut app = app();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![touch(1, egui::TouchPhase::Start, Some(0.5))],
    );
    assert!(app.editor.framework_force_active());
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((10, 10));
    app.editor.type_text("keep");
    let target = app.editor.canvas_text_target();
    assert_eq!(app.editor.canvas_text_content(), Some("keep"));
    app.framework_force_inactive(&[touch(1, egui::TouchPhase::Cancel, None)]);
    assert_eq!(app.editor.canvas_text_target(), target);
    assert_eq!(app.editor.canvas_text_content(), Some("keep"));
    assert!(app.editor.is_editing_text());
}
