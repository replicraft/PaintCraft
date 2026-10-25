//! Explicit Studio PSD/PSB conversion keeps layers and never enables preserving Save.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{pcnd, project_import};
use std::cell::Cell;

const PSD: &[u8] = include_bytes!("psd-fixtures/layers-3.psd");
const PSB: &[u8] = include_bytes!("psd-fixtures/layers-1.psb");

#[test]
fn only_explicit_studio_import_is_admitted_on_both_platforms() {
    use paintcraft_formats::registry::*;
    for platform in [Platform::Native, Platform::Web] {
        for (family, name, bytes) in [
            (Family::Psd, "layers.psd", PSD),
            (Family::Psb, "layers.psb", PSB),
        ] {
            assert_eq!(
                admit_import(Frontend::Studio, name, bytes, platform).unwrap(),
                family
            );
            assert!(
                enabled_extensions(Frontend::Studio, Operation::ImportConvert, platform)
                    .contains(&family.id())
            );
            for operation in [
                Operation::Open,
                Operation::OpenProject,
                Operation::OpenRaster,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
            ] {
                assert!(
                    !admission(Request::new(Frontend::Studio, operation, family, platform))
                        .available()
                );
            }
            for classic in [
                Frontend::ClassicV7,
                Frontend::ClassicV10,
                Frontend::ClassicV11,
            ] {
                assert!(admit_import(classic, name, bytes, platform).is_err());
                assert!(
                    !enabled_extensions(classic, Operation::ImportConvert, platform)
                        .contains(&family.id())
                );
            }
            let wrong_name = if family == Family::Psd {
                "wrong.psb"
            } else {
                "wrong.psd"
            };
            assert!(admit_import(Frontend::Studio, wrong_name, bytes, platform).is_err());
        }
    }
    use paintcraft_formats::registry::Family;
    assert!(project_import::decode_layered(Family::Psd, PSB, 100, &|| Ok(())).is_err());
    assert!(project_import::decode_layered(Family::Psb, PSD, 100, &|| Ok(())).is_err());
}

#[test]
fn raster_layers_keep_order_names_origins_opacity_and_raw_samples_in_pcnd() {
    for bytes in [PSD, PSB] {
        let converted = project_import::decode_psd(bytes, 100, &|| Ok(())).unwrap();
        assert_eq!(converted.losses, ["psd-metadata-ignored"]);
        let doc = converted.document;
        assert_eq!(doc.canvas, (5, 3));
        assert_eq!(doc.roots.len(), 2);
        for (index, (width, height, seed)) in
            [(3u32, 2u32, 9usize), (5, 3, 51)].into_iter().enumerate()
        {
            let layer = &doc.layers[&doc.roots[index]];
            assert_eq!(layer.name, "日本語 duplicate");
            assert_eq!(
                (layer.raster.width(), layer.raster.height()),
                (width, height)
            );
            assert_eq!(
                (layer.origin, layer.visible, layer.opacity),
                if index == 0 {
                    ((-1, 1), false, 127)
                } else {
                    ((0, 0), true, 255)
                }
            );
            for pixel in 0..width as usize * height as usize {
                let expected = std::array::from_fn(|channel| {
                    ((seed + (pixel * 4 + channel) * 29 + pixel * 7) % 256) as u8
                });
                assert_eq!(
                    layer
                        .raster
                        .get(
                            (pixel % width as usize) as i64,
                            (pixel / width as usize) as i64
                        )
                        .0,
                    expected
                );
            }
        }
        assert_ne!(
            doc.roots[0], doc.roots[1],
            "duplicate names retain distinct identity"
        );
        let native = pcnd::read(&pcnd::write_compact(&doc).unwrap()).unwrap();
        assert!(native.same_semantics(&doc));
        assert!(
            project_import::decode_psd(bytes, 100, &|| Ok(()))
                .unwrap()
                .document
                .same_semantics(&doc)
        );
        // The composite was deliberately generated from different samples.
        let rendered = paintcraft_render::render_flat(&doc).unwrap();
        assert_eq!(&rendered[..4], &[51, 80, 109, 138]);
    }
}

#[test]
fn cancellation_at_every_checkpoint_never_returns_a_partial_document() {
    let total = Cell::new(0);
    project_import::decode_psd(PSB, 100, &|| {
        total.set(total.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=total.get() {
        let calls = Cell::new(0);
        let error = project_import::decode_psd(PSB, 100, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel import"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
    }
    assert!(project_import::decode_psd(PSB, 20, &|| Ok(())).is_err());
}

#[test]
fn layerless_psb_zip_prediction_becomes_one_native_composite_layer() {
    let bytes = include_bytes!("psd-fixtures/composite-3.psb");
    let doc = project_import::decode_psd(bytes, 100, &|| Ok(()))
        .unwrap()
        .document;
    assert_eq!(doc.roots.len(), 1);
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(
        (
            layer.name.as_str(),
            layer.origin,
            layer.opacity,
            layer.visible
        ),
        ("Background", (0, 0), 255, true)
    );
    assert_eq!(layer.raster.get(0, 0).0, [17, 212, 151, 255]);
    assert!(
        pcnd::read(&pcnd::write_compact(&doc).unwrap())
            .unwrap()
            .same_semantics(&doc)
    );
}
