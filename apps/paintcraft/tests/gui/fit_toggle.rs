use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Experience, Settings};
use paintcraft_ui::i18n::{Language, translate};

fn shortcut(h: &mut Harness<App>, keycode: Key, modifiers: Modifiers) {
    h.event(key(keycode, modifiers));
    h.event(key_up(keycode, modifiers));
    h.run();
}
fn menu_fit(h: &mut Harness<App>, language: Language) {
    let view = translate(language, "View");
    h.get_by_role_and_label(Role::Button, &view).click();
    h.run();
    h.get_by_role_and_label(Role::Button, &view)
        .get_by_label_contains(&translate(language, "Zoom to window"))
        .click();
    h.run();
}
fn assert_centered(h: &Harness<App>) {
    let image = h.state().canvas_rect().unwrap();
    let viewport = h.state().canvas_viewport().unwrap();
    assert!(
        (image.center() - viewport.center()).length() < 1.0,
        "image {image:?}, viewport {viewport:?}"
    );
}

#[test]
fn studio_fit_shortcut_and_menu_toggle_previous_zoom_and_center_in_every_language() {
    for language in Language::ALL {
        for (ppp, multiple) in [(1.0, false), (2.0, true)] {
            let dir = scratch("studio-fit-toggle");
            Settings {
                experience: Experience::Studio,
                language,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, multiple);
            h.set_size(Vec2::new(1480.0, 960.0));
            h.run();
            for _ in 0..4 {
                shortcut(&mut h, Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT);
            }
            let previous_width = h.state().canvas_rect().unwrap().width();
            let source = paintcraft_formats::pcnd::write(h.state().editor().document()).unwrap();
            let ticket = h.state().editor().history_view().ticket;
            let viewport = h.state().canvas_viewport().unwrap();
            let a = viewport.center();
            let b = a + Vec2::new(100.0, 60.0);
            h.input_mut().events.extend([
                Event::PointerMoved(a),
                Event::PointerButton {
                    pos: a,
                    button: PointerButton::Middle,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerMoved(b),
                Event::PointerButton {
                    pos: b,
                    button: PointerButton::Middle,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ]);
            h.run();
            assert!(
                (h.state().canvas_rect().unwrap().center() - viewport.center()).length() > 40.0
            );
            shortcut(&mut h, Key::B, Modifiers::COMMAND);
            let fitted_width = h.state().canvas_rect().unwrap().width();
            assert!(fitted_width < previous_width);
            assert_centered(&h);
            menu_fit(&mut h, language);
            assert_eq!(h.state().canvas_rect().unwrap().width(), previous_width);
            assert_centered(&h);
            menu_fit(&mut h, language);
            assert!((h.state().canvas_rect().unwrap().width() - fitted_width).abs() < 0.1);
            shortcut(&mut h, Key::B, Modifiers::COMMAND);
            assert_eq!(h.state().canvas_rect().unwrap().width(), previous_width);
            assert_centered(&h);
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert_eq!(
                paintcraft_formats::pcnd::write(h.state().editor().document()).unwrap(),
                source
            );
            assert!(!h.state().editor().is_dirty());
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn manual_zoom_starts_a_new_fit_cycle_using_the_latest_zoom() {
    let (mut h, dir) = super::studio::studio_harness(false);
    shortcut(&mut h, Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT);
    shortcut(&mut h, Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT);
    shortcut(&mut h, Key::B, Modifiers::COMMAND);
    let fit = h.state().canvas_rect().unwrap().width();
    shortcut(&mut h, Key::Minus, Modifiers::COMMAND);
    let manual = h.state().canvas_rect().unwrap().width();
    assert_ne!(manual, fit);
    shortcut(&mut h, Key::B, Modifiers::COMMAND);
    assert!((h.state().canvas_rect().unwrap().width() - fit).abs() < 0.1);
    shortcut(&mut h, Key::B, Modifiers::COMMAND);
    assert_eq!(h.state().canvas_rect().unwrap().width(), manual);
    assert_centered(&h);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
