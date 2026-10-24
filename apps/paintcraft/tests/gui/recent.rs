use super::*;
use eframe::egui::Role;
use paintcraft::settings::{ClassicVariant, Experience, Settings};
use paintcraft_ui::i18n::{Language, translate};

fn project(path: &std::path::Path, width: u32) {
    let doc = paintcraft_document::new_document(
        width,
        5,
        Some(Rgba([12, 34, 56, 255])),
        7,
        paintcraft_document::POLICY,
    )
    .unwrap();
    std::fs::write(path, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
}
fn history(h: &mut Harness<App>, language: Language, classic: Option<ClassicVariant>) {
    let text = |s| translate(language, s);
    h.get_by_role_and_label(Role::Button, &text("File")).click();
    h.run();
    let label = if classic == Some(ClassicVariant::V11) {
        text("Recent")
    } else {
        text("Recent files")
    };
    h.get_by_role_and_label(Role::Button, &label).click();
    h.run();
}

#[test]
fn recent_open_and_clear_work_in_every_language_skin_and_studio_without_replacing_other_tabs() {
    for language in Language::ALL {
        for classic in [
            Some(ClassicVariant::V7),
            Some(ClassicVariant::V10),
            Some(ClassicVariant::V11),
            None,
        ] {
            let dir = scratch("recent-ui");
            let dir = std::fs::canonicalize(dir).unwrap();
            let a = dir.join("first.pcnd");
            let b = dir.join("second.pcnd");
            project(&a, 7);
            project(&b, 11);
            let before_a = std::fs::read(&a).unwrap();
            let before_b = std::fs::read(&b).unwrap();
            Settings {
                experience: if classic.is_some() {
                    Experience::Classic
                } else {
                    Experience::Studio
                },
                classic_variant: classic,
                language,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(
                App::new(Some(a.clone())).with_settings(dir.clone()),
                if classic == Some(ClassicVariant::V10) {
                    2.0
                } else {
                    1.0
                },
                true,
            );
            h.state_mut().set_path_text(b.to_str().unwrap());
            h.event(key(Key::O, Modifiers::COMMAND));
            h.event(key_up(Key::O, Modifiers::COMMAND));
            h.run();
            assert_eq!(h.state().editor().document().canvas, (11, 5));
            let count = h.state().document_count();
            history(&mut h, language, classic);
            let row = format!("first.pcnd\n{}", dir.display());
            h.get_by_role_and_label(Role::Button, &row).click();
            h.run();
            assert_eq!(h.state().editor().document().canvas, (7, 5));
            assert_eq!(h.state().document_count(), count);
            let snapshot = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            history(&mut h, language, classic);
            h.get_by_role_and_label(Role::Button, &translate(language, "Clear recent history"))
                .click();
            h.run();
            assert!(
                h.query_by_label(&translate(language, "No recent files yet."))
                    .is_some()
            );
            h.get_by_role_and_label(Role::Button, &translate(language, "Cancel"))
                .click();
            h.run();
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert_eq!(std::fs::read(&a).unwrap(), before_a);
            assert_eq!(std::fs::read(&b).unwrap(), before_b);
            let value = paintcraft_core::types::json::parse(
                &std::fs::read(dir.join("recent-files.json")).unwrap(),
            )
            .unwrap();
            assert!(
                matches!(value.get("paths"),Some(paintcraft_core::types::json::Value::Array(v)) if v.is_empty())
            );
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn studio_can_reopen_recent_work_after_its_last_tab_is_closed() {
    let dir = scratch("recent-empty-studio");
    let dir = std::fs::canonicalize(dir).unwrap();
    let path = dir.join("closed.pcnd");
    project(&path, 9);
    Settings {
        experience: Experience::Studio,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(
        App::new(Some(path.clone())).with_settings(dir.clone()),
        2.0,
        true,
    );
    h.get_by_label("Close document 1").click();
    h.run();
    assert_eq!(h.state().document_count(), 0);
    history(&mut h, Language::English, None);
    h.get_by_role_and_label(Role::Button, &format!("closed.pcnd\n{}", dir.display()))
        .click();
    h.run();
    assert_eq!(h.state().document_count(), 1);
    assert_eq!(h.state().editor().document().canvas, (9, 5));
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}
