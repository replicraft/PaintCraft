use super::*;
use paintcraft_ui::i18n::{Language, set_language, translate};
use std::sync::Arc;
fn document() -> paintcraft_document::Document {
    paintcraft_document::new_document(
        7,
        5,
        Some(paintcraft_core::types::Rgba([128, 188, 255, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap()
}
fn artifact(doc: &paintcraft_document::Document, settings: Settings) -> (wire::Header, Vec<u8>) {
    let owner = paintcraft_formats::codec_worker::Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 1,
        frontend: 3,
    };

    artifact_owned(doc, settings, owner)
}
fn artifact_owned(
    doc: &paintcraft_document::Document,
    settings: Settings,
    owner: paintcraft_formats::codec_worker::Owner,
) -> (wire::Header, Vec<u8>) {
    let plan = wire::SourcePlan::capture(doc, options(settings)).unwrap();
    let source = plan.snapshot(doc).unwrap();
    let header = wire::Header::capture(&plan, &source, owner, uuid::Uuid::new_v4()).unwrap();
    let mut request = header.encode().to_vec();
    request.extend(source);
    (
        header,
        wire::execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap(),
    )
}
#[test]
fn full_rust_validation_marks_only_a_bound_completion_and_converts_linear_preview() {
    let settings = Settings {
        transfer: controls::Transfer::Linear8,
        ..Default::default()
    };
    let doc = document();
    let (header, bytes) = artifact(&doc, settings);
    let mut completed = false;
    let mut prepared = response::validate(&header, &header.encode(), bytes.clone(), || {
        completed = true
    })
    .unwrap();
    assert!(completed);
    let image = prepared.preview.take().unwrap();
    assert!(
        image
            .pixels
            .iter()
            .all(|p| *p == egui::Color32::from_rgba_unmultiplied(128, 188, 255, 127))
    );
    assert!(
        prepared
            .losses
            .contains(&paintcraft_formats::export::dds::LINEAR_QUANTIZATION_LOSS.to_owned())
    );
    for at in [0, 12, 32, 73, 89, 121, 129, 161, 199, wire::RESPONSE_HEADER] {
        let mut bad = bytes.clone();
        bad[at] ^= 255;
        let mut complete = false;
        assert!(response::validate(&header, &header.encode(), bad, || complete = true).is_err());
        assert!(!complete);
    }
    // A recomputed digest cannot disguise a DDS header whose transfer disagrees.
    let mut bad = bytes.clone();
    let base = wire::RESPONSE_HEADER;
    bad[base + 128..base + 132].copy_from_slice(&29u32.to_le_bytes());
    let end = base + prepared.bytes().len();
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(&bad[base..end]);
    bad[129..161].copy_from_slice(&digest);
    let mut complete = false;
    assert!(response::validate(&header, &header.encode(), bad, || complete = true).is_err());
    assert!(!complete);
}
#[test]
fn post_admission_ui_displays_preview_and_losses_without_touching_source_in_en_and_ja() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    for language in [Language::English, Language::Japanese] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1100.0, 1000.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.state_mut().frontend = Frontend::Studio;
        h.state_mut().editor = Editor::from_document(document()).unwrap();
        h.state_mut().export_confirm = Some(ImageExport::Dds);
        set_language(&h.ctx, language);
        let snapshot = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        let name = h.state().name.clone();
        let settings = Settings {
            transfer: controls::Transfer::Linear8,
            ..Default::default()
        };
        let owner = crate::importing::Owner::capture(h.state());
        let (header, bytes) = artifact_owned(&snapshot, settings, owner.wire());
        let prepared = response::validate(&header, &header.encode(), bytes, || {}).unwrap();
        let frozen = super::super::exporting::Frozen::capture(h.state_mut()).unwrap();
        let plan = wire::SourcePlan::capture(&frozen.document, options(settings)).unwrap();
        h.state_mut().dds.review = Some(Review {
            frozen,
            plan: Some(plan),
            omit_source_metadata: false,
            bake_filters: false,
            owner,
            name: name.clone(),
            filename: "copy.dds".into(),
            settings,
            pending: None,
            prepared: Some(prepared),
            texture: None,
            shown: None,
            approved: false,
            error: None,
        });
        h.run();
        assert!(
            h.query_by_role_and_label(
                egui::Role::Image,
                &translate(language, "Base image preview")
            )
            .is_some()
        );
        assert!(
            h.query_by_label(&translate(
                language,
                "Converting to linear 8-bit samples loses color precision."
            ))
            .is_some()
        );
        let download = translate(language, "Download DDS");
        assert!(h.get_by_label(&download).accesskit_node().is_disabled());
        h.get_by_label(&translate(language, "I accept these export changes."))
            .click();
        h.run();
        assert!(!h.get_by_label(&download).accesskit_node().is_disabled());
        assert!(h.state().dds.review.as_ref().unwrap().approved);
        h.get_by_label(&translate(language, "DDS color samples"))
            .click();
        h.run();
        h.get_by_label(&translate(language, "sRGB samples")).click();
        h.run();
        let review = h.state().dds.review.as_ref().unwrap();
        assert!(
            !review.approved,
            "changing options retires artifact consent"
        );
        assert!(review.prepared.is_none());
        assert!(h.get_by_label(&download).accesskit_node().is_disabled());
        h.get_by_label(&translate(language, "Cancel")).click();
        h.run();
        assert!(h.state().dds.review.is_none());
        assert!(Arc::ptr_eq(&snapshot, &h.state().editor.session.snapshot()));
        assert_eq!(ticket, h.state().editor.session.ticket());
        assert_eq!(name, h.state().name);
    }
}
#[test]
fn default_settings_keep_legacy_capacity_and_stale_settings_cannot_reuse_a_preview() {
    for size in [(1, 1), (2048, 2048)] {
        assert!(!advanced(
            Settings {
                mips: controls::Mips::None,
                filter: controls::Filter::BoxStored,
                quality: controls::Quality::High,
                ..Default::default()
            },
            size
        ));
    }
    let doc = document();
    let (header, bytes) = artifact(
        &doc,
        Settings {
            transfer: controls::Transfer::Linear8,
            ..Default::default()
        },
    );
    let prepared = response::validate(&header, &header.encode(), bytes, || {}).unwrap();
    assert_ne!(
        prepared.header.settings(),
        wire::encode_settings(options(Settings::default()), doc.canvas).unwrap()
    );
}

#[test]
fn validated_refusal_marks_completion_but_malformed_refusal_does_not() {
    let doc = document();
    let plan = wire::SourcePlan::capture(&doc, options(Settings::default())).unwrap();
    let source = plan.snapshot(&doc).unwrap();
    let owner = paintcraft_formats::codec_worker::Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 1,
        frontend: 3,
    };
    let header = wire::Header::capture(&plan, &source, owner, uuid::Uuid::new_v4()).unwrap();
    let mut request = header.encode().to_vec();
    request.extend(source);
    request[wire::REQUEST_HEADER] ^= 1;
    let response = wire::execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
    let mut completed = false;
    let error = response::validate(&header, &header.encode(), response.clone(), || {
        completed = true
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CodecFailed);
    assert!(completed);
    let mut malformed = response;
    malformed[193] = 1; // A refusal cannot claim output losses.
    let mut completed = false;
    assert!(response::validate(&header, &header.encode(), malformed, || completed = true).is_err());
    assert!(!completed);
}

fn pending_review(
    app: &mut WebApp,
    settings: Settings,
) -> (Review, Outcome<Prepared>, Rc<Cell<bool>>) {
    let frozen = super::super::exporting::Frozen::capture(app).unwrap();
    let plan = wire::SourcePlan::capture(&frozen.document, options(settings)).unwrap();
    let result = Rc::new(RefCell::new(None));
    let cancelled = Rc::new(Cell::new(false));
    (
        Review {
            frozen,
            plan: Some(plan),
            omit_source_metadata: false,
            bake_filters: false,
            owner: crate::importing::Owner::capture(app),
            name: app.name.clone(),
            filename: "copy.dds".into(),
            settings,
            pending: Some(Pending {
                omit_source_metadata: false,
                bake_filters: false,
                settings,
                result: result.clone(),
                cancelled: cancelled.clone(),
                #[cfg(target_arch = "wasm32")]
                cancellation: Default::default(),
            }),
            prepared: None,
            texture: None,
            shown: None,
            approved: false,
            error: None,
        },
        result,
        cancelled,
    )
}
#[test]
fn settings_changes_drain_old_completion_without_reusing_its_preview() {
    let mut h = egui_kittest::Harness::builder().build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.state_mut().frontend = Frontend::Studio;
    h.state_mut().editor = Editor::from_document(document()).unwrap();
    h.state_mut().export_confirm = Some(ImageExport::Dds);
    let snapshot = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    let (mut review, target, cancelled) = pending_review(h.state_mut(), Settings::default());
    review.settings.transfer = controls::Transfer::Linear8;
    review.retire_preview();
    h.state_mut().dds.review = Some(review);
    let ctx = h.ctx.clone();
    h.state_mut().poll_dds_export(&ctx);
    assert!(h.state().dds.review.as_ref().unwrap().pending.is_some());
    assert!(
        !cancelled.get(),
        "editing options drains a known job rather than aborting it"
    );
    let (header, bytes) = artifact(&snapshot, Settings::default());
    *target.borrow_mut() = Some(response::validate(&header, &header.encode(), bytes, || {}));
    h.state_mut().poll_dds_export(&ctx);
    let review = h.state().dds.review.as_ref().unwrap();
    assert!(review.prepared.is_none());
    assert!(review.texture.is_none());
    // Host builds explicitly refuse the new Worker launch; they do not encode in UI.
    assert_eq!(
        review.error.as_ref().unwrap().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(Arc::ptr_eq(&snapshot, &h.state().editor.session.snapshot()));
    assert_eq!(ticket, h.state().editor.session.ticket());
}
#[test]
fn source_owner_or_modal_change_retires_pending_work_and_preserves_foreign_modal() {
    for case in 0..4 {
        let mut h = egui_kittest::Harness::builder().build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.state_mut().frontend = Frontend::Studio;
        h.state_mut().editor = Editor::from_document(document()).unwrap();
        h.state_mut().export_confirm = Some(ImageExport::Dds);
        let (review, _target, cancelled) = pending_review(h.state_mut(), Settings::default());
        h.state_mut().dds.review = Some(review);
        match case {
            0 => h.state_mut().name = "renamed.pcnd".into(),
            1 => h.state_mut().frontend = Frontend::Classic,
            2 => h.state_mut().export_confirm = Some(ImageExport::Png),
            _ => h.state_mut().editor = Editor::from_document(document()).unwrap(),
        }
        let snapshot = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        let name = h.state().name.clone();
        let ctx = h.ctx.clone();
        h.state_mut().poll_dds_export(&ctx);
        assert!(cancelled.get());
        assert!(h.state().dds.review.is_none());
        assert_eq!(
            h.state().export_confirm,
            if case == 2 {
                Some(ImageExport::Png)
            } else {
                None
            }
        );
        assert!(Arc::ptr_eq(&snapshot, &h.state().editor.session.snapshot()));
        assert_eq!(ticket, h.state().editor.session.ticket());
        assert_eq!(name, h.state().name);
    }
}

#[test]
fn compact_review_keeps_footer_reachable_and_requires_a_fully_visible_preview() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    for language in [Language::English, Language::Japanese] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(390.0, 580.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        let doc = paintcraft_document::new_document(
            192,
            192,
            Some(paintcraft_core::types::Rgba([128, 188, 255, 127])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        h.state_mut().frontend = Frontend::Studio;
        h.state_mut().editor = Editor::from_document(doc).unwrap();
        h.state_mut().export_confirm = Some(ImageExport::Dds);
        set_language(&h.ctx, language);
        let settings = Settings {
            encoding: controls::Encoding::Bc3,
            transfer: controls::Transfer::Linear8,
            mips: controls::Mips::Full,
            filter: controls::Filter::BoxStored,
            quality: controls::Quality::High,
        };
        let (mut review, _, _) = pending_review(h.state_mut(), settings);
        let (header, bytes) =
            artifact_owned(&review.frozen.document, settings, review.wire_owner());
        let prepared = response::validate(&header, &header.encode(), bytes, || {}).unwrap();
        review.pending = None;
        review.prepared = Some(prepared);
        h.state_mut().dds.review = Some(review);
        h.run();
        let download = translate(language, "Download DDS");
        for label in [download.clone(), translate(language, "Cancel")] {
            assert!(
                h.ctx
                    .content_rect()
                    .contains_rect(h.get_by_label(&label).rect()),
                "{label}"
            );
        }
        assert!(
            h.state().dds.review.as_ref().unwrap().shown.is_none(),
            "a clipped image does not count as a visible preview"
        );
        assert!(h.get_by_label(&download).accesskit_node().is_disabled());
        h.get_by_role_and_label(
            egui::Role::Image,
            &translate(language, "Base image preview"),
        )
        .scroll_to_me();
        h.run();
        assert!(h.state().dds.review.as_ref().unwrap().shown.is_some());
        let consent = translate(language, "I accept these export changes.");
        h.get_by_label(&consent).scroll_to_me();
        h.run();
        h.get_by_label(&consent).click();
        h.run();
        assert!(!h.get_by_label(&download).accesskit_node().is_disabled());
    }
}
