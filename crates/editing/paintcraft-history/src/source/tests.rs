use super::*;
use crate::{Entry, Snapshot};
use paintcraft_core::types::{
    ErrorCode, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::{Admission, BlendMode, RasterLayer, Selection, TileVersion};
use std::{
    cell::Cell,
    future::Future,
    mem::{align_of, size_of},
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
};
#[allow(dead_code)]
#[repr(C, align(2))]
struct MirrorArc<T> {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: T,
}
#[allow(dead_code)]
#[repr(align(32))]
struct Aligned32([u8; 3]);
#[derive(Default)]
struct YieldOnce(bool);
impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
fn run<F: Future>(future: F, mut pending: impl FnMut()) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..10_000 {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(result) => return result,
            Poll::Pending => pending(),
        }
    }
    panic!("owned source continuation bound");
}
fn empty() -> History {
    let doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([19, 43, 71, 0])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    History::new(Snapshot {
        document: Arc::new(doc),
        selection: Selection::Inactive,
        history_id: None,
    })
}
fn document(h: &History) -> &Arc<paintcraft_document::Document> {
    let crate::Storage::Hot(snapshot) = &h.entries[0].storage else {
        panic!("hot fixture")
    };
    &snapshot.document
}
// Independent fresh-table extent from its pre-deletion complete capacity.
fn table_bytes(full: usize) -> u64 {
    if full == 0 {
        return 0;
    }
    let buckets = if full <= 7 { full + 1 } else { full / 7 * 8 };
    let width = size_of::<(TileVersion, Weak<crate::archive::Blob>)>();
    let align = align_of::<(TileVersion, Weak<crate::archive::Blob>)>().max(16);
    ((buckets * width).div_ceil(align) * align + buckets + 16) as u64
}
fn expected(h: &History, retained_table: u64) -> (u64, u64, u64) {
    let errors = h
        .last_cleanup_error
        .as_ref()
        .map_or(0, |e| e.message.capacity())
        + h.last_packing_error
            .as_ref()
            .map_or(0, |e| e.message.capacity());
    let backing = (h.entries.capacity() * size_of::<Entry>() + errors) as u64 + retained_table;
    let retained = backing + size_of::<RetainedHistoryStorage>() as u64;
    let additional =
        (size_of::<MirrorArc<SourceStorage>>() - size_of::<RetainedHistoryStorage>()) as u64;
    (retained, additional, retained + additional)
}
fn spent(counter: &Cell<u64>, limit: u64, units: u64) -> Result<()> {
    let next = counter
        .get()
        .checked_add(units)
        .filter(|n| *n <= limit)
        .ok_or_else(|| archive::limit("test source cumulative work"))?;
    counter.set(next);
    Ok(())
}
fn capture(history: History, scope: BudgetScope, counter: &Cell<u64>) -> HistoryReadSource {
    HistoryReadSource::capture_owned(history, scope, &|n| spent(counter, 1 << 30, n), &|| Ok(()))
        .unwrap()
}
fn same_copied(a: &History, b: &History) {
    assert!(a.entries().eq(b.entries()));
    assert_eq!(a.memory_usage(), b.memory_usage());
    assert_eq!(
        (
            a.undo_depth(),
            a.redo_depth(),
            a.trimmed_total(),
            a.last_trimmed()
        ),
        (
            b.undo_depth(),
            b.redo_depth(),
            b.trimmed_total(),
            b.last_trimmed()
        )
    );
}
#[test]
fn exact_target_arc_layout_and_capacity_quote_are_independently_prescribed() {
    assert_eq!(
        quote::arc_allocation::<SourceStorage>().unwrap(),
        size_of::<MirrorArc<SourceStorage>>() as u64
    );
    assert_eq!(
        quote::arc_allocation::<Aligned32>().unwrap(),
        size_of::<MirrorArc<Aligned32>>() as u64
    );
    assert_eq!(quote::arc_allocation::<Aligned32>().unwrap(), 64);
    let h = empty();
    let (retained, control, total) = expected(&h, 0);
    let q = HistoryReadSource::quote_owned(&h).unwrap();
    assert_eq!(
        (q.retained_bytes, q.shared_control_bytes, q.capture_bytes),
        (retained, control, total)
    );
    assert_eq!(
        q.backing_bytes,
        (h.entries.capacity() * size_of::<Entry>()) as u64
    );
    assert_eq!(q.handle_bytes, size_of::<HistoryReadSource>() as u64);
}
#[test]
fn capture_moves_vectors_tables_errors_and_snapshot_handles_without_source_clone() {
    let mut h = empty();
    let mut rasters = Vec::new();
    h.memo.try_reserve(129).unwrap();
    let memo_bytes = table_bytes(h.memo.capacity());
    for n in 0..129 {
        let mut raster = RasterLayer::new(2, 1, Rgba::TRANSPARENT);
        raster
            .blend_pixel(
                0,
                0,
                Rgba([n as u8, 2, 3, 255]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let key = raster.tile_leases().next().unwrap().1.version();
        let _ = h.memo.insert_reserved(key, Weak::new()).unwrap();
        rasters.push(raster);
    }
    let mut text = String::with_capacity(8192);
    text.push_str("preserve source diagnostic");
    h.last_packing_error = Some(Error::new(ErrorCode::StorageFailed, text).at(5));
    let deny = SharedBudget::new(0);
    let mut account = deny.account(AllocationClass::CoverageMasks);
    h.last_cleanup_error = Some(account.resize(7).unwrap_err());
    let entries = h.entries.as_ptr();
    let key = h
        .memo
        .iter()
        .next()
        .map(|(k, _)| k as *const TileVersion as usize)
        .unwrap();
    let message = h.last_packing_error.as_ref().unwrap().message.as_ptr();
    let doc = Arc::downgrade(document(&h));
    let doc_count = doc.strong_count();
    let (retained, control, total) = expected(&h, memo_bytes);
    let budget = SharedBudget::new(total);
    let scope = budget
        .reserve_capacity(total)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let counter = Cell::new(0);
    let source = capture(h, scope, &counter);
    assert_eq!(counter.get(), 64);
    assert_eq!(source.history().entries.as_ptr(), entries);
    assert_eq!(
        source
            .history()
            .memo
            .iter()
            .next()
            .map(|(k, _)| k as *const TileVersion as usize),
        Some(key)
    );
    assert_eq!(
        source
            .history()
            .last_packing_error
            .as_ref()
            .unwrap()
            .message
            .as_ptr(),
        message
    );
    assert_eq!(doc.strong_count(), doc_count);
    assert_eq!(source.inner.retained.assigned_bytes(), retained);
    assert_eq!(source.inner.control.bytes(), control);
    assert_eq!(source.assigned_bytes().unwrap(), total);
    let denial = source
        .history()
        .last_cleanup_error
        .as_ref()
        .unwrap()
        .allocation_denial()
        .unwrap();
    assert_eq!(denial.required_bytes(), 7);
    assert_eq!(denial.class(), AllocationClass::CoverageMasks);
    drop(source);
    assert_eq!(budget.usage().reserved, 0);
    assert!(doc.upgrade().is_none());
    drop(rasters);
}
#[test]
fn exact_minus_one_and_cancellation_return_original_owner_before_any_move() {
    for kind in 0..5 {
        let h = empty();
        let ptr = h.entries.as_ptr();
        let doc = Arc::downgrade(document(&h));
        let (_, _, total) = expected(&h, 0);
        let capacity = if kind == 0 { total - 1 } else { total };
        let budget = SharedBudget::new(total + (1 << 20));
        let reservation = budget.reserve_capacity(capacity).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let checks = Cell::new(0);
        let counter = Cell::new(0);
        let limit = if kind == 1 { 63 } else { 64 };
        let check = || {
            checks.set(checks.get() + 1);
            if kind >= 2 && checks.get() == kind - 1 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "source capture cancel",
                ))
            } else {
                Ok(())
            }
        };
        let rejected = HistoryReadSource::capture_owned(
            h,
            scope.clone(),
            &|n| spent(&counter, limit, n),
            &check,
        )
        .unwrap_err();
        assert_eq!(rejected.original.entries.as_ptr(), ptr);
        assert!(Arc::ptr_eq(
            document(&rejected.original),
            &doc.upgrade().unwrap()
        ));
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(reservation.available(), capacity);
        assert_eq!(budget.usage().reserved, capacity);
        assert_eq!(counter.get(), if kind == 1 || kind == 2 { 0 } else { 64 });
        assert_eq!(
            rejected.error.code,
            if kind >= 2 {
                ErrorCode::OperationCancelled
            } else {
                ErrorCode::ResourceLimit
            }
        );
        drop(rejected);
        assert!(doc.upgrade().is_none());
    }
}
#[test]
fn deleted_and_retained_empty_memo_extent_is_never_inferred_from_capacity() {
    let mut h = empty();
    h.memo.try_reserve(1024).unwrap();
    let upper = table_bytes(h.memo.capacity());
    assert!(upper > 0);
    let mut rasters = Vec::new();
    for n in 0..2 {
        let mut raster = RasterLayer::new(2, 1, Rgba::TRANSPARENT);
        raster
            .blend_pixel(
                0,
                0,
                Rgba([n, 2, 3, 255]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let key = raster.tile_leases().next().unwrap().1.version();
        let _ = h.memo.insert_reserved(key, Weak::new()).unwrap();
        rasters.push(raster);
    }
    h.memo.retain(|_, _| false);
    assert_eq!(h.memo.len(), 0);
    assert_eq!(
        HistoryReadSource::quote_owned(&h).unwrap().capture_bytes,
        expected(&h, upper).2
    );
    // Deleted/cleared entries do not release the table's retained allocation.
    h.memo.clear();
    assert_eq!(h.memo.len(), 0);
    let (_, _, total) = expected(&h, upper);
    assert_eq!(
        HistoryReadSource::quote_owned(&h).unwrap().capture_bytes,
        total
    );
    let budget = SharedBudget::new(total);
    let source = capture(
        h,
        budget.scope(AllocationClass::PaintMetadata),
        &Cell::new(0),
    );
    assert_eq!(source.assigned_bytes().unwrap(), total);
    drop(source);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn shared_clone_drop_and_unique_recovery_preserve_exact_storage_and_credit() {
    let h = empty();
    let ptr = h.entries.as_ptr();
    let (retained, control, total) = expected(&h, 0);
    let budget = SharedBudget::new(total);
    let source = capture(
        h,
        budget.scope(AllocationClass::PaintMetadata),
        &Cell::new(0),
    );
    let other = source.clone();
    assert!(source.same_source(&other));
    let source = source.try_into_retained().unwrap_err();
    assert!(source.same_source(&other));
    assert_eq!(budget.usage().reserved, total);
    drop(other);
    assert_eq!(budget.usage().reserved, total);
    let owner = source.try_into_retained().unwrap();
    assert_eq!(owner.history().entries.as_ptr(), ptr);
    assert_eq!(owner.assigned_bytes(), retained);
    assert_eq!(budget.usage().reserved, total - control);
    drop(owner);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn arc_block_and_source_storage_retire_before_reentrant_account_notifications() {
    let h = empty();
    let weak = Arc::downgrade(document(&h));
    let (_, _, total) = expected(&h, 0);
    let budget = SharedBudget::new(total);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let source = capture(h, scope.clone(), &Cell::new(0));
    let last = source.clone();
    let arc_gone = Arc::new(AtomicBool::new(false));
    let _observation = retained::observation::install(arc_gone.clone());
    let observed = Arc::new(AtomicBool::new(false));
    let flag = observed.clone();
    let arc = arc_gone.clone();
    let probe = budget.clone();
    let watch = budget
        .watch(total, move || {
            if probe.usage().reserved < total {
                flag.store(
                    arc.load(Ordering::Acquire) && weak.upgrade().is_none(),
                    Ordering::Release,
                );
            }
        })
        .unwrap();
    watch.observe();
    drop(source);
    assert_eq!(budget.usage().reserved, total);
    assert!(!arc_gone.load(Ordering::Acquire));
    drop(last);
    assert!(arc_gone.load(Ordering::Acquire));
    assert!(observed.load(Ordering::Acquire));
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn fork_handoff_retires_only_unused_factory_and_keeps_assigned_storage() {
    for keep_factory in [false, true] {
        let h = empty();
        let budget = SharedBudget::new(1 << 20);
        let reservation = budget.reserve_capacity(1 << 20).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let extra = keep_factory.then(|| scope.clone());
        let counter = Cell::new(0);
        let debit = |n| spent(&counter, 1 << 20, n);
        let fork = run(
            h.cooperative_fork(scope.clone(), &debit, &|| Ok(()), YieldOnce::default),
            || {},
        )
        .unwrap();
        let (assigned, _, _) = expected(fork.history(), 0);
        drop(scope);
        drop(reservation);
        assert_eq!(budget.usage().reserved, 1 << 20, "fork still owns factory");
        let before = counter.get();
        let retained = fork.into_retained_storage().unwrap();
        assert_eq!(counter.get(), before);
        assert_eq!(retained.assigned_bytes(), assigned);
        assert_eq!(
            budget.usage().reserved,
            if keep_factory { 1 << 20 } else { assigned }
        );
        drop(extra);
        assert_eq!(budget.usage().reserved, assigned);
        drop(retained);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn already_funded_source_adds_only_actual_arc_control_without_double_storage_charge() {
    let h = empty();
    let budget = SharedBudget::new(1 << 20);
    let counter = Cell::new(0);
    let debit = |n| spent(&counter, 1 << 20, n);
    let scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let fork = run(
        h.cooperative_fork(scope, &debit, &|| Ok(()), YieldOnce::default),
        || {},
    )
    .unwrap();
    let (assigned, control, total) = expected(fork.history(), 0);
    let ptr = fork.history().entries.as_ptr();
    let retained = fork.into_retained_storage().unwrap();
    assert_eq!(budget.usage().reserved, assigned);
    let control_scope = budget
        .reserve_capacity(control)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let source =
        HistoryReadSource::capture_retained(retained, control_scope, &debit, &|| Ok(())).unwrap();
    assert_eq!(source.history().entries.as_ptr(), ptr);
    assert_eq!(source.assigned_bytes().unwrap(), total);
    assert_eq!(budget.usage().reserved, assigned + control);
    let retained = source.try_into_retained().unwrap();
    assert_eq!(budget.usage().reserved, assigned);
    drop(retained);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn retained_capture_refusal_returns_same_account_and_actual_backing() {
    let h = empty();
    let budget = SharedBudget::new(1 << 20);
    let counter = Cell::new(0);
    let debit = |n| spent(&counter, 1 << 20, n);
    let fork = run(
        h.cooperative_fork(
            budget.scope(AllocationClass::PaintMetadata),
            &debit,
            &|| Ok(()),
            YieldOnce::default,
        ),
        || {},
    )
    .unwrap();
    let (_, control, _) = expected(fork.history(), 0);
    let retained = fork.into_retained_storage().unwrap();
    let assigned = retained.assigned_bytes();
    let ptr = retained.history().entries.as_ptr();
    let reservation = budget.reserve_capacity(control - 1).unwrap();
    let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
    let rejected = HistoryReadSource::capture_retained(retained, scope.clone(), &debit, &|| Ok(()))
        .unwrap_err();
    assert_eq!(rejected.original.assigned_bytes(), assigned);
    assert_eq!(rejected.original.history().entries.as_ptr(), ptr);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(reservation.available(), control - 1);
    drop(rejected);
    drop(scope);
    drop(reservation);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn owned_future_preserves_borrowed_fork_semantics_and_independent_exact_work_boundary() {
    for exact in [true, false] {
        let h = empty();
        let source_entries = h.entries.as_ptr();
        let memory = h.memory_usage();
        let reference = h.try_fork().unwrap();
        let budget = SharedBudget::new(1 << 20);
        let scope = budget
            .reserve_capacity(1 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let counter = Cell::new(0);
        // Capture64 + qualified zero-memo fork64 + reserve2Entries + clone1Entry.
        let exact_work = 128 + (3 * size_of::<Entry>()) as u64;
        let limit = exact_work - u64::from(!exact);
        let debit = |n| spent(&counter, limit, n);
        let source =
            HistoryReadSource::capture_owned(h, scope.clone(), &debit, &|| Ok(())).unwrap();
        let mut polls = 0;
        let result = run(
            source
                .clone()
                .fork_owned(scope.clone(), &debit, &|| Ok(()), YieldOnce::default),
            || polls += 1,
        );
        assert!(polls > 0);
        if exact {
            let copy = result.unwrap();
            same_copied(copy.history(), &reference);
            assert_eq!(counter.get(), exact_work);
            drop(copy);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert!(counter.get() < limit);
        }
        assert_eq!(source.history().entries.as_ptr(), source_entries);
        assert_eq!(source.history().memory_usage(), memory);
        assert_eq!(scope.footprint().live, source.assigned_bytes().unwrap());
        let before = counter.get();
        drop(source);
        assert_eq!(counter.get(), before);
        assert_eq!(scope.footprint().live, 0);
        drop(scope);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn owned_future_cancel_and_drop_keep_real_source_and_spent_debt() {
    for drop_pending in [false, true] {
        for at in 1..=4 {
            let h = empty();
            let budget = SharedBudget::new(1 << 20);
            let scope = budget.scope(AllocationClass::PaintMetadata);
            let counter = Cell::new(0);
            let debit = |n| spent(&counter, 1 << 20, n);
            let source =
                HistoryReadSource::capture_owned(h, scope.clone(), &debit, &|| Ok(())).unwrap();
            let cancelled = Cell::new(false);
            let check = || {
                if cancelled.get() {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "owned source cancelled",
                    ))
                } else {
                    Ok(())
                }
            };
            let mut future = Box::pin(source.clone().fork_owned(
                scope.clone(),
                &debit,
                &check,
                YieldOnce::default,
            ));
            let mut cx = Context::from_waker(Waker::noop());
            for _ in 0..at {
                assert!(future.as_mut().poll(&mut cx).is_pending());
            }
            if drop_pending {
                drop(future);
            } else {
                cancelled.set(true);
                let result = match future.as_mut().poll(&mut cx) {
                    Poll::Ready(value) => value,
                    Poll::Pending => panic!("cancel must stop at resumption"),
                };
                assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
                drop(future);
            }
            assert!(counter.get() >= 128);
            assert_eq!(scope.footprint().live, source.assigned_bytes().unwrap());
            assert_eq!(source.history().undo_depth(), 0);
            let before = counter.get();
            drop(source);
            assert_eq!(counter.get(), before);
            assert_eq!(scope.footprint().live, 0);
            assert_eq!(budget.usage().reserved, 0);
        }
    }
}
#[test]
fn handoff_rejection_never_drops_an_underfunded_original_fork_owner() {
    // Child-module whitebox deliberately underfunds only this malformed owner;
    // the public cooperative constructor never returns it in this state.
    let h = empty();
    let ptr = h.entries.as_ptr();
    let scope = SharedBudget::new(4096).scope(AllocationClass::PaintMetadata);
    let account = scope.account();
    let rejected = retained::from_fork_parts(h, account).unwrap_err();
    assert_eq!(rejected.history.entries.as_ptr(), ptr);
    assert_eq!(rejected.account.bytes(), 0);
    assert_eq!(rejected.error.code, ErrorCode::ResourceLimit);
    assert_eq!(scope.footprint().live, 0);
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn concurrent_last_clone_retirement_releases_actual_source_once() {
    let h = empty();
    let weak = Arc::downgrade(document(&h));
    let (_, _, total) = expected(&h, 0);
    let budget = SharedBudget::new(total);
    let source = capture(
        h,
        budget.scope(AllocationClass::PaintMetadata),
        &Cell::new(0),
    );
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let source = source.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            drop(source);
        }));
    }
    drop(source);
    barrier.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    assert!(weak.upgrade().is_none());
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn future_itself_is_the_only_history_owner_across_pending_and_drop() {
    let h = empty();
    let weak = Arc::downgrade(document(&h));
    let budget = SharedBudget::new(1 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let counter = Cell::new(0);
    let debit = |n| spent(&counter, 1 << 20, n);
    let check = || Ok(());
    let source = HistoryReadSource::capture_owned(h, scope.clone(), &debit, &check).unwrap();
    let mut future = Box::pin(source.fork_owned(scope.clone(), &debit, &check, YieldOnce::default));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert!(
        weak.upgrade().is_some(),
        "future owns real source, no external publisher handle"
    );
    let paid = counter.get();
    drop(future);
    assert!(weak.upgrade().is_none());
    assert_eq!(counter.get(), paid);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn different_retained_and_injected_control_realms_are_preserved_without_fallback() {
    let h = empty();
    let backing_budget = SharedBudget::new(1 << 20);
    let control_budget = SharedBudget::new(1 << 20);
    let counter = Cell::new(0);
    let debit = |n| spent(&counter, 1 << 20, n);
    let fork = run(
        h.cooperative_fork(
            backing_budget
                .reserve_capacity(1 << 20)
                .unwrap()
                .scope(AllocationClass::PaintMetadata),
            &debit,
            &|| Ok(()),
            YieldOnce::default,
        ),
        || {},
    )
    .unwrap();
    let (assigned, control, _) = expected(fork.history(), 0);
    let storage = fork.into_retained_storage().unwrap();
    assert_eq!(backing_budget.usage().reserved, assigned);
    let reservation = control_budget.reserve_capacity(control - 1).unwrap();
    let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
    let before = counter.get();
    let rejected = HistoryReadSource::capture_retained(storage, scope.clone(), &debit, &|| Ok(()))
        .unwrap_err();
    assert_eq!(counter.get(), before + 64);
    assert_eq!(backing_budget.usage().reserved, assigned);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(reservation.available(), control - 1);
    assert_eq!(rejected.error.code, ErrorCode::ResourceLimit);
    let storage = rejected.original;
    drop(scope);
    drop(reservation);
    assert_eq!(control_budget.usage().reserved, 0);
    let reservation = control_budget.reserve_capacity(control).unwrap();
    let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
    let source =
        HistoryReadSource::capture_retained(storage, scope.clone(), &debit, &|| Ok(())).unwrap();
    assert_eq!(
        counter.get(),
        before + 128,
        "denied capture debt was not reset"
    );
    assert_eq!(source.assigned_bytes().unwrap(), assigned + control);
    assert_eq!(backing_budget.usage().reserved, assigned);
    assert_eq!(
        scope.footprint().live,
        control,
        "only new controls belong to injected realm"
    );
    assert_eq!(reservation.available(), 0);
    let storage = source.try_into_retained().unwrap();
    assert_eq!(backing_budget.usage().reserved, assigned);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(reservation.available(), control);
    drop(scope);
    drop(reservation);
    assert_eq!(control_budget.usage().reserved, 0);
    drop(storage);
    assert_eq!(backing_budget.usage().reserved, 0);
}
