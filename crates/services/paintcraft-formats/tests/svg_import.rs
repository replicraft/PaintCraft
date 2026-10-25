use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    pcnd,
    registry::*,
    svg::{self, Fit, Fonts, Options},
};
use std::{cell::Cell, io::Write};
const BYTES: &[u8] = include_bytes!("svg-fixtures/aligned.svg");
#[test]
fn explicit_svg_rasterization_is_studio_only_and_never_stock_or_writable() {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(BYTES).unwrap();
    let svgz = encoder.finish().unwrap();
    for platform in [Platform::Native, Platform::Web] {
        for (name, bytes) in [("scene.svg", BYTES), ("scene.svgz", svgz.as_slice())] {
            assert_eq!(
                admit_import_svg(Frontend::Studio, name, bytes, platform).unwrap(),
                Family::Svg
            );
            assert!(admit_import(Frontend::Studio, name, bytes, platform).is_err());
            for operation in [
                Operation::Open,
                Operation::OpenRaster,
                Operation::OpenProject,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
            ] {
                assert!(
                    !admission(Request::new(
                        Frontend::Studio,
                        operation,
                        Family::Svg,
                        platform
                    ))
                    .available()
                );
            }
            assert!(
                !enabled_extensions(Frontend::Studio, Operation::ImportConvert, platform)
                    .contains(&"svg")
            );
            for classic in [
                Frontend::ClassicV7,
                Frontend::ClassicV10,
                Frontend::ClassicV11,
            ] {
                assert!(admit_import_svg(classic, name, bytes, platform).is_err());
            }
        }
        assert!(admit_import_svg(Frontend::Studio, "wrong.svgz", BYTES, platform).is_err());
        assert!(admit_import_svg(Frontend::Studio, "wrong.svg", &svgz, platform).is_err());
        assert!(admit_import_svg(Frontend::Studio, "wrong.png", BYTES, platform).is_err());
        assert!(admit_import_svg(Frontend::Studio, "wrong.svg", b"not XML", platform).is_err());
    }
}
#[test]
fn rasterization_intent_and_rendered_samples_define_identity_and_survive_pcnd() {
    let options = Options {
        width: 64,
        height: 32,
        dpi: 96.0,
        fit: Fit::Contain,
    };
    let fonts = Fonts::with_bundled(&[], &|| Ok(())).unwrap();
    let convert = |o| {
        svg::prepare(BYTES, o, &fonts, svg::PIXELS_MAX, &|| Ok(()))
            .unwrap()
            .render(&|| Ok(()))
            .unwrap()
            .document("source-sha", &|| Ok(()))
            .unwrap()
    };
    let base = convert(options);
    assert!(convert(options).same_semantics(&base));
    assert!(
        pcnd::read(&pcnd::write_compact(&base).unwrap())
            .unwrap()
            .same_semantics(&base)
    );
    for changed in [
        Options {
            width: 32,
            ..options
        },
        Options {
            dpi: 192.0,
            ..options
        },
        Options {
            fit: Fit::Stretch,
            ..options
        },
    ] {
        let doc = convert(changed);
        assert_ne!(doc.document_id, base.document_id);
        assert_ne!(doc.state_id, base.state_id);
        assert_ne!(doc.roots, base.roots);
    }
    let layer = &base.layers[&base.roots[0]];
    assert_eq!(layer.raster.get(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(layer.raster.get(63, 31).0, [0, 0, 255, 128]);
}
#[test]
fn document_ingest_cancels_without_returning_a_private_prefix() {
    let raster = svg::prepare(
        BYTES,
        Options {
            width: 32,
            height: 16,
            dpi: 96.0,
            fit: Fit::Contain,
        },
        &Fonts::default(),
        svg::PIXELS_MAX,
        &|| Ok(()),
    )
    .unwrap()
    .render(&|| Ok(()))
    .unwrap();
    let total = Cell::new(0);
    raster
        .document("source-sha", &|| {
            total.set(total.get() + 1);
            Ok(())
        })
        .unwrap();
    for stop in 1..=total.get() {
        let calls = Cell::new(0);
        let error = raster
            .document("source-sha", &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel SVG ingest",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
    }
}
