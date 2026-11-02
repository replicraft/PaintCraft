//! Real Session receipts and actual CLI envelope, captured for external schema validation.
use super::*;
use paintcraft_commands::{Batch, LayerRef, Op};
use paintcraft_core::types::{
    LayerBlendMode, Rect, Rgba,
    filters::{BoxBlurV1, Descriptor, VibranceRgbV1},
    resources::SharedBudget,
};
use paintcraft_document::{
    Document, FilterInstance, FilterMask, FilterStack, LayerMask, POLICY, source_metadata::*,
};
use paintcraft_editor::session::Session;
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::collections::BTreeSet;
use uuid::Uuid;

fn apply_mode(session: &mut Session, mode: LayerBlendMode) -> Value {
    let ticket = session.ticket();
    let batch = Batch {
        version: 8,
        policy: POLICY.into(),
        base_state: ticket.state_id,
        base_epoch: ticket.publication_epoch,
        seed: None,
        operations: vec![Op::LayerSetBlendMode {
            layer: LayerRef::Id(session.document().roots[0]),
            mode,
        }],
        digest: format!("{:064x}", 1 + mode as u8),
    };
    session.apply(&batch).unwrap().to_value()
}
fn document(kind: u8, own: bool, metadata: bool) -> Document {
    let mut doc =
        paintcraft_document::new_document(2, 1, Some(Rgba([17, 32, 255, 0])), 0, POLICY).unwrap();
    let budget = SharedBudget::new(2 << 20);
    let coverage = A8Mask::uniform(
        Rect::new(0, 0, 2, 1).unwrap(),
        127,
        &budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    // kind0 has no filters; kind1..8 enumerates all mask/spatial/catalog bits.
    if kind > 0 {
        let bits = kind - 1;
        let mut first = FilterInstance::new(Uuid::from_u128(11), false, Descriptor::InvertRgbV1);
        if bits & 1 != 0 {
            first = first.with_mask(Some(FilterMask::new(false, coverage.clone())));
        }
        let mut filters = vec![first];
        if bits & 2 != 0 {
            filters.push(FilterInstance::new(
                Uuid::from_u128(12),
                false,
                Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
            ));
        }
        if bits & 4 != 0 {
            filters.push(FilterInstance::new(
                Uuid::from_u128(13),
                false,
                Descriptor::VibranceRgbV1(VibranceRgbV1::new(0).unwrap()),
            ));
        }
        layer.filters = FilterStack::new(filters).unwrap();
    }
    if own {
        layer.mask = Some(LayerMask::new(false, coverage));
    }
    if metadata {
        doc.source_metadata = Some(
            SourceMetadata::capture(
                SourceDescriptor {
                    format: SourceFormat::Png,
                    source_model: SourceColorModel::Rgb,
                    source_sha256: [3; 32],
                    source_bytes: 123,
                    selection: None,
                    source_dimensions: (2, 1),
                    output_dimensions: (2, 1),
                    orientation: None,
                    orientation_policy: OrientationPolicy::Unmodified,
                    resolution: None,
                },
                None,
                BYTES_MAX,
                &budget,
                &|| Ok(()),
            )
            .unwrap(),
        );
    }
    doc
}
#[test]
fn actual_session_v6_receipts_and_cli_envelopes_cover_canonical_feature_shapes() {
    let mut rows = Vec::new();
    let mut shapes = BTreeSet::new();
    for kind in 0..=8 {
        for own in [false, true] {
            for metadata in [false, true] {
                let original = document(kind, own, metadata);
                let mut session = Session::new(original.clone());
                let old = apply_mode(&mut session, LayerBlendMode::Normal);
                for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
                    let engine = apply_mode(&mut session, mode);
                    assert_eq!(
                        engine.get("schema").and_then(Value::as_str),
                        Some("paintcraft.result/v6")
                    );
                    shapes.insert(json::canonical(engine.get("document_features").unwrap()));
                    let outer = envelope(Some(engine.clone()), None, "success", 0);
                    let reset = apply_mode(&mut session, LayerBlendMode::Normal);
                    assert_eq!(reset.get("schema"), old.get("schema"));
                    assert_eq!(reset.get("document_features"), old.get("document_features"));
                    assert_eq!(reset.get("required_filters"), old.get("required_filters"));
                    assert!(session.document().same_semantics(&original));
                    rows.push(Value::object([
                        ("kind", Value::Int(i64::from(kind))),
                        ("own", Value::Bool(own)),
                        ("metadata", Value::Bool(metadata)),
                        ("mode", Value::Int(i64::from(mode as u8))),
                        ("engine", engine),
                        ("envelope", outer),
                        ("normal", reset),
                    ]));
                }
            }
        }
    }
    assert_eq!(shapes.len(), 36);
    assert_eq!(rows.len(), 72);
    // Optional evidence capture only; no production hooks or stdout protocol change.
    if let Some(path) = std::env::var_os("PAINTCRAFT_BLEND_SCHEMA_RECEIPTS") {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(&json::canonical(&Value::Array(rows)))
            .unwrap();
    }
}
