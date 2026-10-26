use super::*;
fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
fn space(value: &str) -> LinearSpace {
    match value {
        "rec709" => LinearSpace::Rec709D65,
        "radiance" => LinearSpace::RadianceEqualEnergy,
        _ => panic!("unknown fixture space"),
    }
}
fn binding(settings: Settings) -> Binding {
    Binding::new(
        [17; 32],
        1,
        1,
        ColorMetadata::Declared {
            space: LinearSpace::Rec709D65,
            evidence: ColorEvidence::InteropId,
        },
        SourceAlpha::Opaque,
        settings,
    )
    .unwrap()
}
#[test]
fn independent_decimal_color_tone_and_alpha_vectors_match() {
    let document = json::parse(include_bytes!(
        "../../tests/precision-fixtures/vectors.json"
    ))
    .unwrap();
    let cases = document.get("cases").unwrap().as_array().unwrap();
    assert_eq!(cases.len(), 35);
    for case in cases {
        let id = text(case, "id").unwrap();
        let metadata = match text(case, "space").unwrap() {
            "rec709" => ColorMetadata::Declared {
                space: LinearSpace::Rec709D65,
                evidence: ColorEvidence::InteropId,
            },
            "radiance" => ColorMetadata::RadianceDefault,
            "unspecified" => ColorMetadata::Unspecified,
            _ => panic!("unknown fixture"),
        };
        let alpha = match text(case, "alpha").unwrap() {
            "opaque" => SourceAlpha::Opaque,
            "premultiplied" => SourceAlpha::Premultiplied,
            "straight" => SourceAlpha::Straight,
            _ => panic!("alpha"),
        };
        let mut settings = Settings {
            curve: if text(case, "curve") == Some("clip") {
                Curve::Clip
            } else {
                Curve::ReinhardPerChannel
            },
            negative: if text(case, "negative") == Some("reject") {
                Negative::Reject
            } else {
                Negative::ClipToBlack
            },
            exposure_milli_ev: case.get("exposure_milli_ev").unwrap().as_i64().unwrap() as i32,
            source_assignment: text(case, "assignment").map(space),
            alpha_assignment: text(case, "alpha_assignment").map(|value| {
                if value == "straight" {
                    AlphaAssignment::Straight
                } else {
                    AlphaAssignment::Premultiplied
                }
            }),
            ..Settings::default()
        };
        if let Some(background) = case.get("background").and_then(Value::as_array) {
            settings.alpha_output = AlphaOutput::Flatten {
                srgb_background: std::array::from_fn(|i| background[i].as_i64().unwrap() as u8),
            };
        }
        let samples: Vec<f32> = case
            .get("sample_bits")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap()))
            .collect();
        let original: Vec<u32> = samples.iter().map(|v| v.to_bits()).collect();
        let budget = SharedBudget::new(4);
        let result = Binding::new([17; 32], 1, 1, metadata, alpha, settings)
            .and_then(|binding| convert(binding, &samples, budget.clone(), &|| Ok(())));
        if text(case, "error").is_some() {
            assert_eq!(
                result.unwrap_err().code,
                ErrorCode::CapabilityUnsupported,
                "{id}"
            );
            assert_eq!(budget.usage().reserved, 0);
        } else {
            let converted = result.unwrap_or_else(|error| panic!("{id}: {error}"));
            let expected: Vec<u8> = case
                .get("rgba8")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as u8)
                .collect();
            assert_eq!(converted.rgba(), expected, "{id}");
            assert_eq!(budget.usage().reserved, 4);
            if id == "clip-white-is-white" {
                assert_eq!(converted.losses(), ["hdr-precision-reduced-to-8"]);
            }
            if id == "explicit-reinhard-white-is-188" {
                assert!(converted.losses().contains(&"hdr-tone-mapped"));
            }
            if id == "sub-byte-alpha-is-explicitly-quantized" {
                assert_eq!(converted.rgba()[3], 0);
                assert!(converted.losses().contains(&"hdr-alpha-quantized"));
            }
            if id == "zero-alpha-additive-black" {
                assert_eq!(converted.statistics().additive_zero_alpha_pixels, 1);
                assert!(converted.losses().contains(&"hdr-alpha-flattened"));
            }
            drop(converted);
            assert_eq!(budget.usage().reserved, 0);
        }
        assert_eq!(
            samples.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            original
        );
    }
}
#[test]
fn unlabelled_sources_need_assignment_and_every_setting_binds_to_source() {
    assert!(
        Binding::new(
            [17; 32],
            1,
            1,
            ColorMetadata::Unspecified,
            SourceAlpha::Opaque,
            Settings::default()
        )
        .is_err()
    );
    let settings = Settings {
        source_assignment: Some(LinearSpace::Rec709D65),
        ..Settings::default()
    };
    let assigned = Binding::new(
        [17; 32],
        1,
        1,
        ColorMetadata::Unspecified,
        SourceAlpha::Opaque,
        settings,
    )
    .unwrap();
    let converted = convert(
        assigned.clone(),
        &[0.18, 0.18, 0.18],
        SharedBudget::new(4),
        &|| Ok(()),
    )
    .unwrap();
    assert!(converted.losses().contains(&"hdr-source-space-assigned"));
    assert!(converted.require_losses(&[]).is_err());
    converted
        .require_losses(
            &converted
                .losses()
                .iter()
                .map(|code| (*code).into())
                .collect::<Vec<String>>(),
        )
        .unwrap();
    assert_eq!(
        converted
            .require_losses(&["hdr-guessed".into()])
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let base = binding(Settings::default());
    let mut changed = base.clone();
    changed.source_sha256[0] ^= 1;
    assert_ne!(base.digest(), changed.digest());
    for settings in [
        Settings {
            curve: Curve::ReinhardPerChannel,
            ..Settings::default()
        },
        Settings {
            exposure_milli_ev: 1,
            ..Settings::default()
        },
        Settings {
            negative: Negative::ClipToBlack,
            ..Settings::default()
        },
        Settings {
            alpha_output: AlphaOutput::Flatten {
                srgb_background: [0, 0, 0],
            },
            ..Settings::default()
        },
    ] {
        assert_ne!(base.digest(), binding(settings).digest());
    }
    assert_ne!(base.digest(), assigned.digest());
    assert_eq!(base.settings().curve, Curve::Clip);
    for ev in [-16001, 16001] {
        assert!(
            Binding::new(
                [17; 32],
                1,
                1,
                ColorMetadata::RadianceDefault,
                SourceAlpha::Opaque,
                Settings {
                    exposure_milli_ev: ev,
                    ..Settings::default()
                }
            )
            .is_err()
        );
    }
    assert!(
        Binding::new(
            [17; 32],
            1,
            1,
            ColorMetadata::RadianceDefault,
            SourceAlpha::Opaque,
            Settings {
                alpha_assignment: Some(AlphaAssignment::Straight),
                ..Settings::default()
            }
        )
        .is_err()
    );
}
#[test]
fn output_admission_cancel_and_shape_refusal_never_publish_partial_storage() {
    let tiny = SharedBudget::new(3);
    let denial = convert(
        binding(Settings::default()),
        &[1.0; 3],
        tiny.clone(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(denial.code, ErrorCode::ResourceLimit);
    let context = denial.allocation_denial().unwrap();
    assert_eq!(context.class(), AllocationClass::DecodedTiles);
    assert_eq!(context.required_bytes(), 4);
    assert_eq!(tiny.usage().reserved, 0);
    let budget = SharedBudget::new(4);
    assert_eq!(
        convert(
            binding(Settings::default()),
            &[],
            budget.clone(),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    assert_eq!(budget.usage().reserved, 0);
    for stop in [1, 2, 3] {
        let calls = std::cell::Cell::new(0);
        let error = convert(
            binding(Settings::default()),
            &[1.0; 3],
            budget.clone(),
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
    }
    for (width, height) in [(0, 1), (65537, 1), (65536, 65536)] {
        assert!(
            Binding::new(
                [17; 32],
                width,
                height,
                ColorMetadata::RadianceDefault,
                SourceAlpha::Opaque,
                Settings::default()
            )
            .is_err()
        );
    }
    let wide = Binding::new(
        [17; 32],
        2048,
        1,
        ColorMetadata::Declared {
            space: LinearSpace::Rec709D65,
            evidence: ColorEvidence::InteropId,
        },
        SourceAlpha::Opaque,
        Settings::default(),
    )
    .unwrap();
    let budget = SharedBudget::new(8192);
    let calls = std::cell::Cell::new(0);
    let error = convert(wide, &[0.5; 6144], budget.clone(), &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel after first output block",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(calls.get(), 3);
    assert_eq!(budget.usage().reserved, 0);
}
