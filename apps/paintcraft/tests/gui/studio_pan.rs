use super::*;
use eframe::egui::{CursorIcon, Role};
use paintcraft::settings::{Appearance, Experience, Settings};
use paintcraft_tools::Tool;
use paintcraft_ui::i18n::{Language, translate};

fn button(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}
fn space(pressed: bool) -> Event {
    Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}
fn magnify(h: &mut Harness<App>) {
    for _ in 0..4 {
        let down = key(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT);
        let mut up = down.clone();
        if let Event::Key { pressed, .. } = &mut up {
            *pressed = false;
        }
        h.event(down);
        h.event(up);
        h.run();
    }
}

#[test]
fn studio_pan_tool_space_and_middle_drag_move_the_view_without_edits() {
    for language in Language::ALL {
        for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
            let dir = scratch("studio-pan-view");
            Settings {
                experience: Experience::Studio,
                language,
                appearance,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, ppp == 2.0);
            h.set_size(Vec2::new(1480.0, 960.0));
            h.run();
            magnify(&mut h);
            let source = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            let selection = h.state().editor().selection();
            for mode in ["tool", "space", "middle"] {
                let label = translate(
                    language,
                    match mode {
                        "tool" => "Pan tool",
                        "space" => "Pencil",
                        _ => "Zoom tool",
                    },
                );
                h.get_by_role_and_label(Role::Button, &label).click();
                h.run();
                let original = h.state().canvas_rect().unwrap();
                let a = h.state().canvas_viewport().unwrap().center();
                let delta = Vec2::new(45.0, -30.0);
                let b = a + delta;
                let mut events = Vec::new();
                if mode == "space" {
                    events.push(space(true));
                }
                let mouse = if mode == "middle" {
                    PointerButton::Middle
                } else {
                    PointerButton::Primary
                };
                events.extend([
                    Event::PointerMoved(a),
                    button(a, mouse, true),
                    Event::PointerMoved(b),
                    button(b, mouse, false),
                ]);
                if mode == "space" {
                    events.push(space(false));
                }
                h.input_mut().events.extend(events);
                h.run();
                let changed = h.state().canvas_rect().unwrap();
                assert!(
                    (changed.min - original.min - delta).length() < 1.0,
                    "{mode}: {:?} expected {delta:?}",
                    changed.min - original.min
                );
                assert_eq!(changed.size(), original.size());
                assert!(h.state().editor().document().same_semantics(&source));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                assert_eq!(h.state().editor().selection(), selection);
                assert!(!h.state().editor().gesture_in_progress());
                assert_eq!(
                    h.output().platform_output.cursor_icon,
                    match mode {
                        "tool" => CursorIcon::Grab,
                        "space" => CursorIcon::Crosshair,
                        _ => CursorIcon::ZoomIn,
                    }
                );
                assert_eq!(
                    h.state().editor().tool,
                    match mode {
                        "tool" => Tool::Pan,
                        "space" => Tool::Pencil,
                        _ => Tool::Magnifier,
                    }
                );
            }
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn studio_pan_escape_focus_loss_tool_and_document_changes_end_capture() {
    let (mut h, dir) = super::studio::studio_harness(true);
    h.set_size(Vec2::new(1480.0, 960.0));
    h.run();
    magnify(&mut h);
    for cancel in ["escape", "focus", "tool", "document"] {
        h.key_press(Key::H);
        h.run();
        assert_eq!(h.state().editor().tool, Tool::Pan);
        let a = h.state().canvas_viewport().unwrap().center();
        let b = a + Vec2::new(15.0, -10.0);
        h.input_mut().events.extend([
            Event::PointerMoved(a),
            button(a, PointerButton::Primary, true),
            Event::PointerMoved(b),
        ]);
        h.run_steps(3);
        assert_eq!(h.output().platform_output.cursor_icon, CursorIcon::Grabbing);
        let before = h.state().canvas_rect().unwrap();
        match cancel {
            "escape" => h.key_press(Key::Escape),
            "focus" => h.event(Event::WindowFocused(false)),
            "tool" => h.key_press(Key::Z),
            _ => h.event(key(Key::N, Modifiers::COMMAND | Modifiers::SHIFT)),
        }
        h.run_steps(8);
        h.event(Event::WindowFocused(true));
        h.input_mut().events.extend([
            Event::PointerMoved(b + Vec2::splat(30.0)),
            button(b, PointerButton::Primary, false),
        ]);
        h.run();
        assert!(
            (h.state().canvas_rect().unwrap().min - before.min).length() < 1.0,
            "{cancel}"
        );
        assert_eq!(
            h.state().editor().undo_depth(),
            usize::from(cancel == "document")
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn studio_pan_owns_foreign_buttons_and_preserves_text_drafts_and_typing() {
    let (mut h, dir) = super::studio::studio_harness(true);
    h.set_size(Vec2::new(1480.0, 960.0));
    h.run();
    magnify(&mut h);
    h.get_by_role_and_label(Role::Button, "Pencil").click();
    h.run();
    let a = h.state().canvas_viewport().unwrap().center();
    let b = a + Vec2::new(40.0, -20.0);
    h.input_mut().events.extend([
        Event::PointerMoved(a),
        button(a, PointerButton::Middle, true),
        button(a, PointerButton::Secondary, true),
        button(a, PointerButton::Secondary, false),
        Event::PointerMoved(b),
        button(b, PointerButton::Middle, false),
    ]);
    h.run();
    assert_eq!(
        h.state().editor().undo_depth(),
        0,
        "foreign button must not paint through a pan"
    );
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    h.input_mut()
        .events
        .extend([Event::PointerMoved(a), press(a), release(a)]);
    h.run();
    h.key_press(Key::H);
    h.event(Event::Text("h".into()));
    h.run();
    h.key_press(Key::Space);
    h.event(Event::Text(" ".into()));
    h.run();
    assert_eq!(h.state().editor().tool, Tool::Text);
    let ticket = h.state().editor().history_view().ticket;
    h.get_by_role_and_label(Role::Button, "Pan tool").click();
    h.run();
    h.input_mut().events.extend([
        Event::PointerMoved(a),
        press(a),
        Event::PointerMoved(b),
        release(b),
    ]);
    h.run();
    h.event(Event::WindowFocused(false));
    h.run();
    h.event(Event::WindowFocused(true));
    h.run();
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(
        matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "h ")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "Explicit offscreen Studio Pan captures"]
fn export_studio_pan() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for language in Language::ALL {
        let dir = scratch("studio-pan-capture");
        Settings {
            experience: Experience::Studio,
            language,
            appearance: if language == Language::Japanese {
                Appearance::Dark
            } else {
                Appearance::Light
            },
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1480.0, 960.0));
        h.run();
        h.get_by_role_and_label(Role::Button, &translate(language, "Pencil"))
            .click();
        h.run();
        drag(&mut h, (120.0, 150.0), (200.0, 170.0));
        magnify(&mut h);
        assert!(h.state().canvas_rect().unwrap().width() > 2000.0);
        h.get_by_role_and_label(Role::Button, &translate(language, "Pan tool"))
            .click();
        h.run();
        assert!(h.state().canvas_rect().unwrap().width() > 2000.0);
        let a = h.state().canvas_viewport().unwrap().center();
        let b = a + Vec2::new(-80.0, 45.0);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(output.join(format!("studio-pan-{}.png", language.id())))
            .unwrap();
        h.event(release(b));
        h.run();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
