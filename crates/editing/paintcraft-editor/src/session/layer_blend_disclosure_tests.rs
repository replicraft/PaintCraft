use super::*;
use paintcraft_core::types::{LayerBlendMode, Rgba, filters::Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
fn receipt(doc: &Document) -> Receipt {
    Receipt {
        committed: false,
        state_id: doc.state_id,
        publication_epoch: 0,
        history_id: None,
        aliases: BTreeMap::new(),
        filter_aliases: BTreeMap::new(),
        document_features: masked_document_features(doc),
        required_filters: catalog_document_requirements(doc),
    }
}
#[test]
fn every_stored_non_normal_mode_discloses_v6_and_reset_restores_old_minimum() {
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        for (visible, opacity) in [(true, 255), (false, 255), (true, 0)] {
            let mut doc =
                paintcraft_document::new_document(1, 1, Some(Rgba([17, 32, 255, 0])), 0, POLICY)
                    .unwrap();
            let old = receipt(&doc).to_value();
            let id = doc.roots[0];
            let layer = doc.layers.get_mut(&id).unwrap();
            layer.blend_mode = mode;
            layer.visible = visible;
            layer.opacity = opacity;
            let value = receipt(&doc).to_value();
            assert_eq!(
                value.get("schema").and_then(Value::as_str),
                Some("paintcraft.result/v6")
            );
            assert_eq!(masked_document_features(&doc), ["layer-blend-modes.v1"]);
            assert_eq!(value.get("required_filters"), Some(&Value::Array(vec![])));
            doc.layers.get_mut(&id).unwrap().blend_mode = LayerBlendMode::Normal;
            assert_eq!(receipt(&doc).to_value(), old);
        }
    }
}
#[test]
fn ordinary_point_filters_in_blend_documents_are_truthfully_disclosed_without_fake_profile() {
    let mut doc =
        paintcraft_document::new_document(1, 1, Some(Rgba([0, 0, 0, 255])), 0, POLICY).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.blend_mode = LayerBlendMode::Multiply;
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        uuid::Uuid::from_u128(71),
        false,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    assert_eq!(
        masked_document_features(&doc),
        ["layer-point-filters.v1", "layer-blend-modes.v1"]
    );
    assert_eq!(receipt(&doc).required_filters, ["invert-rgb-v1"]);
    doc.layers.get_mut(&doc.roots[0]).unwrap().blend_mode = LayerBlendMode::Normal;
    assert_eq!(
        receipt(&doc)
            .to_value()
            .get("schema")
            .and_then(Value::as_str),
        Some("paintcraft.result/v1")
    );
}
