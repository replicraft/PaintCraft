use super::*;
use paintcraft_core::types::{Error, ErrorCode, json};
use std::{cell::Cell, path::Path};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("crates/services/paintcraft-formats/tests/bmp-retention-fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn independent_bmp_samples_signed_raw_facts_and_notices_reach_the_actual_raster_target() {
    let manifest = json::parse(&fixture("oracle.json")).unwrap();
    let rows = manifest.get("fixtures").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 13);
    for row in rows {
        let name = row.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        let legacy = image::decode_with_pixel_limit(&bytes, PIXELS_MAX).unwrap();
        assert_eq!(legacy.rgba, fixture(&format!("{name}.rgba")), "{name}");
        let original = bytes.clone();
        let budget = SharedBudget::new(METADATA_BYTES);
        with_budget(&bytes, &legacy, &budget, &|| Ok(())).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(budget.usage().reserved, 0);
        crate::exercise(crate::Target::Raster, &bytes).unwrap();
        assert_eq!(bytes, original);
        let inspected = image::bmp_metadata::inspect(&bytes, PIXELS_MAX, &|| Ok(())).unwrap();
        let notices =
            image::bmp_retention::required_acknowledgements(&inspected, legacy.losses.clone());
        assert_eq!(
            notices
                .iter()
                .any(|s| s == image::bmp_retention::ICC_RETAINED_UNAPPLIED),
            row.get("profile").unwrap().as_bool().unwrap()
        );
        let density = row.get("density").unwrap().as_array().unwrap();
        assert_eq!(
            notices
                .iter()
                .any(|s| s == image::bmp_retention::RESOLUTION_RETAINED_INACTIVE),
            density.iter().any(|v| v.as_i64() != Some(0))
        );
        assert_eq!(
            notices.iter().any(|s| s == "palette-indices-expanded"),
            row.get("bits_per_pixel").unwrap().as_i64().unwrap() <= 8
        );
        assert!(!notices.iter().any(|s| s == "depth-16-reduced-to-8"));
    }
}

#[test]
fn tiny_quota_and_cancel_at_every_reachable_checkpoint_release_capture() {
    for name in [
        "rgb24-v124-profile-positive.bmp",
        "rgb24-v124-profile-negative.bmp",
        "rgb24-v40-density-mixed-zero.bmp",
    ] {
        let bytes = fixture(name);
        let legacy = image::decode_with_pixel_limit(&bytes, PIXELS_MAX).unwrap();
        let budget = SharedBudget::new(METADATA_BYTES);
        let count = Cell::new(0);
        with_budget(&bytes, &legacy, &budget, &|| {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(count.get() > 10);
        for stop in 0..count.get() {
            let actual = Cell::new(0);
            let error = with_budget(&bytes, &legacy, &budget, &|| {
                let at = actual.get();
                actual.set(at + 1);
                if at == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "fuzz cancel"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(
                error.code,
                ErrorCode::OperationCancelled,
                "{name}: checkpoint{stop}"
            );
            assert_eq!(budget.usage().reserved, 0);
        }
        let denied = SharedBudget::new(1);
        assert_eq!(
            with_budget(&bytes, &legacy, &denied, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(denied.usage().reserved, 0);
    }
    let bytes = fixture("rgb24-v40-no-metadata.bmp");
    let legacy = image::decode_with_pixel_limit(&bytes, PIXELS_MAX).unwrap();
    let none = SharedBudget::new(0);
    with_budget(&bytes, &legacy, &none, &|| Ok(())).unwrap();
    assert_eq!(none.usage().peak, 0);
}

#[test]
fn malformed_or_late_pixel_refusals_and_harness_only_profile_quota_do_not_leak() {
    let original = fixture("rgb24-v124-profile-positive.bmp");
    let legacy = image::decode_with_pixel_limit(&original, PIXELS_MAX).unwrap();
    let budget = SharedBudget::new(METADATA_BYTES);
    for (at, value) in [
        (70, image::bmp_metadata::PROFILE_LINKED),
        (126, 0),
        (130, u32::MAX),
    ] {
        let mut bytes = original.clone();
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert!(with_budget(&bytes, &legacy, &budget, &|| Ok(())).is_err());
        assert_eq!(budget.usage().reserved, 0);
    }
    let mut bad = fixture("indexed8-v124-profile-reserved.bmp");
    let reference = image::decode_with_pixel_limit(&bad, PIXELS_MAX).unwrap();
    let offset = u32::from_le_bytes(bad[10..14].try_into().unwrap()) as usize;
    bad[offset] = 2;
    assert!(with_budget(&bad, &reference, &budget, &|| Ok(())).is_err());
    assert!(
        budget.usage().peak > 0,
        "late refusal actually acquired private ICC"
    );
    assert_eq!(budget.usage().reserved, 0);
    let start = 14 + u32::from_le_bytes(original[126..130].try_into().unwrap()) as usize;
    let mut large = original[..start].to_vec();
    let mut profile = original[start..].to_vec();
    profile.resize(32 << 10, 0);
    let count = profile.len() as u32;
    profile[..4].copy_from_slice(&count.to_be_bytes());
    large.extend(profile);
    let length = large.len() as u32;
    large[2..6].copy_from_slice(&length.to_le_bytes());
    large[130..134].copy_from_slice(&count.to_le_bytes());
    assert!(large.len() < crate::INPUT_MAX);
    let legacy = image::decode_with_pixel_limit(&large, PIXELS_MAX).unwrap();
    assert_eq!(legacy.rgba, fixture("rgb24-v124-profile-positive.bmp.rgba"));
    assert_eq!(
        with_budget(&large, &legacy, &budget, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(
        crate::exercise(crate::Target::Raster, &large)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
