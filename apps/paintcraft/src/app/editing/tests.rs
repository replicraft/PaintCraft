use super::*;
use std::time::Instant;
const BLACK: Rgba = Rgba([0, 0, 0, 255]);
const RED: Rgba = Rgba([255, 0, 0, 255]);
fn app(native_save: bool) -> (App, egui::Context) {
    let context = egui::Context::default();
    let mut app = App::new(None);
    app.context = context.clone();
    app.initialize_editing(context.clone());
    if native_save {
        app.saving = Some(saving::NativeSaving::new(context.clone()).unwrap());
    }
    (app, context)
}
fn directory() -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("paintcraft-editing-coordinator-{}", Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    std::fs::canonicalize(path).unwrap()
}
fn pump(app: &mut App, context: &egui::Context, mut done: impl FnMut(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.poll_edits(context);
        app.poll_native_saves(context);
        app.pump_tab_close(context);
        app.pump_interface_switch(context);
        if done(app) {
            break;
        }
        assert!(Instant::now() < deadline, "coordinator stalled: {app:?}");
        std::thread::yield_now();
    }
}
fn idle(app: &App) -> bool {
    !app.any_pending_edits()
        && !app.any_deferred_files()
        && !app
            .saving
            .as_ref()
            .is_some_and(saving::NativeSaving::pending)
}
fn color(doc: &paintcraft_document::Document) -> Rgba {
    doc.layers[&doc.roots[0]].raster.default_pixel()
}

#[test]
fn completed_native_crop_remembers_the_authored_size_after_undo() {
    let (mut app, context) = app(false);
    app.settings.canvas.remember_last = true;
    let original = app.editor.document().canvas;
    app.editor.request(
        8001,
        Action::Select {
            corners: Some(((10, 20), (39, 59))),
        },
    );
    pump(&mut app, &context, idle);
    app.editor.request(8002, Action::CropToSelection);
    pump(&mut app, &context, idle);
    assert_eq!(app.editor.document().canvas, (30, 40));
    assert_eq!(app.settings.canvas.last, Some((30, 40)));
    app.editor.request(8003, Action::Undo);
    pump(&mut app, &context, idle);
    assert_eq!(app.editor.document().canvas, original);
    assert_eq!(app.settings.canvas.last, Some((30, 40)));
}
#[test]
fn save_captures_preceding_edit_while_later_edit_stays_dirty_for_native_and_fallback_io() {
    for native_save in [false, true] {
        let root = directory();
        let path = root.join("captured.pcnd");
        let (mut app, context) = app(native_save);
        let release = app.editing.as_ref().unwrap().pause();
        app.editor.request(1, Action::ClearCanvas { color: BLACK });
        app.do_save(&context, path.clone());
        assert_eq!(app.deferred_files.len(), 1);
        app.editor.request(2, Action::ClearCanvas { color: RED });
        assert!(!path.exists());
        release.send(()).unwrap();
        pump(&mut app, &context, idle);
        assert_eq!(color(Editor::open(&path).unwrap().document()), BLACK);
        assert_eq!(color(app.editor.document()), RED);
        assert!(app.editor.is_dirty());
        assert_eq!(app.bound.as_deref(), Some(path.as_path()));
        app.editor.request(3, Action::Undo);
        pump(&mut app, &context, idle);
        assert_eq!(color(app.editor.document()), BLACK);
        assert!(
            !app.editor.is_dirty(),
            "save acknowledgement survived later preparation"
        );
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn inactive_tabs_receive_their_own_worker_results_and_save_captures() {
    let root = directory();
    let first_path = root.join("first.pcnd");
    let second_path = root.join("second.pcnd");
    let (mut app, context) = app(true);
    app.settings.experience = Experience::Studio;
    let first = app.active_tab_id();
    let release = app.editing.as_ref().unwrap().pause();
    app.editor.request(1, Action::ClearCanvas { color: BLACK });
    app.do_save(&context, first_path.clone());
    app.new_document();
    let second = app.active_tab_id();
    assert_ne!(first, second);
    app.editor.request(1, Action::ClearCanvas { color: RED });
    app.do_save(&context, second_path.clone());
    assert!(app.any_deferred_files());
    release.send(()).unwrap();
    pump(&mut app, &context, idle);
    assert_eq!(app.active_tab_id(), second);
    assert_eq!(color(app.editor.document()), RED);
    assert!(!app.editor.is_dirty());
    app.activate_tab(first, &context);
    assert_eq!(color(app.editor.document()), BLACK);
    assert!(!app.editor.is_dirty());
    assert_eq!(color(Editor::open(&first_path).unwrap().document()), BLACK);
    assert_eq!(color(Editor::open(&second_path).unwrap().document()), RED);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn tab_and_window_close_wait_for_the_pending_edit_before_presenting_unsaved_work() {
    for window in [false, true] {
        let (mut app, context) = app(true);
        let id = app.active_tab_id();
        let release = app.editing.as_ref().unwrap().pause();
        app.editor.request(1, Action::ClearCanvas { color: BLACK });
        if window {
            app.close_after_requested_saves(&context);
        } else {
            app.request_close_tab(id, &context);
        }
        assert!(app.confirm.is_none() && app.tab_closing.is_none());
        assert!(!app.closing_workspace);
        release.send(()).unwrap();
        pump(&mut app, &context, |app| {
            if window {
                app.confirm == Some(Confirm::QuitUnsaved)
            } else {
                app.tab_closing == Some(id)
            }
        });
        assert_eq!(color(app.editor.document()), BLACK);
        assert_eq!(app.document_count(), 1);
        assert!(!app.closing_workspace);
    }
}
#[test]
fn interface_save_gate_waits_for_editing_and_saves_the_completed_revision() {
    let root = directory();
    let path = root.join("classic.pcnd");
    let (mut app, context) = app(true);
    app.path_text = path.to_string_lossy().into_owned();
    let release = app.editing.as_ref().unwrap().pause();
    app.editor.request(1, Action::ClearCanvas { color: BLACK });
    app.begin_interface_switch(Experience::Studio, &context);
    assert_eq!(app.experience(), Experience::Classic);
    assert!(!path.exists());
    release.send(()).unwrap();
    pump(&mut app, &context, |app| {
        app.experience() == Experience::Studio && idle(app)
    });
    assert_eq!(color(Editor::open(&path).unwrap().document()), BLACK);
    assert!(!app.editor.is_dirty());
    app.choose_experience(Experience::Classic);
    pump(&mut app, &context, |app| {
        app.experience() == Experience::Classic && idle(app)
    });
    assert_eq!(color(app.editor.document()), BLACK);
    assert!(!app.editor.is_dirty());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn export_confirmation_holds_its_revision_until_later_edits_can_resume() {
    let root = directory();
    let path = root.join("captured.png");
    let (mut app, context) = app(true);
    let release = app.editing.as_ref().unwrap().pause();
    app.editor.request(1, Action::ClearCanvas { color: BLACK });
    app.export_selected(path.clone());
    app.editor.request(2, Action::ClearCanvas { color: RED });
    release.send(()).unwrap();
    pump(&mut app, &context, |app| {
        matches!(app.confirm, Some(Confirm::Export { .. }))
    });
    assert_eq!(color(app.editor.document()), BLACK);
    let Some(Confirm::Export { format, losses, .. }) = app.confirm.take() else {
        panic!("export confirmation");
    };
    app.do_export(&path, format, &losses);
    pump(&mut app, &context, idle);
    assert_eq!(color(app.editor.document()), RED);
    let decoded = image::decode(&std::fs::read(&path).unwrap()).unwrap();
    assert!(
        decoded
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == BLACK.0)
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn real_two_pass_ui_routes_a_gesture_once_and_pumps_its_background_publication() {
    use eframe::egui::{Event, Modifiers, PointerButton, Vec2};
    use egui_kittest::Harness;
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1200.0, 900.0))
        .build_ui_state(
            |ui, app: &mut App| {
                if app.editing.is_none() {
                    app.initialize_editing(ui.ctx().clone());
                }
                app.ui(ui);
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx()
                        .request_discard("exercise two native editing layout passes");
                }
            },
            App::new(None).with_initial_zoom(1.0),
        );
    harness.run();
    let release = harness.state().editing.as_ref().unwrap().pause();
    harness.state_mut().editor.tool = Tool::Pencil;
    let point = harness.state().canvas_rect().unwrap().min + Vec2::new(30.5, 40.5);
    harness.input_mut().events.extend([
        Event::PointerMoved(point),
        Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    harness.step();
    assert_eq!(harness.state().editor.publications(), 0);
    assert!(harness.state().editor.has_pending_edits());
    assert!(harness.state().editor.preview().is_some());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().editor.has_pending_edits() {
        assert!(Instant::now() < deadline);
        harness.step();
        std::thread::yield_now();
    }
    harness.run_steps(3);
    let app = harness.state();
    assert_eq!(app.editor.publications(), 1);
    let doc = app.editor.document();
    assert_eq!(doc.layers[&doc.roots[0]].raster.get(30, 40), BLACK);
    assert_eq!(&app.rendered()[(40 * 1000 + 30) * 4..][..4], &BLACK.0);
}

#[test]
fn coalesced_translucent_gestures_keep_their_complete_preview_and_publish_once_per_gesture() {
    use eframe::egui::{Event, Modifiers, PointerButton, Vec2};
    use egui_kittest::Harness;
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1200.0, 900.0))
        .build_ui_state(
            |ui, app: &mut App| {
                if app.editing.is_none() {
                    app.initialize_editing(ui.ctx().clone());
                }
                app.ui(ui);
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx()
                        .request_discard("coalesced paint queue in two passes");
                }
            },
            App::new(None).with_initial_zoom(1.0),
        );
    harness.run();
    let release = harness.state().editing.as_ref().unwrap().pause();
    harness.state_mut().editor.tool = Tool::Pencil;
    harness.state_mut().editor.color = Rgba([0, 0, 0, 64]);
    let point = harness.state().canvas_rect().unwrap().min + Vec2::new(30.5, 40.5);
    for _ in 0..3 {
        harness.input_mut().events.extend([
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]);
    }
    harness.step();
    assert_eq!(harness.state().editor.publications(), 0);
    assert!(
        matches!(harness.state().editor.preview(), Some(Preview::Paths(paths)) if paths.len()==3)
    );
    let mut expected = [107, 107, 107, 255];
    let offset = (40 * 1000 + 30) * 4;
    assert_eq!(&harness.state().rendered()[offset..offset + 4], &expected);
    // Keep drawing in later frames while the earlier released strokes still
    // wait. Move/release must continue reaching the admitted live gesture.
    harness.event(Event::PointerButton {
        pos: point,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.step();
    assert!(harness.state().editor.gesture_in_progress());
    let next = point + Vec2::new(1.0, 0.0);
    harness.event(Event::PointerMoved(next));
    harness.step();
    harness.event(Event::PointerButton {
        pos: next,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.step();
    assert!(!harness.state().editor.gesture_in_progress());
    assert!(
        matches!(harness.state().editor.preview(), Some(Preview::Paths(paths)) if paths.len()==4)
    );
    expected = [80, 80, 80, 255];
    assert_eq!(&harness.state().rendered()[offset..offset + 4], &expected);
    assert_eq!(
        &harness.state().rendered()[offset + 4..offset + 8],
        &[191, 191, 191, 255]
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().editor.has_pending_edits() {
        assert!(Instant::now() < deadline);
        harness.step();
        std::thread::yield_now();
    }
    harness.run_steps(3);
    let app = harness.state();
    assert_eq!(app.editor.publications(), 4);
    let doc = app.editor.document();
    assert_eq!(doc.layers[&doc.roots[0]].raster.get(30, 40).0, expected);
    assert_eq!(&app.rendered()[offset..offset + 4], &expected);
    assert_eq!(app.editor.undo_depth(), 4);
}

#[test]
fn native_worker_transforms_captured_selection_then_whole_canvas_and_remembers_authored_size() {
    use paintcraft_core::types::orthogonal::OrthogonalTransform as Transform;
    let (mut app, context) = app(false);
    let layer = app.editor.target().unwrap();
    app.editor.request(
        8101,
        Action::Stroke(paintcraft_tools::Stroke {
            media: Default::default(),
            layer,
            points: vec![(5, 6)],
            point_scale: 1,
            color: RED,
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::Replace,
        }),
    );
    pump(&mut app, &context, idle);
    app.editor.request(
        8102,
        Action::Select {
            corners: Some(((4, 5), (9, 7))),
        },
    );
    pump(&mut app, &context, idle);
    let before = app.editor.document_snapshot();
    let selection = app.editor.selection();
    let count = app.editor.publications();
    app.editor.request(
        8103,
        Action::TransformImage {
            transform: Transform::FlipHorizontal,
            clear: Rgba([255; 4]),
        },
    );
    pump(&mut app, &context, idle);
    assert_eq!(app.editor.publications(), count + 1);
    assert_eq!(app.editor.document().layers[&layer].raster.get(8, 6), RED);
    app.editor.request(8104, Action::Undo);
    pump(&mut app, &context, idle);
    assert!(app.editor.document().same_semantics(&before));
    assert_eq!(app.editor.selection(), selection);
    app.editor.request(8105, Action::Select { corners: None });
    pump(&mut app, &context, idle);
    let size = app.editor.document().canvas;
    app.editor.request(
        8106,
        Action::TransformImage {
            transform: Transform::RotateRight,
            clear: Rgba([255; 4]),
        },
    );
    pump(&mut app, &context, idle);
    assert_eq!(app.editor.document().canvas, (size.1, size.0));
    assert_eq!(app.settings.canvas.last, Some((size.1, size.0)));
    assert_eq!(
        app.editor.document().layers[&layer]
            .raster
            .get(i64::from(size.1) - 7, 5),
        RED
    );
    app.editor.request(8107, Action::Undo);
    pump(&mut app, &context, idle);
    assert_eq!(app.editor.document().canvas, size);
    assert_eq!(app.settings.canvas.last, Some((size.1, size.0)));
}

#[test]
fn icon_export_waits_for_prior_edits_and_holds_later_work_until_cancel() {
    let (mut app, context) = app(false);
    app.settings.experience = Experience::Studio;
    let release = app.editing.as_ref().unwrap().pause();
    app.editor
        .request(7101, Action::ClearCanvas { color: BLACK });
    app.begin_icon_export();
    app.editor.request(7102, Action::ClearCanvas { color: RED });
    release.send(()).unwrap();
    pump(&mut app, &context, |app| {
        app.confirm == Some(Confirm::IconExport)
    });
    assert_eq!(color(app.editor.document()), BLACK);
    app.confirm = None;
    app.icon_export = None;
    pump(&mut app, &context, idle);
    assert_eq!(color(app.editor.document()), RED);
}
