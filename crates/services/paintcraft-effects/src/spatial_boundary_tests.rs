//! Point APIs must never silently approximate a spatial descriptor.
use super::*;
use paintcraft_core::types::filters::BoxBlurV1;

#[test]
fn every_box_radius_refuses_pixel_and_private_point_entrypoints() {
    let pixel = Rgba([31, 93, 251, 0]);
    for radius in 0..=32 {
        let spatial = Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap());
        for filters in [
            vec![spatial],
            vec![Descriptor::InvertRgbV1, spatial],
            vec![spatial, Descriptor::SolarizeRgbV1],
        ] {
            assert_eq!(
                evaluate_pixel(&filters, pixel).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
        }
        assert_eq!(
            point::evaluate(spatial, pixel).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(pixel, Rgba([31, 93, 251, 0]));
}

#[test]
fn full_row_descriptor_validation_precedes_callback_and_any_output_writes() {
    let source = [Rgba([3, 19, 71, 0]); 130];
    let sentinel = Rgba([251, 252, 253, 254]);
    for radius in [0, 1, 32] {
        for position in [0, 1, 4096] {
            let mut filters = vec![Descriptor::InvertRgbV1; 4096];
            filters.insert(
                position,
                Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap()),
            );
            let mut output = [sentinel; 130];
            let error = evaluate_row(&filters, &source, &mut output, &|| {
                panic!("Descriptor refusal precedes callbacks")
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
            assert_eq!(output, [sentinel; 130]);
            assert_eq!(source, [Rgba([3, 19, 71, 0]); 130]);
            assert_eq!(
                evaluate_row(&filters, &[], &mut [], &|| panic!(
                    "Empty rows still validate spatial support"
                ))
                .unwrap_err()
                .code,
                ErrorCode::CapabilityUnsupported
            );
        }
    }
    let spatial = [Descriptor::BoxBlurV1(BoxBlurV1::default())];
    let mut wrong_length = [sentinel];
    assert_eq!(
        evaluate_row(&spatial, &source, &mut wrong_length, &|| panic!(
            "Length refusal precedes work"
        ))
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    assert_eq!(wrong_length, [sentinel]);
}
