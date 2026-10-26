//! Fresh layered output, distinct from foreign preserving Save and public gates.
use paintcraft_core::types::{
    Error, ErrorCode,
    json::{self, Value},
    policy::hex,
};
use paintcraft_formats::{
    project_import,
    psd::{self, Version, WriteLimits},
};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Write};
fn document() -> paintcraft_document::Document {
    project_import::decode_ora(include_bytes!("ora-fixtures/deflated.ora"), 100, &|| Ok(()))
        .unwrap()
        .document
}
fn compare(doc: &paintcraft_document::Document, bytes: &[u8], version: Version) {
    let inspection = psd::inspect(bytes, psd::PIXELS_MAX, &|| Ok(())).unwrap();
    assert_eq!(inspection.version, version);
    assert_eq!(inspection.layers.len(), doc.roots.len());
    let decoded = psd::decode(bytes, psd::PIXELS_MAX, &|| Ok(())).unwrap();
    assert_eq!((decoded.width, decoded.height), doc.canvas);
    for (layer, id) in decoded.layers.iter().zip(&doc.roots) {
        let expected = &doc.layers[id];
        assert_eq!(layer.info.name, expected.name);
        assert_eq!(
            layer.info.origin,
            (expected.origin.0 as i32, expected.origin.1 as i32)
        );
        assert_eq!(layer.info.opacity, expected.opacity);
        assert_eq!(layer.info.visible, expected.visible);
        assert_eq!(
            (layer.info.width, layer.info.height),
            (expected.raster.width(), expected.raster.height())
        );
        for (i, pixel) in layer.rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                *pixel,
                expected
                    .raster
                    .get(
                        (i % layer.info.width as usize) as i64,
                        (i / layer.info.width as usize) as i64
                    )
                    .0
            );
        }
    }
}
#[test]
fn fresh_psd_and_psb_retain_exact_raw_layers_names_offsets_visibility_and_opacity() {
    let mut doc = document();
    let id = doc.roots[1];
    doc.layers.get_mut(&id).unwrap().name = "日本語 & ink\tline\nnext\rreturn".into();
    let before = doc.clone();
    for version in [Version::Psd, Version::Psb] {
        let plan = psd::preflight(&doc, version, &WriteLimits::default(), &|| Ok(())).unwrap();
        assert_eq!(plan.layers, 3);
        assert!(plan.losses.is_empty());
        assert!(plan.transient_bytes > plan.output_capacity as u64);
        let bytes =
            psd::encode_document(&doc, version, &WriteLimits::default(), &[], &|| Ok(())).unwrap();
        compare(&doc, &bytes, version);
        assert_eq!(
            bytes,
            psd::encode_document(&doc, version, &WriteLimits::default(), &[], &|| Ok(())).unwrap()
        );
    }
    assert!(doc.same_semantics(&before));
}
#[test]
fn unsupported_metadata_bad_bounds_and_declared_memory_refuse_before_any_sink_write() {
    let doc = document();
    let limits = WriteLimits::default();
    let cases = [
        WriteLimits {
            encoded_bytes: 32,
            ..limits
        },
        WriteLimits {
            metadata_bytes: 32,
            ..limits
        },
        WriteLimits {
            transient_bytes: 1,
            ..limits
        },
        WriteLimits {
            pixels: 1,
            ..limits
        },
        WriteLimits {
            layers: 1,
            ..limits
        },
        WriteLimits {
            name_utf16: 1,
            ..limits
        },
    ];
    for limit in cases {
        let mut sink = Vec::new();
        assert!(
            psd::write_document_to(&doc, Version::Psd, &limit, &[], &mut sink, &|| Ok(())).is_err()
        );
        assert!(sink.is_empty());
    }
    for variant in 0..5 {
        let mut bad = doc.clone();
        let layer = bad.layers.get_mut(&bad.roots[0]).unwrap();
        match variant {
            0 => layer.origin.0 = i64::from(i32::MAX),
            1 => layer.name = "bad\0name".into(),
            2 => {
                bad.roots.push(bad.roots[0]);
            }
            3 => {
                bad.roots.clear();
                bad.layers.clear();
            }
            _ => {
                bad.roots[0] = uuid::Uuid::new_v4();
            }
        }
        assert!(psd::preflight(&bad, Version::Psd, &limits, &|| Ok(())).is_err());
    }
    let mut named = doc.clone();
    named.title = Some("Native title".into());
    assert_eq!(
        psd::preflight(&named, Version::Psd, &limits, &|| Ok(()))
            .unwrap()
            .losses,
        [psd::TITLE_LOSS]
    );
    assert!(psd::encode_document(&named, Version::Psd, &limits, &[], &|| Ok(())).is_err());
    psd::encode_document(
        &named,
        Version::Psd,
        &limits,
        &[psd::TITLE_LOSS.into()],
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        psd::encode_document(&doc, Version::Psd, &limits, &["unknown".into()], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
#[test]
fn cancelled_private_output_and_failed_staged_sink_do_not_mutate_source() {
    let doc = document();
    let before = doc.clone();
    let count = Cell::new(0);
    psd::encode_document(&doc, Version::Psd, &WriteLimits::default(), &[], &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 0..count.get() {
        let calls = Cell::new(0);
        let error = psd::encode_document(&doc, Version::Psd, &WriteLimits::default(), &[], &|| {
            let n = calls.get();
            calls.set(n + 1);
            if n >= stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    struct Failed;
    impl Write for Failed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        psd::write_document_to(
            &doc,
            Version::Psd,
            &WriteLimits::default(),
            &[],
            &mut Failed,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::StorageFailed
    );
    assert!(doc.same_semantics(&before));
}
fn export_case(doc: &paintcraft_document::Document, root: &std::path::Path, case: &str) -> Value {
    let mut rows = Vec::new();
    for (index, id) in doc.roots.iter().enumerate() {
        let layer = &doc.layers[id];
        let mut pixels = Vec::new();
        for y in 0..layer.raster.height() {
            for x in 0..layer.raster.width() {
                pixels.extend(layer.raster.get(i64::from(x), i64::from(y)).0);
            }
        }
        let name = format!("{case}-layer{index}.rgba");
        std::fs::write(root.join(&name), &pixels).unwrap();
        rows.push(Value::object([
            ("name", Value::str(&layer.name)),
            ("x", Value::Int(layer.origin.0)),
            ("y", Value::Int(layer.origin.1)),
            ("width", Value::Int(i64::from(layer.raster.width()))),
            ("height", Value::Int(i64::from(layer.raster.height()))),
            ("opacity", Value::Int(i64::from(layer.opacity))),
            ("visible", Value::Bool(layer.visible)),
            ("pixels", Value::str(name)),
            ("sha256", Value::str(hex(&Sha256::digest(pixels)))),
        ]));
    }
    let composite = paintcraft_render::render_cancellable(doc, || Ok(())).unwrap();
    let merged = format!("{case}-merged.rgba");
    std::fs::write(root.join(&merged), &composite).unwrap();
    for (extension, version) in [("psd", Version::Psd), ("psb", Version::Psb)] {
        let bytes =
            psd::encode_document(doc, version, &WriteLimits::default(), &[], &|| Ok(())).unwrap();
        std::fs::write(root.join(format!("{case}.{extension}")), bytes).unwrap();
    }
    Value::object([
        ("case", Value::str(case)),
        ("width", Value::Int(i64::from(doc.canvas.0))),
        ("height", Value::Int(i64::from(doc.canvas.1))),
        ("layers", Value::Array(rows)),
        ("merged", Value::str(merged)),
    ])
}
#[test]
fn export_owned_writer_oracles_only_when_explicitly_requested() {
    let Some(path) = std::env::var_os("PAINTCRAFT_PSD_WRITER_OUTPUT") else {
        return;
    };
    let root = std::path::PathBuf::from(path);
    assert!(root.is_dir());
    assert!(
        std::fs::read_dir(&root).unwrap().next().is_none(),
        "refuse existingoutput"
    );
    let mut layers = document();
    layers.layers.get_mut(&layers.roots[1]).unwrap().name = "日本語 😀 & ink\tline\nnext".into();
    let rgba = [
        10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254, 160,
        170, 180, 255,
    ];
    let alpha =
        paintcraft_document::import_image_checked(3, 2, &rgba, "authored-psd-alpha", &|| Ok(()))
            .unwrap();
    let cases = vec![
        export_case(&layers, &root, "layers"),
        export_case(&alpha, &root, "alpha-boundaries"),
    ];
    std::fs::write(
        root.join("expected.json"),
        json::canonical(&Value::Array(cases)),
    )
    .unwrap();
}

#[test]
fn every_u8_opacity_round_trips_without_float_quantization() {
    let mut doc = document();
    let id = doc.roots[1];
    for version in [Version::Psd, Version::Psb] {
        for opacity in 0..=255u8 {
            doc.layers.get_mut(&id).unwrap().opacity = opacity;
            let bytes =
                psd::encode_document(&doc, version, &WriteLimits::default(), &[], &|| Ok(()))
                    .unwrap();
            let info = psd::inspect(&bytes, 100, &|| Ok(())).unwrap();
            assert_eq!(info.layers[1].opacity, opacity);
        }
    }
}
#[test]
fn internal_writer_does_not_enable_any_public_fresh_or_preserving_route() {
    use paintcraft_formats::registry::*;
    for family in [Family::Psd, Family::Psb] {
        for platform in [Platform::Native, Platform::Web] {
            for operation in [
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
            ] {
                for frontend in [
                    Frontend::Studio,
                    Frontend::ClassicV7,
                    Frontend::ClassicV10,
                    Frontend::ClassicV11,
                ] {
                    assert!(
                        !admission(Request::new(frontend, operation, family, platform)).available()
                    );
                }
            }
        }
    }
}
#[test]
fn explicit_psb_uses_32bit_packbits_rows_and_preserves_far_signed_offsets() {
    let width = 65537u32;
    let rgba: Vec<_> = (0..width)
        .flat_map(|x| {
            [
                ((x * 17 + x / 7) % 256) as u8,
                ((x * 31) % 256) as u8,
                ((x * 47) % 256) as u8,
                255,
            ]
        })
        .collect();
    let doc =
        paintcraft_document::import_image_checked(width, 1, &rgba, "widePSD", &|| Ok(())).unwrap();
    assert!(psd::preflight(&doc, Version::Psd, &WriteLimits::default(), &|| Ok(())).is_err());
    let bytes =
        psd::encode_document(&doc, Version::Psb, &WriteLimits::default(), &[], &|| Ok(())).unwrap();
    compare(&doc, &bytes, Version::Psb);
    let mut doc = document();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.origin = (i64::from(i32::MIN), i64::from(i32::MIN));
    for version in [Version::Psd, Version::Psb] {
        let bytes =
            psd::encode_document(&doc, version, &WriteLimits::default(), &[], &|| Ok(())).unwrap();
        compare(&doc, &bytes, version);
    }
}
#[test]
fn shared_utf16_name_boundary_is_enforced_before_model_or_sink_work() {
    assert_eq!(WriteLimits::default().name_utf16, psd::LAYER_NAME_UTF16_MAX);
    let mut doc = document();
    let id = doc.roots[0];
    let relaxed = WriteLimits {
        name_utf16: usize::MAX,
        ..WriteLimits::default()
    };
    for version in [Version::Psd, Version::Psb] {
        for name in [
            "a".repeat(psd::LAYER_NAME_UTF16_MAX),
            "😀".repeat(psd::LAYER_NAME_UTF16_MAX / 2),
        ] {
            doc.layers.get_mut(&id).unwrap().name = name;
            psd::preflight(&doc, version, &relaxed, &|| Ok(())).unwrap();
            let bytes = psd::encode_document(&doc, version, &relaxed, &[], &|| Ok(())).unwrap();
            compare(&doc, &bytes, version);
        }
        for name in [
            "a".repeat(psd::LAYER_NAME_UTF16_MAX + 1),
            "😀".repeat(psd::LAYER_NAME_UTF16_MAX / 2 + 1),
        ] {
            doc.layers.get_mut(&id).unwrap().name = name;
            let before = doc.clone();
            let calls = Cell::new(0);
            let check = || {
                calls.set(calls.get() + 1);
                Ok(())
            };
            assert_eq!(
                psd::preflight(&doc, version, &relaxed, &check)
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            let preflight_calls = calls.get();
            calls.set(0);
            let mut sink = Vec::new();
            assert_eq!(
                psd::write_document_to(&doc, version, &relaxed, &[], &mut sink, &check)
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(
                calls.get(),
                preflight_calls,
                "no model/encoder checkpoints after failed preflight"
            );
            assert!(sink.is_empty());
            assert!(doc.same_semantics(&before));
        }
    }
}
