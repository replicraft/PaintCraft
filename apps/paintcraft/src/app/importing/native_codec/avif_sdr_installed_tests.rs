//! Actual copied V6/V7 helper qualification, using headless egui controls only.
use super::*;
#[test]
fn selected_profile_admission_keeps_frontends_extensions_and_codec_distinct() {
    use registry::Frontend;
    assert!(admit_avif_profile(Frontend::Studio, Some("avif"), wire::Codec::AvifSdr8).is_ok());
    assert!(admit_avif_profile(Frontend::Studio, Some("avif"), wire::Codec::AvifSrgb8).is_ok());
    assert!(admit_avif_profile(Frontend::Studio, Some("png"), wire::Codec::AvifSdr8).is_err());
    assert!(admit_avif_profile(Frontend::ClassicV11, Some("avif"), wire::Codec::AvifSdr8).is_err());
    assert!(admit_avif_profile(Frontend::Studio, Some("avif"), wire::Codec::JxlRgba8).is_err());
}
use super::tests::{app as make_app, draw, retire};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../../../../crates/services/paintcraft-formats/tests/avif-sdr-fixtures/",
            $name
        ))
    };
}
const PRIOR: [u8; 16] = [
    19, 88, 201, 255, 99, 3, 17, 0, 34, 55, 89, 127, 8, 13, 21, 254,
];
fn prior_app(language: Language) -> App {
    let mut app = make_app();
    app.settings.language = language;
    app.install(Editor::with_document(
        paintcraft_document::import_image(2, 2, &PRIOR, &"0".repeat(64)).unwrap(),
        None,
    ))
    .unwrap();
    app
}
fn pixels(doc: &paintcraft_document::Document, width: u32, height: u32, expected: &[u8]) {
    assert_eq!(doc.canvas, (width, height));
    assert_eq!(doc.roots.len(), 1);
    assert_eq!(expected.len(), width as usize * height as usize * 4);
    let layer = &doc.layers[&doc.roots[0]];
    for (i, value) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer
                .raster
                .get((i % width as usize) as i64, (i / width as usize) as i64)
                .0,
            *value,
            "pixel{i}"
        );
    }
}
fn wait_review(h: &mut Harness<App>) {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        h.step();
        let pending = h
            .state()
            .pending_import
            .as_ref()
            .unwrap_or_else(|| panic!("review vanished: {}", h.state().editor.status()));
        let Candidate::NativeCodec(source) = &pending.candidate else {
            panic!("native codec")
        };
        if source.review.is_some() {
            let prepared = source.prepared.as_ref().unwrap();
            assert!(prepared.avif_assessment().is_none());
            assert!(prepared.avif_sdr_assessment().is_some());
            assert_eq!(
                prepared.decoder_profile(),
                wire::NATIVE_AVIF_SDR_IMPORT_VARIANT
            );
            break;
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn save_reopen_and_guard_source(
    h: &mut Harness<App>,
    root: &Path,
    input: &Path,
    original: &[u8],
    language: Language,
    reference: &[u8],
) {
    let ctx = h.state().context.clone();
    let project = root.join(format!("sdr-project-{language:?}.pcnd"));
    h.state_mut().save_selected(&ctx, project.clone());
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        h.state_mut().poll_edits(&ctx);
        h.state_mut().finish_save();
        if project.is_file() && !h.state().any_pending_edits() && !h.state().any_deferred_files() {
            break;
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    let reopened = paintcraft_formats::pcnd::read_with_limits(
        &fs::read(project).unwrap(),
        256,
        paintcraft_core::types::policy::STAGED_PIXELS_MAX,
    )
    .unwrap();
    pixels(&reopened, 17, 13, reference);
    let moved = root.join(format!("sdr-renamed-{language:?}.avif"));
    fs::rename(input, &moved).unwrap();
    let alias = root.join(format!("sdr-alias-{language:?}.pcnd"));
    fs::hard_link(moved, &alias).unwrap();
    h.state_mut().save_selected(&ctx, alias.clone());
    assert_eq!(fs::read(alias).unwrap(), original);
}
#[test]
#[ignore = "requires a copied installation with the exact real AVIF SDR V6/V7 helper"]
fn fixture_real_installed_avif_sdr_review() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    for (case, language) in (0..6)
        .flat_map(|case| [Language::English, Language::Japanese].map(|language| (case, language)))
    {
        let input = root.join(format!("sdr-named-{case}-{language:?}.avif"));
        fs::write(&input, fixture!("named-both.avif")).unwrap();
        let mut app = prior_app(language);
        let before = app.editor.document_snapshot();
        let metadata = app.editing.as_ref().unwrap().metadata_budget();
        let resident = app.editing.as_ref().unwrap().import_residency();
        let baseline = metadata.usage().reserved;
        let resident_before = resident.usage().reserved;
        app.import_selected(input.clone());
        assert!(app.pending_import.is_some(), "{}", app.editor.status());
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
        wait_review(&mut h);
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        pixels(h.state().editor.document(), 2, 2, &PRIOR);
        assert!(metadata.usage().reserved > baseline);
        h.run();
        h.run();
        for label in [
            "Image name",
            "Transparency name",
            "AVIF image item names are not retained in the new document.",
            "Convert YCbCr to 8-bit sRGB with rounding and clipping; 4:2:0 uses bilinear chroma interpolation.",
        ] {
            h.get_by_label(&translate(language, label));
        }
        h.get_by_label("Color image");
        h.get_by_label("Alpha mask");
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .accesskit_node()
                .is_disabled()
        );
        if case == 1 {
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                .click();
            h.run();
        } else {
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
            if case == 2 {
                fs::write(&input, b"changed after visible preview").unwrap();
            }
            if case == 3 {
                h.state_mut().editor.begin_text(Uuid::nil(), (0, 0));
                h.run();
            } else if case == 4 {
                h.state_mut().settings.experience = Experience::Classic;
                h.run();
            } else if case == 5 {
                let ctx = h.state().context.clone();
                h.state_mut()
                    .add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
                    .unwrap();
                h.run();
            } else {
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                    .click();
                h.run();
                h.run();
            }
        }
        assert!(
            h.state().pending_import.is_none(),
            "{}",
            h.state().editor.status()
        );
        assert_eq!(metadata.usage().reserved, baseline);
        if case != 0 {
            if case == 5 {
                assert_eq!(h.state().editor.document().canvas, (3, 3));
                let parked = h
                    .state()
                    .tabs
                    .iter()
                    .filter_map(|tab| tab.state.as_ref())
                    .find(|state| Arc::ptr_eq(&before, &state.editor.document_snapshot()))
                    .expect("original nonempty document remains parked");
                pixels(parked.editor.document(), 2, 2, &PRIOR);
            } else {
                assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
                pixels(h.state().editor.document(), 2, 2, &PRIOR);
            }
            assert!(h.state().auxiliary_sources.is_empty());
            assert_eq!(resident.usage().reserved, resident_before);
        } else {
            pixels(
                h.state().editor.document(),
                17,
                13,
                fixture!("420-vertical-limited-odd-alpha.rational.rgba"),
            );
            assert!(h.state().editor.is_dirty());
            assert!(h.state().editor.path().is_none());
            assert!(h.state().bound.is_none());
            assert_eq!(h.state().auxiliary_sources.len(), 1);
            save_reopen_and_guard_source(
                &mut h,
                &root,
                &input,
                fixture!("named-both.avif"),
                language,
                fixture!("420-vertical-limited-odd-alpha.rational.rgba"),
            );
        }
        retire(h.state());
        println!(
            "AVIF SDR {language:?} named case{case}: exact Fraction pixels and publication/source guards passed"
        );
    }
    for language in [Language::English, Language::Japanese] {
        for (name, bytes, reference) in [
            (
                "420-vertical-alpha",
                fixture!("420-vertical-full-even-alpha.avif").as_slice(),
                fixture!("420-vertical-full-even-alpha.rational.rgba").as_slice(),
            ),
            (
                "444-limited",
                fixture!("444-fullchroma-limited-even.avif").as_slice(),
                fixture!("444-fullchroma-limited-even.rational.rgba").as_slice(),
            ),
        ] {
            let input = root.join(format!("sdr-unnamed-{name}-{language:?}.avif"));
            fs::write(&input, bytes).unwrap();
            let mut app = prior_app(language);
            let before = app.editor.document_snapshot();
            app.import_selected(input);
            let mut h = Harness::builder()
                .with_size(egui::vec2(1360.0, 900.0))
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
            wait_review(&mut h);
            h.run();
            h.run();
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            h.get_by_label(&translate(language,"Convert YCbCr to 8-bit sRGB with rounding and clipping; 4:2:0 uses bilinear chroma interpolation."));
            assert!(
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                    .accesskit_node()
                    .is_disabled()
            );
            h.get_by_role_and_label(
                egui::Role::CheckBox,
                &translate(language, "Accept import limitations"),
            )
            .click();
            h.run();
            h.run();
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .click();
            h.run();
            h.run();
            assert!(h.state().pending_import.is_none());
            pixels(h.state().editor.document(), 16, 16, reference);
            retire(h.state());
            println!(
                "AVIF SDR {language:?} unnamed {name}: explicit reconstruction consent and exact Fraction pixels"
            );
        }
        let input = root.join(format!("sdr-unknown-siting-{language:?}.avif"));
        fs::write(&input, fixture!("420-unknown-limited-odd.avif")).unwrap();
        let mut app = prior_app(language);
        let before = app.editor.document_snapshot();
        let metadata = app.editing.as_ref().unwrap().metadata_budget();
        let baseline = metadata.usage().reserved;
        app.import_selected(input);
        let until = Instant::now() + Duration::from_secs(15);
        while app.pending_import.is_some() {
            draw(&mut app);
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        pixels(app.editor.document(), 2, 2, &PRIOR);
        assert!(app.auxiliary_sources.is_empty());
        assert_eq!(metadata.usage().reserved, baseline);
        assert!(app.editor.status().starts_with("Cannot open:"));
        retire(&app);
        println!(
            "AVIF SDR {language:?} unknown siting: refusal preserves nonempty source document"
        );
    }
}
