//! Real import + navigation layout regression; no Worker or pixel-oracle dependency.
use crate::{WebApp, editor::Editor};
use eframe::egui::{self, Event, Modifiers, PointerButton, Vec2};
use egui_kittest::Harness;
use paintcraft_ui::actions::Frontend;

#[test]
fn imported_studio_document_starts_centered_after_old_pan_and_same_owner_pinch_keeps_anchor() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .with_max_steps(100)
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h.state_mut().frontend = Frontend::Studio;
    h.state_mut().zoom = 4.0;
    h.run();
    let before_pan = h.state().canvas.rect.unwrap();
    let ticket = h.state().editor.session.ticket();
    let p = crate::touch_navigation_tests::canvas_point(&h, 70.0);
    h.event(Event::PointerMoved(p));
    h.run();
    h.event(Event::PointerButton {
        pos: p,
        button: PointerButton::Middle,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.step();
    let moved = p + Vec2::new(250.0, -120.0);
    h.event(Event::PointerMoved(moved));
    h.step();
    h.event(Event::PointerButton {
        pos: moved,
        button: PointerButton::Middle,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run();
    assert!(
        h.state().canvas.rect.unwrap().min.distance(before_pan.min) > 100.0,
        "the old document must have a genuine noncentral retained pan"
    );
    assert_eq!(h.state().editor.session.ticket(), ticket);
    let old_owner = h.state().editor.session.history_ticket().owner;
    h.state_mut()
        .install_import_editor(Editor::new(256, 128).unwrap(), "replacement.pcnd".into());
    h.run();
    assert_ne!(h.state().editor.session.history_ticket().owner, old_owner);
    let image = h.state().canvas.rect.unwrap();
    let viewport = h.state().canvas.touch.hit_region().unwrap().0;
    assert!(
        viewport.expand(1.0).contains_rect(image),
        "new fitted document must be wholly visible: {image:?} in {viewport:?}"
    );
    assert!(
        image.center().distance(viewport.center()) < 2.0,
        "new document must start centered, not inherit old pan: {image:?}/{viewport:?}"
    );
    assert!((image.width() - 256.0 * h.state().zoom).abs() < 1.0);
    assert!((image.height() - 128.0 * h.state().zoom).abs() < 1.0);
    assert!(h.state().canvas.pending_center.is_none());
    let source = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    let center = crate::touch_navigation_tests::canvas_point(&h, 70.0);
    let a = center - Vec2::new(40.0, 0.0);
    let b = center + Vec2::new(40.0, 0.0);
    crate::touch_navigation_tests::start(&mut h, 1, a);
    let document_point = (center - h.state().canvas.rect.unwrap().min) / h.state().zoom;
    crate::touch_navigation_tests::start(&mut h, 2, b);
    h.run();
    let zoom = h.state().zoom;
    let a2 = a - Vec2::new(20.0, 0.0);
    let b2 = b + Vec2::new(20.0, 0.0);
    h.input_mut()
        .events
        .extend([(1, a2), (2, b2)].map(|(id, pos)| Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(id),
            phase: egui::TouchPhase::Move,
            pos,
            force: None,
        }));
    h.run();
    assert!((h.state().zoom - (zoom * 1.5).min(10.0)).abs() < 0.001);
    let anchored = h.state().canvas.rect.unwrap().min + document_point * h.state().zoom;
    assert!(
        anchored.distance(center) < 1.1,
        "{anchored:?} != {center:?}"
    );
    crate::touch_navigation_tests::end(&mut h, 2, b2);
    crate::touch_navigation_tests::end(&mut h, 1, a2);
    h.run();
    let later = h.state().canvas.rect.unwrap().min + document_point * h.state().zoom;
    assert!(later.distance(center) < 1.1, "late {later:?} != {center:?}");
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(
        &source,
        &h.state().editor.session.snapshot()
    ));
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}

#[test]
fn rapid_actual_size_then_command_wheel_keeps_studio_document_point_visible() {
    for panned in [false, true] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(1360.0, 1000.0))
            .with_pixels_per_point(1.0)
            .build_eframe(|cc| {
                let mut app = WebApp::new(&cc.egui_ctx);
                app.frontend = Frontend::Studio;
                app.install(crate::files::Loaded {
                    raster_preview: false,
                    converted_copy: false,
                    document: paintcraft_document::new_document(
                        513,
                        257,
                        Some(paintcraft_core::types::Rgba([255; 4])),
                        0,
                        paintcraft_document::POLICY,
                    )
                    .unwrap(),
                    name: "navigation.pcnd".into(),
                    losses: Vec::new(),
                });
                app
            });
        for _ in 0..2 {
            h.step();
        }
        let modifiers = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        for pressed in [true, false] {
            h.event(Event::Key {
                key: egui::Key::Num0,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            });
            h.step();
        }
        for _ in 0..2 {
            h.step();
        }
        assert_eq!(h.state().zoom, 1.0);
        if panned {
            let start = h.state().canvas.rect.unwrap().center();
            h.event(Event::PointerMoved(start));
            h.step();
            h.event(Event::PointerButton {
                pos: start,
                button: PointerButton::Middle,
                pressed: true,
                modifiers: Modifiers::NONE,
            });
            h.step();
            let end = start + Vec2::new(80.0, -50.0);
            h.event(Event::PointerMoved(end));
            h.step();
            h.event(Event::PointerButton {
                pos: end,
                button: PointerButton::Middle,
                pressed: false,
                modifiers: Modifiers::NONE,
            });
            h.step();
            for _ in 0..4 {
                h.step();
            }
        }
        let source = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        let viewport = h.state().canvas.touch.hit_region().unwrap().0;
        let point = (viewport.center() - h.state().canvas.rect.unwrap().min) / h.state().zoom;
        if panned {
            assert!(
                (point - Vec2::new(256.5, 128.5)).length() > 30.0,
                "real middle-button pan required"
            );
        }
        h.event(Event::PointerMoved(egui::pos2(680.0, 500.0)));
        h.event(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -1280.0),
            modifiers,
            source: egui::MouseWheelSource::Unknown,
            phase: egui::TouchPhase::Move,
        });
        h.step();
        for frame in 0..60 {
            h.step();
            if frame >= 4 {
                let app = h.state();
                assert_eq!(app.zoom, 0.125);
                let viewport = app.canvas.touch.hit_region().unwrap().0;
                let image = app.canvas.rect.unwrap();
                assert!(
                    (image.min + point * app.zoom).distance(viewport.center()) < 1.5,
                    "panned={panned} frame={frame} image={image:?} viewport={viewport:?}"
                );
                assert!(image.intersects(viewport));
                assert_eq!(app.canvas.filter_view.unwrap().level, 3);
            }
        }
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &h.state().editor.session.snapshot()
        ));
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert_eq!(h.state().editor.session.undo_depth(), 0);
    }
}
