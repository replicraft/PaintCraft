use super::*;
use export::filter_bake::{self, Intent, Policy};
use paintcraft_core::types::{Rgba, filters::Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
fn document(enabled: bool) -> Document {
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([0, 0, 0, 255])),
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
fn request(doc: &Document) -> (Header, Vec<u8>, SourcePlan) {
    let target = export::metadata_policy::Target::new(
        crate::registry::Family::Msp,
        crate::registry::Operation::ExportFlat,
    );
    let policy = Policy::capture(doc, target, Intent::Bake, &|| Ok(())).unwrap();
    let plan = SourcePlan::capture_with_policies(
        doc,
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
            SourcePlan::capture(&doc).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        let (header, bytes, _) = request(&doc);
        assert_eq!(header.source_flags, 4);
        assert_eq!(header.bake_intent, 1);
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
            [255, 255, 255, 255]
        } else {
            [0, 0, 0, 255]
        };
        assert_eq!(
            super::super::decode(encoded, &|| Ok(()))
                .unwrap()
                .rgba(&|| Ok(()))
                .unwrap(),
            expected
        );
        assert_eq!(preview, expected);
        assert_eq!(losses, [filter_bake::BAKED_REASON]);
        assert_eq!(pcnd::write(&doc).unwrap(), before);
    }
}
#[test]
fn request_intent_inventory_context_and_old_version_never_imply_bake() {
    let doc = document(true);
    let (header, bytes, _) = request(&doc);
    for at in [122, 128, 159, 114] {
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
    for (at, value) in [(122, 2), (114, 8), (115, 1), (123, 1)] {
        let mut bad = header.encode();
        bad[at] = value;
        assert!(Header::decode(&bad).is_err());
    }
    let out = execute(&bytes, "test", &|| Ok(())).unwrap();
    for at in [193, 266, 267, 288, 319, 320, 383] {
        let mut bad = out.clone();
        bad[at] ^= 1;
        assert!(
            decode_response(&bad, &header, header.owner, "test").is_err(),
            "{at}"
        );
    }
}
#[test]
fn stale_stack_with_same_ids_and_cancelled_filter_preparation_do_not_publish() {
    let mut doc = document(true);
    let (header, bytes, plan) = request(&doc);
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::default();
    assert_eq!(
        plan.snapshot(&doc).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    for stop in [2, 5, 10, 20] {
        let calls = std::cell::Cell::new(0);
        let out = execute(&bytes, "test", &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel filter preparation",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap();
        let Frame::Refused(error) = decode_response(&out, &header, header.owner, "test").unwrap()
        else {
            panic!("cancelled filter encoded")
        };
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
    }
}
