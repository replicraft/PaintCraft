use super::*;
use std::{cell::Cell, path::Path};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(name),
    )
    .unwrap()
}
#[test]
fn original_profiles_propose_current_losses_without_decoding_pixels() {
    let png = fixture("raster-metadata-fixtures/indexed4-trns.png");
    if !png.is_empty() {
        assert_eq!(
            inspect(&png, Limits::default(), &|| Ok(()))
                .unwrap()
                .format(),
            "png"
        );
    }
    let bmp = fixture("bmp-rle-fixtures/rle8-padding.bmp");
    let Source::Bmp(source) = inspect(&bmp, Limits::default(), &|| Ok(())).unwrap() else {
        panic!()
    };
    assert_eq!(source.row_stride, None);
    assert_eq!(source.resolution_raw, [-3780, 0]);
    assert!(source.resolution.is_none());
    let jpeg = fixture("jpeg-fixtures/unmarked-cmyk-orientation-6-II.jpg");
    let inspected = inspect(&jpeg, Limits::default(), &|| Ok(())).unwrap();
    assert_eq!(inspected.dimensions(), (24, 16));
    assert_eq!(inspected.proposed_dimensions(), (16, 24));
    assert!(
        inspected
            .proposed_losses()
            .contains(&"jpeg-color-model-assumed".to_string())
    );
}
#[test]
fn lower_limits_unknown_formats_and_every_cancellation_checkpoint_refuse() {
    let bytes = fixture("bmp-rle-fixtures/rle4-combined.bmp");
    for limits in [
        Limits {
            input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            pixels: 1,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            inspect(&bytes, limits, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    assert_eq!(
        inspect(b"II*\0", Limits::default(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let calls = Cell::new(0);
    inspect(&bytes, Limits::default(), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let at = Cell::new(0);
        assert_eq!(
            inspect(&bytes, Limits::default(), &|| {
                at.set(at.get() + 1);
                if at.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel inspector",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
}
#[test]
fn known_header_losses_match_actual_conversion_and_metadata_limits_only_lower_defaults() {
    for name in [
        "raster-metadata-fixtures/indexed4-trns.png",
        "raster-metadata-fixtures/rgba16-high-byte.png",
        "raster-metadata-fixtures/rgba8-icc-precedence.png",
        "raster-metadata-fixtures/rgba8-exif-orientation.png",
        "bmp-rle-fixtures/rle8-size-zero.bmp",
        "bmp-rle-fixtures/rle4-icc-before.bmp",
        "raster-metadata-fixtures/rgb24-top-down.bmp",
        "bmp16-fixtures/masks565-v40-bottom.bmp",
        "bmp16-fixtures/rgb555-v40-top.bmp",
        "jpeg-fixtures/unmarked-cmyk-orientation-6-II.jpg",
        "jpeg-fixtures/adobe-ycck-color.jpg",
    ] {
        let bytes = fixture(name);
        let source = inspect(&bytes, Limits::default(), &|| Ok(())).unwrap();
        let actual = if let Source::Png(descriptor) = &source {
            let budget = paintcraft_core::types::resources::SharedBudget::new(
                paintcraft_document::source_metadata::BYTES_MAX,
            );
            let retained = image::png_retention::decode(
                &bytes,
                image::DECODE_PIXELS_MAX,
                crate::pcnd::MetadataAdmission {
                    bytes: paintcraft_document::source_metadata::BYTES_MAX,
                    budget: &budget,
                },
                &|| Ok(()),
            )
            .unwrap();
            // The explicit retained workflow changes acknowledgements, not the
            // original declaration reader or legacy pixel-only decode route.
            let legacy = image::decode(&bytes).unwrap();
            assert_eq!(legacy.rgba, retained.image.rgba, "{name}");
            assert_eq!(
                legacy.losses.iter().any(|s| s == "icc-profile-ignored"),
                descriptor.icc.is_some()
            );
            retained.image
        } else if let Source::Bmp(descriptor) = &source {
            let budget = paintcraft_core::types::resources::SharedBudget::new(
                paintcraft_document::source_metadata::BYTES_MAX,
            );
            let retained = image::bmp_retention::decode(
                &bytes,
                image::DECODE_PIXELS_MAX,
                crate::pcnd::MetadataAdmission {
                    bytes: paintcraft_document::source_metadata::BYTES_MAX,
                    budget: &budget,
                },
                &|| Ok(()),
            )
            .unwrap();
            let legacy = image::decode(&bytes).unwrap();
            assert_eq!(legacy.rgba, retained.image.rgba, "{name}");
            assert_eq!(
                legacy.losses.iter().any(|s| s == "icc-profile-ignored"),
                descriptor.icc.is_some()
            );
            retained.image
        } else {
            image::decode(&bytes).unwrap()
        };
        assert_eq!(
            source.proposed_dimensions(),
            (actual.width, actual.height),
            "{name}"
        );
        assert_eq!(source.proposed_losses(), actual.losses, "{name}");
        assert!(
            Limits {
                metadata_bytes: Some(u64::MAX),
                ..Limits::default()
            }
            .effective_metadata_limit(&source)
                <= INPUT_BYTES_MAX.max(image::DECODE_PIXELS_MAX * 8)
        );
    }
    for name in [
        "raster-metadata-fixtures/rgba8-icc-precedence.png",
        "bmp-rle-fixtures/rle4-icc-before.bmp",
        "jpeg-fixtures/rgb-orientation-6-II.jpg",
    ] {
        let bytes = fixture(name);
        assert_eq!(
            inspect(
                &bytes,
                Limits {
                    metadata_bytes: Some(1),
                    ..Limits::default()
                },
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit,
            "{name}"
        );
    }
}

#[test]
fn bmp_signed_density_and_original_profile_proposals_are_not_pixel_validity_or_authority() {
    let manifest = fixture("bmp-retention-fixtures/oracle.json");
    for row in paintcraft_core::types::json::parse(&manifest)
        .unwrap()
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
    {
        let name = row.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(&format!("bmp-retention-fixtures/{name}"));
        let inspected = inspect(&bytes, Limits::default(), &|| Ok(())).unwrap();
        let budget = paintcraft_core::types::resources::SharedBudget::new(
            paintcraft_document::source_metadata::BYTES_MAX,
        );
        let actual = image::bmp_retention::decode(
            &bytes,
            image::DECODE_PIXELS_MAX,
            crate::pcnd::MetadataAdmission {
                bytes: paintcraft_document::source_metadata::BYTES_MAX,
                budget: &budget,
            },
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(inspected.proposed_losses(), actual.image.losses, "{name}");
    }
    let mut bytes = fixture("bmp-retention-fixtures/indexed8-v124-profile-reserved.bmp");
    let at = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
    bytes[at] = 2;
    let source = inspect(&bytes, Limits::default(), &|| Ok(())).unwrap();
    assert!(
        source
            .proposed_losses()
            .iter()
            .any(|s| s == image::bmp_retention::ICC_RETAINED_UNAPPLIED)
    );
    let budget = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
    assert!(
        image::bmp_retention::decode(
            &bytes,
            image::DECODE_PIXELS_MAX,
            crate::pcnd::MetadataAdmission {
                bytes: paintcraft_document::source_metadata::BYTES_MAX,
                budget: &budget
            },
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn source_inspection_is_not_a_pixel_validity_or_import_authorization_proof() {
    let mut bytes = fixture("raster-metadata-fixtures/indexed4-trns.png");
    let mut at = 8;
    loop {
        let n = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        if &bytes[at + 4..at + 8] == b"IDAT" {
            bytes[at + 8..at + 8 + n].fill(0);
            let crc = crc32fast::hash(&bytes[at + 4..at + 8 + n]);
            bytes[at + 8 + n..at + 12 + n].copy_from_slice(&crc.to_be_bytes());
            break;
        }
        at += 12 + n;
    }
    assert!(
        inspect(&bytes, Limits::default(), &|| Ok(())).is_ok(),
        "bounded source facts need no entropy decode"
    );
    assert!(
        image::decode(&bytes).is_err(),
        "actual import still validates samples/entropy"
    );
}
