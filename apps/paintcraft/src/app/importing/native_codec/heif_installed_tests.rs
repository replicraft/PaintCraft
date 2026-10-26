//! Real installed helper + native Studio state/UI, without creating an OS window.
use super::tests::{app as make_app, draw, retire};
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../../../../crates/services/paintcraft-formats/tests/heif-color-fixtures/",
            $name
        ))
    };
}
fn exact_pixels(app: &App, reference: &[u8], identity: bool) {
    let document = app.editor.document();
    assert_eq!(document.canvas, (32, 24));
    assert_eq!(reference.len(), 32 * 24 * 3);
    let raster = &document.layers[&document.roots[0]].raster;
    for (i, expected) in reference.as_chunks::<3>().0.iter().enumerate() {
        let x = i % 32;
        let y = i / 32;
        let pixel = raster.get(x as i64, y as i64).0;
        for c in 0..3 {
            if identity {
                assert_eq!(pixel[c], expected[c]);
            } else {
                assert!(
                    pixel[c].abs_diff(expected[c]) <= 1,
                    "pixel{x},{y},channel{c}"
                );
            }
        }
        assert_eq!(pixel[3], [0, 1, 127, 128, 254, 255][(x + y) % 6]);
    }
}
#[test]
#[ignore = "private copied installation fixture"]
fn fixture_real_installed_heif_review() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    for index in 0..8 {
        let language = if index % 2 == 0 {
            Language::English
        } else {
            Language::Japanese
        };
        let identity = index % 2 == 0;
        let (bytes, reference) = if identity {
            (
                fixture!("identity444-alpha.heic").as_slice(),
                fixture!("identity444.rational-rgb").as_slice(),
            )
        } else {
            (
                fixture!("yuv420-limited-alpha.heic").as_slice(),
                fixture!("yuv420-limited.rational-rgb").as_slice(),
            )
        };
        let path = root.join(format!("heif-review-{index}.heic"));
        fs::write(&path, bytes).unwrap();
        let mut app = make_app();
        app.settings.language = language;
        let before = app.editor.document_snapshot();
        let metadata = app.editing.as_ref().unwrap().metadata_budget();
        let baseline = metadata.usage().reserved;
        app.import_selected(path.clone());
        assert!(app.pending_import.is_some(), "{}", app.editor.status());
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            h.step();
            let pending = h.state().pending_import.as_ref().expect("review pending");
            let Candidate::NativeCodec(source) = &pending.candidate else {
                panic!("native codec")
            };
            if source.review.is_some() {
                break;
            }
            assert!(Instant::now() < until, "{}", h.state().editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert!(metadata.usage().reserved > baseline);
        h.run();
        h.run();
        h.get_by_label(&translate(
            language,
            "HEIF image item names are not retained in the new document.",
        ));
        if !identity {
            h.get_by_label(&translate(language,"Convert YCbCr to 8-bit sRGB with rounding and clipping; 4:2:0 uses bilinear chroma interpolation."));
        }
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .accesskit_node()
                .is_disabled()
        );
        if index / 2 == 1 {
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                .click();
            h.run();
            assert!(h.state().pending_import.is_none());
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            assert_eq!(metadata.usage().reserved, baseline);
            assert!(h.state().auxiliary_sources.is_empty());
            retire(h.state());
            continue;
        }
        h.get_by_role_and_label(
            egui::Role::CheckBox,
            &translate(language, "Accept import limitations"),
        )
        .click();
        h.run();
        h.run();
        assert!(
            !h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .accesskit_node()
                .is_disabled()
        );
        if index / 2 == 2 {
            fs::write(&path, b"changed after preview").unwrap();
        }
        if index / 2 == 3 {
            h.state_mut().editor.begin_text(Uuid::nil(), (0, 0));
            h.run();
            assert!(h.state().pending_import.is_none());
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            assert_eq!(metadata.usage().reserved, baseline);
            assert!(h.state().auxiliary_sources.is_empty());
            retire(h.state());
            continue;
        }
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .click();
        h.run();
        h.run();
        assert!(
            h.state().pending_import.is_none(),
            "{}",
            h.state().editor.status()
        );
        if index / 2 == 2 {
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            assert_eq!(metadata.usage().reserved, baseline);
            assert!(h.state().auxiliary_sources.is_empty());
            retire(h.state());
            continue;
        }
        exact_pixels(h.state(), reference, identity);
        assert!(h.state().editor.is_dirty());
        assert!(h.state().editor.path().is_none());
        assert_eq!(h.state().auxiliary_sources.len(), 1);
        assert_eq!(metadata.usage().reserved, baseline);
        let renamed = root.join(format!("heif-moved-{index}.heic"));
        fs::rename(&path, &renamed).unwrap();
        let alias = root.join(format!("heif-alias-{index}.pcnd"));
        fs::hard_link(&renamed, &alias).unwrap();
        let ctx = h.state().context.clone();
        h.state_mut().save_selected(&ctx, alias.clone());
        assert_eq!(fs::read(alias).unwrap(), bytes);
        retire(h.state());
    }
    // Empty names + identity samples have neither metadata nor color losses.
    for language in [Language::English, Language::Japanese] {
        let path = root.join(format!("heif-anonymous-{language:?}.heic"));
        fs::write(&path, fixture!("anonymous-identity444-alpha.heic")).unwrap();
        let mut app = make_app();
        app.settings.language = language;
        let metadata = app.editing.as_ref().unwrap().metadata_budget();
        let baseline = metadata.usage().reserved;
        app.import_selected(path);
        let until = Instant::now() + Duration::from_secs(15);
        while app.pending_import.is_some() {
            draw(&mut app);
            assert!(Instant::now() < until, "{}", app.editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        exact_pixels(&app, fixture!("identity444.rational-rgb"), true);
        assert!(app.editor.is_dirty());
        assert_eq!(metadata.usage().reserved, baseline);
        retire(&app);
    }
}
#[test]
fn heif_bare_open_classic_drafts_and_full_admission_never_publish() {
    let root = std::env::temp_dir().join(format!("paintcraft-heif-negative-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("source.heic");
    fs::write(&path, fixture!("identity444-alpha.heic")).unwrap();
    let mut app = make_app();
    let before = app.editor.document_snapshot();
    assert!(app.open_path(&path).is_err());
    app.settings.experience = Experience::Classic;
    assert!(app.stage_native_heif(path.clone()).is_err());
    assert!(app.open_path(&path).is_err());
    app.settings.experience = Experience::Studio;
    app.editor.begin_text(Uuid::nil(), (0, 0));
    assert_eq!(
        app.stage_native_heif(root.join("absent.heic"))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    app.editor.cancel_text();
    let lease = app
        .worker_budget
        .reserve(app.worker_budget.usage().limit)
        .unwrap();
    assert_eq!(
        app.stage_native_heif(path).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(lease);
    assert!(app.pending_import.is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.worker_budget.usage().reserved, 0);
    fs::remove_dir_all(root).unwrap();
}
