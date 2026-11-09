use super::*;
use paintcraft_core::types::{Rect, resources::SharedBudget};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
#[test]
fn owned_parts_preserve_descriptor_flags_and_move_the_only_mask_owner() {
    let id = Uuid::from_u128(7);
    let descriptor = Descriptor::InvertRgbV1;
    assert_eq!(
        FilterInstance::from_owned_parts(id, true, descriptor, None),
        FilterInstance::new(id, true, descriptor)
    );
    let budget = SharedBudget::new(1 << 20);
    let coverage = A8Mask::uniform(
        Rect::from_origin_size(0, 0, 2, 1).unwrap(),
        128,
        &budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let identity = coverage.storage_usage().unwrap().0;
    let retained = budget.usage().reserved;
    let value = FilterInstance::from_owned_parts(
        id,
        false,
        descriptor,
        Some(FilterMask::new(false, coverage)),
    );
    assert_eq!(value.id(), id);
    assert!(!value.enabled());
    assert_eq!(value.descriptor(), descriptor);
    let mask = value.mask().unwrap();
    assert!(!mask.enabled());
    assert_eq!(mask.coverage().coverage(0, 0), 128);
    assert_eq!(mask.coverage().storage_usage().unwrap().0, identity);
    assert_eq!(budget.usage().reserved, retained);
    drop(value);
    assert_eq!(budget.usage().reserved, 0);
}
