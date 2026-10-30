use super::*;
use paintcraft_core::types::{Rect, filters::BrightnessContrastV1, resources::SharedBudget};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
fn field(value: u8, budget: &SharedBudget) -> A8Mask {
    A8Mask::uniform(
        Rect::new(-1, 0, 3, 2).unwrap(),
        value,
        budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn unmasked_hash_stays_exact_v1_and_stored_zero_disabled_masks_select_v2() {
    let plain = FilterInstance::new(Uuid::from_u128(1), true, Descriptor::InvertRgbV1);
    let stack = FilterStack::new(vec![plain.clone()]).unwrap();
    // Independent Python hashlib/UUID bytes using the already-published v1 framing.
    assert_eq!(
        paintcraft_core::types::policy::hex(&stack.digest()),
        "247809891a024def1baec3872c933314f8038fc021887532ef17632322746f30"
    );
    let budget = SharedBudget::new(8192);
    let zero = field(0, &budget);
    let enabled = plain.with_mask(Some(FilterMask::new(true, zero.clone())));
    let disabled = plain.with_mask(Some(FilterMask::new(false, zero)));
    let a = FilterStack::new(vec![enabled]).unwrap();
    let b = FilterStack::new(vec![disabled]).unwrap();
    assert_ne!(stack.digest(), a.digest());
    assert_ne!(a.digest(), b.digest());
    assert!(
        a.mask_storage_usage().next().is_none(),
        "allzero stored intent owns no plane allocation"
    );
    assert_eq!(a.iter().next().unwrap().with_mask(None), plain);
    assert_eq!(
        FilterStack::new(vec![a.iter().next().unwrap().with_mask(None)])
            .unwrap()
            .digest(),
        stack.digest()
    );
}
#[test]
fn immutable_with_helpers_preserve_other_fields_and_share_exact_mask_ownership() {
    let budget = SharedBudget::new(8192);
    let coverage = field(127, &budget);
    let usage = coverage.storage_usage();
    let original = FilterInstance::new(Uuid::from_u128(4), false, Descriptor::InvertRgbV1)
        .with_mask(Some(FilterMask::new(false, coverage)));
    let changed = original.with_descriptor(Descriptor::BrightnessContrastV1(
        BrightnessContrastV1::new(2, -3).unwrap(),
    ));
    assert_eq!(changed.mask(), original.mask());
    assert!(!changed.enabled());
    assert_eq!(changed.id(), original.id());
    let toggled = changed.with_enabled(true);
    assert_eq!(toggled.mask(), original.mask());
    let copied = toggled.with_id(Uuid::from_u128(5));
    assert_ne!(copied.id(), original.id());
    assert_eq!(copied.mask(), original.mask());
    let stack =
        FilterStack::new(vec![original, changed.with_id(Uuid::from_u128(6)), copied]).unwrap();
    assert!(stack.mask_storage_usage().all(|u| Some(u) == usage));
    assert_eq!(budget.usage().reserved, usage.unwrap().1);
    let json = inspection(&stack);
    assert!(json.as_array().unwrap()[0].get("mask").is_some());
    drop(stack);
    drop(toggled);
    drop(changed);
    assert_eq!(budget.usage().reserved, 0);
}
