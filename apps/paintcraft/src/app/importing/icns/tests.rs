use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, format_message, translate};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/icns-fixtures/modern.icns"
);
fn setup(lang: Language) -> (Harness<'static, App>, PathBuf, PathBuf) {
    let folder = std::env::temp_dir().join(format!("paintcraft-icns-import-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("source.icns");
    std::fs::write(&path, SOURCE).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.settings.language = lang;
    app.import_selected(path.clone());
    assert!(app.pending_import.is_some(), "{}", app.editor.status());
    let h = Harness::builder()
        .with_max_steps(30)
        .with_size(egui::vec2(1100.0, 1000.0))
        .build_ui_state(
            move |ui, app: &mut App| {
                if ui.ctx().missing_glyph_policy() != egui::epaint::text::MissingGlyphPolicy::Tofu {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                }
                paintcraft_ui::i18n::set_language(ui.ctx(), lang);
                app.confirm_dialog(ui);
            },
            app,
        );
    (h, folder, path)
}
fn select(h: &mut Harness<App>, lang: Language, index: u16, tag: &str) {
    let label = format_message(
        lang,
        "Entry { $number }: { $tag }",
        &[("number", (index + 1).to_string()), ("tag", tag.into())],
    );
    h.get_by_role_and_label(egui::Role::RadioButton, &label)
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(egui::Role::RadioButton, &label)
        .click();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Update preview"))
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, "Update preview"))
        .click();
    h.run();
}
fn approve(h: &mut Harness<App>, lang: Language) {
    let text = translate(lang, "I accept these import changes.");
    h.get_by_role_and_label(egui::Role::CheckBox, &text)
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(egui::Role::CheckBox, &text).click();
    h.run();
}
#[test]
fn translated_selected_icns_preview_and_consent_import_exact_density_entry_as_unbound_copy() {
    for lang in [Language::English, Language::Japanese] {
        let (mut h, folder, path) = setup(lang);
        h.run();
        let before = h.state().editor.document_snapshot();
        assert!(
            h.get_by_label(&translate(lang, "Import selected entry"))
                .accesskit_node()
                .is_disabled()
        );
        select(&mut h, lang, 6, "ic11");
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert!(
            h.get_by_label(&translate(lang, "Import selected entry"))
                .accesskit_node()
                .is_disabled()
        );
        approve(&mut h, lang);
        h.get_by_role_and_label(
            egui::Role::Button,
            &translate(lang, "Import selected entry"),
        )
        .click();
        h.run();
        assert!(h.state().pending_import.is_none());
        assert_eq!(h.state().editor.document().canvas, (32, 32));
        assert!(h.state().editor.is_dirty());
        assert!(h.state().editor.path().is_none());
        assert!(h.state().bound.is_none());
        assert_eq!(h.state().imported_source.as_deref(), Some(path.as_path()));
        assert_eq!(h.state().auxiliary_sources.len(), 1);
        let expected = icns::decode_entry(SOURCE, 6, 4096, &|| Ok(())).unwrap();
        assert_eq!(
            (expected.entry.logical_width, expected.entry.scale),
            (16, 2)
        );
        let doc = h.state().editor.document();
        let layer = &doc.layers[&doc.roots[0]];
        for (i, rgba) in expected.rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(layer.raster.get((i % 32) as i64, (i / 32) as i64).0, *rgba);
        }
        drop(h);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
#[test]
fn pending_and_installed_icns_same_file_sources_remain_protected_across_rename_and_saved_copy() {
    let (mut h, folder, path) = setup(Language::English);
    h.run();
    let moved = folder.join("moved.icns");
    std::fs::rename(path, &moved).unwrap();
    let ctx = h.ctx.clone();
    let alias = folder.join("pending.pcnd");
    std::fs::hard_link(&moved, &alias).unwrap();
    h.state_mut().save_selected(&ctx, alias.clone());
    assert_eq!(std::fs::read(&alias).unwrap(), SOURCE);
    assert!(h.state().pending_import.is_some());
    select(&mut h, Language::English, 6, "ic11");
    approve(&mut h, Language::English);
    h.get_by_role_and_label(egui::Role::Button, "Import selected entry")
        .click();
    h.run();
    let project = folder.join("painting.pcnd");
    h.state_mut().save_selected(&ctx, project.clone());
    h.state_mut().editor.run_pending();
    h.state_mut().finish_save();
    assert!(project.is_file());
    for location in 0..3 {
        let app = h.state_mut();
        match location {
            1 => app
                .add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                .unwrap(),
            2 => {
                let mut parked = tabs::WorkspaceDocuments::new((2, 2), false);
                parked.swap(app);
                app.other_workspace = Some(Box::new(parked));
                app.settings.experience = Experience::Classic;
            }
            _ => {}
        }
        let alias = folder.join(format!("protected-{location}.png"));
        std::fs::hard_link(&moved, &alias).unwrap();
        app.do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
        assert_eq!(std::fs::read(alias).unwrap(), SOURCE);
        let alias = folder.join(format!("protected-{location}.pcnd"));
        std::fs::hard_link(&moved, &alias).unwrap();
        app.save_selected(&ctx, alias.clone());
        assert_eq!(std::fs::read(alias).unwrap(), SOURCE);
    }
    drop(h);
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn cancel_stale_owner_and_classic_icns_routes_cannot_publish() {
    let (mut h, folder, path) = setup(Language::English);
    h.run();
    let before = h.state().editor.document_snapshot();
    select(&mut h, Language::English, 6, "ic11");
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(h.state().pending_import.is_none());
    assert!(h.state().auxiliary_sources.is_empty());
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    h.state_mut().import_selected(path.clone());
    assert!(h.state().pending_import.is_some());
    h.state_mut().editor = Editor::with_document((*before).clone(), None);
    let current = h.state().editor.document_snapshot();
    h.run();
    assert!(h.state().pending_import.is_none());
    assert!(Arc::ptr_eq(&current, &h.state().editor.document_snapshot()));
    h.state_mut().settings.experience = Experience::Classic;
    h.state_mut().import_selected(path.clone());
    assert!(h.state().pending_import.is_none());
    assert!(h.state_mut().open_path(&path).is_err());
    assert!(Arc::ptr_eq(&current, &h.state().editor.document_snapshot()));
    drop(h);
    std::fs::remove_dir_all(folder).unwrap();
}
