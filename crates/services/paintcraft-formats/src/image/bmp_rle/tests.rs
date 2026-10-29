use super::*;
use paintcraft_core::types::json::{self, Value};
use std::{cell::Cell, path::Path};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/bmp-rle-fixtures")
            .join(name),
    )
    .unwrap()
}
fn number(row: &Value, key: &str) -> u64 {
    row.get(key).unwrap().as_i64().unwrap() as u64
}
#[test]
fn independent_ffmpeg_rle4_and_rle8_pixels_holes_and_truthful_storage_facts_match() {
    let manifest = json::parse(&fixture("oracle.json")).unwrap();
    let mut count = 0;
    for row in manifest.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        let source = super::super::bmp_metadata::inspect(&bytes, 1_048_576, &|| Ok(()))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            source.row_stride, None,
            "compressed storage has no physical row stride"
        );
        assert_eq!(
            source.declared_image_bytes as u64,
            number(row, "image_size")
        );
        assert_eq!(source.rle_stream_bytes, Some(number(row, "stream_bytes")));
        assert_eq!(
            source.pixel_data_bytes,
            if source.declared_image_bytes == 0 {
                number(row, "stream_bytes")
            } else {
                number(row, "image_size")
            }
        );
        assert!(source.palette.alpha.is_empty());
        assert_eq!(source.palette_reserved[0], 137);
        let image = crate::image::decode(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        let expected: Vec<u8> = row
            .get("expected_rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|n| u8::try_from(n.as_i64().unwrap()).unwrap())
            .collect();
        assert_eq!(
            (image.width as u64, image.height as u64),
            (number(row, "width"), number(row, "height")),
            "{name}"
        );
        assert_eq!(image.rgba, expected, "{name}");
        assert!(
            image.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
            "palette reserved bytes and skipped areas are not alpha"
        );
        let mut losses = vec!["palette-indices-expanded"];
        if name.contains("-icc-") {
            losses.extend(["icc-profile-ignored", "bmp-color-metadata-ignored"]);
            assert!(source.icc.is_some());
        }
        if name.ends_with("-padding.bmp") {
            losses.push("resolution-metadata-ignored");
            assert_eq!(source.resolution_raw, [-3780, 0]);
            assert!(source.resolution.is_none());
        }
        if name.contains("size-zero") {
            losses.push("bmp-metadata-ignored");
        }
        assert_eq!(image.losses, losses, "{name}");
        count += 1;
    }
    assert_eq!(count, 26);
}
#[test]
fn output_allocation_overflow_is_a_typed_refusal() {
    assert_eq!(buffer(u64::MAX).unwrap_err().code, ErrorCode::ResourceLimit);
}
#[test]
fn malformed_runs_indices_deltas_and_storage_never_return_a_raster() {
    for name in ["rle8-combined.bmp", "rle4-combined.bmp"] {
        let bytes = fixture(name);
        let at = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        let mut topdown = bytes.clone();
        topdown[22..26].copy_from_slice(&(-6i32).to_le_bytes());
        assert_eq!(
            crate::image::decode(&topdown).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        let mut overflow = bytes.clone();
        overflow[at] = 255;
        assert_eq!(
            crate::image::decode(&overflow).unwrap_err().code,
            ErrorCode::CodecFailed
        );
        let mut index = bytes.clone();
        index[at + 1] = 255;
        assert_eq!(
            crate::image::decode(&index).unwrap_err().code,
            ErrorCode::CodecFailed
        );
        let mut short = bytes.clone();
        short[34..38].copy_from_slice(&1u32.to_le_bytes());
        assert!(crate::image::decode(&short).is_err());
        let mut huge = bytes.clone();
        huge[34..38].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(crate::image::decode(&huge).is_err());
        for cut in at..bytes.len() {
            assert!(
                crate::image::decode(&bytes[..cut]).is_err(),
                "{name} cut {cut}"
            );
        }
        assert_eq!(
            crate::image::decode_with_pixel_limit(&bytes, 65)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        let mut pad = fixture(if name.starts_with("rle8") {
            "rle8-padding.bmp"
        } else {
            "rle4-padding.bmp"
        });
        *pad.last_mut().unwrap() = 1;
        assert!(crate::image::decode(&pad).is_err());
    }
}
#[test]
fn scanner_cancellation_is_observed_without_an_authored_output() {
    let bytes = fixture("rle8-combined.bmp");
    let calls = Cell::new(0);
    let result = decode(&bytes, 66, &|| {
        calls.set(calls.get() + 1);
        if calls.get() > 5 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel RLE source",
            ))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
}

#[test]
fn delta_eol_padding_and_missing_eoi_are_source_checked() {
    for depth in [4, 8] {
        let bytes = fixture(&format!("rle{depth}-zero-delta.bmp"));
        let at = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        for (offset, value) in [(2, 4), (3, 1)] {
            let mut invalid = bytes.clone();
            invalid[at + offset] = value;
            assert_eq!(
                crate::image::decode(&invalid).unwrap_err().code,
                ErrorCode::CodecFailed
            );
        }
        let mut extra_eol = fixture(&format!("rle{depth}-final-eol.bmp"));
        let end = extra_eol.len();
        extra_eol[end - 1] = 0;
        assert_eq!(
            crate::image::decode(&extra_eol).unwrap_err().code,
            ErrorCode::CodecFailed
        );
        let mut missing = bytes.clone();
        missing.truncate(missing.len() - 2);
        let size = missing.len() as u32;
        missing[2..6].copy_from_slice(&size.to_le_bytes());
        missing[34..38].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(
            crate::image::decode(&missing).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    let mut pad = fixture("rle8-combined.bmp");
    let at = u32::from_le_bytes(pad[10..14].try_into().unwrap()) as usize;
    pad[at + 9] = 1;
    assert_eq!(
        crate::image::decode(&pad).unwrap_err().code,
        ErrorCode::CodecFailed
    );
}
#[test]
fn icc_bounds_budget_and_eoi_do_not_borrow_profile_bytes() {
    for depth in [4, 8] {
        let bytes = fixture(&format!("rle{depth}-icc-after-size-zero.bmp"));
        assert_eq!(
            super::super::bmp_metadata::inspect_with_metadata_limit(&bytes, 66, 1, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        let pixel = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        let profile = 14 + u32::from_le_bytes(bytes[126..130].try_into().unwrap()) as usize;
        // Remove EOI semantically without changing the ICC source extent.
        let mut missing = bytes.clone();
        missing[profile - 1] = 0;
        assert_eq!(
            crate::image::decode(&missing).unwrap_err().code,
            ErrorCode::CodecFailed
        );
        let mut overlap = bytes.clone();
        overlap[126..130].copy_from_slice(&((pixel - 14) as u32).to_le_bytes());
        assert!(crate::image::decode(&overlap).is_err());
        let mut declared = bytes.clone();
        declared[34..38].copy_from_slice(&((profile - pixel + 1) as u32).to_le_bytes());
        assert_eq!(
            crate::image::decode(&declared).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
}
#[test]
fn later_cancellation_never_exposes_a_private_raster() {
    let bytes = fixture("rle4-combined.bmp");
    let total = Cell::new(0);
    decode(&bytes, 66, &|| {
        total.set(total.get() + 1);
        Ok(())
    })
    .unwrap();
    let calls = Cell::new(0);
    let result = decode(&bytes, 66, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == total.get() {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel after decoding",
            ))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
}
