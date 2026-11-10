use super::*;
use paintcraft_core::types::{
    json,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::LayerBlendMode;
use std::cell::{Cell, RefCell};
fn cases() -> Vec<json::Value> {
    json::parse(include_bytes!(
        "../../../../../tests/counted-blend-snapshot-fixtures/oracle.json"
    ))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn bytes(row: &json::Value) -> Vec<u8> {
    row.get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn read(wire: &[u8]) -> Document {
    let budget = SharedBudget::new(16 << 20);
    crate::pcnd::read_with_admission_checked(
        wire,
        1024,
        1 << 20,
        16,
        crate::pcnd::ReadAdmission {
            metadata: crate::pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget: &budget,
            },
            coverage: crate::pcnd::CoverageAdmission {
                bytes: 8 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn independent_complete_source8_wires_count_and_encode_exactly_on_original_scope() {
    for row in cases() {
        let expected = bytes(&row);
        let source = read(&expected);
        let before = source.clone();
        let budget = SharedBudget::new(8 << 20);
        let scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let check = || Ok(());
        let spend = |n: u64| {
            used.set(used.get().checked_add(n).unwrap());
            Ok(())
        };
        let fact =
            count_portable_blend_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
        assert_eq!(fact.wire_len(), expected.len() as u64);
        assert_eq!(fact.document_id(), source.document_id);
        assert_eq!(fact.state_id(), source.state_id);
        let before_encode = used.get();
        let output = fact.encode().unwrap();
        assert_eq!(output.as_bytes(), expected.as_slice());
        assert_eq!(output.as_bytes()[4], 8);
        assert_eq!(output.len(), expected.len());
        assert!(!output.is_empty());
        assert!(used.get() > before_encode);
        assert!(source.same_semantics(&before));
        let sizes = blend_counted_snapshot_control_bytes();
        assert_eq!(
            scope.footprint().live,
            sizes.fact + sizes.bytes_owner + expected.len() as u64
        );
        drop(fact);
        assert_eq!(
            scope.footprint().live,
            sizes.bytes_owner + expected.len() as u64
        );
        drop(output);
        assert_eq!(scope.footprint().live, 0);
        drop(scope);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn stored_hidden_filterless_and_zero_opacity_blends_still_emit_eight_while_normal_refuses() {
    let expected = bytes(&cases()[0]);
    let mut source = read(&expected);
    for layer in source.layers.values_mut() {
        layer.filters = Default::default();
        layer.mask = None;
    }
    let top = *source.roots.last().unwrap();
    {
        let l = source.layers.get_mut(&top).unwrap();
        l.visible = false;
        l.opacity = 0;
        l.blend_mode = LayerBlendMode::Screen;
    }
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let check = || Ok(());
    let spend = |_| Ok(());
    let fact =
        count_portable_blend_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    let output = fact.encode().unwrap();
    assert_eq!(output.as_bytes()[4], 8);
    drop(output);
    drop(fact);
    for layer in source.layers.values_mut() {
        layer.blend_mode = LayerBlendMode::Normal;
    }
    let error =
        count_portable_blend_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn original_callback_error_is_moved_and_retry_keeps_the_same_paid_counter() {
    let source = read(&bytes(&cases()[0]));
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let original = Error::new(
        ErrorCode::OperationCancelled,
        "original count8 diagnostic".repeat(20),
    )
    .at(31);
    let pointer = original.message.as_ptr();
    let slot = RefCell::new(Some(original));
    let used = Cell::new(7u64);
    let spend = |n: u64| {
        used.set(used.get().checked_add(n).unwrap());
        Ok(())
    };
    let check = || match slot.borrow_mut().take() {
        Some(e) => Err(e),
        None => Ok(()),
    };
    let error =
        count_portable_blend_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap_err();
    assert_eq!(error.message.as_ptr(), pointer);
    assert_eq!(error.operation_index, Some(31));
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    let prefix = if cfg!(target_pointer_width = "64") {
        1456
    } else {
        1344
    };
    assert_eq!(used.get(), 7 + prefix);
    assert_eq!(scope.footprint().live, 0);
    let fact =
        count_portable_blend_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    let output = fact.encode().unwrap();
    assert!(used.get() > 7 + prefix);
    drop(output);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn zeroing_denial_precedes_output_allocation_and_keeps_original_fact_and_debt() {
    let expected = bytes(&cases()[0]);
    let source = read(&expected);
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let deny = Cell::new(false);
    let check = || Ok(());
    let spend = |n: u64| {
        if deny.get() && n == expected.len() as u64 {
            return Err(Error::new(ErrorCode::OperationCancelled, "zero8 denied"));
        }
        used.set(used.get().checked_add(n).unwrap());
        Ok(())
    };
    let fact =
        count_portable_blend_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    let before = used.get();
    deny.set(true);
    let error = fact.encode().unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(
        used.get() - before,
        if cfg!(target_pointer_width = "64") {
            1760
        } else {
            1632
        }
    );
    assert_eq!(
        scope.footprint().live,
        blend_counted_snapshot_control_bytes().fact
    );
    deny.set(false);
    let output = fact.encode().unwrap();
    assert_eq!(output.as_bytes(), expected);
    assert!(used.get() > before);
    drop(output);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn independent_target_control_and_full_result_limits_are_actual_defining_types() {
    validate_layouts().unwrap();
    let s = blend_counted_snapshot_control_bytes();
    assert_eq!(
        [s.fact, s.counting_sink, s.bytes_owner, s.fixed_sink],
        if cfg!(target_pointer_width = "64") {
            [96, 56, 96, 64]
        } else {
            [56, 40, 56, 40]
        }
    );
}

#[test]
fn bytes_alone_keep_original_factory_capacity_after_fact_and_caller_factory_drop() {
    let expected = bytes(&cases()[0]);
    let source = read(&expected);
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let check = || Ok(());
    let spend = |_| Ok(());
    let fact =
        count_portable_blend_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    let bytes = fact.encode().unwrap();
    drop(fact);
    drop(scope);
    assert_eq!(budget.usage().reserved, 4 << 20);
    assert_eq!(bytes.as_bytes(), expected.as_slice());
    drop(bytes);
    assert_eq!(budget.usage().reserved, 0);
}
