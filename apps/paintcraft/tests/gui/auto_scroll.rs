use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Experience, Settings};
use paintcraft_tools::{Preview, Tool};
use paintcraft_ui::i18n::{Language, translate};

fn setup(tool: Tool, enabled: bool, ppp: f32, multiple: bool) -> (Harness<'static, App>, PathBuf) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = scratch(&format!(
        "studio-auto-scroll-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    Settings {
        experience: Experience::Studio,
        studio_auto_scroll: enabled,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, multiple);
    h.set_size(Vec2::new(1480.0, 960.0));
    h.run();
    h.get_by_role_and_label(Role::Button, tool.name()).click();
    h.run();
    // Remove floating overlays so all viewport edges are available to this drag.
    for label in ["Tools", "Colors", "Layers", "History"] {
        h.get_by_role_and_label(Role::Button, "Window").click();
        h.run();
        h.get_by_role_and_label(Role::CheckBox, label).click();
        h.run();
        h.event(key(Key::Escape, Modifiers::NONE));
        h.event(key_up(Key::Escape, Modifiers::NONE));
        h.run();
    }
    for _ in 0..4 {
        h.event(key(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
        h.event(key_up(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
        h.run();
    }
    assert!(
        h.state().canvas_rect().unwrap().width() > h.state().canvas_viewport().unwrap().width()
    );
    (h, dir)
}
fn start(h: &mut Harness<App>, direction: Vec2) -> Pos2 {
    let viewport = h.state().canvas_viewport().unwrap();
    let a = viewport.center();
    let b = a + direction * (viewport.size() * 0.5 - Vec2::splat(2.0));
    h.input_mut()
        .events
        .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
    h.run_steps(2);
    b
}
fn endpoint(h: &Harness<App>) -> (i64, i64) {
    match h.state().editor().preview().unwrap() {
        Preview::Marquee { to, .. } | Preview::Shape { to, .. } => to,
        Preview::Path(stroke) => {
            let point = stroke.points.last().unwrap();
            (
                point.0.div_euclid(i64::from(stroke.point_scale)),
                point.1.div_euclid(i64::from(stroke.point_scale)),
            )
        }
        preview => panic!("Unexpected preview {preview:?}"),
    }
}

#[test]
fn stationary_edge_drags_scroll_and_rebase_selection_and_shapes_once_per_frame() {
    for (ppp, multiple) in [(1.0, false), (2.0, true)] {
        for tool in [Tool::Select, Tool::Line, Tool::Pencil] {
            let (mut h, dir) = setup(tool, true, ppp, multiple);
            let original = h.state().editor().document_snapshot();
            let before_publications = publications(&h);
            let b = start(&mut h, Vec2::new(1.0, 0.0));
            let before = h.state().canvas_rect().unwrap();
            let previous = endpoint(&h);
            h.run_steps(4);
            let after = h.state().canvas_rect().unwrap();
            assert!(after.min.x < before.min.x - 10.0, "{before:?} -> {after:?}");
            assert!(
                endpoint(&h).0 > previous.0,
                "stationary pointer must follow document scroll"
            );
            assert_eq!(publications(&h), before_publications);
            assert!(h.state().editor().document().same_semantics(&original));
            h.event(release(b));
            h.run();
            assert_eq!(publications(&h), before_publications + 1);
            let final_canvas = h.state().canvas_rect().unwrap();
            let scale = final_canvas.width() / 320.0;
            let expected = (
                ((b.x - final_canvas.min.x) / scale).floor() as i64,
                ((b.y - final_canvas.min.y) / scale).floor() as i64,
            );
            if tool == Tool::Select {
                let paintcraft_editor::execute::Selection::Rect(rect) =
                    h.state().editor().selection()
                else {
                    panic!("missing selection")
                };
                assert_eq!((rect.x1 - 1, rect.y1 - 1), expected);
            }
            let stopped = h.state().canvas_rect().unwrap();
            h.run_steps(4);
            assert_eq!(h.state().canvas_rect().unwrap(), stopped);
            h.get_by_label("Undo").click();
            h.run();
            assert!(h.state().editor().document().same_semantics(&original));
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn disabled_hover_cancel_focus_loss_and_view_changes_do_not_keep_scrolling() {
    for enabled in [false, true] {
        let (mut h, dir) = setup(Tool::Select, enabled, 1.0, false);
        let viewport = h.state().canvas_viewport().unwrap();
        let b = Pos2::new(viewport.right() - 2.0, viewport.center().y);
        let centered = h.state().canvas_rect().unwrap();
        h.event(Event::PointerMoved(b));
        h.run_steps(4);
        assert_eq!(
            h.state().canvas_rect().unwrap(),
            centered,
            "hover is not a drag"
        );
        start(&mut h, Vec2::new(1.0, 0.0));
        if !enabled {
            h.run_steps(4);
            assert_eq!(h.state().canvas_rect().unwrap(), centered);
        }
        let published = publications(&h);
        h.event(key(Key::Escape, Modifiers::NONE));
        h.event(key_up(Key::Escape, Modifiers::NONE));
        h.run_steps(2);
        let stopped = h.state().canvas_rect().unwrap();
        h.run_steps(4);
        assert_eq!(h.state().canvas_rect().unwrap(), stopped);
        assert_eq!(publications(&h), published);
        h.event(release(b));
        h.run();
        start(&mut h, Vec2::new(-1.0, 0.0));
        h.event(Event::WindowFocused(false));
        h.run_steps(2);
        let stopped = h.state().canvas_rect().unwrap();
        h.run_steps(4);
        assert_eq!(h.state().canvas_rect().unwrap(), stopped);
        assert!(!h.state().editor().pointer_is_down());
        h.event(Event::WindowFocused(true));
        h.event(release(b));
        h.run();
        start(&mut h, Vec2::new(0.0, 1.0));
        h.event(key(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
        h.event(key_up(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
        h.run_steps(3);
        let zoomed = h.state().canvas_rect().unwrap();
        h.run_steps(4);
        assert_eq!(
            h.state().canvas_rect().unwrap(),
            zoomed,
            "a changed view invalidates the drag scroll source"
        );
        h.event(Event::PointerGone);
        h.run_steps(2);
        let gone = h.state().canvas_rect().unwrap();
        h.run_steps(4);
        assert_eq!(h.state().canvas_rect().unwrap(), gone);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn auto_scroll_setting_is_staged_and_persisted_in_every_language() {
    for language in Language::ALL {
        let dir = scratch("studio-auto-scroll-settings");
        Settings {
            experience: Experience::Studio,
            language,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1480.0, 960.0));
        h.run();
        let label = |s| translate(language, s);
        for apply in [false, true] {
            h.get_by_role_and_label(Role::Button, &label("Settings…"))
                .click();
            h.run();
            h.get_by_role_and_label(Role::Button, &label("User Interface"))
                .click();
            h.run();
            h.get_by_role_and_label(Role::CheckBox, &label("Auto-scroll"))
                .click();
            h.run();
            assert!(Settings::load(&dir).0.studio_auto_scroll);
            h.get_by_role_and_label(Role::Button, &label(if apply { "Apply" } else { "Cancel" }))
                .click();
            h.run();
            assert_eq!(Settings::load(&dir).0.studio_auto_scroll, !apply);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn edge_scroll_reaches_small_image_limits_without_publishing_or_driving_past_them() {
    let (mut h, dir) = setup(Tool::Select, true, 1.0, false);
    for _ in 0..4 {
        h.event(key(Key::Minus, Modifiers::COMMAND));
        h.event(key_up(Key::Minus, Modifiers::COMMAND));
        h.run();
    }
    let source = h.state().editor().document_snapshot();
    let before = publications(&h);
    start(&mut h, Vec2::new(1.0, 1.0));
    h.run_steps(80);
    let image = h.state().canvas_rect().unwrap();
    let viewport = h.state().canvas_viewport().unwrap();
    assert!(
        (image.min - (viewport.min - image.size() * 0.5)).length() < 1.0,
        "image {image:?}, viewport {viewport:?}"
    );
    h.run_steps(8);
    assert_eq!(h.state().canvas_rect().unwrap(), image);
    assert_eq!(publications(&h), before);
    assert!(h.state().editor().document().same_semantics(&source));
    h.event(key(Key::Escape, Modifiers::NONE));
    h.event(key_up(Key::Escape, Modifiers::NONE));
    h.run_steps(2);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn coalesced_cancel_and_new_press_arms_the_new_drag_and_foreign_buttons_do_not_rearm() {
    let (mut h, dir) = setup(Tool::Select, true, 1.0, false);
    let b = start(&mut h, Vec2::new(1.0, 0.0));
    let a = h.state().canvas_viewport().unwrap().center();
    h.input_mut().events.extend([
        key(Key::Escape, Modifiers::NONE),
        key_up(Key::Escape, Modifiers::NONE),
        release(b),
        Event::PointerMoved(a),
        press(a),
        Event::PointerMoved(b),
    ]);
    h.run_steps(2);
    let fresh = h.state().canvas_rect().unwrap();
    h.run_steps(4);
    assert!(h.state().canvas_rect().unwrap().min.x < fresh.min.x - 10.0);
    // A view change cancels auto-scroll. A foreign press must not arm it again
    // while the original accepted tool button is still held.
    h.event(key(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
    h.event(key_up(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run_steps(3);
    let changed = h.state().canvas_rect().unwrap();
    h.event(Event::PointerButton {
        pos: b,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.run_steps(4);
    assert_eq!(h.state().canvas_rect().unwrap(), changed);
    assert_eq!(publications(&h), 0);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
