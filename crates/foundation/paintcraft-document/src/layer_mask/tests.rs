use super::*;
use crate::{FilterInstance, FilterStack, POLICY, new_document};
use paintcraft_core::types::{
    ErrorCode, Rect, Rgba, filters::Descriptor, policy::hex, resources::SharedBudget,
};
use uuid::Uuid;
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
#[test]
fn immutable_mask_hash_binds_enabled_domain_and_coverage_without_filter_hash_change() {
    let budget = SharedBudget::new(8192);
    let coverage = field(&budget, 127);
    // Existing independently assembled A8 canonical golden, also frozen in
    // filters/exposure_tests. New wrapper bytes independently hashed in Python.
    assert_eq!(
        hex(&coverage.digest()),
        "5e38cf1164b88cd9c781811765f578f0ecbb54d0cf24c32a0f5d7b9dabf9a549"
    );
    let a = LayerMask::new(false, coverage.clone());
    let b = a.with_enabled(true);
    assert_eq!(
        hex(&a.digest()),
        "42d8ee64eddf83b947e75e026f5eda4fc1444bb5979db1ed996abdeb2b98b5d7"
    );
    assert_eq!(
        hex(&b.digest()),
        "c2c33acca358be03d4d1f7d1e9aad0358eca6709260ffd4e946363a69ef0e40c"
    );
    assert_eq!(a.coverage().storage_usage(), b.coverage().storage_usage());
    assert!(!a.enabled());
    assert!(b.enabled());
    assert_ne!(b.digest(), b.with_coverage(field(&budget, 128)).digest());
    let shifted = coverage.translated_checked(1, 0, 1024, &|| Ok(())).unwrap();
    assert_ne!(a.digest(), a.with_coverage(shifted).digest());
    let stack = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(1),
        true,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    assert_eq!(
        hex(&stack.digest()),
        "247809891a024def1baec3872c933314f8038fc021887532ef17632322746f30"
    );
}
#[test]
fn none_factory_semantics_and_disabled_zero_full_presence_are_distinct_and_fail_closed() {
    let d = new_document(2, 1, Some(Rgba([4, 5, 6, 0])), 0, POLICY).unwrap();
    let id = d.roots[0];
    assert!(d.layers[&id].mask.is_none());
    assert!(!d.has_layer_masks());
    assert!(!d.requires_flatten());
    d.require_no_layer_masks().unwrap();
    let source = d.layers[&id].raster.clone();
    let filters = d.layers[&id].filters.digest();
    let budget = SharedBudget::new(8192);
    for enabled in [false, true] {
        for value in [0, 127, 255] {
            let mut copy = d.clone();
            copy.layers.get_mut(&id).unwrap().mask =
                Some(LayerMask::new(enabled, field(&budget, value)));
            assert!(copy.has_layer_masks());
            assert!(copy.requires_flatten());
            assert!(!copy.same_semantics(&d));
            assert_eq!(
                copy.require_no_layer_masks().unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(copy.layers[&id].raster, source);
            assert_eq!(copy.layers[&id].filters.digest(), filters);
            let m = copy.layers[&id].mask.as_ref().unwrap();
            assert_eq!(m.coverage().coverage(-1, -1), value);
            assert_eq!(m.coverage().coverage(-2, -1), 0);
            copy.layers.get_mut(&id).unwrap().mask = None;
            assert!(copy.same_semantics(&d));
        }
    }
}
#[test]
fn layer_storage_iterator_exposes_shared_filter_and_layer_backings_for_one_dedup_set() {
    use crate::FilterMask;
    let budget = SharedBudget::new(8192);
    let field = field(&budget, 127);
    let usage = field.storage_usage().unwrap();
    let mut d = new_document(2, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = d.roots[0];
    let layer = d.layers.get_mut(&id).unwrap();
    layer.mask = Some(LayerMask::new(false, field.clone()));
    layer.filters = FilterStack::new(vec![
        FilterInstance::new(Uuid::from_u128(1), true, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(true, field))),
    ])
    .unwrap();
    assert_eq!(
        layer.mask_storage_usage().collect::<Vec<_>>(),
        vec![usage, usage]
    );
    assert_eq!(budget.usage().reserved, usage.1);
    drop(d);
    assert_eq!(budget.usage().reserved, 0);
}
