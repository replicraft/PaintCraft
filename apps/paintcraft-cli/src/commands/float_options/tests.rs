use super::*;
use clap::Parser;
use serde_json::json;
#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    selection: Selection,
}
fn base() -> serde_json::Value {
    json!({"curve":"clip","exposure_milli_ev":0,"negative":"reject","alpha_output":"preserve"})
}
#[test]
fn clap_json_and_fixed_wire_represent_exactly_the_same_explicit_settings() {
    for (curve, curve_name) in [
        (Curve::Clip, "clip"),
        (Curve::ReinhardPerChannel, "reinhard-per-channel"),
    ] {
        for (negative, negative_name) in [
            (Negative::Reject, "reject"),
            (Negative::ClipToBlack, "clip-to-black"),
        ] {
            for exposure in [-16000, -1, 0, 1, 16000] {
                for background in [None, Some([0, 0, 0]), Some([17, 128, 255])] {
                    let alpha_name = if background.is_some() {
                        "flatten"
                    } else {
                        "preserve"
                    };
                    let mut args = vec![
                        "test".to_owned(),
                        "--float-curve".into(),
                        curve_name.into(),
                        "--float-exposure-milli-ev".into(),
                        exposure.to_string(),
                        "--float-negative".into(),
                        negative_name.into(),
                        "--float-alpha".into(),
                        alpha_name.into(),
                        "--float-source-space".into(),
                        "linear-radiance-nominal-ee".into(),
                        "--float-alpha-assignment".into(),
                        "straight".into(),
                    ];
                    if let Some(rgb) = background {
                        args.extend([
                            "--float-background".into(),
                            format!("{},{},{}", rgb[0], rgb[1], rgb[2]),
                        ]);
                    }
                    let cli = Cli::try_parse_from(args).unwrap().selection;
                    let parsed: Selection = serde_json::from_value(json!({"curve":curve_name,"negative":negative_name,"exposure_milli_ev":exposure,"alpha_output":alpha_name,"srgb_background":background,"source_assignment":"linear-radiance-nominal-ee","alpha_assignment":"straight"})).unwrap();
                    assert_eq!(cli, parsed);
                    assert_eq!(cli.curve, curve);
                    assert_eq!(cli.negative, negative);
                    let settings = cli.settings().unwrap();
                    assert_eq!(
                        wire::decode_settings(&cli.wire().unwrap()).unwrap(),
                        settings
                    );
                    assert_eq!(
                        serde_json::from_str::<Selection>(&serde_json::to_string(&cli).unwrap())
                            .unwrap(),
                        cli
                    );
                }
            }
        }
    }
    let selection: Selection = serde_json::from_value(base()).unwrap();
    assert_eq!(
        selection.settings().unwrap(),
        precision::Settings::default()
    );
}
#[test]
fn unknown_missing_nonfinite_and_inactive_fields_are_refused_without_defaults() {
    for name in ["curve", "exposure_milli_ev", "negative", "alpha_output"] {
        let mut value = base();
        value.as_object_mut().unwrap().remove(name);
        assert!(
            serde_json::from_value::<Selection>(value).is_err(),
            "{name}"
        );
    }
    for (field, value) in [
        ("unknown", json!(true)),
        ("curve", json!("automatic")),
        ("negative", json!("ignore")),
        ("alpha_output", json!("auto")),
        ("source_assignment", json!("srgb")),
        ("alpha_assignment", json!("guess")),
        ("exposure_milli_ev", json!(0.5)),
        ("exposure_milli_ev", json!(null)),
        ("srgb_background", json!([0, 1])),
        ("srgb_background", json!([0, 1, 256])),
    ] {
        let mut input = base();
        input[field] = value;
        assert!(
            serde_json::from_value::<Selection>(input).is_err(),
            "{field}"
        );
    }
    for (field, value) in [
        ("exposure_milli_ev", json!(-16001)),
        ("exposure_milli_ev", json!(16001)),
        ("alpha_output", json!("flatten")),
        ("srgb_background", json!([0, 0, 0])),
    ] {
        let mut input = base();
        input[field] = value;
        assert_eq!(
            serde_json::from_value::<Selection>(input)
                .unwrap()
                .settings()
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    assert!(Cli::try_parse_from(["test"]).is_err());
    for value in ["0,0", "0,0,0,0", "0,0,256", "-1,0,0", "0.5,0,0", "0,,0"] {
        assert!(parse_rgb(value).is_err());
    }
    let schema = serde_json::to_value(schemars::schema_for!(Selection)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["required"].as_array().unwrap().len(), 4);
}
#[test]
fn every_assignment_is_explicit_and_changes_the_policy_binding_digest() {
    let parsed: Selection = serde_json::from_value(base()).unwrap();
    let binding = |selection: Selection| {
        precision::Binding::new(
            [17; 32],
            1,
            1,
            precision::ColorMetadata::Declared {
                space: precision::LinearSpace::Rec709D65,
                evidence: precision::ColorEvidence::InteropId,
            },
            precision::SourceAlpha::Premultiplied,
            selection.settings().unwrap(),
        )
        .unwrap()
    };
    let original = binding(parsed).digest();
    for changed in [
        Selection {
            curve: Curve::ReinhardPerChannel,
            ..parsed
        },
        Selection {
            negative: Negative::ClipToBlack,
            ..parsed
        },
        Selection {
            exposure_milli_ev: 1,
            ..parsed
        },
        Selection {
            alpha_output: AlphaOutput::Flatten,
            srgb_background: Some([0, 0, 0]),
            ..parsed
        },
        Selection {
            source_assignment: Some(SourceSpace::Rec709D65),
            ..parsed
        },
        Selection {
            source_assignment: Some(SourceSpace::RadianceEqualEnergy),
            ..parsed
        },
        Selection {
            alpha_assignment: Some(AlphaAssignment::Straight),
            ..parsed
        },
        Selection {
            alpha_assignment: Some(AlphaAssignment::Premultiplied),
            ..parsed
        },
    ] {
        assert_ne!(binding(changed).digest(), original);
        assert_ne!(changed.wire().unwrap(), parsed.wire().unwrap());
    }
}
#[test]
fn explicit_curve_and_exposure_dtos_match_existing_independent_decimal_vectors() {
    use paintcraft_core::types::resources::SharedBudget;
    let fixtures: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/precision-fixtures/vectors.json"
    ))
    .unwrap();
    for id in [
        "clip-white-is-white",
        "explicit-reinhard-white-is-188",
        "fractional-exposure",
    ] {
        let case = fixtures["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == id)
            .unwrap();
        let mut options = base();
        options["curve"] = if case["curve"] == "clip" {
            json!("clip")
        } else {
            json!("reinhard-per-channel")
        };
        options["exposure_milli_ev"] = case["exposure_milli_ev"].clone();
        let selection: Selection = serde_json::from_value(options).unwrap();
        let binding = precision::Binding::new(
            [17; 32],
            1,
            1,
            precision::ColorMetadata::Declared {
                space: precision::LinearSpace::Rec709D65,
                evidence: precision::ColorEvidence::InteropId,
            },
            precision::SourceAlpha::Opaque,
            selection.settings().unwrap(),
        )
        .unwrap();
        let samples: Vec<_> = case["sample_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| f32::from_bits(u32::from_str_radix(v.as_str().unwrap(), 16).unwrap()))
            .collect();
        let actual =
            precision::convert(binding, &samples, SharedBudget::new(4), &|| Ok(())).unwrap();
        let expected: Vec<u8> = case["rgba8"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(actual.rgba(), expected, "{id}");
    }
}
