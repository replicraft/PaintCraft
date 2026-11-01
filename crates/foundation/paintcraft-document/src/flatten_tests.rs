//! Eligibility is a structural intent rule, independent of pixel equality.
use super::*;
use paintcraft_core::types::filters::{AddNoiseRgbV1, BoxBlurV1, Descriptor, HueSaturationRgbV1};
fn plain() -> Document {
    new_document(2, 1, Some(Rgba([255; 4])), 0, POLICY).unwrap()
}
#[test]
fn empty_and_plain_projected_document_are_noops_without_mutation() {
    let mut doc = plain();
    let before = doc.clone();
    assert!(!doc.requires_flatten());
    assert!(doc.same_semantics(&before));
    assert_eq!(doc.state_id, before.state_id);
    doc.title = Some("different title".into());
    doc.layers.get_mut(&doc.roots[0]).unwrap().name = "custom name".into();
    assert!(!doc.requires_flatten());
    doc.layers.clear();
    doc.roots.clear();
    assert!(!doc.requires_flatten());
}
#[test]
fn one_layer_stored_filters_count_even_disabled_identity_and_zero_radius() {
    for descriptor in [
        Descriptor::InvertRgbV1,
        Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
        Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::default()),
        Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(
                0,
                65535,
                true,
                paintcraft_core::types::filters::NoiseDistribution::Gaussian,
            )
            .unwrap(),
        ),
    ] {
        for enabled in [false, true] {
            let mut doc = plain();
            doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
                FilterStack::new(vec![FilterInstance::new(
                    Uuid::from_u128(13),
                    enabled,
                    descriptor,
                )])
                .unwrap();
            let before = doc.clone();
            assert!(doc.requires_flatten());
            assert_eq!(doc.roots.len(), 1);
            assert!(doc.same_semantics(&before));
        }
    }
}
#[test]
fn hidden_opacity_origin_and_extent_require_projection_with_one_actual_layer() {
    for case in 0..6 {
        let mut doc = plain();
        let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
        match case {
            0 => layer.visible = false,
            1 => layer.opacity = 0,
            2 => layer.opacity = 128,
            3 => layer.origin = (1, -1),
            4 => layer.raster = RasterLayer::new(1, 1, Rgba([255; 4])),
            _ => layer.raster = RasterLayer::new(2, 2, Rgba([255; 4])),
        };
        assert!(doc.requires_flatten());
        assert_eq!(doc.roots.len(), 1);
        assert_eq!(doc.layers.len(), 1);
    }
}
#[test]
fn multiple_layers_require_projection_even_when_the_visible_color_is_identical() {
    let mut doc = plain();
    let mut extra = doc.layers[&doc.roots[0]].clone();
    extra.id = Uuid::from_u128(12345);
    doc.roots.push(extra.id);
    doc.layers.insert(extra.id, extra);
    assert_eq!(doc.roots.len(), 2);
    assert!(doc.requires_flatten());
}
