use super::*;
use paintcraft_ui::i18n::{Language, translate};

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
