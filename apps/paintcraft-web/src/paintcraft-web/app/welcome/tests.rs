use super::*;
use egui::{Event, Key, Modifiers, PointerButton, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, translate};

#[test]
fn storage_presence_and_failed_storage_have_explicit_dismissal_semantics() {
    for stored in [
        Some(String::new()),
        Some("0".into()),
        Some("false".into()),
        Some("true".into()),
        Some("1".into()),
    ] {
        assert!(!State::from_lookup(Ok(stored)).open());
    }
    for lookup in [Ok(None), Err(())] {
        let mut welcome = State::from_lookup(lookup);
        assert!(welcome.open());
        let mut writes = Vec::new();
        welcome.dismiss_with(|key, value| {
            writes.push((key.to_owned(), value.to_owned()));
            Err(())
        });
        assert!(!welcome.open());
        assert_eq!(writes, [(DISMISSED_KEY.to_owned(), "false".to_owned())]);
    }
}
fn harness(language: Language, size: Vec2, dark: bool) -> Harness<'static, WebApp> {
    let mut h = Harness::builder().with_size(size).build_eframe(move |cc| {
        let mut app = WebApp::new(&cc.egui_ctx);
        app.preferences.language = language;
        app.preferences.theme = Some(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        set_language(&cc.egui_ctx, language);
        paintcraft_ui::fonts::install_for_locale(&cc.egui_ctx, language.id());
        app.welcome = State::from_lookup(Ok(None));
        app
    });
    h.run();
    h
}
#[test]
fn every_locale_theme_and_mobile_size_has_reachable_close_controls_without_editing() {
    for language in Language::ALL {
        for (size, dark) in [
            (Vec2::new(1360., 900.), false),
            (Vec2::new(320., 568.), true),
            (Vec2::new(390., 300.), false),
        ] {
            for top in [false, true] {
                let mut h = harness(language, size, dark);
                let before = h.state().editor.session.ticket();
                h.get_by_label(&translate(language, "Welcome to PaintCraft"));
                h.get_by_label(&translate(
                    language,
                    "Click Interface in the top right to try the different interfaces",
                ));
                let label = translate(language, if top { "Close welcome dialog" } else { "Close" });
                let rect = h.get_by_role_and_label(Role::Button, &label).rect();
                assert!(
                    rect.left() >= 0.
                        && rect.right() <= size.x
                        && rect.top() >= 0.
                        && rect.bottom() <= size.y,
                    "{language:?}: {rect:?} {size:?}"
                );
                h.get_by_role_and_label(Role::Button, &label).click();
                h.run();
                assert!(!h.state().welcome.open());
                assert_eq!(h.state().editor.session.ticket(), before);
                assert!(!h.state().editor.session.is_dirty());
            }
        }
    }
}
#[test]
fn welcome_owns_pointer_edit_actions_and_shortcuts_and_escape_only_dismisses_it() {
    let mut h = harness(Language::English, Vec2::new(1360., 900.), false);
    let before = h.state().editor.session.ticket();
    let ctx = h.ctx.clone();
    h.state_mut().action(&ctx, Action::Settings);
    h.state_mut().action(&ctx, Action::New);
    h.state_mut().edit(Edit::ClearCanvas { color: WHITE });
    for key in [Key::N, Key::S, Key::O, Key::Z] {
        h.key_combination_modifiers(Modifiers::COMMAND, &[key]);
        h.run();
    }
    let pos = h.state().canvas.rect.unwrap().min + Vec2::splat(12.);
    for pressed in [true, false] {
        h.event(Event::PointerMoved(pos));
        h.event(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.run();
    }
    assert!(h.state().welcome.open());
    assert!(!h.state().settings);
    assert!(!h.state().files.busy);
    assert_eq!(h.state().editor.session.ticket(), before);
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().welcome.open());
    assert_eq!(h.state().editor.session.ticket(), before);
}
