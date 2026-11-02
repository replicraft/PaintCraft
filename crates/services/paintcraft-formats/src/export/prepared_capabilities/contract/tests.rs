use super::*;
use paintcraft_core::types::{
    Rgba,
    filters::{AddNoiseRgbV1, NoiseDistribution},
};
use paintcraft_document::{FilterInstance, FilterStack};
use std::cell::Cell;
fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
fn every_known_pair_has_explicit_class_and_contract_resolution() {
    assert_eq!(registrations().len(), 13);
    for entry in registrations() {
        let result = resolve_for(RegistryContract::Legacy1, entry.profile(), entry.codec());
        if entry.execution() == Execution::DirectNoise {
            assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
            assert_eq!(entry.workload_class(), ("noise-source-work-v1", 1));
        } else {
            assert!(std::ptr::eq(result.unwrap(), entry));
            assert_eq!(entry.workload_class(), ("base-work-v1", 1));
        }
        assert!(std::ptr::eq(
            resolve_for(
                RegistryContract::ScopedNoise2,
                entry.profile(),
                entry.codec()
            )
            .unwrap(),
            entry
        ));
    }
    for revision in [0, 5, u16::MAX] {
        assert_eq!(
            RegistryContract::try_from(revision).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    for contract in [RegistryContract::Legacy1, RegistryContract::ScopedNoise2] {
        for (profile, codec) in [
            ("unknown-v1", "params-v1"),
            ("add-noise-rgb-v1", "params-v2"),
            ("invert-rgb-v1", "unknown"),
        ] {
            assert_eq!(
                resolve_for(contract, profile, codec).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
        }
    }
}
#[test]
fn old_twelve_digests_and_layout_fields_are_unaffected_by_contract_metadata() {
    let mut old = Requirements::default();
    for entry in &registrations()[..12] {
        old.insert(entry.profile(), entry.codec()).unwrap();
    }
    let before = old.digests(&|| Ok(())).unwrap();
    assert_eq!(
        hex(before.0),
        "585a38dbdd4ff739357095aba8ca932ef90785942361e7335b772de331e553c1"
    );
    assert_eq!(
        hex(before.1),
        "19c1306ec948ec8a0ae131be6fe4418e8bb9fc805b41c3bbfbbb6d4bd0539c1b"
    );
    for contract in [RegistryContract::Legacy1, RegistryContract::ScopedNoise2] {
        let checks = Cell::new(0);
        old.validate_contract(contract, &|| {
            checks.set(checks.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(checks.get(), 12);
        assert_eq!(old.digests(&|| Ok(())).unwrap(), before);
    }
    let mut union = old.clone();
    union.insert("add-noise-rgb-v1", "params-v1").unwrap();
    assert_eq!(
        union
            .validate_contract(RegistryContract::Legacy1, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    union
        .validate_contract(RegistryContract::ScopedNoise2, &|| Ok(()))
        .unwrap();
    assert_eq!(old.digests(&|| Ok(())).unwrap(), before);
}
#[test]
fn stored_disabled_and_neutral_noise_still_needs_scoped_contract() {
    for distribution in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        for amount in [0, 10, 100] {
            for enabled in [false, true] {
                let mut doc = paintcraft_document::new_document(
                    1,
                    1,
                    Some(Rgba([10, 11, 12, 0])),
                    0,
                    paintcraft_document::POLICY,
                )
                .unwrap();
                let descriptor = Descriptor::AddNoiseRgbV1(
                    AddNoiseRgbV1::new(amount, 65535, false, distribution).unwrap(),
                );
                doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
                    FilterStack::new(vec![FilterInstance::new(
                        uuid::Uuid::from_u128(13),
                        enabled,
                        descriptor,
                    )])
                    .unwrap();
                assert!(requires_scoped_noise(&doc, &|| Ok(())).unwrap());
                let requirements = Requirements::capture(&doc, &|| Ok(())).unwrap();
                assert_eq!(
                    requirements
                        .validate_contract(RegistryContract::Legacy1, &|| Ok(()))
                        .unwrap_err()
                        .code,
                    ErrorCode::CapabilityUnsupported
                );
                requirements
                    .validate_contract(RegistryContract::ScopedNoise2, &|| Ok(()))
                    .unwrap();
                // General decoding remains valid intent support; no registry grant needed to decode.
                let entry = resolve(descriptor.id(), "params-v1").unwrap();
                let (bytes, len) = descriptor.semantic_parameter_bytes();
                assert_eq!(entry.decode(&bytes[..len]).unwrap(), descriptor);
            }
        }
    }
}
#[test]
fn cancellation_and_unknown_pairs_do_not_turn_into_a_negative_routing_answer() {
    let mut requirements = Requirements::default();
    assert_eq!(
        requirements
            .insert("unknown", "params-v1")
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(requirements.is_empty());
    requirements.insert("invert-rgb-v1", "params-v1").unwrap();
    assert_eq!(
        requirements
            .validate_contract(RegistryContract::ScopedNoise2, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "stop"
            )))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
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
            uuid::Uuid::from_u128(1),
            true,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    assert!(!requires_scoped_noise(&doc, &|| Ok(())).unwrap());
    assert_eq!(
        requires_scoped_noise(&doc, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "stop"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}
