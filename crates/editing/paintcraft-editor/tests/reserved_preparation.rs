use paintcraft_commands::Batch;
use paintcraft_core::types::{Error, ErrorCode, Rgba, resources::SharedBudget};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_raster::{Admission, RasterLayer};
use std::sync::Arc;
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn session() -> Session {
    Session::new(new_document(257, 1, Some(Rgba([255; 4])), 0, POLICY).unwrap())
}
fn batch(session: &Session) -> Batch {
    let t = session.ticket();
    let id = session.document().roots[0];
    Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":1,"height":1}},"color":"#010203FF","mode":"replace"}},{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":256,"y":0,"width":1,"height":1}},"color":"#040506FF","mode":"replace"}}]}}"##,t.state_id,t.publication_epoch).as_bytes()).unwrap()
}
fn admission(budget: &SharedBudget, tiles: u64) -> Admission {
    Admission::with_reserved_residency(16 * 65_536, budget.reserve_capacity(tiles * BYTES).unwrap())
}
#[test]
fn private_reserved_execution_matches_normal_receipts_and_keeps_only_published_tile_charges() {
    let mut owner = session();
    let original = owner.snapshot();
    let command = batch(&owner);
    let mut oracle = Session::new((*original).clone());
    let expected = oracle.apply(&command).unwrap();
    let budget = SharedBudget::new(3 * BYTES);
    let admitted = admission(&budget, 3);
    assert!(budget.reserve(1).is_err());
    let prepared = owner
        .preparation()
        .unwrap()
        .apply_with_admission(&command, admitted, || Ok(()))
        .unwrap();
    assert!(Arc::ptr_eq(&owner.snapshot(), &original));
    assert_eq!(owner.undo_depth(), 0);
    assert_eq!(budget.usage().reserved, 2 * BYTES); // The unused third tile has returned.
    assert_eq!(
        owner.publish_prepared(prepared, || Ok(())).unwrap(),
        expected
    );
    let id = owner.document().roots[0];
    assert_eq!(
        owner.document().layers[&id].raster.get(0, 0),
        Rgba([1, 2, 3, 255])
    );
    assert_eq!(
        owner.document().layers[&id].raster.get(256, 0),
        Rgba([4, 5, 6, 255])
    );
    assert_eq!(
        owner.document().layers[&id].raster.get(255, 0),
        Rgba([255; 4])
    );
    assert_eq!(owner.document().state_id, oracle.document().state_id);
    let reader = owner.snapshot();
    drop(owner);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    drop(reader);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn an_underestimated_reservation_reports_exact_operation_and_rolls_back_all_private_tiles() {
    let owner = session();
    let original = owner.snapshot();
    let ticket = owner.ticket();
    let command = batch(&owner);
    let budget = SharedBudget::new(4 * BYTES);
    let error = owner
        .preparation()
        .unwrap()
        .apply_with_admission(&command, admission(&budget, 1), || Ok(()))
        .unwrap_err();
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(
        error.allocation_denial().unwrap().required_bytes(),
        2 * BYTES
    );
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(owner.ticket(), ticket);
    assert!(Arc::ptr_eq(&owner.snapshot(), &original));
    assert_eq!(owner.undo_depth(), 0);
}
#[test]
fn explicit_capacity_cannot_bypass_staged_policy_or_cancellation_or_stale_publication() {
    let mut owner = session();
    let command = batch(&owner);
    let budget = SharedBudget::new(4 * BYTES);
    owner.set_staged_pixel_budget(65_536);
    let error = owner
        .preparation()
        .unwrap()
        .apply_with_admission(&command, admission(&budget, 2), || Ok(()))
        .unwrap_err();
    assert_eq!(error.operation_index, Some(1));
    assert!(error.allocation_denial().is_none()); // Fixed staged cap is not a capacity wait.
    assert_eq!(budget.usage().reserved, 0);
    owner.set_staged_pixel_budget(2 * 65_536);
    let before = owner.snapshot();
    let prepared = owner
        .preparation()
        .unwrap()
        .apply_with_admission(&command, admission(&budget, 2), || Ok(()))
        .unwrap();
    let error = owner
        .publish_prepared(prepared, || {
            Err(Error::new(ErrorCode::OperationCancelled, "injected"))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&owner.snapshot(), &before));
    let prepared = owner
        .preparation()
        .unwrap()
        .apply_with_admission(&command, admission(&budget, 2), || Ok(()))
        .unwrap();
    owner.apply(&command).unwrap();
    let newer = owner.snapshot();
    let error = owner.publish_prepared(prepared, || Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ConflictStaleBase);
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&owner.snapshot(), &newer));
}
#[test]
fn cancellation_after_staging_releases_reserved_and_materialized_bytes_without_publication() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let owner = session();
    let original = owner.snapshot();
    let budget = SharedBudget::new(3 * BYTES);
    let checks = AtomicUsize::new(0);
    let error = owner
        .preparation()
        .unwrap()
        .apply_with_admission(&batch(&owner), admission(&budget, 3), || {
            if checks.fetch_add(1, Ordering::AcqRel) == 1 {
                Err(Error::new(ErrorCode::OperationCancelled, "after staging"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&owner.snapshot(), &original));
    assert_eq!(owner.undo_depth(), 0);
}
