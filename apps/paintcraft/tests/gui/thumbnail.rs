use super::*;
use eframe::egui::{Role, TextureId, accesskit::Toggled, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn toggle(h: &mut Harness<App>) {
    h.get_by_role_and_label(Role::Button, "View").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "View")
        .get_by_label("Thumbnail")
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::Button, "View")
        .get_by_label("Thumbnail")
        .click();
    h.run();
}

fn image(h: &Harness<App>) -> Option<TextureId> {
    let rect = h
        .query_by_role_and_label(Role::Image, "Canvas thumbnail")?
        .rect();
    h.output().shapes.iter().rev().find_map(|shape| {
        let Shape::Mesh(mesh) = &shape.shape else {
            return None;
        };
        rect.contains_rect(mesh.calc_bounds())
            .then_some(mesh.texture_id)
    })
}

fn ready(h: &mut Harness<App>, previous: Option<TextureId>) -> TextureId {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        h.step();
        if let Some(id) = image(h)
            && Some(id) != previous
        {
            return id;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "thumbnail did not complete"
        );
        std::thread::yield_now();
    }
}

#[test]
fn thumbnail_toggles_closes_and_fits_compact_themes_without_editing_or_persisting() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("thumbnail-layout");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let settings = std::fs::read(dir.join("settings.json")).unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(size);
        h.run();
        let before = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let canvas = h.state().canvas_rect().unwrap();
        toggle(&mut h);
        ready(&mut h, None);
        let preview = h
            .get_by_role_and_label(Role::Image, "Canvas thumbnail")
            .rect();
        let close = h.get_by_label("Close thumbnail").rect();
        // Size controls are intentionally hidden when the canvas viewport is short.
        if let Some(control) = h.query_by_role_and_label(Role::Slider, "Tool size") {
            assert!(
                preview.left() > control.rect().right(),
                "the initial overview must clear the floating size control"
            );
        }
        assert!(
            preview.min.x >= 0.0
                && preview.min.y >= 0.0
                && preview.max.x <= size.x
                && preview.max.y <= size.y,
            "{preview:?}"
        );
        assert!(
            close.min.x >= 0.0
                && close.min.y >= 0.0
                && close.max.x <= size.x
                && close.max.y <= size.y,
            "{close:?}"
        );
        assert_eq!(close.size(), Vec2::splat(40.0));
        assert_eq!(h.state().canvas_rect().unwrap(), canvas);
        h.get_by_role_and_label(Role::Button, "View").click();
        h.run();
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "View")
                .get_by_label("Thumbnail")
                .accesskit_node()
                .toggled(),
            Some(Toggled::True)
        );
        h.key_press(Key::Escape);
        h.run();
        h.get_by_label("Close thumbnail").click();
        h.run();
        assert!(h.query_by_label("Close thumbnail").is_none());
        assert!(h.state().editor().document().same_semantics(&before));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), settings);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn thumbnail_tracks_commits_undo_redo_and_captures_pointer_without_changing_zoom() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.set_size(Vec2::new(1360.0, 900.0));
    h.run();
    toggle(&mut h);
    let initial = ready(&mut h, None);
    let before = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    let canvas = h.state().canvas_rect().unwrap();
    let image_rect = h
        .get_by_role_and_label(Role::Image, "Canvas thumbnail")
        .rect();
    let overlap = image_rect.intersect(canvas);
    assert!(overlap.width() > 10.0 && overlap.height() > 10.0);
    let p = overlap.center();
    for pressed in [true, false] {
        h.event(Event::PointerMoved(p));
        h.event(Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.step();
    }
    h.event(Event::MouseWheel {
        unit: eframe::egui::MouseWheelUnit::Line,
        delta: Vec2::new(0.0, 1.0),
        modifiers: Modifiers::COMMAND,
        source: eframe::egui::MouseWheelSource::Unknown,
        phase: eframe::egui::TouchPhase::Move,
    });
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap(), canvas);
    assert!(h.state().editor().document().same_semantics(&before));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    drag(&mut h, (800.0, 400.0), (820.0, 410.0));
    let painted = ready(&mut h, Some(initial));
    assert_eq!(publications(&h), 1);
    h.get_by_label("Undo").click();
    h.run();
    let undone = ready(&mut h, Some(painted));
    assert!(h.state().editor().document().same_semantics(&before));
    h.get_by_label("Redo").click();
    h.run();
    ready(&mut h, Some(undone));
    assert_eq!(publications(&h), 3);
}

#[test]
fn thumbnail_does_not_commit_or_cancel_editable_text() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.get_by_label("Text").click();
    h.run();
    drag(&mut h, (50.0, 50.0), (50.0, 50.0));
    h.event(Event::Text("Keep this editable".into()));
    h.run();
    let before = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    toggle(&mut h);
    ready(&mut h, None);
    assert!(h.state().editor().is_editing_text());
    assert!(
        matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Keep this editable")
    );
    toggle(&mut h);
    assert!(h.query_by_label("Close thumbnail").is_none());
    assert!(h.state().editor().document().same_semantics(&before));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(h.state().editor().is_editing_text());
}

#[test]
#[ignore = "Explicit Wgpu review of the floating Classic Thumbnail window"]
fn export_thumbnail() {
    let output = PathBuf::from("/tmp/paintcraft-thumbnail-window");
    std::fs::create_dir_all(&output).unwrap();
    for (name, appearance, size) in [
        ("light-desktop", Appearance::Light, Vec2::new(1360.0, 900.0)),
        ("dark-desktop", Appearance::Dark, Vec2::new(1360.0, 900.0)),
        ("dark-compact", Appearance::Dark, Vec2::new(480.0, 360.0)),
    ] {
        let dir = scratch("thumbnail-capture");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        drag(&mut h, (500.0, 160.0), (760.0, 320.0));
        h.set_size(size);
        h.run();
        toggle(&mut h);
        ready(&mut h, None);
        h.render()
            .unwrap()
            .save(output.join(format!("{name}.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

fn current(h: &mut Harness<App>) -> TextureId {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        h.step();
        if let Some(texture) = image(h)
            && h.query_by_label("Canvas overview").is_some()
        {
            return texture;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "live overview did not catch up"
        );
        std::thread::yield_now();
    }
}

#[test]
fn live_shapes_share_canvas_pixels_with_release_and_refresh_the_overview_before_publication() {
    for tool in ["Line", "Rectangle", "Ellipse"] {
        let mut h = harness_with(App::new(None), 1.0, true);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        toggle(&mut h);
        let initial = current(&mut h);
        h.get_by_role_and_label(Role::Button, tool).click();
        h.run();
        let before = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let a = at(&h, 400.0, 120.0);
        let b = at(&h, 650.0, 260.0);
        h.event(Event::PointerMoved(a));
        h.event(press(a));
        h.run();
        h.event(Event::PointerMoved(b));
        h.run();
        let live = current(&mut h);
        assert_ne!(live, initial);
        assert!(h.state().editor().gesture_in_progress());
        assert!(h.state().editor().document().same_semantics(&before));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        let pixels = h.state().rendered().to_vec();
        assert!(
            pixels.as_chunks::<4>().0.contains(&[0, 0, 0, 255]),
            "{tool} must render raster pixels before release"
        );
        h.event(release(b));
        h.run();
        current(&mut h);
        assert_eq!(
            h.state().rendered(),
            pixels,
            "{tool} preview must match publication"
        );
        assert_eq!(publications(&h), 1);
    }
}

#[test]
fn editable_text_updates_the_overview_and_escape_removes_it_without_history() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.set_size(Vec2::new(1360.0, 900.0));
    h.run();
    toggle(&mut h);
    let initial = current(&mut h);
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    drag(&mut h, (400.0, 120.0), (400.0, 120.0));
    h.event(Event::Text("Editable".into()));
    h.run();
    let text = current(&mut h);
    assert_ne!(text, initial);
    assert!(h.state().editor().is_editing_text());
    assert_eq!(publications(&h), 0);
    h.key_press(Key::Backspace);
    h.run();
    assert_ne!(current(&mut h), text);
    h.key_press(Key::Escape);
    h.run();
    current(&mut h);
    assert!(!h.state().editor().is_editing_text());
    assert_eq!(publications(&h), 0);
    assert!(
        h.state()
            .editor()
            .document()
            .layers
            .values()
            .all(|layer| layer.raster.resident_tiles() == 0)
    );
}

#[test]
#[ignore = "Explicit Wgpu review of live shape and editable-text overviews"]
fn export_live_thumbnail() {
    let output = PathBuf::from("/tmp/paintcraft-live-thumbnail");
    std::fs::create_dir_all(&output).unwrap();
    for (name, appearance, tool, compact) in [
        ("light-shape", Appearance::Light, "Rectangle", false),
        ("dark-shape", Appearance::Dark, "Ellipse", false),
        ("dark-text", Appearance::Dark, "Text", false),
        ("dark-text-compact", Appearance::Dark, "Text", true),
    ] {
        let dir = scratch("live-thumbnail-capture");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        toggle(&mut h);
        current(&mut h);
        h.get_by_role_and_label(Role::Button, tool).click();
        h.run();
        if tool == "Text" {
            drag(&mut h, (400.0, 120.0), (400.0, 120.0));
            h.event(Event::Text("Still editable".into()));
            h.run();
        } else {
            let a = at(&h, 400.0, 120.0);
            let b = at(&h, 650.0, 260.0);
            h.event(Event::PointerMoved(a));
            h.event(press(a));
            h.run();
            h.event(Event::PointerMoved(b));
            h.run();
        }
        if compact {
            h.set_size(Vec2::new(480.0, 360.0));
            h.run();
        }
        current(&mut h);
        assert_eq!(publications(&h), 0);
        h.render()
            .unwrap()
            .save(output.join(format!("{name}.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
