//! Frozen independent wire specimens precede this product test source.
use paintcraft_core::types::{
    ErrorCode, LayerBlendMode,
    json::{self, Value},
    resources::SharedBudget,
};
use paintcraft_formats::pcnd;
fn decode_hex(s: &str) -> Vec<u8> {
    assert_eq!(s.len() % 2, 0);
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn read(bytes: &[u8]) -> paintcraft_core::types::Result<paintcraft_document::Document> {
    let budget = SharedBudget::new(8 << 20);
    pcnd::read_with_admission_checked(
        bytes,
        16,
        1 << 20,
        8,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 2 << 20,
                budget: &budget,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 2 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
}
#[test]
fn independent_pcnd8_literals_preserve_intent_and_legacy_minimal_bytes() {
    let oracle = json::parse(include_bytes!("pcnd-blend-fixtures/oracle.json")).unwrap();
    let mut accepted = 0;
    let mut rejected = 0;
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("name").unwrap().as_str().unwrap();
        let bytes = decode_hex(row.get("hex").unwrap().as_str().unwrap());
        if row.get("accepted").unwrap().as_bool() == Some(false) {
            let expected = if row.get("error").unwrap().as_str() == Some("CapabilityUnsupported") {
                ErrorCode::CapabilityUnsupported
            } else {
                ErrorCode::CodecFailed
            };
            assert_eq!(read(&bytes).unwrap_err().code, expected, "{name}");
            rejected += 1;
            continue;
        }
        let doc = read(&bytes).unwrap();
        let header = pcnd::probe(&bytes).unwrap();
        if name != "legacy-normal-v1" {
            assert_eq!(header.container_version, 8);
            assert_eq!(header.document_schema, "paintcraft.document/v8");
            assert!(
                header
                    .required_features
                    .iter()
                    .any(|feature| feature == "layer-blend-modes.v1")
            );
        }
        accepted += 1;
        for (id, mode) in doc
            .roots
            .iter()
            .zip(row.get("modes").unwrap().as_array().unwrap())
        {
            let expected = match mode.as_i64().unwrap() {
                0 => LayerBlendMode::Normal,
                1 => LayerBlendMode::Multiply,
                2 => LayerBlendMode::Screen,
                _ => unreachable!(),
            };
            assert_eq!(doc.layers[id].blend_mode, expected, "{name}");
        }
        assert_eq!(
            doc.source_metadata.is_some(),
            row.get("source").and_then(Value::as_bool).unwrap_or(false)
        );
        if name != "legacy-normal-v1" && name != "explicit-default-record" {
            let layer = &doc.layers[&doc.roots[0]];
            assert_eq!(
                (0..2)
                    .flat_map(|x| layer.raster.get(x, 0).0)
                    .collect::<Vec<_>>(),
                [17, 32, 255, 0, 0, 127, 200, 255]
            );
            assert_eq!(
                !layer.filters.is_empty(),
                row.get("filtered")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            );
            assert_eq!(
                layer.mask.is_some(),
                row.get("masked").and_then(Value::as_bool).unwrap_or(false)
            );
        }
        for encoded in [
            pcnd::write(&doc).unwrap(),
            pcnd::write_compact(&doc).unwrap(),
        ] {
            assert!(doc.same_semantics(&read(&encoded).unwrap()), "{name}");
            assert_eq!(encoded[4], if name == "legacy-normal-v1" { 1 } else { 8 });
        }
        if row.get("canonical_writer_equal").and_then(Value::as_bool) == Some(true) {
            assert_eq!(pcnd::write(&doc).unwrap(), bytes, "{name}");
        }
        let mut normal = doc.clone();
        for layer in normal.layers.values_mut() {
            layer.blend_mode = LayerBlendMode::Normal;
        }
        assert_ne!(pcnd::write(&normal).unwrap()[4], 8);
        if name != "legacy-normal-v1" {
            assert!(!normal.same_semantics(&doc));
        }
    }
    assert_eq!((accepted, rejected), (9, 7));
}
#[test]
fn old_visual_and_foreign_routes_refuse_stored_modes_even_when_invisible_or_zero() {
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        for (visible, opacity) in [(true, 255), (false, 255), (true, 0)] {
            let mut doc = paintcraft_document::new_document(
                1,
                1,
                Some(paintcraft_core::types::Rgba([1, 2, 3, 255])),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap();
            let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
            layer.blend_mode = mode;
            layer.visible = visible;
            layer.opacity = opacity;
            assert_eq!(
                doc.require_normal_blends().unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(
                paintcraft_formats::export::filter_bake::inspect(&doc, &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
        }
    }
}

#[test]
fn framed_identity_preserves_normal_branch_and_binds_all_stacking_tags() {
    let oracle = json::parse(include_bytes!("pcnd-blend-fixtures/oracle.json")).unwrap();
    let scope = SharedBudget::new(4 << 20)
        .scope(paintcraft_core::types::resources::AllocationClass::PaintMetadata);
    for (name, expected) in [
        (
            "multiply",
            "4551efc2f9747bbaa3a0c1a9c8f2ea4dc48d50f44d98b0db795abada21524382",
        ),
        (
            "screen",
            "ceee3f7bd59aa89dbcb2b33274acc0d3edfe81a32591df4eced5b7a3685985ab",
        ),
        (
            "mixed",
            "fb402a6cea78d2373e555a463431b4534b8e31a5a1814c2095838be5a0ddeb00",
        ),
    ] {
        let row = oracle
            .get("fixtures")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r.get("name").and_then(Value::as_str) == Some(name))
            .unwrap();
        let mut doc = read(&decode_hex(row.get("hex").unwrap().as_str().unwrap())).unwrap();
        let pure_filter_digest =
            pcnd::layer_appearance::filter_digest_checked(&doc, &scope, &|| Ok(()), &|_| Ok(()))
                .unwrap();
        let identity = pcnd::layer_blend_identity_checked(&doc, &scope, &|| Ok(()), &|_| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(paintcraft_core::types::policy::hex(&identity), expected);
        for layer in doc.layers.values_mut() {
            layer.blend_mode = LayerBlendMode::Normal;
        }
        assert_eq!(
            pcnd::layer_blend_identity_checked(&doc, &scope, &|| Ok(()), &|_| Ok(())).unwrap(),
            None
        );
        assert_eq!(
            pcnd::layer_appearance::filter_digest_checked(&doc, &scope, &|| Ok(()), &|_| Ok(()))
                .unwrap(),
            pure_filter_digest,
            "filter-only utility grants no execution/loss authority"
        );
        assert_eq!(
            pcnd::layer_blend_identity_checked(&doc, &scope, &|| Ok(()), &|_| Err(
                paintcraft_core::types::Error::new(ErrorCode::ResourceLimit, "no work")
            ))
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
    }
}
