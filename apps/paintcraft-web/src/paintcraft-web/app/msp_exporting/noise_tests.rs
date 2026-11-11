//! Actual versioned request/reply fixtures; authored, not browser lease proof.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::{
    Rgba,
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution, ThresholdLumaV1},
    resources::SharedBudget,
};
use paintcraft_document::{Document, FilterInstance, FilterStack, source_metadata::*};
use paintcraft_formats::{
    codec_worker::Owner,
    export::{self, filter_bake, metadata_policy},
    msp,
};
use std::sync::Arc;
fn source(mode: NoiseDistribution, enabled: bool, amount: i16, threshold: Option<i16>) -> Document {
    let mut doc = paintcraft_document::import_image(
        2,
        1,
        &[64, 16, 128, 255, 64, 16, 128, 255],
        &"13".repeat(32),
    )
    .unwrap();
    let mut filters = vec![FilterInstance::new(
        uuid::Uuid::from_u128(901),
        enabled,
        Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(amount, 1, true, mode).unwrap()),
    )];
    if let Some(level) = threshold {
        filters.push(FilterInstance::new(
            uuid::Uuid::from_u128(902),
            true,
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(level).unwrap()),
        ));
    }
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(filters).unwrap();
    doc
}
fn policies(doc: &Document, omit: bool) -> export::CopyPolicies {
    let target = metadata_policy::Target::new(
        paintcraft_formats::registry::Family::Msp,
        paintcraft_formats::registry::Operation::ExportFlat,
    );
    export::CopyPolicies {
        appearance_bake: paintcraft_formats::export::appearance_bake::Policy::Refuse,
        source_metadata: metadata_policy::Policy::capture(
            doc,
            target,
            if omit {
                metadata_policy::Intent::Omit
            } else {
                metadata_policy::Intent::Refuse
            },
        )
        .unwrap(),
        filter_bake: filter_bake::Policy::capture(doc, target, filter_bake::Intent::Bake, &|| {
            Ok(())
        })
        .unwrap(),
    }
}
fn owner(doc: &Document) -> Owner {
    Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 7,
        frontend: 3,
    }
}
fn artifact(
    doc: &Document,
    version: Version,
    owner: Owner,
    nonce: u128,
) -> (Plan, Request, Vec<u8>) {
    let plan = Plan::capture(doc, version, policies(doc, doc.source_metadata.is_some())).unwrap();
    let source = plan.snapshot(doc).unwrap();
    let request =
        Request::capture_plan(&plan, &source, owner, uuid::Uuid::from_u128(nonce)).unwrap();
    assert!(request.matches_captured(&plan));
    let header = request.encode();
    assert_eq!(header.len(), 704);
    assert_eq!(
        &header[113..121],
        &[
            if version == Version::UncompressedV1 {
                1
            } else {
                2
            },
            0,
            0,
            0,
            0,
            0,
            0,
            0
        ]
    );
    let packet: Vec<_> = header.into_iter().chain(source).collect();
    let response = wire::v8::execute(&packet, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
    (plan, request, response)
}
#[test]
fn default_disabled_and_both_distribution_intents_select8_for_both_encodings() {
    for version in [Version::UncompressedV1, Version::RowRleV2] {
        for (enabled, amount, mode) in [
            (false, 0, NoiseDistribution::Uniform),
            (false, 100, NoiseDistribution::Gaussian),
            (true, 0, NoiseDistribution::Gaussian),
        ] {
            let doc = source(mode, enabled, amount, Some(128));
            let before = doc.clone();
            assert!(needs_noise_worker(&doc));
            assert!(supports_review_version(version, &doc));
            let (plan, request, bytes) = artifact(&doc, version, owner(&doc), 51);
            assert_eq!(plan.protocol_version(), 8);
            assert_eq!(plan.version(), codec_version(version));
            assert_eq!(request.encoding_version(), codec_version(version));
            let mut completed = false;
            let prepared =
                response::validate_request(&request, &request.encode(), bytes, || completed = true)
                    .unwrap();
            assert!(completed);
            assert_eq!(
                msp::inspect(prepared.bytes(), &|| Ok(()))
                    .unwrap()
                    .version(),
                codec_version(version)
            );
            assert!(doc.same_semantics(&before));
        }
    }
    let plain = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([0, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    assert_eq!(Version::default(), Version::UncompressedV1);
    assert!(!needs_noise_worker(&plain));
    assert!(!supports_review_version(Version::UncompressedV1, &plain));
    assert!(matches!(
        Plan::capture(&plain, Version::RowRleV2, policies(&plain, false)).unwrap(),
        Plan::Legacy(_)
    ));
}
#[test]
fn independent_noise_then_explicit_threshold_yields_white_black_in_both_versions() {
    // Pinned seed1 mono deltas: Uniform[22,19], Gaussian[6,5]. Incoming luma34.29
    // lies below both explicit thresholds; Noise makes only first pixel cross.
    let expected = [egui::Color32::WHITE, egui::Color32::BLACK];
    for (mode, level) in [
        (NoiseDistribution::Uniform, 55),
        (NoiseDistribution::Gaussian, 40),
    ] {
        for version in [Version::UncompressedV1, Version::RowRleV2] {
            let doc = source(mode, true, 10, Some(level));
            let (plan, request, bytes) = artifact(&doc, version, owner(&doc), 52);
            let mut prepared =
                response::validate_request(&request, &request.encode(), bytes, || {}).unwrap();
            assert_eq!(prepared.preview.take().unwrap().pixels, expected);
            assert_eq!(
                msp::decode(prepared.bytes(), &|| Ok(())).unwrap().packed(),
                &[128]
            );
            assert!(request.matches_captured(&plan));
        }
    }
}
#[test]
fn color_is_refused_without_implicit_threshold_and_complete_reply_is_classified() {
    for version in [Version::UncompressedV1, Version::RowRleV2] {
        let doc = source(NoiseDistribution::Uniform, true, 10, None);
        let before = doc.clone();
        let (_, request, bytes) = artifact(&doc, version, owner(&doc), 53);
        let mut completed = false;
        let error =
            response::validate_request(&request, &request.encode(), bytes, || completed = true)
                .unwrap_err();
        assert!(completed);
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(doc.same_semantics(&before));
    }
}
fn review(app: &mut WebApp, version: Version) -> Review {
    let frozen = super::super::exporting::Frozen::capture(app).unwrap();
    let o = crate::importing::Owner::capture(app);
    let (plan, header, bytes) = artifact(&frozen.document, version, o.wire(), 54);
    let prepared = response::validate_request(&header, &header.encode(), bytes, || {}).unwrap();
    Review {
        frozen,
        plan: Some(plan),
        omit_source_metadata: false,
        bake_filters: true,
        owner: o,
        name: app.name.clone(),
        filename: "copy.msp".into(),
        version,
        pending: None,
        prepared: Some(prepared),
        texture: None,
        shown: Some(0),
        approved: false,
        error: None,
    }
}
#[test]
fn actual_default_noise_button_opens_v1_review_and_version_flip_retires_pending_owner() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.state_mut().frontend = Frontend::Studio;
    h.state_mut().editor = crate::editor::Editor::from_document(source(
        NoiseDistribution::Uniform,
        true,
        10,
        Some(55),
    ))
    .unwrap();
    h.state_mut().export_confirm = Some(ImageExport::Msp);
    assert_eq!(h.state().msp_version, Version::UncompressedV1);
    h.run();
    h.get_by_label("Preview export").click();
    h.run_steps(3);
    assert_eq!(
        h.state().msp.review.as_ref().unwrap().version,
        Version::UncompressedV1
    );
    let before = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    let mut r = review(h.state_mut(), Version::UncompressedV1);
    let cancelled = Rc::new(Cell::new(false));
    r.prepared = None;
    r.pending = Some(Pending {
        omit_source_metadata: false,
        bake_filters: true,
        version: Version::UncompressedV1,
        result: Rc::new(RefCell::new(None)),
        cancelled: cancelled.clone(),
        #[cfg(target_arch = "wasm32")]
        cancellation: Default::default(),
    });
    h.state_mut().msp.review = Some(r);
    h.run_steps(3);
    h.get_by_label("MSP version").click();
    h.run_steps(2);
    h.get_by_label("Version 2 (compressed)").click();
    h.run_steps(3);
    assert!(cancelled.get());
    assert_eq!(h.state().msp_version, Version::RowRleV2);
    let next = h.state().msp.review.as_ref().unwrap();
    assert_eq!(next.version, Version::RowRleV2);
    assert!(next.prepared.is_none());
    assert!(!next.approved);
    assert!(next.shown.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}
#[test]
fn version_and_owner_changes_reject_prepared_artifact_before_publication() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut output = ctx.run_ui(Default::default(), |_| {});
    output.textures_delta.clear();
    app.frontend = Frontend::Studio;
    app.editor = crate::editor::Editor::from_document(source(
        NoiseDistribution::Gaussian,
        true,
        10,
        Some(40),
    ))
    .unwrap();
    app.export_confirm = Some(ImageExport::Msp);
    let before = app.editor.session.snapshot();
    let mut r = review(&mut app, Version::UncompressedV1);
    r.version = Version::RowRleV2;
    assert!(r.take_ready(10).is_err());
    let doc = app.editor.session.document();
    let (_, q1, bytes) = artifact(doc, Version::UncompressedV1, owner(doc), 61);
    let (p2, q2, _) = artifact(doc, Version::RowRleV2, owner(doc), 62);
    assert!(!q1.matches_captured(&p2));
    assert_ne!(&q1.encode()[65..81], &q2.encode()[65..81]);
    let mut stale = q1.clone();
    stale.owner_mut().epoch += 1;
    let mut callback = false;
    assert!(response::validate_request(&stale, &q1.encode(), bytes, || callback = true).is_err());
    assert!(!callback);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.undo_depth(), 0);
}
#[test]
fn source_metadata_omission_is_explicit_and_same_msp_family_for_both_versions() {
    let mut doc = source(NoiseDistribution::Uniform, true, 10, Some(55));
    doc.source_metadata = Some(
        SourceMetadata::capture(
            SourceDescriptor {
                format: SourceFormat::Png,
                source_model: SourceColorModel::Rgb,
                source_sha256: [7; 32],
                source_bytes: 123,
                selection: None,
                source_dimensions: (2, 1),
                output_dimensions: (2, 1),
                orientation: None,
                orientation_policy: OrientationPolicy::Unmodified,
                resolution: None,
            },
            None,
            1024,
            &SharedBudget::new(4096),
            &|| Ok(()),
        )
        .unwrap(),
    );
    let before = doc.clone();
    for version in [Version::UncompressedV1, Version::RowRleV2] {
        // Each authority is independently required; neither consent grants the other.
        let metadata_denied = Plan::capture(&doc, version, policies(&doc, false)).unwrap_err();
        assert_eq!(metadata_denied.code, ErrorCode::CapabilityUnsupported);
        let mut bake_refused = policies(&doc, true);
        bake_refused.filter_bake = filter_bake::Policy::default();
        let bake_denied = Plan::capture(&doc, version, bake_refused).unwrap_err();
        assert_eq!(bake_denied.code, ErrorCode::CapabilityUnsupported);
        let (_, request, bytes) = artifact(&doc, version, owner(&doc), 63);
        assert!(request.metadata().omit_intent());
        let mut callback = false;
        let prepared =
            response::validate_request(&request, &request.encode(), bytes, || callback = true)
                .unwrap();
        assert!(callback);
        assert_eq!(prepared.header.encoding_version(), codec_version(version));
        assert!(doc.same_semantics(&before));
    }
}
