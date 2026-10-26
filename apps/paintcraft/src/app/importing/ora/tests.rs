use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, import_loss_description, translate};
use std::{
    fs,
    time::{Duration, Instant},
};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/neutral.ora"
);
const MERGED: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/merged.rgba"
);
const PRIOR: [u8; 16] = [
    13, 71, 91, 255, 2, 29, 201, 0, 88, 1, 55, 128, 7, 9, 21, 255,
];
fn app(language: Language) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.settings.language = language;
    app.install(Editor::with_document(
        import_image(2, 2, &PRIOR, &"0".repeat(64)).unwrap(),
        None,
    ))
    .unwrap();
    app
}
fn check(doc: &paintcraft_document::Document) {
    assert_eq!(doc.canvas, (3, 2));
    assert_eq!(doc.roots.len(), 6);
    assert_eq!(paintcraft_render::render_flat(doc).unwrap(), MERGED);
    let names: Vec<_> = doc.layers_in_order().map(|l| l.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Back",
            "Mid",
            "Child visible but ancestor hidden",
            "Child hidden",
            "Bright",
            "Fore"
        ]
    );
    assert!(!doc.layers[&doc.roots[2]].visible);
    assert_eq!(doc.layers[&doc.roots[1]].raster.get(0, 1).0, [81, 8, 13, 0]);
    assert_eq!(doc.layers[&doc.roots[4]].origin, (-1, 0));
}
#[test]
fn actual_en_ja_group_import_reviews_capture_the_source_and_preserve_nonempty_work_on_refusal() {
    let root = std::env::temp_dir().join(format!("paintcraft-ora-native-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    for language in [Language::English, Language::Japanese] {
        for canonical in [false, true] {
            for case in 0..6 {
                let input = root.join(format!(
                    "source-{language:?}-{canonical}-{case}{}",
                    if canonical { ".ora" } else { "" }
                ));
                fs::write(&input, SOURCE).unwrap();
                let mut app = app(language);
                if case == 0 || case == 4 {
                    app.editor.invalidate_saved_state();
                }
                let before = app.editor.document_snapshot();
                let ticket = app.editor.history_view().ticket;
                let dirty = app.editor.is_dirty();
                app.import_selected(input.clone());
                assert!(app.pending_import.is_some(), "{}", app.editor.status());
                assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
                assert_eq!(dirty, app.editor.is_dirty());
                let mut h = Harness::builder()
                    .with_size(egui::vec2(1360., 900.))
                    .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
                h.run();
                h.get_by_label(&translate(
                    language,
                    import_loss_description(ora::GROUP_STRUCTURE_LOSS).unwrap(),
                ));
                match case {
                    2 => h.state_mut().editor.invalidate_saved_state(),
                    3 => {
                        let mut changed = SOURCE.to_vec();
                        *changed.last_mut().unwrap() ^= 1;
                        fs::write(&input, changed).unwrap();
                    }
                    4 => {
                        let target = before.roots[0];
                        h.state_mut().editor.begin_text(target, (0, 0));
                    }
                    5 => h.state_mut().settings.experience = Experience::Classic,
                    _ => {}
                }
                h.get_by_role_and_label(
                    egui::Role::Button,
                    &translate(language, if case == 1 { "Cancel" } else { "Open image" }),
                )
                .click();
                h.run();
                assert!(h.state().pending_import.is_none());
                if case == 0 {
                    check(h.state().editor.document());
                    assert!(h.state().editor.is_dirty());
                    assert!(h.state().bound.is_none() && h.state().editor.path().is_none());
                    let project = root.join(format!("saved-{language:?}-{canonical}.pcnd"));
                    let ctx = h.state().context.clone();
                    h.state_mut().save_selected(&ctx, project.clone());
                    h.state_mut().editor.run_pending();
                    let until = Instant::now() + Duration::from_secs(15);
                    while !project.is_file()
                        || h.state().any_pending_edits()
                        || h.state().any_deferred_files()
                    {
                        h.state_mut().poll_edits(&ctx);
                        h.state_mut().finish_save();
                        assert!(Instant::now() < until);
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    check(&paintcraft_formats::pcnd::read(&fs::read(project).unwrap()).unwrap());
                    let moved = input.with_extension("renamed.ora");
                    fs::rename(&input, &moved).unwrap();
                    let alias = input.with_extension("alias.pcnd");
                    fs::hard_link(&moved, &alias).unwrap();
                    h.state_mut().save_selected(&ctx, alias.clone());
                    h.state_mut().editor.run_pending();
                    h.state_mut().finish_save();
                    assert_eq!(fs::read(&alias).unwrap(), SOURCE);
                    let png_alias = input.with_extension("alias.png");
                    fs::hard_link(&moved, &png_alias).unwrap();
                    h.state_mut().do_export_quality(
                        &png_alias,
                        ImageFormat::Png,
                        &paintcraft_formats::export::KNOWN_LOSSES.map(str::to_owned),
                        90,
                    );
                    assert_eq!(fs::read(moved).unwrap(), SOURCE);
                    assert_eq!(fs::read(png_alias).unwrap(), SOURCE);
                } else {
                    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
                    assert_eq!(ticket, h.state().editor.history_view().ticket);
                }
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn group_import_cannot_retarget_a_tab_and_old_open_classic_unknown_zip_stay_denied() {
    let root = std::env::temp_dir().join(format!("paintcraft-ora-native-gates-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.ora");
    fs::write(&source, SOURCE).unwrap();
    let mut app = app(Language::English);
    let ctx = app.context.clone();
    let old = app.active_tab_id();
    let before = app.editor.document_snapshot();
    app.import_selected(source.clone());
    assert!(app.pending_import.is_some());
    app.add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
        .unwrap();
    let foreign = app.editor.document_snapshot();
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&foreign, &app.editor.document_snapshot()));
    app.activate_tab(old, &ctx);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert!(app.open_path(&source).is_err());
    app.settings.experience = Experience::Classic;
    app.import_selected(source.clone());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.settings.experience = Experience::Studio;
    for bytes in [include_bytes!("../../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/hidden-isolate.ora").as_slice(),include_bytes!("../../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/different-projection.ora"),b"PK\x03\x04otherZIP"]{
        let input=root.join("unsupported");fs::write(&input,bytes).unwrap();app.import_selected(input);assert!(app.pending_import.is_none());assert!(Arc::ptr_eq(&before,&app.editor.document_snapshot()));
    }
    let large = root.join("large");
    let file = fs::File::create(&large).unwrap();
    file.set_len(ora::CONVERSION_BYTES_MAX as u64 + 1).unwrap();
    drop(file);
    assert!(
        crate::format_admission::acquire_if_ora(&large)
            .unwrap()
            .is_none()
    );
    fs::remove_dir_all(root).unwrap();
}
