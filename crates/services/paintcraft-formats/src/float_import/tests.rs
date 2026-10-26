use super::*;
use paintcraft_core::types::json;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/float-import-fixtures")
}
fn load(path: impl AsRef<std::path::Path>) -> Vec<u8> {
    std::fs::read(path).unwrap()
}
fn field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}
fn settings(v: &Value) -> Settings {
    fn space(v: &str) -> precision::LinearSpace {
        match v {
            "rec709" => precision::LinearSpace::Rec709D65,
            "radiance" => precision::LinearSpace::RadianceEqualEnergy,
            _ => panic!("space"),
        }
    }
    Settings {
        curve: if field(v, "curve") == Some("reinhard") {
            precision::Curve::ReinhardPerChannel
        } else {
            precision::Curve::Clip
        },
        negative: if field(v, "negative") == Some("reject") {
            precision::Negative::Reject
        } else {
            precision::Negative::ClipToBlack
        },
        exposure_milli_ev: v.get("exposure_milli_ev").unwrap().as_i64().unwrap() as i32,
        source_assignment: field(v, "assignment").map(space),
        alpha_assignment: field(v, "alpha_assignment").map(|name| match name {
            "straight" => precision::AlphaAssignment::Straight,
            _ => precision::AlphaAssignment::Premultiplied,
        }),
        alpha_output: v.get("background").and_then(Value::as_array).map_or(
            precision::AlphaOutput::Preserve,
            |values| precision::AlphaOutput::Flatten {
                srgb_background: std::array::from_fn(|i| values[i].as_i64().unwrap() as u8),
            },
        ),
    }
}
#[test]
fn actual_decode_conversion_matches_independent_external_float_oracles_and_refusals() {
    let manifest = json::parse(&load(root().join("manifest.json"))).unwrap();
    let oracle_source = load(
        root()
            .parent()
            .unwrap()
            .join("precision-fixtures/reference.py"),
    );
    assert_eq!(
        hex(&Sha256::digest(&oracle_source)),
        field(&manifest, "policy_oracle_sha256").unwrap()
    );
    for case in manifest.get("cases").unwrap().as_array().unwrap() {
        let id = field(case, "id").unwrap();
        let family = field(case, "family").unwrap();
        let name = field(case, "file").unwrap();
        let group = if family == "hdr" {
            "radiance-fixtures"
        } else {
            "exr-fixtures"
        };
        let source = load(if name == "aspect-2.hdr" {
            root().join(name)
        } else {
            root().parent().unwrap().join(group).join(name)
        });
        let before = source.clone();
        let budget = SharedBudget::new(64 << 20);
        let choice = settings(case.get("settings").unwrap());
        let result = if family == "hdr" {
            prepare_hdr(
                &source,
                precision::PIXELS_MAX,
                choice,
                budget.clone(),
                &|| Ok(()),
            )
        } else {
            prepare_exr(
                &source,
                precision::PIXELS_MAX,
                choice,
                budget.clone(),
                &|| Ok(()),
            )
        };
        assert_eq!(source, before);
        let hash: [u8; 32] = Sha256::digest(&source).into();
        assert_eq!(hex(&hash), field(case, "source_sha256").unwrap());
        let external = load(
            root()
                .parent()
                .unwrap()
                .join(field(case, "external_float_file").unwrap()),
        );
        assert_eq!(
            hex(&Sha256::digest(&external)),
            field(case, "external_float_sha256").unwrap()
        );
        if let Some(error) = field(case, "expected_error") {
            assert_eq!(format!("{:?}", result.unwrap_err().code), error, "{id}");
            assert_eq!(budget.usage().reserved, 0);
            continue;
        }
        let prepared = result.unwrap_or_else(|e| panic!("{id}: {e}"));
        let partial = if choice.alpha_output == precision::AlphaOutput::Preserve
            && field(case.get("settings").unwrap(), "alpha") != Some("opaque")
        {
            external
                .as_chunks::<16>()
                .0
                .iter()
                .filter(|pixel| {
                    let alpha = f32::from_le_bytes(pixel[12..16].try_into().unwrap());
                    alpha > 0.0 && alpha < 1.0
                })
                .count() as u64
        } else {
            0
        };
        assert_eq!(
            prepared.assessment().retained_partial_alpha_pixels(),
            partial,
            "{id}"
        );
        assert_eq!(
            prepared
                .assessment()
                .losses()
                .contains(&"hdr-linear-alpha-compositing-changed"),
            partial > 0,
            "{id}"
        );

        let expected = load(root().join(field(case, "expected_rgba_file").unwrap()));
        assert_eq!(prepared.preview_rgba(), expected, "{id}");
        assert_eq!(
            hex(&Sha256::digest(&expected)),
            field(case, "rgba_sha256").unwrap()
        );
        assert_eq!(prepared.assessment().binding().source_sha256(), hash);
        assert_eq!(prepared.assessment().binding().settings(), choice);
        assert!(
            budget.usage().reserved < 1 << 20,
            "float and scratch buffers must release before preview"
        );
        let receipt = prepared.assessment().to_value();
        assert_eq!(field(&receipt, "rgba_sha256"), field(case, "rgba_sha256"));
        assert_eq!(
            receipt
                .get("metadata_preserved_in_document")
                .unwrap()
                .as_bool(),
            Some(false)
        );
        assert!(!prepared.assessment().source_header().is_empty());
        let digest = prepared.assessment().digest();
        let allowed = prepared
            .assessment()
            .losses()
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>();
        let retained = budget.usage().reserved;
        let approved = prepared.approve(digest, &allowed).unwrap();
        assert_eq!(approved.rgba(), expected);
        assert_eq!(budget.usage().reserved, retained);
        drop(approved);
        assert_eq!(budget.usage().reserved, 0);
    }
}
fn hdr(source: &[u8], budget: SharedBudget, exposure: i32) -> Prepared {
    prepare_hdr(
        source,
        4,
        Settings {
            negative: precision::Negative::ClipToBlack,
            exposure_milli_ev: exposure,
            ..Settings::default()
        },
        budget,
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn approval_requires_current_exact_settings_and_all_actual_losses() {
    let source = load(
        root()
            .parent()
            .unwrap()
            .join("radiance-fixtures/literal-2x2.hdr"),
    );
    for mode in 0..3 {
        let budget = SharedBudget::new(64 << 20);
        let prepared = hdr(&source, budget.clone(), 0);
        let mut digest = prepared.assessment().digest();
        let allowed = match mode {
            0 => vec![],
            1 => vec!["unknown-loss".to_owned()],
            _ => {
                digest[0] ^= 1;
                prepared
                    .assessment()
                    .losses()
                    .iter()
                    .map(|v| v.to_string())
                    .collect()
            }
        };
        let error = prepared.approve(digest, &allowed).unwrap_err();
        assert_eq!(
            error.code,
            if mode == 0 {
                ErrorCode::CapabilityUnsupported
            } else {
                ErrorCode::InputSchema
            }
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let budget = SharedBudget::new(64 << 20);
    let original = hdr(&source, budget.clone(), 0);
    let changed = hdr(&source, budget.clone(), 1000);
    assert_ne!(
        original.assessment().digest(),
        changed.assessment().digest()
    );
    let allowed = changed
        .assessment()
        .losses()
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        changed
            .approve(original.assessment().digest(), &allowed)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    drop(original);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn source_metadata_aspect_and_coordinates_are_bound_with_precise_consent() {
    let fixtures = root().parent().unwrap().to_path_buf();
    let budget = SharedBudget::new(64 << 20);
    let plain = hdr(
        &load(fixtures.join("radiance-fixtures/literal-2x2.hdr")),
        budget.clone(),
        0,
    );
    let calibrated = hdr(
        &load(fixtures.join("radiance-fixtures/calibrated-2x2.hdr")),
        budget.clone(),
        0,
    );
    assert_eq!(plain.preview_rgba(), calibrated.preview_rgba());
    assert_ne!(
        plain.assessment().digest(),
        calibrated.assessment().digest()
    );
    assert!(
        !plain
            .assessment()
            .losses()
            .contains(&"hdr-metadata-not-retained")
    );
    assert!(
        calibrated
            .assessment()
            .losses()
            .contains(&"hdr-metadata-not-retained")
    );
    assert!(
        plain
            .assessment()
            .losses()
            .contains(&"hdr-rgbe-lower-edge-representative")
    );
    let aspect = hdr(&load(root().join("aspect-2.hdr")), budget.clone(), 0);
    assert_eq!(aspect.preview_rgba(), plain.preview_rgba());
    assert_eq!(aspect.assessment().pixel_aspect(), 2.0);
    assert!(
        aspect
            .assessment()
            .losses()
            .contains(&"hdr-pixel-aspect-normalized")
    );
    let exr = prepare_exr(
        &load(fixtures.join("exr-fixtures/float-rgb-zips.exr")),
        precision::PIXELS_MAX,
        Settings {
            negative: precision::Negative::ClipToBlack,
            ..Settings::default()
        },
        budget.clone(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(exr.assessment().origin(), [12, -9]);
    assert!(exr.assessment().losses().contains(&"exr-origin-normalized"));
    assert!(
        exr.assessment()
            .losses()
            .contains(&"hdr-metadata-not-retained")
    );
    let mut source = load(fixtures.join("exr-fixtures/float-rgb-zips.exr"));
    let marker = b"pixelAspectRatio\0float\0\x04\0\0\0";
    let at = source
        .windows(marker.len())
        .position(|v| v == marker)
        .unwrap()
        + marker.len();
    source[at..at + 4].copy_from_slice(&2.0f32.to_le_bytes());
    let exr_aspect = prepare_exr(
        &source,
        precision::PIXELS_MAX,
        Settings {
            negative: precision::Negative::ClipToBlack,
            ..Settings::default()
        },
        budget.clone(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(exr_aspect.preview_rgba(), exr.preview_rgba());
    assert_eq!(exr_aspect.assessment().pixel_aspect(), 2.0);
    assert!(
        exr_aspect
            .assessment()
            .losses()
            .contains(&"hdr-pixel-aspect-normalized")
    );
    drop((plain, calibrated, aspect, exr, exr_aspect));
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn admitted_float_buffer_survives_until_rgba_preparation_and_failures_release_all() {
    let source = load(
        root()
            .parent()
            .unwrap()
            .join("radiance-fixtures/literal-2x2.hdr"),
    );
    let header = crate::radiance::inspect(&source, 4, &|| Ok(()))
        .unwrap()
        .header()
        .data_offset;
    let budget = SharedBudget::new(64 << 20);
    let saw = AtomicBool::new(false);
    let overlap = (3 << 20) + header as u64 + 4096 + 4 * 3 * 4 + 4 * 4;
    let error = prepare_hdr(
        &source,
        4,
        Settings {
            negative: precision::Negative::ClipToBlack,
            ..Settings::default()
        },
        budget.clone(),
        &|| {
            if budget.usage().reserved == overlap {
                saw.store(true, Ordering::SeqCst);
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel while float and RGBA storage both held",
                ))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(saw.load(Ordering::SeqCst));
    assert_eq!(budget.usage().reserved, 0);
    let tiny = SharedBudget::new(1);
    let error = prepare_hdr(&source, 4, Settings::default(), tiny.clone(), &|| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(tiny.usage().reserved, 0);
}
#[test]
fn retained_linear_partial_alpha_reports_actual_encoded_editor_blending_limit() {
    use paintcraft_core::types::Rgba;
    let mut source = load(
        root()
            .parent()
            .unwrap()
            .join("exr-fixtures/float-rgba-special.exr"),
    );
    let admitted = crate::exr::inspect(&source, 6, &|| Ok(())).unwrap();
    let table = admitted.raw_header().len();
    let names = admitted
        .header()
        .channels
        .iter()
        .map(|c| c.name.to_owned())
        .collect::<Vec<_>>();
    // This owned uncompressed reference has channel-planar FLOAT rows. Replace
    // every sample with the exact normative example: premult RGB .25, A .5.
    let offsets = (0..2)
        .map(|row| {
            u64::from_le_bytes(
                source[table + row * 8..table + row * 8 + 8]
                    .try_into()
                    .unwrap(),
            ) as usize
        })
        .collect::<Vec<_>>();
    for offset in offsets {
        for (index, name) in names.iter().enumerate() {
            let value = if name == "A" { 0.5f32 } else { 0.25 };
            for x in 0..3 {
                let at = offset + 8 + (index * 3 + x) * 4;
                source[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    let budget = SharedBudget::new(64 << 20);
    let preserved =
        prepare_exr(&source, 6, Settings::default(), budget.clone(), &|| Ok(())).unwrap();
    assert_eq!(preserved.preview_rgba(), [188, 188, 188, 128].repeat(6));
    assert_eq!(preserved.assessment().retained_partial_alpha_pixels(), 6);
    assert!(
        preserved
            .assessment()
            .losses()
            .contains(&"hdr-linear-alpha-compositing-changed")
    );
    let receipt = preserved.assessment().to_value();
    assert_eq!(
        field(&receipt, "destination_blending"),
        Some("encoded-srgb-u8-source-over")
    );
    let actual = Rgba([188, 188, 188, 128]).source_over(Rgba([0, 0, 0, 255]));
    assert_eq!(actual, Rgba([94, 94, 94, 255]));
    let flattened = prepare_exr(
        &source,
        6,
        Settings {
            alpha_output: precision::AlphaOutput::Flatten {
                srgb_background: [0, 0, 0],
            },
            ..Settings::default()
        },
        budget.clone(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(flattened.preview_rgba(), [137, 137, 137, 255].repeat(6));
    assert_eq!(flattened.assessment().retained_partial_alpha_pixels(), 0);
    assert!(
        !flattened
            .assessment()
            .losses()
            .contains(&"hdr-linear-alpha-compositing-changed")
    );
    assert_ne!(actual.0[..3], flattened.preview_rgba()[..3]);
    let digest = preserved.assessment().digest();
    let allowed = preserved
        .assessment()
        .losses()
        .iter()
        .filter(|v| **v != "hdr-linear-alpha-compositing-changed")
        .map(|v| v.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        preserved.approve(digest, &allowed).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    drop(flattened);
    assert_eq!(budget.usage().reserved, 0);
}
