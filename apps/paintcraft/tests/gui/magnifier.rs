use super::*;
use eframe::egui::{CursorIcon, Role};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_tools::Tool;
use paintcraft_ui::i18n::{Language, translate};

fn click_canvas(h: &mut Harness<App>, button: PointerButton) {
    let p = h.state().canvas_rect().unwrap().center();
    click_at(h, button, p);
}

fn click_at(h: &mut Harness<App>, button: PointerButton, p: Pos2) {
    h.event(Event::PointerMoved(p));
    h.event(Event::PointerButton {
        pos: p,
        button,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.event(Event::PointerButton {
        pos: p,
        button,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run();
}

#[test]
fn magnifier_clicks_step_zoom_without_pixels_or_history_in_every_skin() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10, ClassicVariant::V11] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("magnifier-view");
                Settings {
                    classic_variant: Some(variant),
                    language,
                    appearance,
                    ..Default::default()
                }
                .save(&dir)
                .unwrap();
                let mut h =
                    harness_with(App::new(None).with_settings(dir.clone()), ppp, ppp == 2.0);
                h.set_size(Vec2::new(1360.0, 900.0));
                h.run();
                let source = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                let width = h.state().canvas_rect().unwrap().width();
                h.get_by_role_and_label(Role::Button, &translate(language, "Magnifier"))
                    .click();
                h.run();
                assert_eq!(h.state().editor().tool, Tool::Magnifier);
                let original = h.state().canvas_rect().unwrap();
                let target = original.min + original.size() * Vec2::new(0.4, 0.42);
                click_at(&mut h, PointerButton::Primary, target);
                if ppp == 1.0 {
                    let mapped =
                        h.state().canvas_rect().unwrap().min + (target - original.min) * 2.0;
                    assert!(
                        (mapped - original.center()).length() < 3.0,
                        "clicked point should be centered: {mapped:?}, viewport center {:?}",
                        original.center()
                    );
                }
                assert!((h.state().canvas_rect().unwrap().width() - width * 2.0).abs() < 0.1);
                assert_eq!(h.output().platform_output.cursor_icon, CursorIcon::ZoomIn);
                click_canvas(&mut h, PointerButton::Secondary);
                assert!((h.state().canvas_rect().unwrap().width() - width).abs() < 0.1);
                assert!(h.state().editor().document().same_semantics(&source));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                assert!(!h.state().editor().gesture_in_progress());
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
fn magnifier_preserves_editable_text_until_returning_to_text_tool() {
    let mut h = harness();
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    drag(&mut h, (40.0, 50.0), (40.0, 50.0));
    h.event(Event::Text("Still editable".into()));
    h.run();
    let ticket = h.state().editor().history_view().ticket;
    h.get_by_role_and_label(Role::Button, "Magnifier").click();
    h.run();
    click_canvas(&mut h, PointerButton::Primary);
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(
        matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Still editable")
    );
}

#[test]
#[ignore = "Explicit offscreen Magnifier captures"]
fn export_magnifier() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10, ClassicVariant::V11] {
        for language in Language::ALL {
            let dir = scratch("magnifier-capture");
            Settings {
                classic_variant: Some(variant),
                language,
                appearance: Appearance::Light,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(Vec2::new(1100.0, 720.0));
            h.run();
            drag(&mut h, (460.0, 230.0), (540.0, 270.0));
            h.get_by_role_and_label(Role::Button, &translate(language, "Magnifier"))
                .click();
            h.run();
            click_canvas(&mut h, PointerButton::Primary);
            h.event(Event::PointerMoved(
                h.state().canvas_rect().unwrap().center(),
            ));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(output.join(format!(
                    "classic-{}-magnifier-{}.png",
                    variant.id(),
                    language.id()
                )))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
