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
        let actual = image::decode(&bytes).unwrap();
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
