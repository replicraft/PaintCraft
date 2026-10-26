use super::super::tests::{app, draw, retire};
use super::*;
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
fn folder() -> PathBuf {
    let path = std::env::temp_dir().join(format!("paintcraft-float-ui-{}", Uuid::new_v4()));
    fs::create_dir(&path).unwrap();
    path
}
#[test]
fn private_float_seam_retains_source_and_requires_qualified_route_no_classic_drafts_or_large_files()
{
    let root = folder();
    let path = root.join("picture.hdr");
    fs::write(&path, b"test input is not decoded during settings staging").unwrap();
    let mut app = app();
    let before = app.editor.document_snapshot();
    let mut admission = Request::new(
        registry::Frontend::Studio,
        Operation::ImportConvert,
        Family::Hdr,
        Platform::Native,
    );
    admission.extension = Some("hdr");
    admission.variant = Some(wire::NATIVE_HDR_SDR_IMPORT_VARIANT);
    let qualified = matches!(
        registry::admission(admission),
        registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent)
    );
    assert_eq!(app.stage_native_float(path.clone()).is_ok(), qualified);
    app.pending_import = None;
    app.confirm = None;
    assert!(app.open_path(&path).is_err());
    app.settings.experience = Experience::Classic;
    assert!(
        app.stage_native_float_admitted(path.clone(), wire::Codec::HdrSdr8)
            .is_err()
    );
    app.settings.experience = Experience::Studio;
    app.editor.begin_text(Uuid::nil(), (0, 0));
    assert!(
        app.stage_native_float_admitted(path.clone(), wire::Codec::HdrSdr8)
            .is_err()
    );
    app.editor.cancel_text();
    let large = root.join("large.exr");
    fs::File::create(&large)
        .unwrap()
        .set_len(wire::float::RAW_INPUT_BYTES_MAX as u64 + 1)
        .unwrap();
    assert_eq!(
        app.stage_native_float_admitted(large, wire::Codec::ExrSdr8)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    app.stage_native_float_admitted(path.clone(), wire::Codec::HdrSdr8)
        .unwrap();
    let pending = app.pending_import.as_ref().unwrap();
    assert_eq!(pending.auxiliary_sources.len(), 1);
    assert_eq!(pending.auxiliary_sources[0].path, path);
    let Candidate::NativeCodec(source) = &pending.candidate else {
        panic!()
    };
    assert!(source.job.is_none() && source.prepared.is_none() && source.review.is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.pending_import = None;
    retire(&app);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn unresolved_source_space_refusal_preserves_settings_and_nonempty_document() {
    let root = folder();
    let path = root.join("unlabelled.exr");
    fs::write(
        &path,
        include_bytes!(
            "../../../../../../crates/services/paintcraft-formats/tests/exr-fixtures/unlabelled.exr"
        ),
    )
    .unwrap();
    let mut app = app();
    let before = app.editor.document_snapshot();
    app.stage_native_float_admitted(path, wire::Codec::ExrSdr8)
        .unwrap();
    let mut pending = app.pending_import.take().unwrap();
    let Candidate::NativeCodec(source) = &mut pending.candidate else {
        panic!()
    };
    app.prepare_float_preview(source).unwrap();
    app.pending_import = Some(pending);
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        draw(&mut app);
        let Candidate::NativeCodec(source) = &app
            .pending_import
            .as_ref()
            .expect("editable settings retained")
            .candidate
        else {
            panic!()
        };
        if let Some(error) = &source.float.as_ref().unwrap().error {
            assert!(
                error
                    .to_string()
                    .contains("Choose the image's source color space"),
                "{error}"
            );
            assert!(source.prepared.is_none() && source.review.is_none());
            assert_eq!(
                source.float.as_ref().unwrap().settings.source_assignment,
                None
            );
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.pending_import = None;
    retire(&app);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn finite_interpretation_controls_and_limitations_are_translated_in_all_languages() {
    for language in Language::ALL {
        if language == Language::English {
            continue;
        }
        for label in [
            "Source color space",
            "Use source metadata",
            "Linear Rec.709 / D65",
            "Linear Radiance / equal-energy white",
            "Source transparency",
            "Premultiplied alpha",
            "Straight alpha",
            "Tone mapping",
            "Clip to display range",
            "Reinhard per channel",
            "Exposure (EV)",
            "Negative samples",
            "Refuse negative samples",
            "Clip negative samples to black",
            "Flatten transparency onto a color",
            "Prepare preview",
            "Choose the image's source color space before conversion",
        ] {
            assert_ne!(translate(language, label), label, "{language:?}: {label}");
        }
    }
    for loss in paintcraft_formats::precision::KNOWN_LOSSES
        .iter()
        .chain(paintcraft_formats::float_import::KNOWN_LOSSES)
    {
        assert!(import_loss_description(loss).is_some(), "{loss}");
    }
}

#[test]
fn settings_controls_are_accessible_and_do_not_decode_or_publish_before_preview() {
    use egui_kittest::{Harness, kittest::Queryable};
    let root = folder();
    let path = root.join("controls.hdr");
    fs::write(&path, b"no decoding before explicit preview").unwrap();
    let mut app = app();
    let before = app.editor.document_snapshot();
    app.stage_native_float_admitted(path, wire::Codec::HdrSdr8)
        .unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200., 1100.))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    h.run();
    h.get_by_role_and_label(egui::Role::SpinButton, "Exposure (EV)");
    h.get_by_role_and_label(egui::Role::ComboBox, "Tone mapping")
        .click();
    h.run();
    h.get_by_label("Reinhard per channel").click();
    h.run();
    h.get_by_role_and_label(egui::Role::CheckBox, "Flatten transparency onto a color")
        .click();
    h.run();
    let Candidate::NativeCodec(source) = &h.state().pending_import.as_ref().unwrap().candidate
    else {
        panic!()
    };
    let settings = source.float.as_ref().unwrap().settings;
    assert_eq!(settings.curve, Curve::ReinhardPerChannel);
    assert_eq!(
        settings.alpha_output,
        AlphaOutput::Flatten {
            srgb_background: [255; 3]
        }
    );
    assert!(source.job.is_none() && source.review.is_none() && source.prepared.is_none());
    assert_eq!(h.state().worker_budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(h.state().pending_import.is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn float_picker_exposes_only_exact_studio_conversion_variants() {
    let mut app = app();
    for experience in [Experience::Studio, Experience::Classic] {
        app.settings.experience = experience;
        for (family, extension, variant) in [
            (Family::Hdr, "hdr", wire::NATIVE_HDR_SDR_IMPORT_VARIANT),
            (Family::Exr, "exr", wire::NATIVE_EXR_SDR_IMPORT_VARIANT),
        ] {
            let mut request = Request::new(
                app.format_frontend(),
                Operation::ImportConvert,
                family,
                Platform::Native,
            );
            request.extension = Some(extension);
            request.variant = Some(variant);
            let expected = registry::admission(request).available();
            assert_eq!(
                app.format_filters(Kind::Import)
                    .iter()
                    .any(|filter| filter.extensions.iter().any(|value| value == extension)),
                expected
            );
            assert!(
                !app.format_filters(Kind::Open)
                    .iter()
                    .any(|filter| filter.extensions.iter().any(|value| value == extension))
            );
        }
    }
}

#[test]
fn preparation_refusals_offer_translated_settings_actions_without_changing_policy() {
    for detail in [
        "Negative source values require an explicit clipping policy",
        "Out-of-gamut negative color requires an explicit clipping policy",
        "Zero-alpha additive color requires an opaque background",
        "Nonfinite float color or alpha cannot be converted",
        "Float alpha must be finite and between zero and one",
        "An opaque source has no alpha convention to assign",
    ] {
        let error = Error::new(ErrorCode::CapabilityUnsupported, detail);
        let text = refusal_text(&error);
        assert!(!text.starts_with("Could not prepare"));
        for language in Language::ALL {
            if language != Language::English {
                assert_ne!(translate(language, text), text);
            }
        }
    }
}
