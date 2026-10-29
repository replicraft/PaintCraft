use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_document::source_metadata::{
    BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat, SourceMetadata,
};

fn document() -> Document {
    paintcraft_document::new_document(2, 1, None, 0, paintcraft_document::POLICY).unwrap()
}
fn attach(doc: &mut Document, tag: u8) {
    doc.source_metadata = Some(
        SourceMetadata::capture(
            SourceDescriptor {
                format: SourceFormat::Png,
                source_model: SourceColorModel::Rgb,
                source_sha256: [tag; 32],
                source_bytes: 64,
                selection: None,
                source_dimensions: doc.canvas,
                output_dimensions: doc.canvas,
                orientation: Some(1),
                orientation_policy: OrientationPolicy::Unmodified,
                resolution: None,
            },
            None,
            BYTES_MAX,
            &SharedBudget::new(BYTES_MAX),
            &|| Ok(()),
        )
        .unwrap(),
    );
}
const PNG: Target = Target::new(Family::Png, Operation::ExportFlat);

#[test]
fn only_explicit_omit_can_authorize_one_attached_snapshot_without_changing_it() {
    let mut doc = document();
    attach(&mut doc, 3);
    let before = doc.clone();
    assert_eq!(
        Policy::default().validate(&doc, PNG).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        Policy::capture(&doc, PNG, Intent::Refuse)
            .unwrap()
            .validate(&doc, PNG)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(
        OMITTED_REASON.parse::<Intent>().is_err(),
        "receipt reason cannot mint explicit intent"
    );
    let policy = Policy::capture(&doc, PNG, Intent::Omit).unwrap();
    assert!(policy.validate(&doc, PNG).unwrap());
    assert!(
        policy.validate(&doc, PNG).unwrap(),
        "immutable witness can validate the same snapshot twice"
    );
    assert!(doc.same_semantics(&before));
    assert_eq!(
        doc.source_metadata.as_ref().unwrap().digest(),
        before.source_metadata.as_ref().unwrap().digest()
    );
}

#[test]
fn document_state_attachment_family_and_operation_are_independent_binding_guards() {
    let mut doc = document();
    attach(&mut doc, 3);
    let policy = Policy::capture(&doc, PNG, Intent::Omit).unwrap();
    for change in 0..4 {
        let mut changed = doc.clone();
        match change {
            0 => changed.document_id = Uuid::new_v4(),
            1 => changed.state_id = Uuid::new_v4(),
            2 => attach(&mut changed, 4),
            _ => changed.source_metadata = None,
        }
        assert_eq!(
            policy.validate(&changed, PNG).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
    for target in [
        Target::new(Family::Jpeg, Operation::ExportFlat),
        Target::new(Family::Png, Operation::SaveAsCopy),
    ] {
        assert_eq!(
            policy.validate(&doc, target).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
}

#[test]
fn no_metadata_omission_is_not_a_fake_loss_but_still_binds_the_explicit_request() {
    let doc = document();
    assert!(!Policy::default().validate(&doc, PNG).unwrap());
    let policy = Policy::capture(&doc, PNG, Intent::Omit).unwrap();
    assert!(!policy.validate(&doc, PNG).unwrap());
    let mut changed = doc.clone();
    attach(&mut changed, 3);
    assert_eq!(
        policy.validate(&changed, PNG).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    changed = doc.clone();
    changed.state_id = Uuid::new_v4();
    assert!(policy.validate(&changed, PNG).is_err());
    assert!(
        policy
            .validate(&doc, Target::new(Family::Jpeg, Operation::ExportFlat))
            .is_err()
    );
}

#[test]
fn pcnd_preserving_read_and_non_document_asset_routes_never_accept_omit() {
    for with_metadata in [false, true] {
        let mut doc = document();
        if with_metadata {
            attach(&mut doc, 3);
        }
        for target in [
            Target::new(Family::Pcnd, Operation::SaveAsCopy),
            Target::new(Family::Pcnd, Operation::ExportFlat),
            Target::new(Family::Png, Operation::SavePreserving),
            Target::new(Family::Png, Operation::Open),
            Target::new(Family::Png, Operation::ImportConvert),
            Target::new(Family::Png, Operation::ExportAsset),
        ] {
            assert_eq!(
                Policy::capture(&doc, target, Intent::Omit)
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
        for target in [
            Target::new(Family::Png, Operation::ExportFlat),
            Target::new(Family::Ora, Operation::SaveAsCopy),
            Target::new(Family::Ico, Operation::ExportAsset),
            Target::new(Family::Cur, Operation::ExportAsset),
            Target::new(Family::Icns, Operation::ExportAsset),
        ] {
            assert_eq!(
                Policy::capture(&doc, target, Intent::Omit)
                    .unwrap()
                    .validate(&doc, target)
                    .unwrap(),
                with_metadata
            );
        }
    }
}

#[test]
fn explicit_metadata_intent_reports_only_real_omission_and_keeps_other_loss_gates() {
    use crate::{export, image::ImageFormat};
    let mut doc = document();
    attach(&mut doc, 9);
    let before = doc.clone();
    for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Bmp] {
        let target = Target::new(
            Family::from_id(format.name()).unwrap(),
            Operation::ExportFlat,
        );
        let mut options = export::FlatOptions::new(format, 90);
        options.source_metadata = Policy::capture(&doc, target, Intent::Omit).unwrap();
        let allowed = export::KNOWN_LOSSES.map(str::to_owned);
        let flat = export::export_flat_options(&doc, options, &allowed).unwrap();
        assert_eq!(
            flat.losses
                .iter()
                .filter(|code| code.as_str() == OMITTED_REASON)
                .count(),
            1
        );
        assert!(doc.same_semantics(&before));
        assert!(doc.source_metadata.is_some());
        if format == ImageFormat::Jpeg {
            let error = export::export_flat_options(&doc, options, &[]).unwrap_err();
            assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
            assert!(error.message.contains("lossy-jpeg"));
        }
    }
    doc.source_metadata = None;
    let mut options = export::FlatOptions::new(ImageFormat::Png, 90);
    options.source_metadata = Policy::capture(&doc, PNG, Intent::Omit).unwrap();
    assert!(
        !export::export_flat_options(&doc, options, &[])
            .unwrap()
            .losses
            .iter()
            .any(|code| code == OMITTED_REASON)
    );
}
