//! Draft controls, registered only with the borrowed requirements API.
use super::*;
use paintcraft_core::types::{
    Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
fn limits() -> RequirementsLimits {
    RequirementsLimits {
        layers: 8,
        tiles: 8,
        metadata_bytes: DOCUMENT_MAX as usize,
        coverage: Default::default(),
        work: 1_000_000,
    }
}
fn document(descriptor: Option<Descriptor>, enabled: bool) -> Document {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([20, 115, 200, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    if let Some(descriptor) = descriptor {
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                Uuid::from_u128(17),
                enabled,
                descriptor,
            )])
            .unwrap();
    }
    doc
}
fn scan(data: &[u8], resolve: impl FnMut(&str, &str) -> Result<()>) -> Result<()> {
    let witness = CheckedContainer::verify(data, &|| Ok(()))?;
    let budget = SharedBudget::new(REQUIREMENTS_CONTROL_BYTES);
    let reserve = budget.reserve_capacity(REQUIREMENTS_CONTROL_BYTES)?;
    witness.resolve_requirements_checked(
        limits(),
        &reserve.scope(AllocationClass::SharedBudget),
        &|| Ok(()),
        resolve,
    )
}
#[test]
fn actual_legacy_requirements_reject_missing_execution_or_pricing_without_unused_requirements() {
    for enabled in [false, true] {
        let doc = document(Some(Descriptor::InvertRgbV1), enabled);
        let source = crate::pcnd::write_compact(&doc).unwrap();
        assert_eq!(source[4], 3);
        assert!(crate::pcnd::read(&source).is_ok());
        for why in ["missing implementation", "missing pricing"] {
            let seen = Cell::new(0);
            let error = scan(&source, |profile, codec| {
                seen.set(seen.get() + 1);
                assert_eq!((profile, codec), ("invert-rgb-v1", "params-v1"));
                Err(Error::new(ErrorCode::CapabilityUnsupported, why))
            })
            .unwrap_err();
            assert_eq!(error.message, why);
            assert_eq!(seen.get(), 1);
        }
        // Registry lacks Box, which is unused. It must not block Invert adoption.
        scan(&source, |profile, _| {
            if profile == "box-blur-v1" {
                return Err(unsupported("unused Box unavailable"));
            }
            assert_eq!(profile, "invert-rgb-v1");
            Ok(())
        })
        .unwrap();
    }
    let empty = crate::pcnd::write(&document(None, true)).unwrap();
    scan(&empty, |_, _| {
        panic!("PCND1 contains no algorithm requirements")
    })
    .unwrap();
}
#[test]
fn bad_inner_trailer_precedes_all_requirement_callbacks() {
    let mut data = crate::pcnd::write(&document(Some(Descriptor::InvertRgbV1), true)).unwrap();
    *data.last_mut().unwrap() ^= 1;
    let visits = Cell::new(0);
    let error = scan(&data, |_, _| {
        visits.set(visits.get() + 1);
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CodecFailed);
    assert_eq!(visits.get(), 0);
}
#[test]
fn borrowed_legacy_mask_and_spatial_records_do_not_materialize_or_require_unused_profiles() {
    for (data, expected) in [
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/pcnd-mask-fixtures/zero.pcnd"
            ))
            .as_slice(),
            None,
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/pcnd-spatial-fixtures/box-disabled-r0.pcnd"
            ))
            .as_slice(),
            Some("box-blur-v1"),
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/pcnd-spatial-fixtures/box-half-mask.pcnd"
            ))
            .as_slice(),
            Some("box-blur-v1"),
        ),
    ] {
        let found = Cell::new(false);
        scan(data, |profile, codec| {
            assert_eq!(codec, "params-v1");
            if let Some(expected) = expected {
                assert_eq!(profile, expected);
            }
            found.set(true);
            Ok(())
        })
        .unwrap();
        assert!(found.get());
    }
}
#[test]
fn assessment_control_admission_and_work_and_cancel_precede_any_owned_adoption() {
    let bytes = crate::pcnd::write(&document(Some(Descriptor::InvertRgbV1), true)).unwrap();
    let witness = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(REQUIREMENTS_CONTROL_BYTES - 1);
    let reserve = budget
        .reserve_capacity(REQUIREMENTS_CONTROL_BYTES - 1)
        .unwrap();
    let visits = Cell::new(0);
    let error = witness
        .resolve_requirements_checked(
            limits(),
            &reserve.scope(AllocationClass::SharedBudget),
            &|| Ok(()),
            |_, _| {
                visits.set(visits.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(visits.get(), 0);
    let budget = SharedBudget::new(REQUIREMENTS_CONTROL_BYTES);
    let reserve = budget.reserve_capacity(REQUIREMENTS_CONTROL_BYTES).unwrap();
    let scope = reserve.scope(AllocationClass::SharedBudget);
    let mut bounds = limits();
    bounds.work = 0;
    assert_eq!(
        witness
            .resolve_requirements_checked(bounds, &scope, &|| Ok(()), |_, _| panic!())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let cancel = || {
        Err(Error::new(
            ErrorCode::OperationCancelled,
            "caller cancelled",
        ))
    };
    assert_eq!(
        witness
            .resolve_requirements_checked(limits(), &scope, &cancel, |_, _| panic!())
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
}

#[test]
fn one_checked_container_shares_inner_integrity_across_requirements_header_and_adoption() {
    let original = document(Some(Descriptor::InvertRgbV1), true);
    let bytes = crate::pcnd::write_compact(&original).unwrap();
    PAYLOAD_HASH_PASSES.with(|passes| passes.set(0));
    let witness = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(REQUIREMENTS_CONTROL_BYTES + (4 << 20));
    let reservation = budget.reserve_capacity(REQUIREMENTS_CONTROL_BYTES).unwrap();
    let scope = reservation.scope(AllocationClass::SharedBudget);
    let pairs = Cell::new(0);
    witness
        .resolve_requirements_checked(limits(), &scope, &|| Ok(()), |p, c| {
            assert_eq!((p, c), ("invert-rgb-v1", "params-v1"));
            pairs.set(pairs.get() + 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(pairs.get(), 1);
    assert_eq!(witness.header().unwrap().container_version, 3);
    let restored = witness
        .read_with_admission_checked(
            2,
            65_536,
            1,
            ReadAdmission {
                metadata: MetadataAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                },
                coverage: CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
    assert!(original.same_semantics(&restored));
    PAYLOAD_HASH_PASSES.with(|passes| assert_eq!(passes.get(), 1));
    // The ordinary APIs retain their own integrity checks, not a hidden cache.
    crate::pcnd::probe(&bytes).unwrap();
    crate::pcnd::read(&bytes).unwrap();
    PAYLOAD_HASH_PASSES.with(|passes| assert_eq!(passes.get(), 3));
}

#[test]
fn interleaved_encoded_tiles_are_skipped_before_later_actual_profile_resolution() {
    let raw = [10, 11, 128, 0, 20, 115, 200, 128];
    let mut doc = paintcraft_document::import_image(2, 1, &raw, "borrowed tile scan").unwrap();
    let first = doc.roots[0];
    doc.layers.get_mut(&first).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(51),
        false,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    let mut later = doc.layers[&first].clone();
    later.id = Uuid::from_u128(52);
    later.filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(53),
        false,
        Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
    )])
    .unwrap();
    doc.roots.push(later.id);
    doc.layers.insert(later.id, later);
    for data in [
        crate::pcnd::write(&doc).unwrap(),
        crate::pcnd::write_compact(&doc).unwrap(),
    ] {
        assert_eq!(data[4], 5);
        let mut encountered = Vec::new();
        let error = scan(&data, |profile, _| {
            encountered.push(profile.to_owned());
            if profile == "box-blur-v1" {
                return Err(unsupported("missing Box resource contract"));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.message, "missing Box resource contract");
        assert_eq!(encountered, ["invert-rgb-v1", "box-blur-v1"]);
    }
}

#[test]
fn missing_execution_refuses_before_cold_payload_decoder_and_witness_reuses_integrity() {
    let raw = [10, 11, 128, 0, 20, 115, 200, 128];
    let mut doc = paintcraft_document::import_image(2, 1, &raw, "cold payload").unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(91),
            false,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    let mut bytes = crate::pcnd::write(&doc).unwrap();
    let end = bytes.len() - 32;
    let block = end - 1 - raw.len() - 4;
    assert_eq!(&bytes[block..block + 4], &[1, 0, 0, 8]);
    bytes[block] = 2; // RLE storage of length8 is invalid: every run is six bytes.
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
    let witness = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(REQUIREMENTS_CONTROL_BYTES + (4 << 20));
    let reservation = budget.reserve_capacity(REQUIREMENTS_CONTROL_BYTES).unwrap();
    let scope = reservation.scope(AllocationClass::SharedBudget);
    let error = witness
        .resolve_requirements_checked(limits(), &scope, &|| Ok(()), |p, _| {
            assert_eq!(p, "invert-rgb-v1");
            Err(unsupported("execution registry refuses before RLE"))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert_eq!(error.message, "execution registry refuses before RLE");
    // A permitting registry can finish the borrowed scan without decoding. The
    // actual typed adoption still rejects the encoded payload on this witness.
    witness
        .resolve_requirements_checked(limits(), &scope, &|| Ok(()), |_, _| Ok(()))
        .unwrap();
    let error = witness
        .read_with_admission_checked(
            2,
            65_536,
            1,
            ReadAdmission {
                metadata: MetadataAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                },
                coverage: CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::CodecFailed);
    assert_eq!(error.message, "invalid RGBA run extent");
}
