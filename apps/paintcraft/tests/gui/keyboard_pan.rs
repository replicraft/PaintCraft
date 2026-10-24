use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, Experience, Settings};
use paintcraft_tools::Tool;
use paintcraft_ui::i18n::{Language, translate};

fn pair(keycode: Key, modifiers: Modifiers) -> [Event; 2] {
    let down = key(keycode, modifiers);
    let mut up = down.clone();
    if let Event::Key { pressed, .. } = &mut up {
        *pressed = false;
    }
    [down, up]
}
fn chord(h: &mut Harness<App>, arrow: Key, ctrl: bool) {
    let modifiers = Modifiers {
        ctrl,
        ..Modifiers::NONE
    };
    let [space_down, space_up] = pair(Key::Space, modifiers);
    h.input_mut().events.extend([space_down]);
    h.input_mut().events.extend(pair(arrow, modifiers));
    h.input_mut().events.push(space_up);
    h.run();
}
fn zoom_steps(h: &mut Harness<App>, keycode: Key, count: usize) {
    let modifiers = if keycode == Key::Plus {
        Modifiers::COMMAND | Modifiers::SHIFT
    } else {
        Modifiers::COMMAND
    };
    for _ in 0..count {
        h.input_mut().events.extend(pair(keycode, modifiers));
        h.run();
    }
}

#[test]
fn studio_keyboard_pan_has_exact_screen_steps_at_each_dpi_and_inverse_zoom_motion() {
    for language in Language::ALL {
        for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
            let dir = scratch("studio-keyboard-pan");
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
            h.get_by_role_and_label(Role::Button, &translate(language, "Pencil"))
                .click();
            h.run();
            let source = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            for ctrl in [false, true] {
                for (arrow, unit) in [
                    (Key::ArrowRight, Vec2::new(-1.0, 0.0)),
                    (Key::ArrowDown, Vec2::new(0.0, -1.0)),
                    (Key::ArrowLeft, Vec2::new(1.0, 0.0)),
                    (Key::ArrowUp, Vec2::new(0.0, 1.0)),
                ] {
                    let before = h.state().canvas_rect().unwrap();
                    chord(&mut h, arrow, ctrl);
                    let actual = h.state().canvas_rect().unwrap().min - before.min;
                    let expected = unit * if ctrl { 10.0 / ppp } else { 1.0 / ppp };
                    assert!(
                        (actual - expected).length() < 0.05,
                        "{arrow:?} ctrl={ctrl} ppp={ppp}: {actual:?} vs {expected:?}"
                    );
                }
            }
            for (keycode, count, zoom) in [(Key::Minus, 3, 0.125), (Key::Plus, 7, 16.0)] {
                zoom_steps(&mut h, keycode, count);
                let before = h.state().canvas_rect().unwrap();
                assert!((before.width() * ppp / source.canvas.0 as f32 - zoom).abs() < 0.01);
                chord(&mut h, Key::ArrowRight, false);
                let after = h.state().canvas_rect().unwrap();
                let document_motion =
                    (after.min.x - before.min.x) / (before.width() / source.canvas.0 as f32);
                assert!(
                    (document_motion + 1.0 / zoom).abs() < 0.001,
                    "{document_motion} at {zoom}"
                );
            }
            assert_eq!(h.state().editor().tool, Tool::Pencil);
            assert!(h.state().editor().document().same_semantics(&source));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert!(!h.state().editor().gesture_in_progress());
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn keyboard_pan_counts_repeats_in_order_and_stops_at_space_release_or_focus_loss() {
    let (mut h, dir) = super::studio::studio_harness(true);
    h.set_size(Vec2::new(1480.0, 960.0));
    h.run();
    let before = h.state().canvas_rect().unwrap();
    let [space_down, space_up] = pair(Key::Space, Modifiers::NONE);
    let [right_down, right_up] = pair(Key::ArrowRight, Modifiers::NONE);
    let mut repeat = right_down.clone();
    if let Event::Key { repeat, .. } = &mut repeat {
        *repeat = true;
    }
    h.input_mut().events.extend([
        space_down,
        right_down,
        repeat.clone(),
        repeat,
        right_up,
        space_up,
    ]);
    h.run();
    assert!(
        (h.state().canvas_rect().unwrap().min - before.min - Vec2::new(-3.0, 0.0)).length() < 0.05
    );
    let stationary = h.state().canvas_rect().unwrap();
    h.input_mut()
        .events
        .extend(pair(Key::ArrowRight, Modifiers::NONE));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap(), stationary);
    let [space_down, space_up] = pair(Key::Space, Modifiers::NONE);
    h.input_mut().events.extend([space_down, space_up]);
    h.input_mut()
        .events
        .extend(pair(Key::ArrowRight, Modifiers::NONE));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap(), stationary);
    let [space_down, _] = pair(Key::Space, Modifiers::NONE);
    h.input_mut().events.push(space_down);
    h.run();
    h.event(Event::WindowFocused(false));
    h.run();
    h.event(Event::WindowFocused(true));
    h.run();
    h.input_mut()
        .events
        .extend(pair(Key::ArrowRight, Modifiers::NONE));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap(), stationary);
    h.event(Event::Ime(eframe::egui::ImeEvent::Preedit {
        text: "あ".into(),
        active_range_chars: None,
    }));
    h.run();
    chord(&mut h, Key::ArrowRight, true);
    assert_eq!(h.state().canvas_rect().unwrap(), stationary);
    h.event(Event::Ime(eframe::egui::ImeEvent::Commit(String::new())));
    h.run();
    assert_eq!(h.state().editor().undo_depth(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn keyboard_pan_leaves_text_fields_canvas_text_and_popups_in_control() {
    let (mut h, dir) = super::studio::studio_harness(true);
    h.set_size(Vec2::new(1480.0, 960.0));
    h.run();
    let before = h.state().canvas_rect().unwrap();
    h.get_by_label("Layer name").focus();
    h.run();
    chord(&mut h, Key::ArrowLeft, true);
    assert_eq!(h.state().canvas_rect().unwrap(), before);
    h.key_press(Key::Escape);
    h.run();
    h.get_by_role_and_label(Role::Button, "View").click();
    h.run();
    chord(&mut h, Key::ArrowRight, false);
    assert_eq!(h.state().canvas_rect().unwrap(), before);
    h.key_press(Key::Escape);
    h.run();
    h.get_by_role_and_label(Role::Button, "Settings…").click();
    h.run();
    chord(&mut h, Key::ArrowRight, true);
    assert_eq!(h.state().canvas_rect().unwrap(), before);
    h.get_by_role_and_label(Role::Button, "Cancel").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    let p = h.state().canvas_rect().unwrap().center();
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), press(p), release(p)]);
    h.run();
    h.event(Event::Text("Keep editable".into()));
    h.run();
    let ticket = h.state().editor().history_view().ticket;
    let text_view = h.state().canvas_rect().unwrap();
    chord(&mut h, Key::ArrowRight, true);
    assert_eq!(h.state().canvas_rect().unwrap(), text_view);
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(
        matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Keep editable")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
