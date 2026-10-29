use super::*;
use paintcraft_core::types::json;

const FIXTURES: &[(&str, &[u8])] = &[
    (
        "rgba8-le-strips-none.tif",
        include_bytes!("../../tests/tiff-fixtures/rgba8-le-strips-none.tif"),
    ),
    (
        "rgba16-be-strips-lzw.tif",
        include_bytes!("../../tests/tiff-fixtures/rgba16-be-strips-lzw.tif"),
    ),
    (
        "rgba8-le-tiles-deflate.tif",
        include_bytes!("../../tests/tiff-fixtures/rgba8-le-tiles-deflate.tif"),
    ),
    (
        "unsupported-first-rgba16.tif",
        include_bytes!("../../tests/tiff-fixtures/unsupported-first-rgba16.tif"),
    ),
    (
        "associated-first-rgba8.tif",
        include_bytes!("../../tests/tiff-fixtures/associated-first-rgba8.tif"),
    ),
    (
        "gray-endian-packbits.tif",
        include_bytes!("../../tests/tiff-fixtures/gray-endian-packbits.tif"),
    ),
    (
        "gray1-le-fax4.tif",
        include_bytes!("../../tests/tiff-fixtures/gray1-le-fax4.tif"),
    ),
    (
        "bigtiff-unqualified.tif",
        include_bytes!("../../tests/tiff-fixtures/bigtiff-unqualified.tif"),
    ),
    (
        "float-first-rgba8.tif",
        include_bytes!("../../tests/tiff-fixtures/float-first-rgba8.tif"),
    ),
    (
        "icc-srgb-rgba8.tif",
        include_bytes!("../../tests/tiff-fixtures/icc-srgb-rgba8.tif"),
    ),
    (
        "jpeg-gray-rgb8.tif",
        include_bytes!("../../tests/tiff-fixtures/jpeg-gray-rgb8.tif"),
    ),
    (
        "gray-alpha-8-16-both-photos.tif",
        include_bytes!("../../tests/tiff-fixtures/gray-alpha-8-16-both-photos.tif"),
    ),
    (
        "icc-srgb-larger-rgba8.tif",
        include_bytes!("../../tests/tiff-fixtures/icc-srgb-larger-rgba8.tif"),
    ),
];
#[test]
fn libtiff_raw_sample_oracles_cover_endian_depth_alpha_strips_tiles_and_compressions() {
    let oracle = json::parse(include_bytes!("../../tests/tiff-fixtures/oracle.json")).unwrap();
    let fixtures = oracle.get("fixtures").unwrap().as_array().unwrap();
    for (name, bytes) in FIXTURES {
        let record = fixtures
            .iter()
            .find(|r| r.get("file").unwrap().as_str() == Some(name))
            .unwrap();
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(bytes)),
            record.get("sha256").unwrap().as_str().unwrap()
        );
        if record.get("whole_file_qualified").unwrap().as_bool() == Some(false) {
            assert_eq!(
                inspect(bytes, &|| Ok(())).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            continue;
        }
        let inventory = inspect(bytes, &|| Ok(())).unwrap();
        let expected = record.get("pages").unwrap().as_array().unwrap();
        assert_eq!(inventory.pages().len(), expected.len(), "{name}");
        for (index, page) in expected.iter().enumerate() {
            let selected = &inventory.pages()[index];
            if page.get("qualified_integer_profile").unwrap().as_bool() == Some(false) {
                assert!(selected.unsupported_reason.is_some(), "{name} {index}");
                assert_eq!(
                    decode_page(bytes, index as u16, PIXELS_MAX, &|| Ok(()))
                        .unwrap_err()
                        .code,
                    ErrorCode::CapabilityUnsupported
                );
                continue;
            }
            assert!(selected.unsupported_reason.is_none(), "{name} {index}");
            let decoded = decode_page(bytes, index as u16, PIXELS_MAX, &|| Ok(()))
                .unwrap_or_else(|error| {
                    let raw_icc=::tiff::decoder::Decoder::new(std::io::Cursor::new(bytes)).map(|mut decoder|decoder.get_tag_u8_vec(::tiff::tags::Tag::IccProfile).map(|v|v.len()));
                    panic!("{name} {index}: {error}; descriptor={:?}; ordinary_losses={:?}; raw_icc={raw_icc:?}",selected.descriptor,crate::image::decode(bytes).map(|image|image.losses))
                });
            let rgba: Vec<u8> = page
                .get("expected_rgba8")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| u8::try_from(v.as_i64().unwrap()).unwrap())
                .collect();
            assert_eq!(decoded.rgba, rgba, "{name} {index}");
            assert_eq!(
                decoded.assessment.source_depth(),
                page.get("bits").unwrap().as_i64().unwrap() as u32
            );
            assert_eq!(
                decoded.assessment.descriptor().compression,
                page.get("compression").unwrap().as_i64().unwrap() as u32
            );
            assert_eq!(
                decoded.assessment.descriptor().storage,
                if page.get("tiled").unwrap().as_bool().unwrap() {
                    2
                } else {
                    1
                }
            );
            let icc = page.get("icc_hex").unwrap().as_str().unwrap();
            assert_eq!(
                decoded.assessment.descriptor().icc_bytes as usize,
                icc.len() / 2
            );
            assert_eq!(
                decoded
                    .losses
                    .iter()
                    .any(|loss| loss == "icc-profile-ignored"),
                !icc.is_empty()
            );
            assert_eq!(
                paintcraft_document::hex(&decoded.assessment.output_sha256()),
                page.get("expected_rgba8_sha256").unwrap().as_str().unwrap()
            );
            assert_eq!(
                decoded
                    .losses
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                decoded.assessment.losses()
            );
            decoded
                .assessment
                .validate_result(
                    inventory.source_sha256(),
                    bytes.len() as u64,
                    index as u16,
                    decoded.width,
                    decoded.height,
                    &rgba,
                )
                .unwrap();
            assert_eq!(
                Assessment::decode(&decoded.assessment.encode()).unwrap(),
                decoded.assessment
            );
        }
    }
}

#[test]
fn ordinary_tiff_icc_consent_survives_optional_lookup_limits_without_duplicate_losses() {
    use ::image::ImageDecoder;
    for (bytes, lookup_succeeds) in [
        (
            include_bytes!("../../tests/tiff-fixtures/icc-srgb-rgba8.tif").as_slice(),
            false,
        ),
        (
            include_bytes!("../../tests/tiff-fixtures/icc-srgb-larger-rgba8.tif").as_slice(),
            true,
        ),
    ] {
        let mut facade =
            ::image::codecs::tiff::TiffDecoder::new(std::io::Cursor::new(bytes)).unwrap();
        let mut limits = ::image::Limits::default();
        limits.max_alloc = Some(PIXELS_MAX * 8);
        facade.set_limits(limits).unwrap();
        assert_eq!(facade.icc_profile().unwrap().is_some(), lookup_succeeds);
        let ordinary = crate::image::decode(bytes).unwrap();
        assert_eq!(
            ordinary.losses,
            ["tiff-metadata-ignored", "icc-profile-ignored"]
        );
        let selected = decode_page(bytes, 0, PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(selected.rgba, ordinary.rgba);
        assert_eq!(selected.losses, ordinary.losses);
        assert_eq!(selected.assessment.descriptor().icc_bytes, 588);
    }
}
