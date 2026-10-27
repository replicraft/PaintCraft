use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::Rgba;
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
fn document(pixel: [u8; 4]) -> paintcraft_document::Document {
    paintcraft_document::new_document(7, 5, Some(Rgba(pixel)), 0, paintcraft_document::POLICY)
        .unwrap()
}
fn app(language: Language, source: &Path, pixel: [u8; 4]) -> App {
    fs::write(
        source,
        paintcraft_formats::pcnd::write(&document(pixel)).unwrap(),
    )
    .unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.settings.language = language;
    app.initialize_editing(app.context.clone());
    app.open_path(source).unwrap();
    app
}
fn harness(app: App) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_max_steps(40)
        .with_size(egui::vec2(1100.0, 1000.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    h.run();
    h
}
fn prepared(h: &mut Harness<App>) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        h.step();
        let s = h
            .state()
            .msp_export
            .as_ref()
            .expect("MSP review remains open");
        assert!(s.error.is_none(), "{:?}", s.error);
        if s.texture.is_some() && s.shown.is_some() {
            break;
        }
        assert!(Instant::now() < until, "MSP preparation stalled");
        std::thread::sleep(Duration::from_millis(1));
    }
    h.run();
}
fn click(h: &mut Harness<App>, lang: Language, label: &str) {
    h.get_by_role_and_label(egui::Role::Button, &translate(lang, label))
        .click();
    h.run();
}
fn root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("paintcraft-msp-ui-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    root
}
#[test]
fn msp_version_admission_is_native_studio_only() {
    for version in [Version::UncompressedV1, Version::RowRleV2] {
        assert!(
            admit(
                registry::Frontend::Studio,
                Path::new("picture.msp"),
                version
            )
            .is_ok()
        );
        assert!(
            admit(
                registry::Frontend::Studio,
                Path::new("picture.png"),
                version
            )
            .is_err()
        );
    }
    for frontend in [
        registry::Frontend::ClassicV7,
        registry::Frontend::ClassicV10,
        registry::Frontend::ClassicV11,
    ] {
        assert!(admit(frontend, Path::new("picture.msp"), Version::RowRleV2).is_err());
    }
}
#[test]
fn actual_both_versions_in_english_and_japanese_publish_prepared_pixels_without_retargeting() {
    for lang in [Language::English, Language::Japanese] {
        for version in [Version::UncompressedV1, Version::RowRleV2] {
            let root = root();
            let source = root.join("source.pcnd");
            let target = root.join("copy.msp");
            if version == Version::RowRleV2 {
                fs::write(&target, b"existing destination").unwrap();
            }
            let mut app = app(lang, &source, [255; 4]);
            app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
            let original = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            let dirty = app.editor.is_dirty();
            let bound = app.bound.clone();
            let text = app.path_text.clone();
            app.begin_msp_export(version);
            let mut h = harness(app);
            prepared(&mut h);
            let state = h.state().msp_export.as_ref().unwrap();
            let bytes = state.prepared.as_ref().unwrap().artifact.bytes().to_vec();
            let decoded = msp::decode(&bytes, &|| Ok(())).unwrap();
            assert_eq!(decoded.header().version(), worker::version(version));
            assert_eq!(decoded.rgba(&|| Ok(())).unwrap(), vec![255; 7 * 5 * 4]);
            if version == Version::UncompressedV1 {
                assert_eq!(
                    bytes,
                    msp::encode_v1_rgba_exact(7, 5, &[255; 7 * 5 * 4], &|| Ok(())).unwrap()
                );
            }
            assert!(
                h.query_all_by_label_contains(&translate(lang, "Export preview"))
                    .count()
                    > 0
            );
            click(&mut h, lang, "Choose destination…");
            if version == Version::RowRleV2 {
                assert_eq!(fs::read(&target).unwrap(), b"existing destination");
                assert!(
                    h.state()
                        .msp_export
                        .as_ref()
                        .unwrap()
                        .destination
                        .as_ref()
                        .unwrap()
                        .1
                        .is_some()
                );
            } else {
                assert!(!target.exists());
            }
            assert_eq!(
                h.state()
                    .msp_export
                    .as_ref()
                    .unwrap()
                    .prepared
                    .as_ref()
                    .unwrap()
                    .artifact
                    .bytes(),
                bytes
            );
            click(&mut h, lang, "Export now");
            assert_eq!(fs::read(&target).unwrap(), bytes);
            if let Some(directory) = std::env::var_os("PAINTCRAFT_MSP_UI_EVIDENCE") {
                fs::create_dir_all(&directory).unwrap();
                fs::write(
                    Path::new(&directory).join(format!("{lang:?}-{version:?}.msp")),
                    &bytes,
                )
                .unwrap();
            }
            assert!(Arc::ptr_eq(
                &original,
                &h.state().editor.document_snapshot()
            ));
            assert_eq!(ticket, h.state().editor.history_view().ticket);
            assert_eq!(dirty, h.state().editor.is_dirty());
            assert_eq!(bound, h.state().bound);
            assert_eq!(text, h.state().path_text);
            assert!(h.state().msp_export.is_none());
            drop(h);
            fs::remove_dir_all(root).unwrap();
        }
    }
}
#[test]
fn cancel_stale_owner_frontend_draft_and_renamed_source_alias_do_not_publish() {
    for case in 0..6 {
        let root = root();
        let source = root.join("source.pcnd");
        let target = root.join("copy.msp");
        let mut app = app(Language::English, &source, [0, 0, 0, 255]);
        app.path_text = target.with_extension("pcnd").to_string_lossy().into_owned();
        let source_bytes = fs::read(&source).unwrap();
        app.begin_msp_export(Version::RowRleV2);
        let mut h = harness(app);
        prepared(&mut h);
        match case {
            0 => click(&mut h, Language::English, "Cancel"),
            1 => {
                h.state_mut().settings.experience = Experience::Classic;
                h.step();
            }
            2 => {
                h.state_mut().editor.cancel_pending_edits();
                h.step();
            }
            3 => {
                h.state_mut().path_text.push_str("changed");
                h.step();
            }
            4 => {
                let renamed = root.join("renamed.pcnd");
                fs::rename(&source, &renamed).unwrap();
                fs::hard_link(&renamed, &target).unwrap();
                click(&mut h, Language::English, "Choose destination…");
                assert_eq!(fs::read(&target).unwrap(), source_bytes);
            }
            5 => {
                click(&mut h, Language::English, "Choose destination…");
                h.state_mut().editor.cancel_pending_edits();
                h.state_mut().finish_msp_export();
            }
            _ => unreachable!(),
        }
        h.run();
        assert!(h.state().msp_export.is_none());
        if case != 4 {
            assert!(!target.exists());
        }
        drop(h);
        fs::remove_dir_all(root).unwrap();
    }
}
#[path = "worker_tests.rs"]
mod worker_tests;

#[path = "guard_tests.rs"]
mod guard_tests;
