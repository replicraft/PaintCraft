//! A catalog descriptor never authorizes hidden per-pixel LUT compilation.
use super::*;
use paintcraft_core::types::filters::LevelsRgbV1;
#[test]
fn standalone_point_facade_refuses_levels_before_row_callbacks_or_writes() {
    let descriptor = Descriptor::LevelsRgbV1(LevelsRgbV1::default());
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
