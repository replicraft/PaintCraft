//! A catalog descriptor never authorizes hidden per-pixel LUT compilation.
use super::*;
use paintcraft_core::types::filters::{ExposureSrgbV1, LevelsRgbV1};
#[test]
fn standalone_point_facade_refuses_levels_before_row_callbacks_or_writes() {
    assert_prepared_required(Descriptor::LevelsRgbV1(LevelsRgbV1::default()));
}
#[test]
fn standalone_point_facade_refuses_exposure_even_at_identity_before_row_callbacks_or_writes() {
    for ev in [0, -800, 100, 800] {
        assert_prepared_required(Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(ev).unwrap()));
    }
}
fn assert_prepared_required(descriptor: Descriptor) {
    let source = [Rgba([1, 4, 239, 0]), Rgba([4, 1, 31, 255])];
    let before = source;
    let mut out = [Rgba([7, 8, 9, 10]); 2];
    assert_eq!(
        evaluate_pixel(&[descriptor], source[0]).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        evaluate_row(
            &[Descriptor::InvertRgbV1, descriptor],
            &source,
            &mut out,
            &|| panic!("unprepared descriptor called back")
        )
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(out, [Rgba([7, 8, 9, 10]); 2]);
    assert_eq!(source, before);
}

#[test]
fn standalone_point_facade_refuses_curves_even_identity_before_row_callbacks_or_writes() {
    use paintcraft_core::types::filters::CurvesRgbV1;
    for outputs in [
        [0, 64, 128, 192, 255],
        [0; 5],
        [255, 0, 255, 0, 255],
        [23, 9, 240, 73, 201],
    ] {
        let descriptor = Descriptor::CurvesRgbV1(CurvesRgbV1::new(outputs));
        assert_prepared_required(descriptor);
        assert_eq!(
            evaluate_pixel(&[descriptor], Rgba([64, 16, 128, 0]))
                .unwrap_err()
                .message,
            "Curves filter requires its admitted prepared program"
        );
    }
}
