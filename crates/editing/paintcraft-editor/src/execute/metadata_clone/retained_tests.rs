use super::*;
use paintcraft_commands::{FilterRef, LayerRef};
use paintcraft_core::types::{
    json::Value,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterMask, LayerMask};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::cell::{Cell, RefCell};
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
fn quote_pays_own_math_not_future_source_walk_and_keeps_clone_decomposition() {
    let source = document();
    let batch = batch(&source);
    let trace = RefCell::new(Vec::new());
    let quote = quote_checked(&source, &batch, &|| Ok(()), &|n| {
        trace.borrow_mut().push(n);
        Ok(())
    })
    .unwrap();
    assert_eq!(std::mem::size_of::<Quote>(), 56);
    assert_eq!(*trace.borrow(), [64, 1, 64]);
    assert_eq!(quote.raster_seed_supplement_work, 330);
    assert!(quote.stage_work >= 2 * quote.seed_bytes);
    assert_eq!(source.layers[&source.roots[0]].raster.resident_tiles(), 1);
}
#[test]
fn main_style_two_quotes_do_not_debit_future_scan_twice() {
    let source = document();
    let before = source.clone();
    let batch = batch(&source);
    let trace = RefCell::new(Vec::new());
    let spend = |n| {
        trace.borrow_mut().push(n);
        Ok(())
    };
    let quote = quote_checked(&source, &batch, &|| Ok(()), &spend).unwrap();
    let budget = SharedBudget::new(quote.peak_bytes);
    let scope = budget
        .reserve_capacity(quote.peak_bytes)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let staged = stage_checked(
        &source,
        &batch,
        Admission::new(0),
        &scope,
        &|| Ok(()),
        &spend,
    )
    .unwrap();
    let trace = trace.borrow();
    assert_eq!(&trace[..6], [64, 1, 64, 64, 1, 64]);
    assert_eq!(
        trace.windows(2).filter(|pair| *pair == [64, 145]).count(),
        1
    );
    assert_eq!(
        staged.document().layers[&source.roots[0]].raster.get(0, 0),
        Rgba([7, 8, 9, 0])
    );
    assert!(source.same_semantics(&before));
    drop(staged);
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn quote_math_minus_one_keeps_original_paid_root_prefix() {
    let source = document();
    let batch = batch(&source);
    for (ceiling, paid, ok) in [
        (63, 0, false),
        (64, 64, false),
        (65, 65, false),
        (128, 65, false),
        (129, 129, true),
    ] {
        let used = Cell::new(0u64);
        let result = quote_checked(&source, &batch, &|| Ok(()), &|n| {
            let next = used.get().checked_add(n).ok_or_else(limit)?;
            if next > ceiling {
                return Err(limit());
            }
            used.set(next);
            Ok(())
        });
        assert_eq!(used.get(), paid);
        assert_eq!(result.is_ok(), ok);
        if let Ok(q) = result {
            assert_eq!(q.raster_seed_supplement_work, 330);
        }
    }
}
#[test]
fn stage_peak_denial_pays_quote_math_but_never_actual_seed_supplement() {
    let source = document();
    let before = source.clone();
    let batch = batch(&source);
    let q = quote_checked(&source, &batch, &|| Ok(()), &|_| Ok(())).unwrap();
    let budget = SharedBudget::new(q.peak_bytes);
    let scope = budget
        .reserve_capacity(q.peak_bytes - 1)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let trace = RefCell::new(Vec::new());
    let error = stage_checked(
        &source,
        &batch,
        Admission::new(0),
        &scope,
        &|| Ok(()),
        &|n| {
            trace.borrow_mut().push(n);
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(*trace.borrow(), [64, 1, 64]);
    assert_eq!(scope.footprint().live, 0);
    assert!(source.same_semantics(&before));
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn empty_raster_forecast_and_stage_never_charge_a_retained_scan() {
    let mut source = document();
    let root = source.roots[0];
    source.layers.get_mut(&root).unwrap().raster = RasterLayer::new(1, 1, Rgba::TRANSPARENT);
    let batch = batch(&source);
    let trace = RefCell::new(Vec::new());
    let q = quote_checked(&source, &batch, &|| Ok(()), &|n| {
        trace.borrow_mut().push(n);
        Ok(())
    })
    .unwrap();
    assert_eq!(*trace.borrow(), [64, 1, 64]);
    assert_eq!(q.raster_seed_supplement_work, 64);
    trace.borrow_mut().clear();
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
        &|n| {
            trace.borrow_mut().push(n);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(&trace.borrow()[..3], [64, 1, 64]);
    assert_eq!(trace.borrow().iter().filter(|n| **n == 64).count(), 3);
    assert_eq!(staged.document().layers[&root].raster.resident_tiles(), 0);
    drop(staged);
    assert_eq!(scope.footprint().live, 0);
}

mod lifecycle;

#[test]
fn operation_math_debit_precedes_even_global_shape_layout_work() {
    let mut source = document();
    let batch = batch(&source);
    let mut orphan = source.layers[&source.roots[0]].clone();
    orphan.id = Uuid::from_u128(99);
    source.layers.insert(orphan.id, orphan);
    let check = Cell::new(0);
    let paid = Cell::new(0);
    let error = Error::new(ErrorCode::ResourceLimit, "global quote math first").at(41);
    let result = quote_checked(
        &source,
        &batch,
        &|| {
            check.set(check.get() + 1);
            Ok(())
        },
        &|n| {
            assert_eq!(n, 64);
            Err(error.clone())
        },
    );
    assert_eq!(result.unwrap_err(), error);
    assert_eq!(check.get(), 1);
    assert_eq!(paid.get(), 0);
    let result = quote_checked(&source, &batch, &|| Ok(()), &|n| {
        paid.set(paid.get() + n);
        Ok(())
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
    assert_eq!(paid.get(), 64);
}
