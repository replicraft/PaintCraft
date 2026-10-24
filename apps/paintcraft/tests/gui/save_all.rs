use super::*;
use eframe::egui::Role;
use paintcraft::controller::Editor;
use paintcraft::settings::{Experience, Settings};
use paintcraft_ui::i18n::{Language, format_message, translate};

#[test]
fn studio_save_all_menu_and_shortcut_save_dirty_tabs_skip_clean_tabs_and_restore_focus() {
    for language in Language::ALL {
        for (ppp, multiple) in [(1.0, false), (2.0, true)] {
            let dir = scratch("studio-save-all-ui");
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
            let label = |source| translate(language, source);
            h.get_by_role_and_label(Role::Button, &label("File"))
                .click();
            h.run();
            assert!(
                h.get_by_role_and_label(Role::Button, &label("File"))
                    .get_by_label_contains(&label("Save All"))
                    .accesskit_node()
                    .is_disabled()
            );
            h.event(key(Key::Escape, Modifiers::NONE));
            h.event(key_up(Key::Escape, Modifiers::NONE));
            h.run();
            let mut snapshots = Vec::new();
            for number in 0..3 {
                if number != 0 {
                    h.get_by_label(&label("New")).click();
                    h.run();
                }
                if number != 1 {
                    h.get_by_label(&label("Add layer")).click();
                    h.run();
                }
                h.state_mut()
                    .set_path_text(dir.join(format!("doc-{number}.pcnd")).to_str().unwrap());
                snapshots.push(h.state().editor().document_snapshot());
            }
            // Save All stays enabled when only other tabs are dirty.
            let origin = format_message(
                language,
                "Document { $number }: { $title }",
                &[("number", "2".into()), ("title", "Untitled 2".into())],
            );
            h.get_by_role_and_label(Role::Button, &origin).click();
            h.run();
            let ticket = h.state().editor().history_view().ticket;
            if multiple {
                for _ in 0..2 {
                    h.event(key(Key::S, Modifiers::COMMAND | Modifiers::ALT));
                    h.event(key_up(Key::S, Modifiers::COMMAND | Modifiers::ALT));
                }
            } else {
                h.get_by_role_and_label(Role::Button, &label("File"))
                    .click();
                h.run();
                let file_label = label("File");
                let save_all_label = label("Save All");
                let command = h
                    .get_by_role_and_label(Role::Button, &file_label)
                    .get_by_label_contains(&save_all_label);
                assert!(!command.accesskit_node().is_disabled());
                command.click();
            }
            h.run_steps(8);
            h.run();
            assert_eq!(h.state().document_count(), 3);
            assert!(h.state().editor().document().same_semantics(&snapshots[1]));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert!(!h.state().editor().is_dirty());
            assert!(
                !dir.join("doc-1.pcnd").exists(),
                "clean unnamed document must not be written"
            );
            for number in [0, 2] {
                assert!(
                    Editor::open(&dir.join(format!("doc-{number}.pcnd")))
                        .unwrap()
                        .document()
                        .same_semantics(&snapshots[number])
                );
            }
            h.get_by_role_and_label(Role::Button, &label("File"))
                .click();
            h.run();
            assert!(
                h.get_by_role_and_label(Role::Button, &label("File"))
                    .get_by_label_contains(&label("Save All"))
                    .accesskit_node()
                    .is_disabled()
            );
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
