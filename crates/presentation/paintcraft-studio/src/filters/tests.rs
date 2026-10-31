use super::*;
use paintcraft_ui::i18n::{Language, translate};

#[test]
fn exposure_controls_retain_signed_hundredths_and_have_seven_translations() {
    assert_eq!(
        Parameters::new(Kind::Exposure).descriptor().unwrap(),
        Descriptor::ExposureSrgbV1(ExposureSrgbV1::default())
    );
    for stops in [-800, -799, -1, 0, 1, 799, 800] {
        let descriptor = Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(stops).unwrap());
        let draft = Parameters::from_descriptor(descriptor);
        assert_eq!(draft.kind, Kind::Exposure);
        assert_eq!(draft.stops_hundredths, stops);
        assert_eq!(draft.descriptor().unwrap(), descriptor);
    }
    for invalid in [-801, 801] {
        let mut draft = Parameters::new(Kind::Exposure);
        draft.stops_hundredths = invalid;
        assert_eq!(
            draft.descriptor().unwrap_err().code,
            paintcraft_core::types::ErrorCode::InputSchema
        );
    }
    for language in Language::ALL {
        for source in ["Exposure", "Exposure (EV)"] {
            let translated = translate(language, source);
            assert!(!translated.trim().is_empty());
            if language != Language::English {
                assert_ne!(translated, source, "{language:?}: {source}");
            }
        }
    }
}

#[test]
fn complete_levels_controls_roundtrip_gamma_and_equal_or_inverted_outputs() {
    assert_eq!(translate(Language::English, Kind::Levels.label()), "Levels");
    assert_eq!(
        paintcraft_ui::i18n::format_message(Language::Japanese, Kind::Levels.label(), &[]),
        "レベル補正"
    );
    assert_eq!(
        Parameters::new(Kind::Levels).descriptor().unwrap(),
        Descriptor::LevelsRgbV1(LevelsRgbV1::default())
    );
    for fields in [
        [17, 201, 999, 240, 16],
        [0, 255, 10, 73, 73],
        [127, 128, 333, 0, 255],
    ] {
        let descriptor = Descriptor::LevelsRgbV1(
            LevelsRgbV1::new(fields[0], fields[1], fields[2], fields[3], fields[4]).unwrap(),
        );
        let draft = Parameters::from_descriptor(descriptor);
        assert_eq!(draft.kind, Kind::Levels);
        assert_eq!(
            [
                draft.input_black,
                draft.input_white,
                draft.gamma_hundredths,
                draft.output_black,
                draft.output_white
            ],
            fields
        );
        assert_eq!(draft.descriptor().unwrap(), descriptor);
    }
    for language in Language::ALL {
        for source in [
            "Input black",
            "Input white",
            "Gamma",
            "Output black",
            "Output white",
        ] {
            assert!(!translate(language, source).trim().is_empty());
        }
    }
}

#[test]
fn new_parameter_defaults_bounds_and_descriptor_roundtrip_are_exact() {
    let defaults = [
        (
            Kind::BlackWhite,
            Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(100).unwrap()),
        ),
        (
            Kind::Threshold,
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(128).unwrap()),
        ),
        (
            Kind::Posterize,
            Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(6).unwrap()),
        ),
        (Kind::Solarize, Descriptor::SolarizeRgbV1),
    ];
    for (kind, descriptor) in defaults {
        let params = Parameters::new(kind);
        assert!(params.preview);
        assert_eq!(params.descriptor().unwrap(), descriptor);
        assert_eq!(Parameters::from_descriptor(descriptor), params);
        assert_eq!(Kind::of(descriptor), kind);
    }
    for strength in [0, 37, 100] {
        let mut p = Parameters::new(Kind::BlackWhite);
        p.strength = strength;
        assert_eq!(Parameters::from_descriptor(p.descriptor().unwrap()), p);
    }
    for threshold in [0, 127, 128, 255] {
        let mut p = Parameters::new(Kind::Threshold);
        p.threshold = threshold;
        assert_eq!(Parameters::from_descriptor(p.descriptor().unwrap()), p);
    }
    for levels in [2, 3, 6, 32] {
        let mut p = Parameters::new(Kind::Posterize);
        p.levels = levels;
        assert_eq!(Parameters::from_descriptor(p.descriptor().unwrap()), p);
    }
    for (kind, invalid) in [
        (Kind::BlackWhite, -1),
        (Kind::BlackWhite, 101),
        (Kind::Threshold, -1),
        (Kind::Threshold, 256),
        (Kind::Posterize, 1),
        (Kind::Posterize, 33),
    ] {
        let mut p = Parameters::new(kind);
        match kind {
            Kind::BlackWhite => p.strength = invalid,
            Kind::Threshold => p.threshold = invalid,
            Kind::Posterize => p.levels = invalid,
            _ => unreachable!(),
        }
        assert!(p.descriptor().is_err());
    }
    assert_eq!(
        Parameters::new(Kind::Invert).descriptor().unwrap(),
        Descriptor::InvertRgbV1
    );
    let legacy = Parameters::new(Kind::BrightnessContrast);
    assert_eq!(
        (legacy.brightness, legacy.contrast, legacy.preview),
        (0, 0, true)
    );
}

#[test]
fn new_filter_labels_are_present_in_all_seven_locales() {
    for language in Language::ALL {
        for source in [
            "Black & White",
            "Threshold",
            "Posterize",
            "Solarize",
            "Strength (%)",
            "Levels",
        ] {
            let translated = translate(language, source);
            assert!(!translated.trim().is_empty());
            if language != Language::English {
                assert_ne!(translated, source, "{language:?}: {source}");
            }
        }
    }
}

#[test]
fn box_parameters_are_real_typed_spatial_values_with_closed_radius_bounds() {
    let default = Parameters::new(Kind::BoxBlur);
    assert_eq!(default.radius, 2);
    assert!(default.preview);
    assert_eq!(
        default.descriptor().unwrap(),
        Descriptor::BoxBlurV1(BoxBlurV1::default())
    );
    assert_eq!(
        Kind::ALL
            .iter()
            .filter(|&&kind| kind == Kind::BoxBlur)
            .count(),
        1
    );
    for radius in 0..=32 {
        let descriptor = Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap());
        let draft = Parameters::from_descriptor(descriptor);
        assert_eq!(draft.kind, Kind::BoxBlur);
        assert_eq!(draft.radius, radius);
        assert_eq!(draft.descriptor().unwrap(), descriptor);
        assert!(descriptor.is_spatial());
    }
    for radius in [i16::MIN, -1, 33, i16::MAX] {
        let mut invalid = default;
        invalid.radius = radius;
        assert_eq!(
            invalid.descriptor().unwrap_err().code,
            paintcraft_core::types::ErrorCode::InputSchema
        );
    }
}

#[test]
fn box_and_radius_controls_have_all_seven_real_translations() {
    for language in Language::ALL {
        for source in ["Box Blur", "Radius (px)"] {
            let translated = translate(language, source);
            assert!(!translated.trim().is_empty());
            if language != Language::English {
                assert_ne!(translated, source, "{language:?}: {source}");
            }
        }
    }
}

#[test]
fn curves_controls_keep_fixed_identity_and_all_free_output_vertices() {
    let default = Parameters::new(Kind::Curves);
    assert!(default.preview);
    assert_eq!(default.curve_outputs, [0, 64, 128, 192, 255]);
    assert_eq!(
        default.descriptor().unwrap(),
        Descriptor::CurvesRgbV1(CurvesRgbV1::default())
    );
    assert_eq!(Kind::ALL.last(), Some(&Kind::Curves));
    assert_eq!(
        Kind::ALL
            .iter()
            .filter(|&&kind| kind == Kind::Curves)
            .count(),
        1
    );
    for vertex in 0u8..32 {
        let outputs = std::array::from_fn(|i| if vertex & (1 << i) == 0 { 0 } else { 255 });
        let descriptor = Descriptor::CurvesRgbV1(CurvesRgbV1::new(outputs));
        let draft = Parameters::from_descriptor(descriptor);
        assert_eq!(draft.kind, Kind::Curves);
        assert_eq!(draft.curve_outputs, outputs);
        assert_eq!(draft.descriptor().unwrap(), descriptor);
        assert!(!descriptor.is_spatial());
    }
    for outputs in [
        [255, 191, 127, 63, 0],
        [23, 9, 240, 73, 201],
        [37; 5],
        [0, 128, 128, 128, 255],
    ] {
        let mut draft = default;
        draft.curve_outputs = outputs;
        let descriptor = draft.descriptor().unwrap();
        assert_eq!(
            descriptor,
            Descriptor::CurvesRgbV1(CurvesRgbV1::new(outputs))
        );
        assert_eq!(Parameters::from_descriptor(descriptor), draft);
    }
    // Each input position remains independent: changing an output must not
    // sort neighboring values, lock endpoints or silently constrain the curve.
    for index in 0..5 {
        for output in [0, 1, 127, 128, 254, 255] {
            let mut draft = default;
            draft.curve_outputs[index] = output;
            let Descriptor::CurvesRgbV1(actual) = draft.descriptor().unwrap() else {
                panic!("Curves descriptor");
            };
            assert_eq!(actual.outputs(), draft.curve_outputs);
        }
    }
}

#[test]
fn curves_title_and_five_fixed_input_captions_have_seven_real_translations() {
    let captions = [
        "Curves",
        "Black (input 0)",
        "Shadows (input 64)",
        "Midtones (input 128)",
        "Highlights (input 192)",
        "White (input 255)",
    ];
    assert_eq!(Kind::Curves.label(), captions[0]);
    for language in Language::ALL {
        let mut translated = std::collections::BTreeSet::new();
        for source in captions {
            let label = translate(language, source);
            assert!(!label.trim().is_empty(), "{language:?}: {source}");
            if language != Language::English {
                assert_ne!(label, source, "{language:?}: {source}");
            }
            assert!(
                translated.insert(label),
                "distinct controls need distinct captions"
            );
        }
    }
}
