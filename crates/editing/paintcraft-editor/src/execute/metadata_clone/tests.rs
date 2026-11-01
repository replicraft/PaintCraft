use super::*;
use paintcraft_commands::{FilterRef, LayerRef};
use paintcraft_core::types::{
    json::Value,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterMask, LayerMask};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::cell::Cell;
fn document() -> Document {
    let mut doc = paintcraft_document::import_image(1, 1, &[7, 8, 9, 0], "metadata-stage").unwrap();
    let coverage = A8Mask::uniform(
        Rect::from_origin_size(-1, 0, 3, 1).unwrap(),
        127,
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.mask = Some(LayerMask::new(false, coverage.clone()));
    layer.filters = FilterStack::new(vec![
        FilterInstance::new(
            Uuid::from_u128(7),
            true,
            paintcraft_core::types::filters::Descriptor::InvertRgbV1,
        )
        .with_mask(Some(FilterMask::new(true, coverage))),
    ])
    .unwrap();
    doc
}
fn batch(doc: &Document) -> Batch {
    let op = FilterOp::Duplicate {
        layer: LayerRef::Id(doc.roots[0]),
        filter: FilterRef::Id(Uuid::from_u128(7)),
        alias: "copy".into(),
        to_index: None,
    };
    let operations = vec![op.to_value().unwrap()];
    Batch::from_generated_value(Value::object([
        (
            "schema",
            Value::str(Batch::schema_for_operations(&operations)),
        ),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(paintcraft_document::POLICY)),
        (
            "base",
            Value::object([
                ("state_id", Value::str(doc.state_id.to_string())),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ]))
    .unwrap()
}
#[test]
fn churned_source_uses_same_real_stage_aliases_masks_and_hidden_source_without_history_capacity() {
    let mut source = document();
    let clean = source.clone();
    let original_layer = source.layers[&source.roots[0]].clone();
    for i in 100..612 {
        let mut layer = original_layer.clone();
        layer.id = Uuid::from_u128(i);
        source.layers.insert(layer.id, layer);
    }
    for i in 100..612 {
        source.layers.remove(&Uuid::from_u128(i));
    }
    let batch = batch(&source);
    let a = quote_checked(&source, &batch, &|| Ok(()), &|_| Ok(())).unwrap();
    let b = quote_checked(&clean, &batch, &|| Ok(()), &|_| Ok(())).unwrap();
    assert_eq!(a.peak_bytes, b.peak_bytes);
    let expected = execute_with_admission_checked(
        &source,
        Selection::Inactive,
        &batch,
        Admission::new(0),
        &|| Ok(()),
    )
    .unwrap();
    let budget = SharedBudget::new(a.peak_bytes);
    let scope = budget
        .reserve_capacity(a.peak_bytes)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let actual = stage_checked(
        &source,
        &batch,
        Admission::new(0),
        &scope,
        &|| Ok(()),
        &|_| Ok(()),
    )
    .unwrap();
    assert_document_equal(actual.document(), &expected.doc);
    assert_eq!(actual.filter_aliases(), &expected.filter_aliases);
    assert_eq!(
        actual.document().layers.capacity(),
        fresh_map_layout::<Uuid, Layer>(1).unwrap().capacity
    );
    assert_document_equal(&source, &clean);
    let root = source.roots[0];
    assert_eq!(
        actual.document().layers[&root].raster.get(0, 0),
        Rgba([7, 8, 9, 0])
    );
    let source_backing = source.layers[&root]
        .mask
        .as_ref()
        .unwrap()
        .coverage()
        .storage_usage()
        .unwrap()
        .0;
    assert_eq!(
        actual.document().layers[&root]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .storage_usage()
            .unwrap()
            .0,
        source_backing
    );
    assert!(scope.footprint().live < a.peak_bytes);
    drop(actual);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn peak_minus_one_refuses_before_seed_and_cancellation_keeps_source_and_aliases_unpublished() {
    let source = document();
    let before = source.clone();
    let batch = batch(&source);
    let quoted_calls = Cell::new(0);
    let q = quote_checked(
        &source,
        &batch,
        &|| {
            quoted_calls.set(quoted_calls.get() + 1);
            Ok(())
        },
        &|_| Ok(()),
    )
    .unwrap();
    let budget = SharedBudget::new(q.peak_bytes * 2);
    let scope = budget
        .reserve_capacity(q.peak_bytes - 1)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let calls = Cell::new(0);
    assert_eq!(
        stage_checked(
            &source,
            &batch,
            Admission::new(0),
            &scope,
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
            &|_| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(calls.get(), quoted_calls.get());
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    let scope = budget
        .reserve_capacity(q.peak_bytes)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == quoted_calls.get() + 2 {
            Err(Error::new(ErrorCode::OperationCancelled, "stop"))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        stage_checked(&source, &batch, Admission::new(0), &scope, &check, &|_| Ok(
            ()
        ))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(scope.footprint().live, 0);
    assert_document_equal(&source, &before);
}
#[test]
fn cumulative_work_exhaustion_cannot_return_a_candidate_or_change_original() {
    let source = document();
    let before = source.clone();
    let batch = batch(&source);
    let q = quote_checked(&source, &batch, &|| Ok(()), &|_| Ok(())).unwrap();
    let budget = SharedBudget::new(q.peak_bytes);
    let scope = budget
        .reserve_capacity(q.peak_bytes)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let remaining = Cell::new(q.stage_work - 1);
    let spend = |n| {
        remaining.set(remaining.get().checked_sub(n).ok_or_else(limit)?);
        Ok(())
    };
    assert_eq!(
        stage_checked(
            &source,
            &batch,
            Admission::new(0),
            &scope,
            &|| Ok(()),
            &spend
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(scope.footprint().live, 0);
    assert_document_equal(&source, &before);
}
#[test]
fn retained_quote_uses_actual_vectors_under_explicit_fresh_map_provenance() {
    // This fixture's maps are fresh from import_image; this is a structural
    // capacity control, not a claim of exercising the PCND reader here.
    let mut source = document();
    source.title = Some(String::from("metadata-stage"));
    let before = retained_quote_checked(&source, &|| Ok(()), &|_| Ok(())).unwrap();
    let old = source.title.as_ref().unwrap().capacity();
    source.title.as_mut().unwrap().reserve_exact(4096);
    let extra = source.title.as_ref().unwrap().capacity() - old;
    assert_eq!(
        retained_quote_checked(&source, &|| Ok(()), &|_| Ok(())).unwrap(),
        before + extra as u64
    );
    // A visible nonminimal map shape is refused. Equality in the accepted case
    // is only a consistency check; it cannot discover arbitrary hidden buckets.
    source.layers.reserve(1024);
    assert_eq!(
        retained_quote_checked(&source, &|| Ok(()), &|_| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}

fn assert_document_equal(actual: &Document, expected: &Document) {
    assert!(actual.same_semantics(expected));
    assert_eq!(actual.state_id, expected.state_id);
    assert_eq!(actual.layers.len(), expected.layers.len());
}
