use super::*;
use paintcraft_core::types::{
    json,
    resources::{AllocationClass, SharedBudget},
};
use std::cell::{Cell, RefCell};
mod boundary;
mod controls;
mod errors;
mod parity;
fn rows() -> Vec<json::Value> {
    json::parse(include_bytes!(
        "../../../../../tests/normal-rooted-snapshot-fixtures/oracle.json"
    ))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn bytes(row: &json::Value, key: &str) -> Vec<u8> {
    row.get(key)
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
fn fixture(name: &str) -> (Document, Vec<u8>) {
    let row = rows()
        .into_iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap();
    (read(&bytes(&row, "input_hex")), bytes(&row, "expected_hex"))
}
fn error<T>(result: Result<T>) -> Error {
    match result {
        Ok(_) => panic!("expected refusal"),
        Err(error) => error,
    }
}
#[test]
fn twenty_literal_counts_and_full_real_wire_are_exact_on_same_original_ledger() {
    let sizes = normal_counted_snapshot_control_bytes();
    for row in rows() {
        let source = read(&bytes(&row, "input_hex"));
        let expected = bytes(&row, "expected_hex");
        let before = source.clone();
        let budget = SharedBudget::new(8 << 20);
        let scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let trace = RefCell::new(Vec::new());
        let used = Cell::new(0u64);
        let spend = |n: u64| {
            used.set(used.get().checked_add(n).unwrap());
            trace.borrow_mut().push(n);
            Ok(())
        };
        let check = || Ok(());
        let fact =
            count_portable_normal_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
        assert_eq!(fact.wire_len(), expected.len() as u64);
        assert_eq!(fact.document_id(), source.document_id);
        assert_eq!(fact.state_id(), source.state_id);
        assert_eq!(scope.footprint().live, sizes.fact);
        let first = trace.borrow().clone();
        let first_work = used.get();
        trace.borrow_mut().clear();
        let output = fact.encode().unwrap();
        assert_eq!(output.as_bytes(), expected);
        assert_eq!(output.len(), expected.len());
        assert!(!output.is_empty());
        let second = trace.borrow();
        assert_eq!(second[0], expected.len() as u64);
        assert_eq!(&second[1..], first);
        assert_eq!(used.get(), 2 * first_work + expected.len() as u64);
        drop(second);
        assert_eq!(
            scope.footprint().live,
            sizes.fact + sizes.bytes_owner + expected.len() as u64
        );
        assert!(source.same_semantics(&read(output.as_bytes())));
        assert!(source.same_semantics(&before));
        assert_eq!(source.state_id, before.state_id);
        drop(fact);
        assert_eq!(
            scope.footprint().live,
            sizes.bytes_owner + expected.len() as u64
        );
        drop(scope);
        assert_eq!(
            budget.usage().reserved,
            sizes.bytes_owner + expected.len() as u64
        );
        assert_eq!(output.as_bytes(), expected);
        drop(output);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn nonnormal_stored_even_hidden_or_zero_opacity_refuses_without_fact() {
    for hidden in [false, true] {
        let (mut source, _) = fixture("v1-default");
        let layer = source.layers.get_mut(&source.roots[0]).unwrap();
        layer.blend_mode = paintcraft_core::types::LayerBlendMode::Multiply;
        layer.visible = !hidden;
        layer.opacity = if hidden { 255 } else { 0 };
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let failure = error(count_portable_normal_snapshot_rooted_checked(
            &source,
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        ));
        assert_eq!(failure.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn malformed_roots_return_no_fact_and_preserve_every_document_field() {
    for malformed in 0..3 {
        let (mut source, _) = fixture("v1-default");
        match malformed {
            0 => source.roots.push(source.roots[0]),
            1 => source.roots[0] = uuid::Uuid::from_u128(99),
            _ => source.layers.get_mut(&source.roots[0]).unwrap().id = uuid::Uuid::from_u128(99),
        }
        let before = source.clone();
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let failure = error(count_portable_normal_snapshot_rooted_checked(
            &source,
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        ));
        assert_eq!(failure.code, ErrorCode::CodecFailed);
        assert_eq!(source.layers, before.layers);
        assert_eq!(source.roots, before.roots);
        assert_eq!(source.document_id, before.document_id);
        assert_eq!(source.state_id, before.state_id);
        assert_eq!(source.canvas, before.canvas);
        assert_eq!(source.title, before.title);
        assert_eq!(source.source_metadata, before.source_metadata);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
