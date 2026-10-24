use super::*;
use eframe::egui::{Role, accesskit::Toggled};
use paintcraft::settings::{Appearance, Experience, Settings};
use paintcraft_ui::i18n::{Language, translate};

fn pan(h: &mut Harness<App>, delta: Vec2) {
    let a = h.state().canvas_viewport().unwrap().center();
    let b = a + delta;
    h.input_mut().events.extend([
        Event::PointerMoved(a),
        press(a),
        Event::PointerMoved(b),
        release(b),
    ]);
    h.run();
}
fn zoom_large(h: &mut Harness<App>) {
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
fn settings(h: &mut Harness<App>, language: Language) {
    h.get_by_role_and_label(Role::Button, &translate(language, "Settings…"))
        .click();
    h.run();
    h.get_by_role_and_label(Role::Button, &translate(language, "User Interface"))
        .click();
    h.run();
}

#[test]
fn studio_overscroll_limits_small_and_large_images_without_document_changes() {
    for language in Language::ALL {
        for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
            let dir = scratch("studio-overscroll-limits");
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
            h.get_by_role_and_label(Role::Button, &translate(language, "Pan tool"))
                .click();
            h.run();
            let source = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            let centered = h.state().canvas_rect().unwrap();
            assert!(
                (centered.center() - h.state().canvas_viewport().unwrap().center()).length() < 1.0
            );
            for large in [false, true] {
                if large {
                    zoom_large(&mut h);
                }
                for positive in [true, false] {
                    pan(
                        &mut h,
                        Vec2::splat(if positive { 100_000.0 } else { -100_000.0 }),
                    );
                    let canvas = h.state().canvas_rect().unwrap();
                    let viewport = h.state().canvas_viewport().unwrap();
                    let actual = if positive { canvas.min } else { canvas.max };
                    let expected = if large {
                        viewport.center()
                    } else if positive {
                        viewport.max - canvas.size() * 0.5
                    } else {
                        viewport.min + canvas.size() * 0.5
                    };
                    assert!(
                        (actual - expected).length() < 2.0,
                        "large={large} positive={positive}: {actual:?} vs {expected:?}"
                    );
                    assert!(h.state().editor().document().same_semantics(&source));
                    assert_eq!(h.state().editor().history_view().ticket, ticket);
                }
            }
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn studio_overscroll_preferences_stage_cancel_apply_and_restart() {
    for language in Language::ALL {
        let dir = scratch("studio-overscroll-preferences");
        Settings {
            experience: Experience::Studio,
            language,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
        h.set_size(Vec2::new(1480.0, 960.0));
        h.run();
        let label = |s| translate(language, s);
        h.get_by_role_and_label(Role::Button, &label("Pan tool"))
            .click();
        h.run();
        let source = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        pan(&mut h, Vec2::new(100.0, 60.0));
        let before = h.state().canvas_rect().unwrap();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        settings(&mut h, language);
        let preference = label("Scroll past the edge of the image (overscroll)");
        assert_eq!(
            h.get_by_role_and_label(Role::CheckBox, &preference)
                .accesskit_node()
                .toggled(),
            Some(Toggled::True)
        );
        h.get_by_role_and_label(Role::CheckBox, &preference).click();
        h.run();
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        h.get_by_role_and_label(Role::Button, &label("Cancel"))
            .click();
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap(), before);
        settings(&mut h, language);
        h.get_by_role_and_label(Role::CheckBox, &preference).click();
        h.run();
        h.get_by_role_and_label(Role::Button, &label("Apply"))
            .click();
        h.run();
        assert!(!Settings::load(&dir).0.studio_overscroll);
        let constrained = h.state().canvas_rect().unwrap();
        assert!(
            (constrained.center() - h.state().canvas_viewport().unwrap().center()).length() < 1.0
        );
        pan(&mut h, Vec2::new(70.0, 40.0));
        assert_eq!(h.state().canvas_rect().unwrap(), constrained);
        let mut restarted = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        restarted.set_size(Vec2::new(1480.0, 960.0));
        restarted.run();
        restarted
            .get_by_role_and_label(Role::Button, &label("Pan tool"))
            .click();
        restarted.run();
        let original = restarted.state().canvas_rect().unwrap();
        pan(&mut restarted, Vec2::new(80.0, 40.0));
        assert_eq!(restarted.state().canvas_rect().unwrap(), original);
        settings(&mut h, language);
        h.get_by_role_and_label(Role::CheckBox, &preference).click();
        h.run();
        h.get_by_role_and_label(Role::Button, &label("Apply"))
            .click();
        h.run();
        assert!(Settings::load(&dir).0.studio_overscroll);
        let enabled = h.state().canvas_rect().unwrap();
        assert!((enabled.center() - constrained.center()).length() < 1.0);
        let delta = Vec2::new(50.0, -30.0);
        pan(&mut h, delta);
        assert!((h.state().canvas_rect().unwrap().min - enabled.min - delta).length() < 1.0);
        h.set_size(Vec2::new(1280.0, 800.0));
        h.run();
        let resized = h.state().canvas_rect().unwrap();
        assert!(
            (resized.center() - h.state().canvas_viewport().unwrap().center() - delta).length()
                < 1.0
        );
        assert!(h.state().editor().document().same_semantics(&source));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        drop(restarted);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn studio_navigation_settings_stay_reachable_in_a_small_window() {
    for language in Language::ALL {
        let dir = scratch("studio-overscroll-small-settings");
        Settings {
            experience: Experience::Studio,
            language,
            appearance: Appearance::Dark,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 2.0, true);
        h.set_size(Vec2::new(1100.0, 800.0));
        h.run();
        settings(&mut h, language);
        h.set_size(Vec2::new(360.0, 360.0));
        h.run();
        let auto = translate(language, "Auto-scroll");
        h.get_by_role_and_label(Role::CheckBox, &auto)
            .scroll_to_me();
        h.run();
        assert!(
            h.ctx
                .content_rect()
                .contains_rect(h.get_by_role_and_label(Role::CheckBox, &auto).rect())
        );
        let label = translate(language, "Scroll past the edge of the image (overscroll)");
        h.get_by_role_and_label(Role::CheckBox, &label)
            .scroll_to_me();
        h.run();
        let bounds = h.get_by_role_and_label(Role::CheckBox, &label).rect();
        assert!(h.ctx.content_rect().contains_rect(bounds), "{bounds:?}");
        for label in ["Apply", "Cancel"] {
            assert!(
                h.ctx.content_rect().contains_rect(
                    h.get_by_role_and_label(Role::Button, &translate(language, label))
                        .rect()
                )
            );
        }
        h.get_by_role_and_label(Role::CheckBox, &label).click();
        h.run();
        h.get_by_role_and_label(Role::Button, &translate(language, "Cancel"))
            .click();
        h.run();
        assert!(Settings::load(&dir).0.studio_overscroll);
        assert_eq!(h.state().editor().undo_depth(), 0);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit offscreen Studio overscroll and preferences captures"]
fn export_studio_overscroll() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for language in Language::ALL {
        let dir = scratch("studio-overscroll-capture");
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
        h.get_by_role_and_label(Role::Button, &translate(language, "Pan tool"))
            .click();
        h.run();
        let canvas = h.state().canvas_rect().unwrap();
        let viewport = h.state().canvas_viewport().unwrap();
        let target = viewport.left_center() + Vec2::new(canvas.width() * 0.25, 0.0);
        pan(&mut h, target - canvas.center());
        h.event(Event::PointerMoved(viewport.center()));
        h.run_steps(4);
        let shifted = h.state().canvas_rect().unwrap();
        assert!(shifted.left() < h.state().canvas_viewport().unwrap().left());
        h.render()
            .unwrap()
            .save(output.join(format!("studio-overscroll-{}.png", language.id())))
            .unwrap();
        settings(&mut h, language);
        h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(output.join(format!("studio-navigation-settings-{}.png", language.id())))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
