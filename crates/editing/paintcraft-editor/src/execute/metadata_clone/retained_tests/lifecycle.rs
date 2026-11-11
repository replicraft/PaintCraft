use super::*;
use paintcraft_core::types::filters::{BrightnessContrastV1, Descriptor};
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount, SharedBudget};
fn typed(source: &Document, op: FilterOp) -> Batch {
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
                ("state_id", Value::str(source.state_id.to_string())),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ]))
    .unwrap()
}
fn stage(source: &Document, batch: &Batch) -> StagedMetadata {
    let q = quote_checked(source, batch, &|| Ok(()), &|_| Ok(())).unwrap();
    let budget = SharedBudget::new(q.peak_bytes);
    let scope = budget
        .reserve_capacity(q.peak_bytes)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    stage_checked(
        source,
        batch,
        Admission::new(0),
        &scope,
        &|| Ok(()),
        &|_| Ok(()),
    )
    .unwrap()
}
#[test]
fn target_layout_guards_are_literal_and_peak_storage_is_assigned_separately() {
    assert_eq!(size_of::<Quote>(), 64);
    let guard = if cfg!(target_pointer_width = "64") {
        104
    } else {
        88
    };
    assert_eq!(
        size_of::<BudgetAccount>(),
        if cfg!(target_pointer_width = "64") {
            40
        } else {
            24
        }
    );
    assert_eq!(quote_control_bytes(), guard);
    let source = document();
    let batch = batch(&source);
    let q = quote_checked(&source, &batch, &|| Ok(()), &|_| Ok(())).unwrap();
    assert_eq!(q.peak_bytes, q.storage_peak_bytes + guard);
    for amount in [guard - 1, guard] {
        let budget = SharedBudget::new(1 << 20);
        let scope = budget
            .reserve_capacity(amount)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let checks = Cell::new(0);
        let paid = Cell::new(0);
        let result = stage_checked(
            &source,
            &batch,
            Admission::new(0),
            &scope,
            &|| {
                checks.set(checks.get() + 1);
                Ok(())
            },
            &|n| {
                paid.set(paid.get() + n);
                Ok(())
            },
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(paid.get(), if amount == guard { 129 } else { 0 });
        assert_eq!(checks.get(), if amount == guard { 2 } else { 0 });
        assert_eq!(scope.footprint().live, 0);
        drop(scope);
        assert_eq!(budget.usage().reserved, 0);
    }
    let budget = SharedBudget::new(q.peak_bytes);
    let scope = budget
        .reserve_capacity(q.peak_bytes)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let staged = stage_checked(
        &source,
        &batch,
        Admission::new(0),
        &scope,
        &|| Ok(()),
        &|_| Ok(()),
    )
    .unwrap();
    assert_eq!(scope.footprint().peak, q.peak_bytes);
    assert!(scope.footprint().live < q.storage_peak_bytes);
    drop(staged);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn target_aux_and_alias_bounds_are_independent_of_actual_vector_len() {
    let source = document();
    let root = LayerRef::Id(source.roots[0]);
    let plain = FilterOp::SetEnabled {
        layer: root.clone(),
        filter: FilterRef::Id(Uuid::from_u128(7)),
        enabled: false,
    };
    let w = size_of::<FilterInstance>() as u64;
    assert_eq!(
        work::target(super::super::MetadataOp::Filter(&plain)).unwrap(),
        1030 * w + 4100
    );
    let duplicate = FilterOp::Duplicate {
        layer: root,
        filter: FilterRef::Id(Uuid::from_u128(7)),
        alias: "copy".into(),
        to_index: None,
    };
    let a = fresh_map_layout::<String, Option<(Uuid, Uuid)>>(1)
        .unwrap()
        .bytes;
    let alias = 2 * (2 * (4 + 65) + size_of::<String>() as u64 + 64 + (a + 4) + 4 + 1) + 257;
    assert_eq!(
        work::target(super::super::MetadataOp::Filter(&duplicate)).unwrap(),
        1030 * w + 4100 + alias
    );
}
#[test]
fn all_six_actual_planning_replay_operations_preserve_source_and_match_semantics() {
    let mut source = document();
    let root = source.roots[0];
    let first = source.layers[&root].filters.as_slice()[0].clone();
    source.layers.get_mut(&root).unwrap().filters =
        FilterStack::new(vec![first.clone(), first.with_id(Uuid::from_u128(8))]).unwrap();
    let layer = LayerRef::Id(root);
    let filter = FilterRef::Id(Uuid::from_u128(7));
    let operations = [
        FilterOp::Add {
            layer: layer.clone(),
            alias: "add".into(),
            descriptor: Descriptor::InvertRgbV1,
            enabled: true,
            to_index: Some(0),
        },
        FilterOp::Duplicate {
            layer: layer.clone(),
            filter: filter.clone(),
            alias: "dup".into(),
            to_index: Some(0),
        },
        FilterOp::Update {
            layer: layer.clone(),
            filter: filter.clone(),
            descriptor: Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(10, 0).unwrap()),
        },
        FilterOp::SetEnabled {
            layer: layer.clone(),
            filter: filter.clone(),
            enabled: false,
        },
        FilterOp::Reorder {
            layer: layer.clone(),
            filter: filter.clone(),
            to_index: 1,
        },
        FilterOp::Remove { layer, filter },
    ];
    let before = source.clone();
    for (index, op) in operations.into_iter().enumerate() {
        let batch = typed(&source, op);
        let expected = execute_with_admission_checked(
            &source,
            Selection::Inactive,
            &batch,
            Admission::new(0),
            &|| Ok(()),
        )
        .unwrap();
        let actual = stage(&source, &batch);
        assert!(actual.document().same_semantics(&expected.doc));
        assert_eq!(actual.filter_aliases(), &expected.filter_aliases);
        assert_eq!(
            actual.document().layers[&root].filters.len(),
            match index {
                0 | 1 => 3,
                5 => 1,
                _ => 2,
            }
        );
        if index == 3 {
            assert!(!actual.document().layers[&root].filters.as_slice()[0].enabled());
        }
        assert_eq!(
            actual.document().layers[&root].raster.get(0, 0),
            Rgba([7, 8, 9, 0])
        );
        assert!(source.same_semantics(&before));
    }
}
#[test]
fn target_cap_and_noop_paths_preserve_original_limits() {
    let mut source = document();
    let root = source.roots[0];
    let first = source.layers[&root].filters.as_slice()[0].clone();
    source.layers.get_mut(&root).unwrap().filters = FilterStack::new(
        (0..255)
            .map(|i| first.with_id(Uuid::from_u128(i + 7)))
            .collect(),
    )
    .unwrap();
    let op = FilterOp::Duplicate {
        layer: LayerRef::Id(root),
        filter: FilterRef::Id(Uuid::from_u128(7)),
        alias: "max".into(),
        to_index: None,
    };
    let actual = stage(&source, &typed(&source, op.clone()));
    assert_eq!(actual.document().layers[&root].filters.len(), 256);
    drop(actual);
    source.layers.get_mut(&root).unwrap().filters = FilterStack::new(
        (0..256)
            .map(|i| first.with_id(Uuid::from_u128(i + 7)))
            .collect(),
    )
    .unwrap();
    let batch = typed(&source, op);
    let q = quote_checked(&source, &batch, &|| Ok(()), &|_| Ok(())).unwrap();
    let budget = SharedBudget::new(q.peak_bytes);
    let scope = budget
        .reserve_capacity(q.peak_bytes)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    assert_eq!(
        stage_checked(
            &source,
            &batch,
            Admission::new(0),
            &scope,
            &|| Ok(()),
            &|_| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(source.layers[&root].filters.len(), 256);
    for op in [
        FilterOp::SetEnabled {
            layer: LayerRef::Id(root),
            filter: FilterRef::Id(Uuid::from_u128(7)),
            enabled: true,
        },
        FilterOp::Update {
            layer: LayerRef::Id(root),
            filter: FilterRef::Id(Uuid::from_u128(7)),
            descriptor: Descriptor::InvertRgbV1,
        },
        FilterOp::Reorder {
            layer: LayerRef::Id(root),
            filter: FilterRef::Id(Uuid::from_u128(7)),
            to_index: 0,
        },
    ] {
        let output = stage(&source, &typed(&source, op));
        assert!(source.same_semantics(output.document()));
    }
}
#[test]
fn planning_and_replay_working_vectors_cancel_at_source_frozen_checks() {
    let source = document();
    let before = source.clone();
    let batch = batch(&source);
    let q = quote_checked(&source, &batch, &|| Ok(()), &|_| Ok(())).unwrap();
    let layer = &source.layers[&source.roots[0]];
    let legacy_seed = fresh_map_layout::<Uuid, Layer>(1).unwrap().bytes
        + 16
        + size_of::<Layer>() as u64
        + 486
        + size_of::<FilterInstance>() as u64
        + 190
        + layer.name.len() as u64
        + source.title.as_ref().map_or(0, |s| s.len()) as u64;
    // EmptyStyle rawcheck adds1; style-query layer/op add2 paid checks.
    // Retain the actual post-vector planning/replay cuts (old11/15), not
    // the obsolete ordinals that now land before vector construction.
    for (stop, callbacks_paid) in [(14, 6), (18, 10)] {
        let budget = SharedBudget::new(q.peak_bytes);
        let scope = budget
            .reserve_capacity(q.peak_bytes)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let calls = Cell::new(0);
        let paid = Cell::new(0u64);
        let sentinel = Error::new(ErrorCode::OperationCancelled, "working target vector").at(19);
        let result = stage_checked(
            &source,
            &batch,
            Admission::new(0),
            &scope,
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(sentinel.clone())
                } else {
                    Ok(())
                }
            },
            &|n| {
                paid.set(paid.get() + n);
                Ok(())
            },
        );
        assert_eq!(result.unwrap_err(), sentinel);
        assert_eq!(calls.get(), stop);
        assert_eq!(
            paid.get(),
            129 + q.generic_lifecycle_work + legacy_seed + q.stage_work + callbacks_paid
        );
        assert_eq!(scope.footprint().live, 0);
        assert!(source.same_semantics(&before));
        drop(scope);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn future_lifecycle_denial_keeps_original_quote_debt_and_never_constructs_seed() {
    let source = document();
    let before = source.clone();
    let batch = batch(&source);
    let q = quote_checked(&source, &batch, &|| Ok(()), &|_| Ok(())).unwrap();
    let budget = SharedBudget::new(q.peak_bytes);
    let scope = budget
        .reserve_capacity(q.peak_bytes)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let ordinal = Cell::new(0);
    let paid = Cell::new(0u64);
    let error = Error::new(ErrorCode::ResourceLimit, "lifecycle denied").at(17);
    let result = stage_checked(
        &source,
        &batch,
        Admission::new(0),
        &scope,
        &|| Ok(()),
        &|n| {
            ordinal.set(ordinal.get() + 1);
            if ordinal.get() == 4 {
                assert_eq!(n, q.generic_lifecycle_work);
                return Err(error.clone());
            }
            paid.set(paid.get() + n);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), error);
    assert_eq!(paid.get(), 129);
    assert_eq!(scope.footprint().live, 0);
    assert!(source.same_semantics(&before));
}
