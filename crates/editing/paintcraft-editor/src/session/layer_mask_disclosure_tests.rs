use super::*;
use paintcraft_core::types::{
    Rect, Rgba,
    filters::{BoxBlurV1, Descriptor, LevelsRgbV1},
    resources::SharedBudget,
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack, LayerMask};
use paintcraft_raster::coverage_mask::A8Mask;
fn base() -> Document {
    paintcraft_document::new_document(2, 1, Some(Rgba([7, 31, 89, 0])), 0, POLICY).unwrap()
}
fn field(budget: &SharedBudget, value: u8) -> A8Mask {
    A8Mask::uniform(
        Rect::new(-1, -1, 4, 3).unwrap(),
        value,
        budget,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap()
}
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
fn schema(receipt: &Receipt) -> String {
    receipt
        .to_value()
        .get("schema")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}
#[test]
fn filterless_stored_layer_presence_selects_v5_for_every_enabled_and_zero_full_intent() {
    let budget = SharedBudget::new(8192);
    for enabled in [false, true] {
        for value in [0, 127, 255] {
            let mut doc = base();
            let id = doc.roots[0];
            let original = receipt(&doc).to_value();
            doc.layers.get_mut(&id).unwrap().mask =
                Some(LayerMask::new(enabled, field(&budget, value)));
            assert_eq!(masked_document_features(&doc), ["layer-masks.v1"]);
            assert_eq!(schema(&receipt(&doc)), "paintcraft.result/v5");
            let mut owner = Session::new(doc.clone());
            let batch = format!(
                r#"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"0"}},"operations":[{{"op":"selection.clear"}}]}}"#,
                doc.state_id
            );
            let emitted = owner.apply_json(batch.as_bytes()).unwrap();
            assert_eq!(schema(&emitted), "paintcraft.result/v5");
            assert_eq!(emitted.document_features, ["layer-masks.v1"]);
            assert_eq!(
                emitted.to_value().get("required_filters"),
                Some(&Value::Array(Vec::new()))
            );
            assert!(emitted.required_filters.is_empty());
            assert!(owner.document().layers[&id].mask.is_some());
            doc.layers.get_mut(&id).unwrap().mask = None;
            assert_eq!(receipt(&doc).to_value(), original);
            assert_eq!(
                paintcraft_core::types::json::canonical(&receipt(&doc).to_value()),
                paintcraft_core::types::json::canonical(&original)
            );
        }
    }
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn combined_mask_and_catalog_facts_are_retained_and_v5_takes_precedence() {
    let budget = SharedBudget::new(8192);
    let mut doc = base();
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(
            Uuid::from_u128(1),
            false,
            Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        )
        .with_mask(Some(FilterMask::new(false, field(&budget, 0)))),
        FilterInstance::new(
            Uuid::from_u128(2),
            false,
            Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
        ),
    ])
    .unwrap();
    let previous = receipt(&doc).to_value();
    assert_eq!(schema(&receipt(&doc)), "paintcraft.result/catalog-v1");
    doc.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(false, field(&budget, 255)));
    let result = receipt(&doc);
    assert_eq!(
        result.document_features,
        [
            "layer-point-filters.v1",
            "layer-filter-masks.v1",
            "layer-spatial-filters.v1",
            "layer-filter-catalog.v1",
            "layer-masks.v1"
        ]
    );
    assert_eq!(result.required_filters, ["box-blur-v1", "levels-rgb-v1"]);
    assert_eq!(schema(&result), "paintcraft.result/v5");
    assert!(result.to_value().get("required_filters").is_some());
    doc.layers.get_mut(&id).unwrap().mask = None;
    assert_eq!(receipt(&doc).to_value(), previous);
}
#[test]
fn none_keeps_old_alias_mask_spatial_and_catalog_profiles_and_features_exact() {
    let budget = SharedBudget::new(8192);
    let mut doc = base();
    let id = doc.roots[0];
    let mut plain = receipt(&doc);
    assert_eq!(schema(&plain), "paintcraft.result/v1");
    assert!(plain.to_value().get("document_features").is_none());
    plain
        .filter_aliases
        .insert("f".into(), (id, Uuid::from_u128(1)));
    assert_eq!(schema(&plain), "paintcraft.result/v2");
    let f = FilterInstance::new(Uuid::from_u128(1), false, Descriptor::InvertRgbV1)
        .with_mask(Some(FilterMask::new(false, field(&budget, 0))));
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![f]).unwrap();
    assert_eq!(
        masked_document_features(&doc),
        ["layer-point-filters.v1", "layer-filter-masks.v1"]
    );
    assert_eq!(schema(&receipt(&doc)), "paintcraft.result/v3");
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(2),
        false,
        Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
    )])
    .unwrap();
    assert_eq!(
        masked_document_features(&doc),
        ["layer-point-filters.v1", "layer-spatial-filters.v1"]
    );
    assert_eq!(schema(&receipt(&doc)), "paintcraft.result/v4");
}

#[test]
fn own_mask_legacy_only_filters_disclose_point_feature_and_sorted_all_ids() {
    let budget = SharedBudget::new(8192);
    for descriptor in [
        Descriptor::InvertRgbV1,
        Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
    ] {
        let mut doc = base();
        let id = doc.roots[0];
        doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
            FilterInstance::new(Uuid::from_u128(1), false, descriptor),
            FilterInstance::new(Uuid::from_u128(2), true, descriptor),
        ])
        .unwrap();
        let before = receipt(&doc).to_value();
        assert!(catalog_document_requirements(&doc).is_empty());
        doc.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(false, field(&budget, 0)));
        let mut expected = vec!["layer-point-filters.v1"];
        if descriptor.is_spatial() {
            expected.push("layer-spatial-filters.v1");
        }
        expected.push("layer-masks.v1");
        assert_eq!(masked_document_features(&doc), expected);
        assert!(!masked_document_features(&doc).contains(&"layer-filter-catalog.v1"));
        assert_eq!(catalog_document_requirements(&doc), [descriptor.id()]);
        assert_eq!(receipt(&doc).required_filters, [descriptor.id()]);
        assert_eq!(schema(&receipt(&doc)), "paintcraft.result/v5");
        doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
            FilterInstance::new(Uuid::from_u128(3), false, Descriptor::InvertRgbV1),
            FilterInstance::new(
                Uuid::from_u128(4),
                false,
                Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
            ),
        ])
        .unwrap();
        assert_eq!(
            catalog_document_requirements(&doc),
            ["box-blur-v1", "invert-rgb-v1"]
        );
        doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
            FilterInstance::new(Uuid::from_u128(1), false, descriptor),
            FilterInstance::new(Uuid::from_u128(2), true, descriptor),
        ])
        .unwrap();
        doc.layers.get_mut(&id).unwrap().mask = None;
        assert_eq!(
            paintcraft_core::types::json::canonical(&receipt(&doc).to_value()),
            paintcraft_core::types::json::canonical(&before)
        );
    }
}
