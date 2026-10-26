//! Narrow helper profile, backed by independent checked-in cjxl fixtures.
//! These calls use trusted owned fixtures; hostile input belongs in the helper.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::jxl::{self, Samples};
const RGBA: &[u8] = include_bytes!("jxl-fixtures/rgba8.jxl");
#[test]
fn independent_rgb8_and_single_pixel_fixture_samples_are_exact() {
    let expected = [
        10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254, 160,
        170, 180, 255,
    ];
    let decoded = jxl::decode_worker_rgba8_checked(RGBA, 6, &|| Ok(())).unwrap();
    assert_eq!((decoded.width, decoded.height), (3, 2));
    assert_eq!(decoded.samples, Samples::Rgba8(expected.to_vec()));
    assert!(decoded.losses.is_empty());
    let single =
        jxl::decode_worker_rgba8_checked(include_bytes!("jxl-fixtures/single.jxl"), 1, &|| Ok(()))
            .unwrap();
    assert_eq!(single.samples, Samples::Rgba8(vec![17, 29, 91, 137]));
}
#[test]
fn unsupported_precision_color_animation_and_metadata_refuse_without_conversion() {
    for bytes in [
        include_bytes!("jxl-fixtures/rgba16.jxl").as_slice(),
        include_bytes!("jxl-fixtures/icc.jxl"),
        include_bytes!("jxl-fixtures/linear.jxl"),
        include_bytes!("jxl-fixtures/animation.jxl"),
    ] {
        assert_eq!(
            jxl::decode_worker_rgba8_checked(bytes, jxl::PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let mut metadata = RGBA.to_vec();
    metadata.extend(9u32.to_be_bytes());
    metadata.extend(b"xml x");
    assert_eq!(
        jxl::decode_worker_rgba8_checked(&metadata, 6, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(metadata[..RGBA.len()], *RGBA);
}
#[test]
fn size_and_cancellation_guards_do_not_return_partial_pixels() {
    assert_eq!(
        jxl::decode_worker_rgba8_checked(RGBA, 5, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        jxl::decode_worker_rgba8_checked(RGBA, 6, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}
