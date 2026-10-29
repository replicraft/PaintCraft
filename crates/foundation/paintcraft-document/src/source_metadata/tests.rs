use super::*;
use std::cell::Cell;
const SRGB: &[u8] = include_bytes!("../../../../../assets/profiles/sRGB2014.icc");
fn descriptor() -> SourceDescriptor {
    SourceDescriptor {
        format: SourceFormat::Png,
        source_model: SourceColorModel::Rgb,
        source_sha256: [3; 32],
        source_bytes: 4096,
        selection: None,
        source_dimensions: (3, 2),
        output_dimensions: (2, 3),
        orientation: Some(6),
        orientation_policy: OrientationPolicy::ExifMaterializedV1,
        resolution: Some(Resolution {
            horizontal: Rational {
                numerator: 30000,
                denominator: 100,
            },
            vertical: Rational {
                numerator: 29999,
                denominator: 100,
            },
            unit: ResolutionUnit::Inch,
            raw_unit: Some(2),
        }),
    }
}
#[test]
fn exact_icc_original_rationals_orientation_and_identity_round_trip() {
    let budget = SharedBudget::new(BYTES_MAX);
    let a =
        SourceMetadata::capture(descriptor(), Some(SRGB), BYTES_MAX, &budget, &|| Ok(())).unwrap();
    assert_eq!(a.profile(), Some(SRGB));
    assert_eq!(a.profile_header().unwrap().data_color_space, *b"RGB ");
    assert_eq!(a.working_color(), WorkingColor::UnmanagedRgba8);
    let other = SharedBudget::new(BYTES_MAX);
    let b =
        SourceMetadata::from_wire(&a.wire_header(), SRGB, BYTES_MAX, &other, &|| Ok(())).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.descriptor().resolution.unwrap().vertical.numerator, 29999);
    assert_eq!(budget.usage().reserved, a.storage_usage().1);
    let clone = a.clone();
    assert_eq!(a.storage_usage(), clone.storage_usage());
    drop(a);
    assert_ne!(budget.usage().reserved, 0);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn structural_cmyk_provenance_and_negative_bmp_density_never_activate_working_color() {
    let budget = SharedBudget::new(BYTES_MAX);
    // This source header is deliberately only structurally valid: no claim
    // that RGB tags form a suitable CMYK transform is made by retention.
    let mut profile = SRGB.to_vec();
    profile[16..20].copy_from_slice(b"CMYK");
    let mut origin = descriptor();
    origin.format = SourceFormat::Jpeg;
    origin.source_model = SourceColorModel::Cmyk;
    let a =
        SourceMetadata::capture(origin, Some(&profile), BYTES_MAX, &budget, &|| Ok(())).unwrap();
    assert_eq!(a.profile_header().unwrap().data_color_space, *b"CMYK");
    assert_eq!(a.working_color(), WorkingColor::UnmanagedRgba8);
    let b = SourceMetadata::from_wire(&a.wire_header(), &profile, BYTES_MAX, &budget, &|| Ok(()))
        .unwrap();
    assert_eq!(a, b);
    let mut bmp = descriptor();
    bmp.format = SourceFormat::Bmp;
    bmp.resolution = Some(Resolution {
        horizontal: Rational {
            numerator: -3780,
            denominator: 1,
        },
        vertical: Rational {
            numerator: 0,
            denominator: 1,
        },
        unit: ResolutionUnit::Metre,
        raw_unit: None,
    });
    let a = SourceMetadata::capture(bmp, None, BYTES_MAX, &budget, &|| Ok(())).unwrap();
    assert!(a.descriptor().resolution.unwrap().physical().is_none());
    let b =
        SourceMetadata::from_wire(&a.wire_header(), &[], BYTES_MAX, &budget, &|| Ok(())).unwrap();
    assert_eq!(a, b);
}
#[test]
fn limits_cancellation_and_malformed_profiles_release_every_charge() {
    let budget = SharedBudget::new(BYTES_MAX);
    let calls = Cell::new(0);
    let a = SourceMetadata::capture(descriptor(), Some(SRGB), BYTES_MAX, &budget, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    drop(a);
    for stop in 0..calls.get() {
        let count = Cell::new(0);
        assert!(
            SourceMetadata::capture(descriptor(), Some(SRGB), BYTES_MAX, &budget, &|| {
                let at = count.get();
                count.set(at + 1);
                if at == stop {
                    Err(resource("cancel control"))
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        assert_eq!(budget.usage().reserved, 0, "checkpoint{stop}");
    }
    assert_eq!(
        SourceMetadata::capture(descriptor(), Some(SRGB), 1, &budget, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        SourceMetadata::capture(
            descriptor(),
            Some(SRGB),
            BYTES_MAX,
            &SharedBudget::new(1),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    for bytes in [&SRGB[..131], &SRGB[..SRGB.len() - 1]] {
        assert!(
            SourceMetadata::capture(descriptor(), Some(bytes), BYTES_MAX, &budget, &|| Ok(()))
                .is_err()
        );
    }
    assert_eq!(budget.usage().reserved, 0);
    let mut bad = descriptor();
    bad.resolution.as_mut().unwrap().horizontal.denominator = 0;
    assert!(SourceMetadata::capture(bad, None, BYTES_MAX, &budget, &|| Ok(())).is_err());
    let mut missing = descriptor();
    missing.orientation = None;
    assert!(SourceMetadata::capture(missing, None, BYTES_MAX, &budget, &|| Ok(())).is_err());
}
#[test]
fn wire_digest_profile_extent_reserved_fields_and_version_are_bound() {
    let budget = SharedBudget::new(BYTES_MAX);
    let a =
        SourceMetadata::capture(descriptor(), Some(SRGB), BYTES_MAX, &budget, &|| Ok(())).unwrap();
    let retained = budget.usage().reserved;
    for at in [8, 10, 20, 56, 73, 84, 109, 112, 120, 152, 188] {
        let mut header = a.wire_header();
        header[at] ^= 255;
        assert!(
            SourceMetadata::from_wire(&header, SRGB, BYTES_MAX, &budget, &|| Ok(())).is_err(),
            "offset{at}"
        );
        assert_eq!(budget.usage().reserved, retained);
    }
    let mut profile = SRGB.to_vec();
    let end = profile.len() - 1;
    profile[end] ^= 1;
    assert!(
        SourceMetadata::from_wire(&a.wire_header(), &profile, BYTES_MAX, &budget, &|| Ok(()))
            .is_err()
    );
    assert_eq!(budget.usage().reserved, retained);
}

#[test]
fn attachment_choice_is_semantic_and_deterministically_changes_state_not_document_identity() {
    let budget = SharedBudget::new(BYTES_MAX);
    let metadata =
        SourceMetadata::capture(descriptor(), Some(SRGB), BYTES_MAX, &budget, &|| Ok(())).unwrap();
    let plain = crate::new_document(2, 3, None, 0, crate::POLICY).unwrap();
    let a = plain
        .with_source_metadata_checked(Some(metadata.clone()), &|| Ok(()))
        .unwrap();
    let b = plain
        .with_source_metadata_checked(Some(metadata.clone()), &|| Ok(()))
        .unwrap();
    assert_eq!(a.document_id, plain.document_id);
    assert_ne!(a.state_id, plain.state_id);
    assert!(!a.same_semantics(&plain));
    assert_eq!(a.state_id, b.state_id);
    assert!(a.same_semantics(&b));
    assert_eq!(
        a.with_source_metadata_checked(Some(metadata), &|| Ok(()))
            .unwrap()
            .state_id,
        a.state_id
    );
    let changed = a.with_source_metadata_checked(None, &|| Ok(())).unwrap();
    assert_ne!(changed.state_id, a.state_id);
    assert!(changed.same_semantics(&plain));
}
