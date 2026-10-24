use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, Experience, Settings};
use paintcraft_tools::Tool;
use paintcraft_ui::i18n::{Language, translate};

fn begin(h: &mut Harness<App>, a: Pos2, b: Pos2) {
    h.input_mut()
        .events
        .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
    h.run_steps(2);
}

#[test]
fn studio_zoom_rectangle_fits_without_selection_or_history_and_clicks_still_step() {
    for language in Language::ALL {
        for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
            let dir = scratch("studio-zoom-rectangle");
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
            h.get_by_role_and_label(Role::Button, &translate(language, "Zoom tool"))
                .click();
            h.run();
            assert_eq!(h.state().editor().tool, Tool::Magnifier);
            assert_eq!(h.state().editor().tool, Tool::Magnifier);
            let source = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            let selection = h.state().editor().selection();
            let canvas = h.state().canvas_rect().unwrap();
            let a = canvas.min + Vec2::new(80.0, 80.0) / ppp;
            let b = canvas.min + Vec2::new(240.0, 180.0) / ppp;
            begin(&mut h, b, a);
            assert_eq!(
                h.state().canvas_rect().unwrap().size(),
                canvas.size(),
                "no zoom before release"
            );
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            let viewport = h.state().canvas_viewport().unwrap();
            let expected_zoom = (viewport.width() * ppp / 160.0)
                .min(viewport.height() * ppp / 100.0)
                .min(64.0);
            h.event(release(a));
            h.run();
            let changed = h.state().canvas_rect().unwrap();
            assert!(
                (changed.width() / canvas.width() - expected_zoom).abs() < 0.01,
                "actual {} expected {expected_zoom}, canvas {canvas:?} viewport {viewport:?}, from {b:?} to {a:?}",
                changed.width() / canvas.width()
            );
            let fit = eframe::egui::Rect::from_min_max(
                changed.min + Vec2::new(80.0, 80.0) * expected_zoom / ppp,
                changed.min + Vec2::new(240.0, 180.0) * expected_zoom / ppp,
            );
            let viewport = h.state().canvas_viewport().unwrap();
            assert!(
                viewport.expand(2.0).contains_rect(fit),
                "{fit:?} outside {viewport:?}"
            );
            assert!(h.state().editor().document().same_semantics(&source));
            assert_eq!(h.state().editor().selection(), selection);
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            let p = viewport.center();
            h.input_mut().events.extend([
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button: PointerButton::Secondary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: p,
                    button: PointerButton::Secondary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ]);
            h.run();
            assert!(h.state().canvas_rect().unwrap().width() < changed.width());
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn studio_zoom_escape_focus_loss_and_document_changes_cancel_the_box() {
    let (mut h, dir) = super::studio::studio_harness(true);
    h.set_size(Vec2::new(1480.0, 960.0));
    h.run();
    for cancel in ["escape", "focus", "document"] {
        h.get_by_role_and_label(Role::Button, "Zoom tool").click();
        h.run();
        let canvas = h.state().canvas_rect().unwrap();
        let a = canvas.min + Vec2::new(80.0, 80.0);
        let b = canvas.min + Vec2::new(240.0, 180.0);
        begin(&mut h, a, b);
        match cancel {
            "escape" => h.key_press(Key::Escape),
            "focus" => h.event(Event::WindowFocused(false)),
            _ => h.event(key(Key::N, Modifiers::COMMAND | Modifiers::SHIFT)),
        }
        h.run_steps(8);
        h.event(Event::WindowFocused(true));
        h.event(release(b));
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().size(), canvas.size());
        assert_eq!(
            h.state().editor().undo_depth(),
            usize::from(cancel == "document")
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn studio_zoom_shortcut_respects_text_and_cancel_keeps_the_draft() {
    let (mut h, dir) = super::studio::studio_harness(true);
    h.set_size(Vec2::new(1480.0, 960.0));
    h.run();
    h.key_press(Key::Z);
    h.run();
    assert_eq!(h.state().editor().tool, Tool::Magnifier);
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    let canvas = h.state().canvas_rect().unwrap();
    let p = canvas.center();
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), press(p), release(p)]);
    h.run();
    h.key_press(Key::Z);
    h.event(Event::Text("z".into()));
    h.run();
    assert_eq!(h.state().editor().tool, Tool::Text);
    let ticket = h.state().editor().history_view().ticket;
    h.get_by_role_and_label(Role::Button, "Zoom tool").click();
    h.run();
    let a = canvas.min + Vec2::new(80.0, 80.0);
    let b = canvas.min + Vec2::new(240.0, 180.0);
    begin(&mut h, a, b);
    h.key_press(Key::Escape);
    h.event(release(b));
    h.run();
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(
        matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "z")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "Explicit offscreen Studio rectangle zoom captures"]
fn export_studio_zoom() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for language in Language::ALL {
        let dir = scratch("studio-zoom-capture");
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
        drag(&mut h, (120.0, 100.0), (200.0, 140.0));
        h.get_by_role_and_label(Role::Button, &translate(language, "Zoom tool"))
            .click();
        h.run();
        let canvas = h.state().canvas_rect().unwrap();
        let a = canvas.min + Vec2::new(80.0, 80.0);
        let b = canvas.min + Vec2::new(240.0, 180.0);
        begin(&mut h, a, b);
        h.render()
            .unwrap()
            .save(output.join(format!("studio-zoom-box-{}.png", language.id())))
            .unwrap();
        h.event(release(b));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("studio-zoom-result-{}.png", language.id())))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
