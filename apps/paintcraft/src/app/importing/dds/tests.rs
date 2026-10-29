use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, translate};
use std::{
    fs,
    time::{Duration, Instant},
};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/dds-mip-fixtures/dx10-rgba.dds"
);
const TOP: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/dds-mip-fixtures/top.rgba"
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
fn pixels(doc: &paintcraft_document::Document, expected: &[u8]) {
    let (width, height) = doc.canvas;
    assert_eq!(expected.len(), width as usize * height as usize * 4);
    let raster = &doc.layers[&doc.roots[0]].raster;
    for (i, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            raster
                .get((i % width as usize) as i64, (i / width as usize) as i64)
                .0,
            *pixel
        );
    }
}
#[test]
fn actual_dds_open_and_import_review_cancel_stale_and_source_change_preserve_nonempty_projects() {
    let root = std::env::temp_dir().join(format!("paintcraft-dds-native-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    for language in [Language::English, Language::Japanese] {
        for convert in [false, true] {
            for case in 0..6 {
                let input = root.join(if case == 0 {
                    format!("input-{language:?}-{convert}-{case}")
                } else {
                    format!("input-{language:?}-{convert}-{case}.dds")
                });
                fs::write(&input, SOURCE).unwrap();
                let mut app = app(language);
                // Preserve both a saved clean project and a deliberately unsaved project.
                if case == 0 || case == 4 {
                    app.editor.invalidate_saved_state();
                }
                let before = app.editor.document_snapshot();
                let ticket = app.editor.history_view().ticket;
                let dirty = app.editor.is_dirty();
                if convert {
                    app.import_selected(input.clone());
                } else {
                    app.open_selected(input.clone());
                }
                assert!(app.pending_import.is_some(), "{}", app.editor.status());
                assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
                assert_eq!(ticket, app.editor.history_view().ticket);
                assert_eq!(dirty, app.editor.is_dirty());
                let mut h = Harness::builder()
                    .with_size(egui::vec2(1360., 900.))
                    .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
                h.run();
                h.get_by_label(&translate(language,"Only the full-size DDS image will be imported. Its smaller mip levels will stay in the source file."));
                if case != 1 {
                    h.get_by_label(&translate(language, "Accept import limitations"))
                        .click();
                    h.run();
                }
                match case {
                    0 => {
                        h.get_by_role_and_label(
                            egui::Role::Button,
                            &translate(language, "Open image"),
                        )
                        .click();
                        h.run();
                    }
                    1 => {
                        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                            .click();
                        h.run();
                    }
                    2 => {
                        h.state_mut().editor.invalidate_saved_state();
                        h.get_by_role_and_label(
                            egui::Role::Button,
                            &translate(language, "Open image"),
                        )
                        .click();
                        h.run();
                    }
                    3 => {
                        let mut changed = SOURCE.to_vec();
                        *changed.last_mut().unwrap() ^= 1;
                        fs::write(&input, changed).unwrap();
                        h.get_by_role_and_label(
                            egui::Role::Button,
                            &translate(language, "Open image"),
                        )
                        .click();
                        h.run();
                    }
                    4 => {
                        h.state_mut().editor.begin_text(Uuid::nil(), (0, 0));
                        h.get_by_role_and_label(
                            egui::Role::Button,
                            &translate(language, "Open image"),
                        )
                        .click();
                        h.run();
                    }
                    _ => {
                        h.state_mut().settings.experience = Experience::Classic;
                        h.get_by_role_and_label(
                            egui::Role::Button,
                            &translate(language, "Open image"),
                        )
                        .click();
                        h.run();
                    }
                }
                assert!(h.state().pending_import.is_none());
                if case == 0 {
                    pixels(h.state().editor.document(), TOP);
                    assert!(h.state().editor.is_dirty());
                    assert!(h.state().bound.is_none() && h.state().editor.path().is_none());
                    let project = root.join(format!("saved-{language:?}-{convert}.pcnd"));
                    let ctx = h.state().context.clone();
                    h.state_mut().save_selected(&ctx, project.clone());
                    h.state_mut().editor.run_pending();
                    let end = Instant::now() + Duration::from_secs(15);
                    while !project.is_file()
                        || h.state().any_pending_edits()
                        || h.state().any_deferred_files()
                    {
                        h.state_mut().poll_edits(&ctx);
                        h.state_mut().finish_save();
                        assert!(Instant::now() < end);
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    pixels(
                        &paintcraft_formats::pcnd::read(&fs::read(project).unwrap()).unwrap(),
                        TOP,
                    );
                    let moved = input.with_extension("renamed.dds");
                    fs::rename(&input, &moved).unwrap();
                    let alias = input.with_extension("alias.pcnd");
                    fs::hard_link(&moved, &alias).unwrap();
                    assert!(h.state().guard_save_destination(&alias).is_err());
                    assert!(h.state().guard_export_destination(&alias).is_err());
                    assert_eq!(fs::read(moved).unwrap(), SOURCE);
                } else {
                    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
                    pixels(h.state().editor.document(), &PRIOR);
                    assert_eq!(ticket, h.state().editor.history_view().ticket);
                }
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn single_level_dds_open_import_and_classic_behavior_remain_distinct() {
    let root = std::env::temp_dir().join(format!("paintcraft-dds-single-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let input = root.join("single.dds");
    let rgba = [12, 34, 56, 0, 200, 100, 7, 255];
    let bytes = paintcraft_formats::dds::encode_rgba8_checked(
        2,
        1,
        &rgba,
        paintcraft_formats::dds::Transfer::Srgb,
        &|| Ok(()),
    )
    .unwrap();
    fs::write(&input, &bytes).unwrap();
    for convert in [false, true] {
        let mut app = app(Language::English);
        if convert {
            app.import_selected(input.clone());
        } else {
            app.open_selected(input.clone());
        }
        assert!(app.pending_import.is_none());
        pixels(app.editor.document(), &rgba);
        assert_eq!(app.editor.is_dirty(), convert);
        assert_eq!(app.auxiliary_sources.len(), 1);
    }
    let mut app = app(Language::English);
    app.settings.experience = Experience::Classic;
    let before = app.editor.document_snapshot();
    assert!(app.open_path(&input).is_err());
    app.import_selected(input);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn extensionless_probe_does_not_apply_dds_limit_or_read_generic_body() {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!("paintcraft-dds-probe-{}", Uuid::new_v4()));
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(b"PCND").unwrap();
    file.set_len(paintcraft_formats::dds::BYTES_MAX as u64 + 1)
        .unwrap();
    assert!(
        crate::format_admission::acquire_if_dds(&path)
            .unwrap()
            .is_none()
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn dds_review_cannot_publish_to_another_tab_or_replace_its_parked_source_project() {
    let path = std::env::temp_dir().join(format!("paintcraft-dds-tab-{}.dds", Uuid::new_v4()));
    fs::write(&path, SOURCE).unwrap();
    let mut app = app(Language::English);
    let ctx = app.context.clone();
    let old = app.active_tab_id();
    let before = app.editor.document_snapshot();
    app.open_selected(path.clone());
    assert!(app.pending_import.is_some());
    app.add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
        .unwrap();
    let foreign = app.editor.document_snapshot();
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&foreign, &app.editor.document_snapshot()));
    app.activate_tab(old, &ctx);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    pixels(app.editor.document(), &PRIOR);
    assert_eq!(fs::read(&path).unwrap(), SOURCE);
    fs::remove_file(path).unwrap();
}
