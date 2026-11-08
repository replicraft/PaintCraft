use super::*;
use crate::{Compression, Label, Packing, Retention, Snapshot, Storage};
use paintcraft_core::types::{
    ErrorCode, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::{Admission, BlendMode, RasterLayer, Selection, TileVersion};
use std::{
    cell::Cell,
    future::Future,
    pin::pin,
    sync::{Arc, Weak},
    task::{Context, Poll, Waker},
};

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
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..10_000 {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => pending(),
        }
    }
    panic!("test continuation failed to finish");
}
fn empty() -> History {
    let doc =
        paintcraft_document::new_document(2, 1, Some(Rgba([0; 4])), 0, paintcraft_document::POLICY)
            .unwrap();
    History::new(Snapshot {
        document: Arc::new(doc),
        selection: Selection::Inactive,
        history_id: None,
    })
}
// Independent table arithmetic: derive bucket count by load-capacity search,
// rather than calling production fresh_map_layout/Quote/scan_work_upper.
fn layout(count: usize) -> (usize, u64, u64) {
    if count == 0 {
        return (0, 0, 0);
    }
    let mut buckets = 4usize;
    loop {
        let capacity = if buckets <= 8 {
            buckets - 1
        } else {
            buckets / 8 * 7
        };
        if capacity >= count {
            let align = std::mem::align_of::<(TileVersion, Weak<archive::Blob>)>().max(16);
            let payload = buckets * size_of::<(TileVersion, Weak<archive::Blob>)>();
            let control = payload.div_ceil(align) * align;
            let bytes = (control + buckets + 16) as u64;
            return (capacity, bytes, bytes + buckets as u64);
        }
        buckets *= 2;
    }
}
fn memo_source(count: usize) -> (History, Vec<RasterLayer>, u64) {
    let mut history = empty();
    history.memo.try_reserve(count).unwrap();
    let full = history.memo.capacity();
    let scan = layout(full).2;
    let mut rasters = Vec::new();
    for n in 0..count {
        let mut raster = RasterLayer::new(2, 1, Rgba::TRANSPARENT);
        raster
            .blend_pixel(
                0,
                0,
                Rgba([n as u8, 4, 7, 255]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let version = raster.tile_leases().next().unwrap().1.version();
        let _ = history.memo.insert_reserved(version, Weak::new()).unwrap();
        rasters.push(raster);
    }
    (history, rasters, scan)
}
fn expected(source: &History, survivors: usize, scan: u64) -> (u64, u64) {
    let errors = source
        .last_cleanup_error
        .as_ref()
        .map_or(0, |e| e.message.len())
        + source
            .last_packing_error
            .as_ref()
            .map_or(0, |e| e.message.len());
    let initialization =
        layout(survivors).1 + ((source.entries.len() + 1) * size_of::<Entry>() + errors) as u64;
    let bytes = size_of::<ForkedHistory>() as u64 + initialization;
    let visits = (source.memo.len() * 2 * (size_of::<TileVersion>() + 8)) as u64;
    let copies = survivors as u64
        * ((size_of::<(TileVersion, Weak<archive::Blob>)>() + size_of::<TileVersion>()) as u64
            + 2 * (layout(survivors).2 - layout(survivors).1)
            + (2 * size_of::<usize>()) as u64);
    let entries = (source.entries.len() * size_of::<Entry>()) as u64;
    (
        bytes,
        64 + scan * 2 + visits + copies + initialization + errors as u64 + entries,
    )
}
fn debit(remaining: &Cell<u64>, units: u64) -> Result<()> {
    match remaining.get().checked_sub(units) {
        Some(next) => {
            remaining.set(next);
            Ok(())
        }
        None => Err(archive::limit("test cumulative work denied")),
    }
}
fn copy(source: &History, budget: &SharedBudget, work: u64) -> Result<ForkedHistory> {
    let remaining = Cell::new(work);
    let spend = |n| debit(&remaining, n);
    run(
        source.cooperative_fork(
            budget.scope(AllocationClass::PaintMetadata),
            &spend,
            &|| Ok(()),
            YieldOnce::default,
        ),
        || {},
    )
}
fn same(left: &History, right: &History) {
    assert_eq!(
        left.entries().collect::<Vec<_>>(),
        right.entries().collect::<Vec<_>>()
    );
    assert_eq!(left.cursor, right.cursor);
    assert_eq!(left.memory_usage(), right.memory_usage());
    assert_eq!(left.memo.capacity(), right.memo.capacity());
    assert_eq!(left.memo.len(), right.memo.len());
    assert_eq!(left.budget, right.budget);
    assert_eq!(left.trimmed_total, right.trimmed_total);
    assert_eq!(left.last_trimmed, right.last_trimmed);
    assert_eq!(left.packing, right.packing);
    assert_eq!(left.retention, right.retention);
    assert_eq!(left.compression, right.compression);
    assert_eq!(left.restore_pixels, right.restore_pixels);
    assert_eq!(left.last_cleanup_error, right.last_cleanup_error);
    assert_eq!(left.last_packing_error, right.last_packing_error);
    for (a, b) in left.entries.iter().zip(&right.entries) {
        match (&a.storage, &b.storage) {
            (Storage::Hot(a), Storage::Hot(b)) => assert!(Arc::ptr_eq(&a.document, &b.document)),
            (Storage::Cold(a), Storage::Cold(b)) => assert!(Arc::ptr_eq(a, b)),
            _ => panic!("fork changed storage class"),
        }
    }
}
#[test]
fn independent_layout_cases_and_legacy_limit_are_fixed_before_execution() {
    for (count, expected_buckets) in [
        (0, 0),
        (1, 4),
        (3, 4),
        (4, 8),
        (7, 8),
        (8, 16),
        (14, 16),
        (15, 32),
        (28, 32),
        (29, 64),
    ] {
        let (_, bytes, scan) = layout(count);
        assert_eq!(scan - bytes, expected_buckets);
    }
    let entry = 2 * size_of::<Entry>();
    let slot = size_of::<(TileVersion, Weak<archive::Blob>)>() + 160;
    let max = ((64 << 20) - entry) / slot;
    assert!(quote::legacy(max, 1).is_ok());
    assert!(quote::legacy(max + 1, 1).is_err());
    assert!(quote::legacy(usize::MAX, 1).is_err());
    assert!(quote::legacy(0, usize::MAX).is_err());
}
#[test]
fn exact_independent_work_and_bytes_succeed_minus_one_refuses_without_source_change() {
    let (source, _keep, scan) = memo_source(129);
    let reference = source.try_fork().unwrap();
    let (bytes, work) = expected(&source, 129, scan);
    let budget = SharedBudget::new(bytes);
    let copied = copy(&source, &budget, work).unwrap();
    assert_eq!(copied.copied_bytes(), bytes);
    assert_eq!(budget.usage().reserved, bytes);
    same(copied.history(), &reference);
    drop(copied);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(
        copy(&source, &budget, work - 1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
    let tiny = SharedBudget::new(bytes - 1);
    assert_eq!(
        copy(&source, &tiny, work).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(tiny.usage().reserved, 0);
    same(&source.try_fork().unwrap(), &reference);
    assert_eq!(source.memo.len(), 129);
}
#[test]
fn dead_versions_tombstones_and_empty_retained_table_are_priced_not_compacted_in_source() {
    let (mut source, mut keep, scan) = memo_source(129);
    let old_capacity = source.memo.capacity();
    // Dead keys remain in the source table but must not survive the copy.
    keep.truncate(1);
    let (bytes, work) = expected(&source, 1, scan);
    let budget = SharedBudget::new(bytes);
    let copy1 = copy(&source, &budget, work).unwrap();
    same(copy1.history(), &source.try_fork().unwrap());
    assert_eq!(source.memo.len(), 129);
    assert_eq!(source.memo.capacity(), old_capacity);
    drop(copy1);
    source.memo.retain(|key, _| key.resident_bytes() != 0);
    assert_eq!(source.memo.len(), 1);
    assert_eq!(source.memo.scan_work_upper().unwrap(), scan);
    let (bytes, work) = expected(&source, 1, scan);
    let next = copy(&source, &SharedBudget::new(bytes), work).unwrap();
    same(next.history(), &source.try_fork().unwrap());
    drop(next);
    source.memo.clear();
    assert_eq!(source.memo.scan_work_upper().unwrap(), scan);
    let (bytes, work) = expected(&source, 0, scan);
    assert!(work > expected(&empty(), 0, 0).1);
    let empty_copy = copy(&source, &SharedBudget::new(bytes), work).unwrap();
    assert!(empty_copy.history().memo.is_empty());
    assert_eq!(empty_copy.history().memo.capacity(), 0);
    assert!(source.memo.capacity() > 0);
}
#[test]
fn every_yield_cancellation_and_partial_future_drop_releases_copied_storage() {
    let (source, _keep, scan) = memo_source(129);
    let (bytes, work) = expected(&source, 129, scan);
    let yields = 4 + 2 * (129 / 64) + source.entries.len() / 64;
    for at in 1..=yields {
        let budget = SharedBudget::new(bytes);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let cancelled = Cell::new(false);
        let remaining = Cell::new(work);
        let spend = |n| debit(&remaining, n);
        let check = || {
            if cancelled.get() {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel test fork",
                ))
            } else {
                Ok(())
            }
        };
        let mut seen = 0;
        let result = run(
            source.cooperative_fork(scope.clone(), &spend, &check, YieldOnce::default),
            || {
                seen += 1;
                if seen == at {
                    cancelled.set(true);
                }
            },
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(seen, at);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
        assert!(
            remaining.get() < work,
            "cancellation never refunds spent work"
        );
    }
    for at in 1..=yields {
        let budget = SharedBudget::new(bytes);
        let remaining = Cell::new(work);
        let spend = |n| debit(&remaining, n);
        let check = || Ok(());
        let mut future = Box::pin(source.cooperative_fork(
            budget.scope(AllocationClass::PaintMetadata),
            &spend,
            &check,
            YieldOnce::default,
        ));
        let mut context = Context::from_waker(Waker::noop());
        for _ in 0..at {
            assert!(future.as_mut().poll(&mut context).is_pending());
        }
        drop(future);
        assert_eq!(budget.usage().reserved, 0);
    }
    assert_eq!(source.memo.len(), 129);
}
#[test]
fn tiny_original_reservation_cannot_borrow_global_headroom() {
    let source = empty();
    let (bytes, work) = expected(&source, 0, 0);
    let budget = SharedBudget::new(1 << 20);
    let scope = budget
        .reserve_capacity(bytes - 1)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let remaining = Cell::new(work);
    let spend = |n| debit(&remaining, n);
    let result = run(
        source.cooperative_fork(scope.clone(), &spend, &|| Ok(()), YieldOnce::default),
        || {},
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, bytes - 1);
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn hot_cold_redo_policy_errors_and_original_denial_are_preserved() {
    let mut source = empty();
    source.set_packing(Packing::Shared);
    source.set_retention(Retention::TrimOldest);
    source.set_compression(Compression::Lz4);
    source.set_restore_budget(123456);
    for value in [21, 87, 143] {
        let mut document = (*source.restore(source.cursor, &|| Ok(())).unwrap().document).clone();
        let id = document.roots[0];
        document
            .layers
            .get_mut(&id)
            .unwrap()
            .raster
            .blend_pixel(
                0,
                0,
                Rgba([value, 37, 91, 255]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        document.state_id = uuid::Uuid::new_v4();
        source
            .record_labeled(
                Snapshot {
                    document: Arc::new(document),
                    selection: Selection::Inactive,
                    history_id: Some(uuid::Uuid::new_v4()),
                },
                Label::Brush,
            )
            .unwrap();
    }
    source.compact(1, 64 << 20, || Ok(())).unwrap();
    source.undo().unwrap().unwrap();
    let denial_budget = SharedBudget::new(0);
    let mut denied = denial_budget.account(AllocationClass::CoverageMasks);
    source.last_cleanup_error = Some(denied.resize(11).unwrap_err());
    let mut text = String::with_capacity(1024);
    text.push_str("copied policy diagnostic");
    source.last_packing_error = Some(Error::new(ErrorCode::StorageFailed, text).at(3));
    let reference = source.try_fork().unwrap();
    let survivors = source
        .memo
        .iter()
        .filter(|(key, _)| key.resident_bytes() != 0)
        .count();
    let scan = source.memo.scan_work_upper().unwrap();
    let (bytes, work) = expected(&source, survivors, scan);
    let budget = SharedBudget::new(bytes);
    let copied = copy(&source, &budget, work).unwrap();
    same(copied.history(), &reference);
    let denial = copied
        .history()
        .last_cleanup_error
        .as_ref()
        .unwrap()
        .allocation_denial()
        .unwrap();
    assert_eq!(denial.required_bytes(), 11);
    assert_eq!(denial.class(), AllocationClass::CoverageMasks);
    assert!(!denial.is_possible());
    assert_eq!(
        copied
            .history()
            .last_packing_error
            .as_ref()
            .unwrap()
            .message
            .capacity(),
        "copied policy diagnostic".len()
    );
    assert!(copied.history().redo_depth() > 0);
    assert!(
        copied
            .history()
            .entries
            .iter()
            .any(|entry| matches!(entry.storage, Storage::Cold(_)))
    );
}
#[test]
fn reentrant_release_observes_history_handles_retired_before_account() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let source = empty();
    let Storage::Hot(initial) = &source.entries[0].storage else {
        panic!("initial hot")
    };
    let weak = Arc::downgrade(&initial.document);
    let baseline = Arc::strong_count(&initial.document);
    let (bytes, work) = expected(&source, 0, 0);
    let budget = SharedBudget::new(bytes);
    let copied = copy(&source, &budget, work).unwrap();
    assert_eq!(Arc::strong_count(&initial.document), baseline + 1);
    let observed = Arc::new(AtomicBool::new(false));
    let flag = observed.clone();
    let probe = budget.clone();
    let watch = budget
        .watch(1, move || {
            if probe.usage().reserved == 0 {
                flag.store(weak.strong_count() == baseline, Ordering::Release);
            }
        })
        .unwrap();
    watch.observe();
    drop(copied);
    assert!(observed.load(Ordering::Acquire));
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn one_shot_cancellation_at_every_check_cannot_be_swallowed_by_a_later_ok() {
    let (source, _keep, scan) = memo_source(129);
    let (bytes, work) = expected(&source, 129, scan);
    // Independent control-flow count, not a successful-run counter used as a grant.
    let checkpoints =
        10 + 4 * 129 + 129 + 2 * source.entries.len() + 2 * (129 / 64) + source.entries.len() / 64;
    for at in 1..=checkpoints {
        let budget = SharedBudget::new(bytes);
        let remaining = Cell::new(work);
        let calls = Cell::new(0usize);
        let spend = |n| debit(&remaining, n);
        let check = || {
            let next = calls.get() + 1;
            calls.set(next);
            if next == at {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "one-shot cancellation",
                ))
            } else {
                Ok(())
            }
        };
        let result = run(
            source.cooperative_fork(
                budget.scope(AllocationClass::PaintMetadata),
                &spend,
                &check,
                YieldOnce::default,
            ),
            || {},
        );
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "check {at}"
        );
        assert_eq!(calls.get(), at, "no callback after the first failure");
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(source.memo.len(), 129);
    }
}
