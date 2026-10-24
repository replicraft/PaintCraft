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
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
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
