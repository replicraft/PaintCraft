use super::*;
#[test]
fn original_control_and_retained_scope_exact_minus_one_never_borrow_parent_headroom() {
    for short in [true, false] {
        let r = row("empty-layer");
        let b = bytes(&r);
        let checked = CheckedContainer::verify(&b, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = budget.scope(AllocationClass::PaintMetadata);
        let sc = budget
            .reserve_capacity(owner_c() + stream_c() - u64::from(short))
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let m = budget.account(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || Ok(());
        let plan = checked
            .portable_source8_plan_checked(limits(&r, None), &ps, &check, &spend)
            .unwrap();
        let owner = plan.coverage_materializer(&sc, &m).unwrap();
        let before = used.get();
        let value = owner.document_intentions_checked();
        if short {
            assert_eq!(value.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(used.get() - before, 64);
        } else {
            drop(value.unwrap());
            assert_eq!(used.get() - before, if native() { 696 } else { 672 });
        }
        assert_eq!(sc.footprint().live, owner_c());
        assert_eq!(sc.footprint().peak, owner_c() + stream_c());
    }
    for short_m in [false, true] {
        let r = row("filter128");
        let b = bytes(&r);
        let checked = CheckedContainer::verify(&b, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = budget.scope(AllocationClass::PaintMetadata);
        let sc = budget
            .reserve_capacity(n(params(&r), "A_scope_exact") - u64::from(!short_m))
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let mut m = budget.account(AllocationClass::PaintMetadata);
        m.resize(n(params(&r), "M") - u64::from(short_m)).unwrap();
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || Ok(());
        let plan = checked
            .portable_source8_plan_checked(limits(&r, None), &ps, &check, &spend)
            .unwrap();
        let owner = plan.coverage_materializer(&sc, &m).unwrap();
        let before = used.get();
        assert_eq!(read(&owner, 1).unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(used.get() - before, if native() { 25035 } else { 24963 });
        assert_eq!(sc.footprint().live, owner_c());
    }
}
#[test]
fn one_global_remaining_limits_across_layers_and_original_smallwork_cases() {
    let r = row("two-layer-global");
    for work in [65536, 6, 5] {
        let b = bytes(&r);
        let checked = CheckedContainer::verify(&b, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = budget.scope(AllocationClass::PaintMetadata);
        let sc = budget
            .reserve_capacity(n(params(&r), "A_scope_exact"))
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let mut m = budget.account(AllocationClass::PaintMetadata);
        m.resize(n(params(&r), "M")).unwrap();
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || Ok(());
        let result =
            checked.portable_source8_plan_checked(limits(&r, Some(work)), &ps, &check, &spend);
        if work == 5 {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(sc.footprint().live, 0);
            continue;
        }
        let plan = result.unwrap();
        let owner = plan.coverage_materializer(&sc, &m).unwrap();
        let before = used.get();
        let doc = owner.document_intentions_checked().unwrap();
        let (_, layer) = doc.begin_layer().unwrap();
        if work == 6 {
            assert_eq!(
                layer.finish_layer().unwrap_err().code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(used.get() - before, if native() { 21331 } else { 21259 });
            assert_eq!(sc.footprint().live, owner_c());
            continue;
        }
        let (doc, mask1) = layer.finish_layer().unwrap();
        assert!(mask1.is_some());
        assert_eq!(
            (doc.remaining.bands, doc.remaining.runs, doc.remaining.work),
            (1, 1, 65533)
        );
        let (_, layer) = doc.begin_layer().unwrap();
        let (doc, mask2) = layer.finish_layer().unwrap();
        assert!(mask2.is_some());
        assert_eq!(
            (doc.remaining.bands, doc.remaining.runs, doc.remaining.work),
            (0, 0, 65530)
        );
        let receipt = doc.finish().unwrap();
        assert_eq!(
            (
                receipt.layers(),
                receipt.filters(),
                receipt.mask_present(),
                receipt.mask_nonempty(),
                receipt.bands(),
                receipt.runs()
            ),
            (2, 0, 2, 2, 2, 2)
        );
        assert_eq!(used.get() - before, n(params(&r), "total_work"));
        assert_eq!(plan.work_used(), used.get());
        drop(mask1);
        drop(mask2);
    }
}
#[test]
fn exact_sequence_errors_consume_state_and_none_returns_are_paid() {
    for (name, which) in [
        ("empty-layer", 0),
        ("one-unmasked", 1),
        ("one-unmasked", 2),
        ("empty-layer", 3),
    ] {
        let r = row(name);
        let b = bytes(&r);
        let checked = CheckedContainer::verify(&b, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = budget.scope(AllocationClass::PaintMetadata);
        let sc = budget
            .reserve_capacity(1 << 20)
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let m = budget.account(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || Ok(());
        let plan = checked
            .portable_source8_plan_checked(limits(&r, None), &ps, &check, &spend)
            .unwrap();
        let owner = plan.coverage_materializer(&sc, &m).unwrap();
        let before = used.get();
        let doc = owner.document_intentions_checked().unwrap();
        match which {
            0 => {
                let (_, layer) = doc.begin_layer().unwrap();
                let (doc, mask) = layer.finish_layer().unwrap();
                assert!(mask.is_none());
                assert_eq!(doc.begin_layer().unwrap_err().code, ErrorCode::CodecFailed);
                assert_eq!(used.get() - before, if native() { 10588 } else { 10492 });
            }
            1 => {
                assert_eq!(doc.finish().unwrap_err().code, ErrorCode::CodecFailed);
                assert_eq!(used.get() - before, if native() { 1272 } else { 1248 });
            }
            2 => {
                let (_, layer) = doc.begin_layer().unwrap();
                assert_eq!(
                    layer.finish_layer().unwrap_err().code,
                    ErrorCode::CodecFailed
                );
                assert_eq!(used.get() - before, if native() { 8819 } else { 8747 });
            }
            _ => {
                let (_, layer) = doc.begin_layer().unwrap();
                let (layer, value) = layer.next_filter().unwrap();
                assert!(value.is_none());
                let (doc, mask) = layer.finish_layer().unwrap();
                assert!(mask.is_none());
                assert_eq!(doc.finish().unwrap().filters(), 0);
                assert_eq!(used.get() - before, if native() { 11412 } else { 11276 });
            }
        }
        assert_eq!(sc.footprint().live, owner_c());
    }
}
#[test]
fn every_literal_debit_and_callback_cut_preserves_typed_error_and_same_paid_retry() {
    let r = row("filter128");
    let events = params(&r)
        .get("events")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    let checks = events
        .iter()
        .filter(|e| e.get("check").unwrap().as_bool().unwrap())
        .count();
    for cancel in [false, true] {
        for stop in 0..if cancel { checks } else { events.len() } {
            let b = bytes(&r);
            let checked = CheckedContainer::verify(&b, &|| Ok(())).unwrap();
            let original = b.clone();
            let budget = SharedBudget::new(32 << 20);
            let ps = budget.scope(AllocationClass::PaintMetadata);
            let sc = budget
                .reserve_capacity(n(params(&r), "A_scope_exact"))
                .unwrap()
                .scope(AllocationClass::CoverageMasks);
            let mut m = budget.account(AllocationClass::PaintMetadata);
            m.resize(n(params(&r), "M")).unwrap();
            let used = Cell::new(0u64);
            let ordinal = Cell::new(0usize);
            let calls = Cell::new(0usize);
            let armed = Cell::new(false);
            let refusing = Cell::new(true);
            let sentinel = Error::new(
                if cancel {
                    ErrorCode::OperationCancelled
                } else {
                    ErrorCode::ResourceLimit
                },
                "original intention stream cut",
            )
            .at(91);
            let spend = |n| {
                if armed.get() {
                    let i = ordinal.get();
                    ordinal.set(i + 1);
                    if !cancel && refusing.get() && i == stop {
                        return Err(sentinel.clone());
                    }
                }
                used.set(
                    used.get()
                        .checked_add(n)
                        .filter(|v| *v <= WORK)
                        .ok_or_else(bound)?,
                );
                Ok(())
            };
            let check = || {
                if armed.get() {
                    let i = calls.get();
                    calls.set(i + 1);
                    if cancel && refusing.get() && i == stop {
                        return Err(sentinel.clone());
                    }
                }
                Ok(())
            };
            let plan = checked
                .portable_source8_plan_checked(limits(&r, None), &ps, &check, &spend)
                .unwrap();
            let owner = plan.coverage_materializer(&sc, &m).unwrap();
            let before = used.get();
            let before_checks = plan.check_calls();
            armed.set(true);
            assert_eq!(read(&owner, 1).unwrap_err(), sentinel);
            let prefix = if cancel {
                let end = events
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.get("check").unwrap().as_bool().unwrap())
                    .nth(stop)
                    .unwrap()
                    .0;
                events[..=end].iter().map(|e| n(e, "debit")).sum::<u64>()
            } else {
                events[..stop].iter().map(|e| n(e, "debit")).sum()
            };
            assert_eq!(used.get() - before, prefix);
            assert_eq!(sc.footprint().live, owner_c());
            refusing.set(false);
            let (receipt, filters, masks) = read(&owner, 1).unwrap();
            assert_eq!(receipt.mask_nonempty(), 1);
            assert_eq!(used.get() - before, prefix + n(params(&r), "total_work"));
            assert_eq!(plan.work_used(), used.get());
            assert_eq!(plan.check_calls() - before_checks, calls.get() as u64);
            drop(filters);
            drop(masks);
            assert_eq!(sc.footprint().live, owner_c());
            assert_eq!(b, original);
        }
    }
}
