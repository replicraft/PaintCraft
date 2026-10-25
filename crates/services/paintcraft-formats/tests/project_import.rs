//! Foreign layered conversion keeps raw samples and requires explicit limitations.
use paintcraft_core::types::{Error, ErrorCode, json};
use paintcraft_formats::{
    pcnd, project_import,
    registry::{self, Frontend, Platform},
};
use sha2::{Digest, Sha256};
use std::cell::Cell;
const SAMPLE: &[u8] =
    include_bytes!("../../../adapters/paint-lib/tests/fixtures/mspaintlib-sample.paint");

#[test]
fn openraster_conversion_keeps_layers_matches_independent_merged_preview_and_roundtrips() {
    let bytes = include_bytes!("ora-fixtures/deflated.ora");
    for platform in [Platform::Native, Platform::Web] {
        assert!(registry::admit_open(Frontend::Studio, "sample.ora", bytes, platform).is_err());
        registry::admit_import(Frontend::Studio, "sample.ora", bytes, platform).unwrap();
        for classic in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            assert!(registry::admit_import(classic, "sample.ora", bytes, platform).is_err());
        }
    }
    let converted = project_import::decode_ora(bytes, 100, &|| Ok(())).unwrap();
    assert_eq!(converted.losses, ["ora-resolution-ignored"]);
    let doc = converted.document;
    assert_eq!(doc.roots.len(), 3);
    let top = &doc.layers[&doc.roots[1]];
    assert_eq!(top.name, "日本語 & paint");
    assert_eq!(top.origin, (1, 1));
    assert_eq!(top.raster.get(1, 0).0, [11, 12, 13, 0]);
    assert!(!doc.layers[&doc.roots[2]].visible);
    let frame = paintcraft_render::render_flat(&doc).unwrap();
    assert_eq!(
        paintcraft_document::hex(&Sha256::digest(frame)),
        "b9ab523f4f328e250ee9fc690c7ffa0cc8187bc83d832703b1188f4b3320a44b"
    );
    assert!(
        pcnd::read(&pcnd::write_compact(&doc).unwrap())
            .unwrap()
            .same_semantics(&doc)
    );
    assert!(project_import::decode_ora(bytes, 7, &|| Ok(())).is_err());
}

#[test]
fn converted_paint_layers_match_independent_hashes_and_roundtrip_pcnd() {
    assert!(
        registry::admit_open(Frontend::Studio, "sample.paint", SAMPLE, Platform::Native).is_err()
    );
    registry::admit_import(Frontend::Studio, "sample.paint", SAMPLE, Platform::Native).unwrap();
    let converted = project_import::decode_paint(SAMPLE, 4_194_304, &|| Ok(())).unwrap();
    assert_eq!(converted.losses, project_import::PAINT_LOSSES);
    let doc = converted.document;
    assert_eq!(doc.canvas, (333, 271));
    assert_eq!(
        doc.roots.len(),
        5,
        "four source layers plus explicit converted canvas fill"
    );
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.default_pixel().0,
        [107, 107, 107, 255]
    );
    let oracle = json::parse(include_bytes!(
        "../../../adapters/paint-lib/tests/fixtures/sample-oracle.json"
    ))
    .unwrap();
    let expected = oracle.get("layers").unwrap().as_array().unwrap();
    for (id, entry) in doc.roots[1..].iter().zip(expected) {
        let layer = &doc.layers[id];
        assert_eq!(
            (layer.visible, layer.opacity, layer.origin),
            (true, 255, (0, 0))
        );
        let mut hash = Sha256::new();
        for y in 0..i64::from(layer.raster.height()) {
            for x in 0..i64::from(layer.raster.width()) {
                hash.update(layer.raster.get(x, y).0);
            }
        }
        assert_eq!(
            paintcraft_document::hex(&hash.finalize()),
            entry.get("raw_rgba_sha256").unwrap().as_str().unwrap()
        );
    }
    assert!(
        pcnd::read(&pcnd::write_compact(&doc).unwrap())
            .unwrap()
            .same_semantics(&doc)
    );
    let again = project_import::decode_paint(SAMPLE, 4_194_304, &|| Ok(())).unwrap();
    assert!(again.document.same_semantics(&doc));
}
#[test]
fn conversion_refuses_bad_graphs_budgets_and_cancellation_without_partial_document() {
    assert_eq!(
        project_import::decode_paint(SAMPLE, 1, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut bad = SAMPLE.to_vec();
    bad[..4].copy_from_slice(&7u32.to_be_bytes());
    assert!(project_import::decode_paint(&bad, 4_194_304, &|| Ok(())).is_err());
    let count = Cell::new(0usize);
    project_import::decode_paint(SAMPLE, 4_194_304, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in [1, 5, 20, count.get() / 2, count.get()] {
        let observed = Cell::new(0usize);
        let result = project_import::decode_paint(SAMPLE, 4_194_304, &|| {
            observed.set(observed.get() + 1);
            if observed.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "injected"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    }
}

#[test]
fn pdn_conversion_keeps_verified_layers_names_visibility_opacity_and_pixels() {
    let bytes = include_bytes!("../../../adapters/pdn-lib/tests/fixtures/paintdotjs-2-layer.pdn");
    registry::admit_open(Frontend::Studio, "sample.pdn", bytes, Platform::Native).unwrap();
    assert!(
        registry::admit_open(Frontend::ClassicV11, "sample.pdn", bytes, Platform::Native).is_err()
    );
    let converted = project_import::decode_pdn(bytes, 100, &|| Ok(())).unwrap();
    assert_eq!(converted.losses, [project_import::PDN_LOSS]);
    let doc = converted.document;
    assert_eq!(doc.canvas, (3, 2));
    assert_eq!(doc.roots.len(), 2);
    let base = &doc.layers[&doc.roots[0]];
    let overlay = &doc.layers[&doc.roots[1]];
    assert_eq!(base.name, "Base 日本語");
    assert_eq!(overlay.name, "Overlay é");
    assert_eq!((base.visible, base.opacity), (true, 201));
    assert_eq!((overlay.visible, overlay.opacity), (false, 73));
    for (layer, expected) in [
        (
            base,
            "d6931877bd07347276a7149f0ff06c97a16de2b64c416385f72e649facad7c56",
        ),
        (
            overlay,
            "4f570b69e8ee688b9e255dab9f8842d18f632314066f17580b70460852dc50c8",
        ),
    ] {
        let mut hash = Sha256::new();
        for y in 0..2 {
            for x in 0..3 {
                hash.update(layer.raster.get(x, y).0);
            }
        }
        assert_eq!(paintcraft_document::hex(&hash.finalize()), expected);
    }
    assert!(
        pcnd::read(&pcnd::write_compact(&doc).unwrap())
            .unwrap()
            .same_semantics(&doc)
    );
    assert_eq!(
        project_import::decode_pdn(bytes, 1, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
