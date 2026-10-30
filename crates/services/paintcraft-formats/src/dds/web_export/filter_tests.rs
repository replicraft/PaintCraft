use super::*;
use export::filter_bake::{self, Intent, Policy};
use paintcraft_core::types::{Rgba, filters::Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
fn document(enabled: bool) -> Document {
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([255, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(17),
            enabled,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    doc
}
fn options() -> TextureOptions {
    TextureOptions {
        encoding: Encoding::Rgba8,
        transfer: Transfer::Srgb,
        alpha: SourceAlpha::Straight,
        mips: MipCount::Levels(1),
        filter: MipFilter::Nearest,
        quality: TextureQuality::Normal,
    }
}
fn request(doc: &Document) -> (Header, Vec<u8>, SourcePlan) {
    let target = export::metadata_policy::Target::new(
        crate::registry::Family::Dds,
        crate::registry::Operation::ExportFlat,
    );
    let policy = Policy::capture(doc, target, Intent::Bake, &|| Ok(())).unwrap();
    let plan = SourcePlan::capture_with_policies(
        doc,
        options(),
        export::CopyPolicies {
            filter_bake: policy,
            ..Default::default()
        },
    )
    .unwrap();
    let source = plan.snapshot(doc).unwrap();
    let owner = Owner {
        session: Uuid::from_u128(21),
        state: doc.state_id,
        epoch: 9,
        frontend: 3,
    };
    let header = Header::capture(&plan, &source, owner, Uuid::from_u128(22)).unwrap();
    let mut bytes = header.encode().to_vec();
    bytes.extend(source);
    (header, bytes, plan)
}
#[test]
fn actual_worker_bakes_enabled_pixels_and_names_disabled_intention_loss() {
    for enabled in [false, true] {
        let doc = document(enabled);
        let before = pcnd::write(&doc).unwrap();
        assert_eq!(
            SourcePlan::capture(&doc, options()).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        let (header, bytes, _) = request(&doc);
        assert_eq!(header.source_flags, 4);
        assert_eq!(header.bake_intent, 1);
        assert_eq!(
            header.filter_digest,
            filter_bake::inspect(&doc, &|| Ok(())).unwrap().digest()
        );
        let out = execute(&bytes, "test", &|| Ok(())).unwrap();
        let Frame::Success {
            encoded,
            preview,
            losses,
            ..
        } = decode_response(&out, &header, header.owner, "test").unwrap()
        else {
            panic!("actual filter encode refused")
        };
        let expected = if enabled {
            [0, 255, 255, 255]
        } else {
            [255, 0, 0, 255]
        };
        assert_eq!(
            decode_top_mip_checked(encoded, 1, &|| Ok(())).unwrap().rgba,
            expected
        );
        assert_eq!(preview, expected);
        assert_eq!(losses, [filter_bake::BAKED_REASON]);
        assert_eq!(pcnd::write(&doc).unwrap(), before);
    }
}
#[test]
fn intent_digest_flags_version_and_same_identity_stack_changes_fail_closed() {
    let mut doc = document(true);
    let (header, bytes, plan) = request(&doc);
    for at in [122, 128, 159, 121] {
        let mut changed = bytes.clone();
        changed[at] ^= 1;
        let altered_header = Header::decode(&changed[..REQUEST_HEADER]).unwrap();
        let out = execute(&changed, "test", &|| Ok(())).unwrap();
        assert!(
            matches!(
                decode_response(&out, &altered_header, header.owner, "test").unwrap(),
                Frame::Refused(_)
            ),
            "{at}"
        );
    }
    let mut old = bytes.clone();
    old[7] = b'2';
    old[8..10].copy_from_slice(&2_u16.to_be_bytes());
    assert!(execute(&old, "test", &|| Ok(())).is_err());
    let mut invalid = header.encode();
    invalid[122] = 2;
    assert!(Header::decode(&invalid).is_err());
    let mut invalid = header.encode();
    invalid[121] = 8;
    assert!(Header::decode(&invalid).is_err());
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::default();
    assert_eq!(
        plan.snapshot(&doc).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn filters_keep_typed_cancellation_and_bound_both_response_echoes() {
    let doc = document(true);
    let (header, bytes, _) = request(&doc);
    let out = execute(&bytes, "test", &|| Ok(())).unwrap();
    for at in [266, 267, 288, 319, 320, 383] {
        let mut changed = out.clone();
        changed[at] ^= 1;
        assert!(
            decode_response(&changed, &header, header.owner, "test").is_err(),
            "{at}"
        );
    }
    for stop in [2, 5, 10, 20] {
        let calls = std::cell::Cell::new(0);
        let result = execute(&bytes, "test", &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel filter preparation",
                ))
            } else {
                Ok(())
            }
        });
        let response = result.unwrap();
        let Frame::Refused(error) =
            decode_response(&response, &header, header.owner, "test").unwrap()
        else {
            panic!("cancelled filter encoded")
        };
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
    }
}
