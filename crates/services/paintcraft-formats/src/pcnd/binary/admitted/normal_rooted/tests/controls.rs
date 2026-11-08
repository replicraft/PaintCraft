use super::*;
#[test]
fn original_controls_minus_one_and_first_lookup_work_refuse_before_output() {
    let source = doc("v1-default");
    let c = plan::control_bytes();
    for short in [false, true] {
        let capacity = if short { c - 1 } else { 4 << 20 };
        let budget = SharedBudget::new(8 << 20);
        let reservation = budget.reserve_capacity(capacity).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let mut output = Vec::new();
        let debit = |n: u64| {
            let next = used
                .get()
                .checked_add(n)
                .filter(|n| *n < 2048)
                .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "root prefix refused"))?;
            used.set(next);
            Ok(())
        };
        let error = write_portable_normal_snapshot_rooted_admitted_checked(
            &source,
            &mut output,
            &scope,
            &|| Ok(()),
            &debit,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(output.is_empty());
        assert_eq!(used.get(), 0);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(reservation.available(), capacity);
        assert_eq!(budget.usage().reserved, capacity);
        drop(scope);
        drop(reservation);
        assert_eq!(budget.usage().reserved, 0);
    }
    // Actual control width is known before this independent exact/-1 intervention.
    let budget = SharedBudget::new(c);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut account = scope.account();
    account.resize(c).unwrap();
    assert_eq!(scope.footprint().live, c);
    drop(account);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn optional_catalog_and_output_root_passes_are_paid_before_their_helpers() {
    for catalog in [false, true] {
        let source = doc(if catalog {
            "v6-catalog-sort-order"
        } else {
            "v1-default"
        });
        let before = source.clone();
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut output = Fixed::new(&scope);
        let used = Cell::new(0u64);
        let roots = Cell::new(0usize);
        let denied = Cell::new(false);
        let spend = |n: u64| {
            if n == 2048 {
                roots.set(roots.get() + 1);
            }
            let refuse = if catalog {
                n == 2048 + 768 * 2
            } else {
                n == 2048 && roots.get() == 3
            };
            if refuse {
                denied.set(true);
                return Err(Error::new(ErrorCode::ResourceLimit, "phase refused"));
            }
            used.set(used.get().checked_add(n).unwrap());
            Ok(())
        };
        let error = write_portable_normal_snapshot_rooted_admitted_checked(
            &source,
            &mut output,
            &scope,
            &|| Ok(()),
            &spend,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(denied.get());
        assert!(used.get() > 0);
        if catalog {
            assert!(output.bytes.is_empty());
        } else {
            assert!(!output.bytes.is_empty());
        }
        assert_eq!(scope.footprint().live, 65536);
        drop(output);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
        assert!(source.same_semantics(&before));
        assert_eq!(source.state_id, before.state_id);
    }
}
#[test]
fn malformed_roots_refuse_with_no_wire_and_preserve_full_source_fields() {
    for kind in 0..4 {
        let mut source = doc("v1-default");
        let id = source.roots[0];
        match kind {
            0 => source.roots.push(id),
            1 => {
                let mut extra = source.layers[&id].clone();
                extra.id = uuid::Uuid::from_u128(999);
                source.layers.insert(extra.id, extra);
                source.roots.push(id);
            }
            2 => source.roots[0] = uuid::Uuid::from_u128(1000),
            _ => source.layers.get_mut(&id).unwrap().id = uuid::Uuid::from_u128(1001),
        }
        let before = source.clone();
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut output = Vec::new();
        let error = write_portable_normal_snapshot_rooted_admitted_checked(
            &source,
            &mut output,
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CodecFailed);
        assert!(output.is_empty());
        // same_semantics indexes roots and is invalid for deliberately missing IDs.
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
#[test]
fn cancellation_keeps_original_scope_and_source_without_publishing_private_prefix() {
    let source = doc("v7-filter-and-own-stored");
    let before = source.clone();
    for stop in [1, 2, 4, 8, 16] {
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let mut output = Fixed::new(&scope);
        let calls = Cell::new(0);
        let work = Cell::new(0u64);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "normal writer cancel",
                ))
            } else {
                Ok(())
            }
        };
        let error = write_portable_normal_snapshot_rooted_admitted_checked(
            &source,
            &mut output,
            &scope,
            &check,
            &|n| {
                work.set(work.get().checked_add(n).unwrap());
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
        assert_eq!(scope.footprint().live, 65536);
        let paid = work.get();
        drop(output);
        assert_eq!(work.get(), paid);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
        assert!(source.same_semantics(&before));
        assert_eq!(source.state_id, before.state_id);
    }
}
#[test]
fn aggregate_coverage_counts_every_shared_stored_occurrence_before_output() {
    use paintcraft_core::types::Rect;
    use paintcraft_document::{FilterMask, FilterStack, LayerMask};
    use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};
    let rows = 131073usize;
    let runs: Vec<_> = (0..rows)
        .map(|i| Run {
            x0: (i % 2) as i64,
            x1: (i % 2) as i64 + 1,
            coverage: 128,
        })
        .collect();
    let bands: Vec<_> = runs
        .iter()
        .enumerate()
        .map(|(i, r)| Band {
            y0: i as i64,
            y1: i as i64 + 1,
            runs: std::slice::from_ref(r),
        })
        .collect();
    let backing = SharedBudget::new(64 << 20);
    let mask = A8Mask::from_bands(
        Rect::new(0, 0, 2, rows as i64).unwrap(),
        &bands,
        &backing,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!((mask.stats().bands, mask.stats().runs), (rows, rows));
    let mut source = doc("v7-repeated-mask-occurrence");
    let id = source.roots[0];
    let layer = source.layers.get_mut(&id).unwrap();
    let filter = layer
        .filters
        .iter()
        .next()
        .unwrap()
        .clone()
        .with_mask(Some(FilterMask::new(true, mask.clone())));
    layer.filters = FilterStack::new(vec![filter]).unwrap();
    layer.mask = Some(LayerMask::new(false, mask));
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut output = Vec::new();
    let error = write_portable_normal_snapshot_rooted_admitted_checked(
        &source,
        &mut output,
        &scope,
        &|| Ok(()),
        &|_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(output.is_empty());
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn statically_prescribed_default_source_control_peak_exact_and_minus_one() {
    let source = doc("v1-default");
    let exact =
        plan::control_bytes() + crate::pcnd::layer_appearance::flat_shape_scratch_quote(1).unwrap();
    for short in [false, true] {
        let capacity = exact - u64::from(short);
        let budget = SharedBudget::new(exact + 65536);
        let reservation = budget.reserve_capacity(capacity).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        // Fixed caller stack sink makes no source Vec allocation or grant.
        let mut output = std::io::Cursor::new([0u8; 512]);
        let result = write_portable_normal_snapshot_rooted_admitted_checked(
            &source,
            &mut output,
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        );
        if short {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        } else {
            result.unwrap();
            assert!(output.position() > 0);
        }
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(reservation.available(), capacity);
        assert_eq!(budget.usage().reserved, capacity);
        drop(scope);
        drop(reservation);
        assert_eq!(budget.usage().reserved, 0);
    }
}
