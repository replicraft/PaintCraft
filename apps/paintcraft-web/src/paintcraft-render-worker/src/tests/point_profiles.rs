//! Independent scalar literals through the real private Worker batch selector.
use super::*;
use paintcraft_core::types::filters::{BlackWhiteV1, PosterizeRgbV1, ThresholdLumaV1};
use paintcraft_document::{FilterInstance, FilterStack};

fn point_source(masked: bool) -> Document {
    let mut doc =
        paintcraft_document::import_image(1, 1, &[20, 115, 0, 255], "point-plane").unwrap();
    let mut filter = FilterInstance::new(Uuid::from_u128(99), true, Descriptor::InvertRgbV1);
    if masked {
        let specimen = masked_plane();
        filter = filter.with_mask(
            specimen.layers[&specimen.roots[0]].filters.as_slice()[0]
                .mask()
                .cloned(),
        );
    }
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(vec![filter]).unwrap();
    doc
}
fn submit(
    service: &mut Service,
    plan: &Value,
    doc: &Document,
    generation: u64,
    operation: Value,
) -> Vec<u8> {
    let binding = service
        .prepare(&text(&request(
            plan,
            generation,
            Some(doc.roots[0]),
            operation,
            true,
            [0, 0, 1, 1],
            0,
        )))
        .unwrap();
    assert_eq!(binding.get("kind"), Some(&Value::str("frame")));
    finish(service).1
}

#[test]
fn four_new_update_profiles_and_solarize_add_route_real_private_batches_without_source_mutation() {
    let rows = [
        (
            Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(100).unwrap()),
            [87, 87, 87, 255],
            [54, 101, 44, 255],
        ),
        (
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(128).unwrap()),
            [0, 0, 0, 255],
            [10, 57, 0, 255],
        ),
        (
            Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(6).unwrap()),
            [0, 102, 0, 255],
            [10, 108, 0, 255],
        ),
        (
            Descriptor::SolarizeRgbV1,
            [20, 115, 0, 255],
            [20, 115, 0, 255],
        ),
    ];
    for masked in [false, true] {
        let doc = point_source(masked);
        let before = pcnd::write(&doc).unwrap();
        let (mut service, plan) = open(&doc);
        let layer = doc.roots[0];
        let original = &doc.layers[&layer].filters.as_slice()[0];
        for (index, (descriptor, plain, mixed)) in rows.into_iter().enumerate() {
            let operation = FilterOp::Update {
                layer: LayerRef::Id(layer),
                filter: FilterRef::Id(original.id()),
                descriptor,
            }
            .to_value()
            .unwrap();
            assert_eq!(
                submit(&mut service, &plan, &doc, index as u64 + 1, operation),
                if masked { mixed } else { plain }
            );
            let candidate = service.candidate.as_ref().unwrap();
            let actual = &candidate.layers[&layer].filters.as_slice()[0];
            assert_eq!(actual.descriptor(), descriptor);
            assert_eq!(actual.id(), original.id());
            assert_eq!(actual.mask(), original.mask());
            assert_eq!(
                candidate.layers[&layer].raster.get(0, 0),
                Rgba([20, 115, 0, 255])
            );
            assert_eq!(pcnd::write(&service.base).unwrap(), before);
            service.retire();
        }
        if !masked {
            let operation = FilterOp::Add {
                layer: LayerRef::Id(layer),
                alias: "solarize".into(),
                descriptor: Descriptor::SolarizeRgbV1,
                enabled: true,
                to_index: None,
            }
            .to_value()
            .unwrap();
            assert_eq!(
                submit(&mut service, &plan, &doc, 5, operation),
                [20, 115, 0, 255]
            );
            let stack = &service.candidate.as_ref().unwrap().layers[&layer].filters;
            assert_eq!(stack.len(), 2);
            assert_eq!(stack.as_slice()[0].descriptor(), Descriptor::InvertRgbV1);
            assert_eq!(stack.as_slice()[1].descriptor(), Descriptor::SolarizeRgbV1);
            service.retire();
            assert_eq!(pcnd::write(&service.base).unwrap(), before);
        }
    }
}

#[test]
fn malformed_mask_body_is_refused_before_command_schema_or_a8_decode_and_base_remains_reusable() {
    let doc = point_source(false);
    let before = pcnd::write(&doc).unwrap();
    let (mut service, plan) = open(&doc);
    // Deliberately not a valid MaskOp payload: the name must be refused before
    // any command schema or coverage numeric-array admission can inspect it.
    let operation = Value::object([
        ("op", Value::str("filter.mask.set")),
        (
            "layer_ref",
            Value::object([("id", Value::str(doc.roots[0].to_string()))]),
        ),
        (
            "filter_ref",
            Value::object([("id", Value::str(Uuid::from_u128(99).to_string()))]),
        ),
        (
            "coverage",
            Value::str("invalid unbounded-mask body is never decoded"),
        ),
        ("enabled", Value::Int(-1)),
    ]);
    assert_eq!(
        service
            .prepare(&text(&request(
                &plan,
                1,
                Some(doc.roots[0]),
                operation,
                true,
                [0, 0, 1, 1],
                0
            )))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(service.candidate.is_none());
    assert!(service.frame.is_none());
    assert_eq!(pcnd::write(&service.base).unwrap(), before);
    let update = FilterOp::Update {
        layer: LayerRef::Id(doc.roots[0]),
        filter: FilterRef::Id(Uuid::from_u128(99)),
        descriptor: Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(100).unwrap()),
    }
    .to_value()
    .unwrap();
    assert_eq!(
        submit(&mut service, &plan, &doc, 2, update),
        [87, 87, 87, 255]
    );
    service.retire();
    assert!(service.candidate.is_none() && service.frame.is_none());
    assert_eq!(pcnd::write(&service.base).unwrap(), before);
}
