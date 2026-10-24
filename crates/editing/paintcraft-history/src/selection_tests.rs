use super::*;
use paintcraft_core::types::{ErrorCode, Rect, resources::SharedBudget, text::MaskSpan};
use paintcraft_document::{POLICY, new_document};
use paintcraft_raster::selection::{BinaryMask, MaskLimits};
use std::cell::Cell;

fn mask(budget: &SharedBudget) -> BinaryMask {
    BinaryMask::from_spans(
        Rect::new(0, 0, 32, 32).unwrap(),
        false,
        &[MaskSpan {
            y: 3,
            x0: 2,
            x1: 5,
            coverage: 255,
        }],
        budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap()
}

fn document() -> Arc<Document> {
    Arc::new(new_document(32, 32, None, 0, POLICY).unwrap())
}

fn snapshot(doc: &Arc<Document>, selection: Selection) -> Snapshot {
    Snapshot {
        document: Arc::clone(doc),
        selection,
        history_id: Some(Uuid::new_v4()),
    }
}

#[test]
fn cold_snapshots_restore_exact_shared_mask_views_and_count_unique_payload() {
    let budget = SharedBudget::new(1 << 20);
    let original = mask(&budget);
    let bytes = budget.usage().reserved;
    assert_eq!(original.storage_usage().unwrap().1, bytes);
    let inverse = original.inverted();
    let shifted = inverse.translated(-2, 9).unwrap();
    let doc = document();
    let initial = snapshot(&doc, Selection::Mask(original.clone()));
    let initial_id = initial.history_id;
    let mut history = History::new(initial);
    history
        .record(snapshot(&document(), Selection::Mask(inverse.clone())))
        .unwrap();
    history
        .record(snapshot(&document(), Selection::Mask(shifted.clone())))
        .unwrap();
    assert_eq!(history.memory_usage().selection_bytes, bytes);
    let before = history.memory_usage();
    let compact = history.compact(1, 1 << 20, || Ok(())).unwrap();
    assert_eq!(compact.packed_entries, 2);
    assert_eq!(history.memory_usage().selection_bytes, bytes);
    assert_eq!(history.memory_usage().decoded_bytes, before.decoded_bytes);
    let weak_doc = Arc::downgrade(&doc);
    drop(doc);
    assert!(
        weak_doc.upgrade().is_none(),
        "cold restore must reconstruct the document"
    );
    let (restored, changed) = history.seek(initial_id).unwrap().unwrap();
    assert!(changed);
    assert_eq!(restored.selection, Selection::Mask(original.clone()));
    let Selection::Mask(restored_mask) = &restored.selection else {
        panic!("mask selection was lost during packing");
    };
    assert!(restored_mask.shares_storage(&original));
    assert_eq!(restored_mask.coverage(2, 3), 255);
    let next = history.redo().unwrap().unwrap();
    assert_eq!(next.selection, Selection::Mask(inverse.clone()));
    assert_eq!(inverse.coverage(2, 3), 0);
    assert_eq!(inverse.coverage(0, 0), 255);
    let last = history.redo().unwrap().unwrap();
    assert_eq!(last.selection, Selection::Mask(shifted));
    assert_eq!(history.memory_usage().selection_bytes, bytes);
    drop(original);
    drop(inverse);
    drop(history);
    assert_eq!(
        budget.usage().reserved,
        bytes,
        "restored readers keep ownership"
    );
    drop(restored);
    drop(next);
    drop(last);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn independent_masks_count_separately_but_uniform_fields_have_no_payload() {
    let budget = SharedBudget::new(1 << 20);
    let first = mask(&budget);
    let first_bytes = first.storage_usage().unwrap().1;
    let second = mask(&budget);
    assert_ne!(
        first.storage_usage().unwrap().0,
        second.storage_usage().unwrap().0
    );
    let doc = document();
    let mut history = History::new(snapshot(&doc, Selection::Mask(first)));
    history
        .record(snapshot(&doc, Selection::Mask(second)))
        .unwrap();
    history
        .record(snapshot(
            &doc,
            Selection::Mask(BinaryMask::uniform(Rect::new(0, 0, 32, 32).unwrap(), false).unwrap()),
        ))
        .unwrap();
    assert_eq!(history.memory_usage().selection_bytes, 2 * first_bytes);
    assert_eq!(budget.usage().reserved, 2 * first_bytes);
    drop(history);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn failed_preserving_record_releases_incoming_mask_without_changing_timeline() {
    let budget = SharedBudget::new(1 << 20);
    let doc = document();
    let original = mask(&budget);
    let original_bytes = budget.usage().reserved;
    let initial = snapshot(&doc, Selection::Mask(original.clone()));
    let initial_id = initial.history_id;
    let mut history = History::new(initial);
    history.set_budget(0);
    let before: Vec<_> = history.entries().collect();
    let error = history
        .record(snapshot(&doc, Selection::Mask(mask(&budget))))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(history.entries().collect::<Vec<_>>(), before);
    assert_eq!(history.undo_depth(), 0);
    assert_eq!(history.memory_usage().selection_bytes, original_bytes);
    assert_eq!(budget.usage().reserved, original_bytes);
    let (retained, changed) = history.seek(initial_id).unwrap().unwrap();
    assert!(!changed);
    assert_eq!(retained.selection, Selection::Mask(original));
}

#[test]
fn mask_payload_pressure_can_trim_only_under_explicit_policy() {
    let budget = SharedBudget::new(1 << 20);
    let doc = document();
    let mut history = History::new(snapshot(&doc, Selection::Mask(mask(&budget))));
    assert!(history.memory_usage().selection_bytes > 0);
    history.set_budget(0);
    history.set_retention(Retention::TrimOldest);
    history.record(snapshot(&doc, Selection::Inactive)).unwrap();
    assert!(
        history.memory_usage().selection_bytes > 0,
        "last undo state is retained"
    );
    history.record(snapshot(&doc, Selection::Empty)).unwrap();
    assert_eq!(history.last_trimmed(), 1);
    assert_eq!(history.memory_usage().selection_bytes, 0);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(
        history.undo().unwrap().unwrap().selection,
        Selection::Inactive
    );
}

#[test]
fn cancelled_record_and_compaction_keep_mask_identity_and_release_private_owners() {
    let budget = SharedBudget::new(1 << 20);
    let original = mask(&budget);
    let original_bytes = budget.usage().reserved;
    let doc = document();
    let mut history = History::new(snapshot(&doc, Selection::Mask(original.clone())));
    let before: Vec<_> = history.entries().collect();
    let calls = Cell::new(0);
    let error = history
        .record_labeled_checked(
            snapshot(&doc, Selection::Mask(mask(&budget))),
            Label::Selection,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel private record",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(history.entries().collect::<Vec<_>>(), before);
    assert_eq!(budget.usage().reserved, original_bytes);
    history.record(snapshot(&doc, Selection::Empty)).unwrap();
    let before: Vec<_> = history.entries().collect();
    assert!(history.compact(1, 0, || Ok(())).is_err());
    assert_eq!(history.entries().collect::<Vec<_>>(), before);
    assert_eq!(history.memory_usage().selection_bytes, original_bytes);
    assert_eq!(
        history.undo().unwrap().unwrap().selection,
        Selection::Mask(original)
    );
}
