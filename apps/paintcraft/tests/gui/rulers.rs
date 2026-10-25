use super::*;
use paintcraft::settings::{ClassicVariant, Settings};

fn toggle(h: &mut Harness<App>) {
    h.input_mut().events.extend([
        key(Key::R, Modifiers::COMMAND),
        key_up(Key::R, Modifiers::COMMAND),
    ]);
    h.run();
}

#[test]
fn rulers_reserve_viewport_space_persist_and_preserve_document_pixel_mapping() {
    for ppp in [1.0, 2.0] {
        let dir = scratch(&format!("rulers-{ppp}"));
        Settings {
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.get_by_label("Pencil").click();
        h.run();
        let base = h.state().editor().document_snapshot();
        let viewport = h.state().canvas_viewport().unwrap();
        let depth = h.state().editor().undo_depth();
        h.get_by_role_and_label(eframe::egui::Role::Button, "View")
            .click();
        h.run();
        h.get_by_label_contains("Rulers").click();
        h.run();
        let ruled = h.state().canvas_viewport().unwrap();
        assert_eq!(
            viewport.size() - ruled.size(),
            Vec2::splat(paintcraft_classic::RULER_SIZE)
        );
        assert!(h.query_by_label("Horizontal ruler (pixels)").is_some());
        assert!(h.query_by_label("Vertical ruler (pixels)").is_some());
        assert!(Settings::load(&dir).0.classic_rulers);
        assert!(h.state().editor().document().same_semantics(&base));
        assert_eq!(h.state().editor().undo_depth(), depth);
        let header = ruled.min - Vec2::new(5.0, 5.0);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(header), press(header), release(header)]);
        h.run();
        assert!(h.state().editor().document().same_semantics(&base));
        let origin = h.state().canvas_rect().unwrap().min;
        let scale = h.state().canvas_rect().unwrap().width()
            / h.state().editor().document().canvas.0 as f32;
        let a = origin + Vec2::splat(20.5) * scale;
        let b = origin + Vec2::new(30.5, 20.5) * scale;
        h.input_mut().events.extend([
            Event::PointerMoved(a),
            press(a),
            Event::PointerMoved(b),
            release(b),
        ]);
        h.run();
        assert_eq!(top_pixel(&h, 20, 20), Rgba([0, 0, 0, 255]));
        assert_eq!(top_pixel(&h, 30, 20), Rgba([0, 0, 0, 255]));
        assert_eq!(top_pixel(&h, 25, 21), Rgba([255, 255, 255, 255]));
        drop(h);
        let restarted = harness_with(App::new(None).with_settings(dir.clone()), ppp, false);
        assert!(
            restarted
                .query_by_label("Horizontal ruler (pixels)")
                .is_some()
        );
        drop(restarted);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn toggling_rulers_preserves_scrolled_document_origin_and_pixels() {
    let dir = scratch("rulers-scroll");
    let editor = paintcraft::controller::Editor::new_document(1600, 1200).unwrap();
    let path = dir.join("large.pcnd");
    std::fs::write(
        &path,
        paintcraft_formats::pcnd::write(editor.document()).unwrap(),
    )
    .unwrap();
    let mut h = harness_with(App::new(Some(path)), 1.0, true);
    let base = h.state().editor().document_snapshot();
    let initial_offset =
        h.state().canvas_viewport().unwrap().min - h.state().canvas_rect().unwrap().min;
    let pointer = h.state().canvas_viewport().unwrap().center();
    h.input_mut().events.extend([
        Event::PointerMoved(pointer),
        Event::MouseWheel {
            unit: eframe::egui::MouseWheelUnit::Point,
            delta: Vec2::new(-120.0, -160.0),
            phase: eframe::egui::TouchPhase::Move,
            source: eframe::egui::MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.run();
    let offset = h.state().canvas_viewport().unwrap().min - h.state().canvas_rect().unwrap().min;
    // Account for the stable tool-rail inset: verify real scroll displacement,
    // then preserve the resulting document origin when rulers are toggled.
    assert!(offset.x - initial_offset.x > 100.0 && offset.y - initial_offset.y > 100.0);
    toggle(&mut h);
    let ruled = h.state().canvas_viewport().unwrap().min - h.state().canvas_rect().unwrap().min;
    assert!(
        (offset - ruled).length() < 0.1,
        "scroll offset changed: {offset:?} -> {ruled:?}"
    );
    // With a scrolled canvas behind the strips, even their innermost edge must
    // be excluded from painting, regardless of the ScrollArea's clip margin.
    let viewport = h.state().canvas_viewport().unwrap();
    for point in [
        eframe::egui::pos2(viewport.center().x, viewport.top() - 0.25),
        eframe::egui::pos2(viewport.left() - 0.25, viewport.center().y),
    ] {
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point), release(point)]);
        h.run();
        assert!(h.state().editor().document().same_semantics(&base));
    }
    toggle(&mut h);
    let plain = h.state().canvas_viewport().unwrap().min - h.state().canvas_rect().unwrap().min;
    assert!((offset - plain).length() < 0.1);
    assert!(h.state().editor().document().same_semantics(&base));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rulers_cancel_pending_geometry_and_do_not_bind_studio_command_r() {
    let mut h = harness();
    let base = h.state().editor().document_snapshot();
    let point = at(&h, 20.0, 20.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(point), press(point)]);
    h.run();
    assert!(h.state().editor().preview().is_some());
    toggle(&mut h);
    assert!(h.state().editor().preview().is_none());
    h.event(release(point));
    h.run();
    assert!(h.state().editor().document().same_semantics(&base));
    let (mut studio, dir) = super::studio::studio_harness(true);
    toggle(&mut studio);
    assert!(studio.query_by_label("Horizontal ruler (pixels)").is_none());
    drop(studio);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires a native GPU; exports Classic rulers in both themes and scales"]
fn render_classic_rulers_for_review() {
    use paintcraft::settings::Appearance;
    let output = PathBuf::from("/tmp/paintcraft-classic-rulers");
    std::fs::create_dir_all(&output).unwrap();
    for (name, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
        for ppp in [1.0, 2.0] {
            let dir = scratch(&format!("rulers-review-{name}-{ppp}"));
            Settings {
                classic_variant: Some(ClassicVariant::V11),
                appearance,
                classic_rulers: true,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, false);
            h.set_render_cursor(false);
            h.event(Event::PointerMoved(at(&h, 70.0, 80.0)));
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{name}-{ppp}x.png")))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
