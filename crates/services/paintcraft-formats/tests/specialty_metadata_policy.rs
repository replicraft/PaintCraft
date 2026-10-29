//! Actual specialty copies require a snapshot- and target-bound omission choice.
use export::metadata_policy::{Intent, OMITTED_REASON, Policy, Target};
use paintcraft_core::types::{ErrorCode, Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_formats::registry::{Family, Operation};
use paintcraft_formats::{dcx, dds, export, icns, icons, msp, ora, pcnd, pcx};
fn document() -> Document {
    let budget = SharedBudget::new(16 << 10);
    let fixture = pcnd::read_with_metadata_admission_checked(
        include_bytes!("pcnd-metadata-fixtures/rgb-profile.pcnd"),
        32,
        131072,
        8,
        pcnd::MetadataAdmission {
            bytes: 16 << 10,
            budget: &budget,
        },
        &|| Ok(()),
    )
    .unwrap();
    new_document(4, 4, Some(Rgba([255; 4])), 0, POLICY)
        .unwrap()
        .with_source_metadata_checked(fixture.source_metadata.clone(), &|| Ok(()))
        .unwrap()
}
fn policy(doc: &Document, family: Family, operation: Operation) -> Policy {
    Policy::capture(doc, Target::new(family, operation), Intent::Omit).unwrap()
}
fn texture() -> dds::TextureOptions {
    dds::TextureOptions {
        encoding: dds::Encoding::Rgba8,
        transfer: dds::Transfer::Srgb,
        alpha: dds::SourceAlpha::Straight,
        mips: dds::MipCount::Levels(1),
        filter: dds::MipFilter::Nearest,
        quality: dds::TextureQuality::Normal,
    }
}
#[test]
fn explicit_flat_specialty_copies_report_actual_omission_without_changing_source() {
    let doc = document();
    let original = pcnd::write(&doc).unwrap();
    let render = |doc: &Document, check: &dyn Fn() -> paintcraft_core::types::Result<()>| {
        paintcraft_render::render_cancellable(doc, check)
    };
    let dds = export::dds::prepare_document_with_metadata_policy(
        &doc,
        texture(),
        &[],
        |_, _| Ok(()),
        render,
        &|| Ok(()),
        policy(&doc, Family::Dds, Operation::ExportFlat),
    )
    .unwrap();
    assert_eq!(dds.losses(), &[OMITTED_REASON]);
    let msp = export::msp::prepare_document_with_metadata_policy(
        &doc,
        msp::Version::RowRleV2,
        &[],
        |_, _| Ok(()),
        |_| Ok(()),
        render,
        &|| Ok(()),
        policy(&doc, Family::Msp, Operation::ExportFlat),
    )
    .unwrap();
    assert_eq!(msp.losses(), &[OMITTED_REASON]);
    let rgb = export::jxr::prepare_opaque_rgb_with_metadata_policy(
        &doc,
        1024,
        &[],
        |_, _| Ok(()),
        render,
        &|| Ok(()),
        policy(&doc, Family::Jxr, Operation::ExportFlat),
    )
    .unwrap();
    assert_eq!(rgb.losses(), &[OMITTED_REASON]);
    assert!(rgb.rgb().iter().all(|v| *v == 255));
    let dcx = dcx::prepare_document_with_metadata_policy(
        &doc,
        pcx::Mode::Rgb24,
        &[],
        dcx::DocumentLimits::default(),
        &|| Ok(()),
        |d| paintcraft_render::render_cancellable(d, || Ok(())),
        policy(&doc, Family::Dcx, Operation::ExportFlat),
    )
    .unwrap();
    assert_eq!(dcx.losses(), &[OMITTED_REASON]);
    assert_eq!(pcnd::write(&doc).unwrap(), original);
}
#[test]
fn asset_and_layered_copies_keep_legacy_losses_and_private_encode_authority() {
    let doc = document();
    for (kind, family) in [
        (icons::Kind::Icon, Family::Ico),
        (icons::Kind::Cursor, Family::Cur),
    ] {
        let specs = [icons::EntrySpec {
            width: 4,
            height: 4,
            encoding: icons::Encoding::Png,
            hotspot: (kind == icons::Kind::Cursor).then_some((0, 0)),
        }];
        let prepared = icons::prepare_document_with_metadata_policy(
            &doc,
            kind,
            &specs,
            icons::Filter::Nearest,
            icons::WriteLimits::default(),
            &|| Ok(()),
            policy(&doc, family, Operation::ExportAsset),
        )
        .unwrap();
        assert_eq!(prepared.losses(), &[OMITTED_REASON]);
        assert!(!prepared.encode(&[], &|| Ok(())).unwrap().is_empty());
    }
    let prepared = icns::prepare_document_with_metadata_policy(
        &doc,
        &[icns::IconType::Retina16],
        icons::Filter::Nearest,
        &|| Ok(()),
        policy(&doc, Family::Icns, Operation::ExportAsset),
    )
    .unwrap();
    assert!(prepared.losses().iter().any(|s| s == OMITTED_REASON));
    assert_eq!(
        prepared.encode(&[], &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(
        !prepared
            .encode(&["icon-resized".into()], &|| Ok(()))
            .unwrap()
            .is_empty()
    );
    let p = policy(&doc, Family::Ora, Operation::SaveAsCopy);
    assert_eq!(
        ora::preflight_with_metadata_policy(&doc, &ora::Limits::default(), p).unwrap(),
        [OMITTED_REASON]
    );
    assert!(
        !ora::encode_document_with_metadata_policy(
            &doc,
            &ora::Limits::default(),
            &[],
            &|| Ok(()),
            p
        )
        .unwrap()
        .is_empty()
    );
}
#[test]
fn wrong_target_or_changed_snapshot_refuses_before_specialty_work() {
    let doc = document();
    let p = policy(&doc, Family::Msp, Operation::ExportFlat);
    assert_eq!(
        export::dds::prepare_document_with_metadata_policy(
            &doc,
            texture(),
            &[],
            |_, _| panic!("before admission"),
            |_, _| panic!("before render"),
            &|| Ok(()),
            p
        )
        .unwrap_err()
        .code,
        ErrorCode::ConflictStaleBase
    );
    let p = policy(&doc, Family::Jxr, Operation::ExportFlat);
    let mut changed = doc.clone();
    changed.state_id = uuid::Uuid::new_v4();
    assert_eq!(
        export::jxr::prepare_opaque_rgb_with_metadata_policy(
            &changed,
            1024,
            &[],
            |_, _| panic!("before admission"),
            |_, _| panic!("before render"),
            &|| Ok(()),
            p
        )
        .unwrap_err()
        .code,
        ErrorCode::ConflictStaleBase
    );
    let mut sink = Vec::new();
    assert_eq!(
        ora::write_document_to_with_metadata_policy(
            &doc,
            &mut sink,
            &ora::Limits::default(),
            &[],
            &|| Ok(()),
            p
        )
        .unwrap_err()
        .code,
        ErrorCode::ConflictStaleBase
    );
    assert!(sink.is_empty());
}
