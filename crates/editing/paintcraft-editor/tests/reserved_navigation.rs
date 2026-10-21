use paintcraft_commands::Batch;
use paintcraft_core::types::{Error, ErrorCode, Rgba, resources::SharedBudget};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_raster::{Admission, RasterLayer};
use std::sync::Arc;
use uuid::Uuid;
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn fixture() -> (Session, SharedBudget, Option<Uuid>, Uuid) {
    let budget = SharedBudget::new(4 * BYTES);
    let mut owner = Session::new(new_document(257, 1, Some(Rgba([255; 4])), 0, POLICY).unwrap());
    let mut target = None;
    let mut state = owner.document().state_id;
    for (index, (x, width, color)) in [
        (0, 257, "#000000FF"),
        (0, 1, "#FF0000FF"),
        (256, 1, "#0000FFFF"),
    ]
    .into_iter()
    .enumerate()
    {
        let t = owner.ticket();
        let id = owner.document().roots[0];
        let batch = Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":{x},"y":0,"width":{width},"height":1}},"color":"{color}","mode":"replace"}}]}}"##,t.state_id,t.publication_epoch).as_bytes()).unwrap();
        let prepared = owner
            .preparation()
            .unwrap()
            .apply_with_admission(
                &batch,
                Admission::with_residency(4 * 65_536, budget.clone()),
                || Ok(()),
            )
            .unwrap();
        let receipt = owner.publish_prepared(prepared, || Ok(())).unwrap();
        if index == 0 {
            target = receipt.history_id;
            state = receipt.state_id;
        }
    }
    assert_eq!(budget.usage().reserved, 4 * BYTES);
    owner.compact_history(1, 4 << 20, || Ok(())).unwrap();
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    owner.mark_saved();
    (owner, budget, target, state)
}
fn reserved(budget: &SharedBudget, tiles: u64) -> Admission {
    Admission::with_reserved_residency(u64::MAX, budget.reserve_capacity(tiles * BYTES).unwrap())
}
#[test]
fn partial_cold_seek_rolls_back_then_reserved_retry_restores_exact_recorded_pixels_and_identity() {
    let (mut owner, budget, target, state) = fixture();
    let before = owner.snapshot();
    let view = owner.history_view();
    let blocker = budget.reserve(BYTES).unwrap();
    let error = owner
        .preparation()
        .unwrap()
        .navigate_history_with_admission(
            owner.history_ticket(),
            target,
            Admission::with_residency(u64::MAX, budget.clone()),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().required_bytes(),
        2 * BYTES
    );
    assert!(!error.allocation_denial().unwrap().can_retry());
    assert_eq!(budget.usage().reserved, 3 * BYTES);
    assert_eq!(owner.history_view(), view);
    assert!(Arc::ptr_eq(&before, &owner.snapshot()));
    assert!(!owner.is_dirty());
    drop(blocker);
    let reservation = error
        .allocation_denial()
        .unwrap()
        .reserve_capacity()
        .unwrap();
    let prepared = owner
        .preparation()
        .unwrap()
        .navigate_history_with_admission(
            owner.history_ticket(),
            target,
            Admission::with_reserved_residency(u64::MAX, reservation),
            || Ok(()),
        )
        .unwrap();
    assert_eq!(owner.history_view(), view);
    let navigation = owner
        .publish_navigation(prepared, || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(navigation.state_id, state);
    assert_eq!(navigation.history_id, target);
    assert_eq!(
        navigation.publication_epoch,
        view.ticket.base.publication_epoch + 1
    );
    assert_eq!(owner.undo_depth(), 1);
    assert_eq!(owner.redo_depth(), 2);
    assert!(owner.is_dirty());
    let raster = &owner.document().layers[&owner.document().roots[0]].raster;
    for x in 0..257 {
        assert_eq!(raster.get(x, 0), Rgba([0, 0, 0, 255]));
    }
}
#[test]
fn explicit_undo_redo_share_live_versions_and_zero_capacity_can_restore_a_fully_resident_target() {
    let (mut owner, budget, _, first_state) = fixture();
    let prepared = owner
        .preparation()
        .unwrap()
        .undo_with_admission(reserved(&budget, 1), || Ok(()))
        .unwrap();
    owner.publish_navigation(prepared, || Ok(())).unwrap();
    assert_eq!(budget.usage().reserved, 3 * BYTES);
    let prepared = owner
        .preparation()
        .unwrap()
        .undo_with_admission(reserved(&budget, 1), || Ok(()))
        .unwrap();
    owner.publish_navigation(prepared, || Ok(())).unwrap();
    assert_eq!(owner.document().state_id, first_state);
    assert_eq!(budget.usage().reserved, 4 * BYTES);
    let prepared = owner
        .preparation()
        .unwrap()
        .redo_with_admission(reserved(&budget, 0), || Ok(()))
        .unwrap();
    owner.publish_navigation(prepared, || Ok(())).unwrap();
    let raster = &owner.document().layers[&owner.document().roots[0]].raster;
    assert_eq!(raster.get(0, 0), Rgba([255, 0, 0, 255]));
    assert_eq!(raster.get(256, 0), Rgba([0, 0, 0, 255]));
    assert_eq!(owner.undo_depth(), 2);
    assert_eq!(owner.redo_depth(), 1);
}
#[test]
fn reserved_restoration_preserves_fixed_restore_limits_and_cancelled_or_stale_publication() {
    let (mut owner, budget, target, _) = fixture();
    let before = owner.snapshot();
    let view = owner.history_view();
    owner.set_history_restore_budget(65_536);
    let error = owner
        .preparation()
        .unwrap()
        .navigate_history_with_admission(
            owner.history_ticket(),
            target,
            reserved(&budget, 2),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_none());
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    owner.set_history_restore_budget(2 * 65_536);
    let prepared = owner
        .preparation()
        .unwrap()
        .navigate_history_with_admission(
            owner.history_ticket(),
            target,
            reserved(&budget, 2),
            || Ok(()),
        )
        .unwrap();
    let error = owner
        .publish_navigation(prepared, || {
            Err(Error::new(ErrorCode::OperationCancelled, "injected"))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    let prepared = owner
        .preparation()
        .unwrap()
        .navigate_history_with_admission(
            owner.history_ticket(),
            target,
            reserved(&budget, 2),
            || Ok(()),
        )
        .unwrap();
    owner.set_history_restore_budget(2 * 65_536); // Retire the captured policy even with equal limits.
    let error = owner.publish_navigation(prepared, || Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ConflictStaleBase);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    assert_eq!(owner.history_view(), view);
    assert!(Arc::ptr_eq(&before, &owner.snapshot()));
}
#[test]
fn cancellation_after_a_partial_decode_returns_capacity_and_keeps_cursor_redo_and_saved_state() {
    let (owner, budget, target, _) = fixture();
    let before = owner.snapshot();
    let view = owner.history_view();
    let error = owner
        .preparation()
        .unwrap()
        .navigate_history_with_admission(
            owner.history_ticket(),
            target,
            Admission::with_residency(u64::MAX, budget.clone()),
            || {
                if budget.usage().reserved > 2 * BYTES {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "after first decode",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    assert_eq!(owner.history_view(), view);
    assert!(Arc::ptr_eq(&before, &owner.snapshot()));
    assert!(!owner.is_dirty());
}
