use super::*;

#[test]
fn missing_retained_fields_need_no_realm_and_calibration_is_still_omitted() {
    for name in [
        "rgb24-v40-no-metadata.bmp",
        "rgb24-v108-calibrated-no-retention.bmp",
    ] {
        let budget = SharedBudget::new(0);
        let result = prepared(&own(name), &budget, &|| Ok(())).unwrap();
        assert!(result.attachment.is_none());
        assert_eq!(budget.usage().peak, 0);
        assert_eq!(
            result
                .image
                .losses
                .iter()
                .any(|s| s == "bmp-color-metadata-ignored"),
            name.contains("calibrated")
        );
    }
    let original = own("rgb24-v124-profile-only.bmp");
    let result = prepared(&original, &SharedBudget::new(1 << 20), &|| Ok(())).unwrap();
    assert!(
        result
            .attachment
            .as_ref()
            .unwrap()
            .descriptor()
            .resolution
            .is_none()
    );
    assert!(
        !result
            .image
            .losses
            .iter()
            .any(|s| s == RESOLUTION_RETAINED_INACTIVE)
    );
}

#[test]
fn malformed_linked_profile_source_indices_and_inspected_digest_refuse_without_leaks() {
    let original = own("rgb24-v124-profile-positive.bmp");
    for (at, value) in [
        (70, bmp_metadata::PROFILE_LINKED),
        (126, 0),
        (126, u32::MAX),
        (130, u32::MAX),
        (134, 1),
    ] {
        let mut bytes = original.clone();
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        let budget = SharedBudget::new(1 << 20);
        assert!(prepared(&bytes, &budget, &|| Ok(())).is_err());
        assert_eq!(budget.usage().reserved, 0);
    }
    let mut overlap = original.clone();
    let pixel_offset = u32::from_le_bytes(overlap[10..14].try_into().unwrap());
    overlap[126..130].copy_from_slice(&(pixel_offset - 14).to_le_bytes());
    assert!(prepared(&overlap, &SharedBudget::new(1 << 20), &|| Ok(())).is_err());
    let mut bad_index = own("indexed8-v124-profile-reserved.bmp");
    let pixels = u32::from_le_bytes(bad_index[10..14].try_into().unwrap()) as usize;
    bad_index[pixels] = 2;
    let budget = SharedBudget::new(1 << 20);
    assert!(prepared(&bad_index, &budget, &|| Ok(())).is_err());
    assert!(
        budget.usage().peak > 0,
        "pixel failure occurs after private profile capture"
    );
    assert_eq!(budget.usage().reserved, 0);
    let mut source = bmp_metadata::inspect(&original, PIXELS, &|| Ok(())).unwrap();
    source.icc.as_mut().unwrap().payload.sha256[0] ^= 1;
    assert!(
        super::super::capture::source_metadata(
            &original,
            &source,
            crate::pcnd::MetadataAdmission {
                bytes: BYTES_MAX,
                budget: &budget
            },
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn cancel_every_reachable_checkpoint_and_shared_realm_contention_release_all_retention() {
    for bytes in [
        own("rgba32-v124-profile-hidden.bmp"),
        fixture("bmp-rle-fixtures", "rle8-icc-before.bmp"),
    ] {
        let budget = SharedBudget::new(1 << 20);
        let calls = Cell::new(0);
        drop(
            prepared(&bytes, &budget, &|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
        );
        assert!(calls.get() > 10);
        for stop in 0..calls.get() {
            let count = Cell::new(0);
            let result = prepared(&bytes, &budget, &|| {
                let at = count.get();
                count.set(at + 1);
                if at == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel BMP retention",
                    ))
                } else {
                    Ok(())
                }
            });
            assert_eq!(
                result.unwrap_err().code,
                ErrorCode::OperationCancelled,
                "checkpoint{stop}"
            );
            assert_eq!(budget.usage().reserved, 0);
        }
        let first = prepared(&bytes, &budget, &|| Ok(())).unwrap();
        let charge = first.attachment.as_ref().unwrap().storage_usage().1;
        let exact = SharedBudget::new(charge);
        let held = prepared(&bytes, &exact, &|| Ok(())).unwrap();
        assert_eq!(
            prepared(&bytes, &exact, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        let project = crate::pcnd::write_compact(&document(&held).unwrap()).unwrap();
        assert_eq!(
            crate::pcnd::read_with_metadata_admission_checked(
                &project,
                PIXELS,
                PIXELS,
                paintcraft_document::LAYER_COUNT_MAX,
                crate::pcnd::MetadataAdmission {
                    bytes: BYTES_MAX,
                    budget: &exact
                },
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        drop(held);
        assert_eq!(exact.usage().reserved, 0);
        drop(first);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn retentive_limit_is_exact_and_does_not_narrow_the_large_legacy_omission_route() {
    let bytes = own("rgb24-v124-profile-positive.bmp");
    let profile = original_profile(&bytes).len() as u64;
    for allowed in [191, profile + 191, profile + 192] {
        let budget = SharedBudget::new(1 << 20);
        let result = decode(
            &bytes,
            PIXELS,
            crate::pcnd::MetadataAdmission {
                bytes: allowed,
                budget: &budget,
            },
            &|| Ok(()),
        );
        assert_eq!(result.is_ok(), allowed == profile + 192);
        match result {
            Ok(value) => drop(value),
            Err(error) => assert_eq!(error.code, ErrorCode::ResourceLimit),
        }
        assert_eq!(budget.usage().reserved, 0);
    }
    let large = with_profile_size(BYTES_MAX as usize + 1);
    let legacy = super::super::super::decode_with_pixel_limit(&large, PIXELS).unwrap();
    assert_eq!(legacy.rgba, own("rgb24-v124-profile-positive.bmp.rgba"));
    assert!(legacy.losses.iter().any(|s| s == "icc-profile-ignored"));
    let budget = SharedBudget::new(BYTES_MAX * 2);
    assert_eq!(
        prepared(&large, &budget, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
}
