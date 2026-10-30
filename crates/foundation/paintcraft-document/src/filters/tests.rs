use super::*;
use paintcraft_core::types::{Rgba, filters::BrightnessContrastV1};
fn instance(id: u128) -> FilterInstance {
    FilterInstance::new(Uuid::from_u128(id), true, Descriptor::InvertRgbV1)
}
#[test]
fn bounded_unique_instances_and_oversized_capacity_are_checked_without_nil_restriction() {
    assert!(FilterStack::new(vec![instance(0)]).is_ok());
    assert_eq!(
        FilterStack::new(vec![instance(1), instance(1)])
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        FilterStack::new((0..=FILTERS_MAX).map(|i| instance(i as u128)).collect())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        FilterStack::new((0..FILTERS_MAX).map(|i| instance(i as u128)).collect())
            .unwrap()
            .len(),
        FILTERS_MAX
    );
    let mut oversized = Vec::with_capacity(4096);
    oversized.push(instance(1));
    let stack = FilterStack::new(oversized).unwrap();
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.iter().len(), 1);
    assert_eq!(stack.as_slice()[0].id(), Uuid::from_u128(1));
    assert!(stack.storage_bytes() <= FILTERS_MAX * size_of::<FilterInstance>());
    assert!(stack.storage_bytes() >= size_of::<FilterInstance>());
    assert_eq!(
        FilterStack::default().digest(),
        FilterStack::new(Vec::with_capacity(4096)).unwrap().digest()
    );
}
#[test]
fn digest_and_equality_bind_order_ids_enablement_descriptor_and_parameters() {
    let inv = instance(1);
    let bc = FilterInstance::new(
        Uuid::from_u128(2),
        false,
        Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-19, 23).unwrap()),
    );
    let original = FilterStack::new(vec![inv.clone(), bc.clone()]).unwrap();
    assert_eq!(original, original.clone());
    for changed in [
        vec![bc.clone(), inv.clone()],
        vec![
            FilterInstance::new(inv.id(), false, inv.descriptor()),
            bc.clone(),
        ],
        vec![instance(3), bc.clone()],
        vec![
            inv.clone(),
            FilterInstance::new(
                bc.id(),
                bc.enabled(),
                Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-18, 23).unwrap()),
            ),
        ],
        vec![
            inv,
            FilterInstance::new(bc.id(), bc.enabled(), Descriptor::InvertRgbV1),
        ],
    ] {
        let changed = FilterStack::new(changed).unwrap();
        assert_ne!(original, changed);
        assert_ne!(original.digest(), changed.digest());
    }
    assert_ne!(original.digest(), FilterStack::default().digest());
}
#[test]
fn document_semantics_and_raw_source_versions_are_independent_of_stack_edits() {
    let original =
        crate::import_image(2, 1, &[13, 71, 201, 0, 250, 1, 20, 255], "filters").unwrap();
    let id = original.roots[0];
    let raw = &original.layers[&id].raster;
    assert!(raw.resident_tiles() > 0);
    let versions: Vec<_> = raw.tile_leases().map(|(c, t)| (c, t.version())).collect();
    let mut changed = original.clone();
    changed.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![instance(1)]).unwrap();
    assert!(!original.same_semantics(&changed));
    assert!(changed.same_semantics(&changed.clone()));
    let layer = &changed.layers[&id];
    assert_eq!(layer.raster.get(0, 0), Rgba([13, 71, 201, 0]));
    assert_eq!(
        layer
            .raster
            .tile_leases()
            .map(|(c, t)| (c, t.version()))
            .collect::<Vec<_>>(),
        versions
    );
    assert_eq!(layer.clone().filters, layer.filters);
    assert!(original.layers[&id].filters.is_empty());
}
