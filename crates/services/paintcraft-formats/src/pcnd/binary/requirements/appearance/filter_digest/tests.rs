use super::*;
use paintcraft_core::types::{Rect, Rgba, filters::Descriptor, resources::AllocationClass};
use paintcraft_document::{FilterInstance, FilterStack, LayerMask};
fn document() -> Document {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([17, 31, 47, 0])),
        7,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let first = doc.roots[0];
    doc.layers.get_mut(&first).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(99),
        false,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    let mut second = doc.layers[&first].clone();
    second.id = Uuid::from_u128(44);
    second.origin = (-7, 11);
    second.visible = false;
    second.opacity = 127;
    doc.roots.push(second.id);
    doc.layers.insert(second.id, second);
    doc
}
#[test]
fn exact_v1_root_order_fingerprint_does_not_mint_own_mask_bake_authority() {
    let doc = document();
    let budget = SharedBudget::new(8 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let expected = crate::export::filter_bake::inspect(&doc, &|| Ok(()))
        .unwrap()
        .digest();
    assert_eq!(
        filter_digest_checked(&doc, &scope, &|| Ok(()), &|_| Ok(())).unwrap(),
        expected
    );
    let mut reversed = doc.clone();
    reversed.roots.reverse();
    let other = filter_digest_checked(&reversed, &scope, &|| Ok(()), &|_| Ok(())).unwrap();
    assert_ne!(other, expected);
    assert_eq!(
        other,
        crate::export::filter_bake::inspect(&reversed, &|| Ok(()))
            .unwrap()
            .digest()
    );
    for (enabled, coverage) in [(true, 0), (false, 0), (true, 255), (false, 255)] {
        let mut own = doc.clone();
        let id = own.roots[0];
        let mask = A8Mask::uniform(
            Rect::new(-1, 0, 4, 1).unwrap(),
            coverage,
            &budget,
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        own.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(enabled, mask));
        assert_eq!(
            filter_digest_checked(&own, &scope, &|| Ok(()), &|_| Ok(())).unwrap(),
            expected
        );
        assert_eq!(
            crate::export::filter_bake::inspect(&own, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            crate::export::filter_bake::Policy::capture(
                &own,
                crate::export::metadata_policy::Target::new(
                    crate::registry::Family::Png,
                    crate::registry::Operation::ExportFlat
                ),
                crate::export::filter_bake::Intent::Bake,
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
#[test]
fn whole_shape_caller_scratch_and_spent_parent_refuse_without_changing_source() {
    let doc = document();
    let before = crate::export::filter_bake::inspect(&doc, &|| Ok(()))
        .unwrap()
        .digest();
    let small = SharedBudget::new(0);
    let small = small.scope(AllocationClass::SharedBudget);
    assert!(filter_digest_checked(&doc, &small, &|| Ok(()), &|_| Ok(())).is_err());
    let budget = SharedBudget::new(8 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let mut bad = doc.clone();
    bad.roots[1] = bad.roots[0];
    assert!(filter_digest_checked(&bad, &scope, &|| Ok(()), &|_| Ok(())).is_err());
    let mut bad = doc.clone();
    bad.roots.pop();
    assert!(filter_digest_checked(&bad, &scope, &|| Ok(()), &|_| Ok(())).is_err());
    let mut bad = doc.clone();
    bad.layers.get_mut(&bad.roots[0]).unwrap().id = Uuid::from_u128(999);
    assert!(filter_digest_checked(&bad, &scope, &|| Ok(()), &|_| Ok(())).is_err());
    assert_eq!(
        filter_digest_checked(&doc, &scope, &|| Ok(()), &|_| Err(resource()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        filter_digest_checked(
            &doc,
            &scope,
            &|| Err(Error::new(ErrorCode::OperationCancelled, "stop")),
            &|_| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(
        crate::export::filter_bake::inspect(&doc, &|| Ok(()))
            .unwrap()
            .digest(),
        before
    );
}
