use super::*;
use crate::{INPUT_MAX, Outcome, classify};
use paintcraft_core::types::json;
use std::path::Path;

#[test]
fn every_reviewed_stock_seed_reaches_the_real_entrypoint_with_its_expected_outcome() {
    let manifest = json::parse(include_bytes!("../../stock-seeds.json")).unwrap();
    let rows = manifest.get("seeds").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 29);
    for row in rows {
        let target = Target::parse(row.get("target").unwrap().as_str().unwrap()).unwrap();
        let source = row.get("source").unwrap().as_str().unwrap();
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(source),
        )
        .unwrap();
        let result = crate::exercise(target, &bytes);
        let detail = format!("{result:?}");
        assert_eq!(
            classify(result),
            if row.get("accepted").unwrap().as_bool().unwrap() {
                Outcome::Accepted
            } else {
                Outcome::Rejected
            },
            "{source}: {detail}"
        );
    }
}

#[test]
fn stock_targets_refuse_oversized_and_malformed_inputs_without_dispatching_elsewhere() {
    for target in [Target::Raster, Target::TiffPages, Target::Icons] {
        assert_eq!(
            crate::exercise(target, &vec![0; INPUT_MAX + 1])
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert!(crate::exercise(target, &[]).is_err());
    }
    let source = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/raster-metadata-fixtures/indexed1-trns.png"
    );
    assert!(
        super::exercise(Target::Raster, source, &|| Err(Error::new(
            ErrorCode::ResourceLimit,
            "callback control"
        )))
        .is_err()
    );
    let tiff = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/tiff-fixtures/unsupported-first-rgba16.tif"
    );
    assert_eq!(
        super::exercise(Target::TiffPages, tiff, &|| Err(Error::new(
            ErrorCode::ResourceLimit,
            "callback control"
        )))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn real_small_encoded_raster_and_icon_sources_cannot_bypass_pixel_allowance() {
    let png = image::encode(
        image::ImageFormat::Png,
        1,
        PIXELS_MAX as u32 + 1,
        &vec![255; (PIXELS_MAX as usize + 1) * 4],
        90,
    )
    .unwrap();
    assert!(png.len() < INPUT_MAX);
    assert_eq!(
        crate::exercise(Target::Raster, &png).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let fixture = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/bmp-indexed-fixtures/rgb1-bottom.ico"
    );
    let mut icon = fixture[..70].to_vec(); // directory22 + DIB40 + twoRGBQUADs
    icon[6] = 255;
    icon[7] = 255;
    icon[26..30].copy_from_slice(&255u32.to_le_bytes());
    icon[30..34].copy_from_slice(&510u32.to_le_bytes());
    let storage = 32 * 255;
    icon[42..46].copy_from_slice(&(storage as u32).to_le_bytes());
    icon.resize(70 + storage * 2, 0);
    let payload = (icon.len() - 22) as u32;
    icon[14..18].copy_from_slice(&payload.to_le_bytes());
    assert!(icons::inspect(&icon).is_ok());
    assert_eq!(
        crate::exercise(Target::Icons, &icon).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn tiff_orientation6_is_normalized_exactly_and_invalid9_and_old_wire_still_refuse() {
    let source = include_bytes!("../../fixtures/orientation6.tif");
    // Independent libtiff fixture oracle is3x2, row-major A B C / D E F.
    // EXIF6 gives D A / E B / F C, preserving straight hidden RGB and alpha.
    let oracle = json::parse(include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/tiff-fixtures/oracle.json"
    ))
    .unwrap();
    let row = oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("file").unwrap().as_str() == Some("icc-srgb-rgba8.tif"))
        .unwrap();
    let original: Vec<_> = row.get("pages").unwrap().as_array().unwrap()[0]
        .get("expected_rgba8")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u8::try_from(v.as_i64().unwrap()).unwrap())
        .collect();
    let expected: Vec<_> = [3usize, 0, 4, 1, 5, 2]
        .into_iter()
        .flat_map(|i| original[i * 4..i * 4 + 4].iter().copied())
        .collect();
    let ordinary = image::decode_with_pixel_limit(source, PIXELS_MAX).unwrap();
    let selected = tiff_pages::decode_page(source, 0, PIXELS_MAX, &|| Ok(())).unwrap();
    for (width, height, rgba) in [
        (ordinary.width, ordinary.height, &ordinary.rgba),
        (selected.width, selected.height, &selected.rgba),
    ] {
        assert_eq!((width, height), (2, 3));
        assert_eq!(rgba, &expected);
    }
    let mut old_report = selected.assessment.encode();
    old_report[8..10].copy_from_slice(&1u16.to_be_bytes());
    assert!(tiff_pages::Assessment::decode(&old_report).is_err());

    // Build a valid current inventory header, verify it against actual source,
    // then change ONLY its framing magic/version back to the previous ABI.
    let mut request = [0; tiff_pages::browser_wire::REQUEST_HEADER];
    request[..8].copy_from_slice(b"PCTIF002");
    request[8..10].copy_from_slice(&2u16.to_be_bytes());
    request[10] = 1;
    request[12..14].copy_from_slice(&u16::MAX.to_be_bytes());
    request[16..24].copy_from_slice(&(source.len() as u64).to_be_bytes());
    request[64] = 3;
    request[81..113].copy_from_slice(&selected.assessment.source_sha256());
    request[145..149].copy_from_slice(&(PIXELS_MAX as u32).to_be_bytes());
    tiff_pages::browser_wire::Header::decode(&request)
        .unwrap()
        .verify_source(source)
        .unwrap();
    request[..8].copy_from_slice(b"PCTIF001");
    request[8..10].copy_from_slice(&1u16.to_be_bytes());
    assert!(tiff_pages::browser_wire::Header::decode(&request).is_err());
    let invalid = include_bytes!("../../fixtures/invalid-orientation9.tif");
    for target in [Target::Raster, Target::TiffPages] {
        assert_eq!(
            classify(crate::exercise(target, invalid)),
            Outcome::Rejected
        );
    }
}
