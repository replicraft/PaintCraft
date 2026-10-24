use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::{
    execute::{Selection, execute},
    session::Session,
};
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::Cell;
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn batch(doc: &Document, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "0".repeat(64),
        operations,
    }
}
fn compound(doc: &Document) -> Batch {
    let id = doc.roots[0];
    batch(
        doc,
        vec![
            Op::FillRect {
                layer: LayerRef::Id(id),
                rect: Rect::new(0, 0, 1, 1).unwrap(),
                color: Rgba([1, 2, 3, 255]),
                mode: Mode::Replace,
            },
            Op::FloodFill {
                layer: LayerRef::Id(id),
                at: (0, 0),
                color: Rgba([17, 88, 156, 128]),
                mode: Mode::Replace,
                tolerance: 255,
            },
        ],
    )
}
#[test]
fn cancelling_classification_after_logical_prefix_preserves_owner_without_decoded_allocation() {
    let doc = new_document(8192, 1, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let batch = compound(&doc);
    // Calibrate the complete one-command lifecycle; a further 1024 checks in
    // this wide data-dependent batch occur inside the second command's preparation.
    let prefix_calls = Cell::new(0);
    let mut prefix = batch.clone();
    prefix.operations.truncate(1);
    paintcraft_editor::execute::execute_with_admission_checked(
        &doc,
        Selection::Inactive,
        &prefix,
        Admission::new(32 * 65_536),
        &|| {
            prefix_calls.set(prefix_calls.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    let stop = prefix_calls.get() + 1024;
    let session = Session::new(doc);
    let snapshot = session.snapshot();
    let ticket = session.ticket();
    let history = session.history_view();
    let budget = SharedBudget::new(32 * BYTES);
    let seen = Cell::new(0);
    let error = session
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch,
            Admission::with_residency(32 * 65_536, budget.clone()),
            || {
                {
                    seen.set(seen.get() + 1);
                    if seen.get() == stop {
                        return Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel flood classification",
                        ));
                    }
                }
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(budget.usage().peak, 0);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.history_view(), history);
    assert!(std::sync::Arc::ptr_eq(&snapshot, &session.snapshot()));
}
#[test]
fn scratch_policy_survives_admission_clone_and_is_not_a_decoded_retry() {
    let doc = new_document(8192, 1, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let batch = compound(&doc);
    let session = Session::new(doc);
    let ticket = session.ticket();
    let history = session.history_view();
    let budget = SharedBudget::new(32 * BYTES);
    let admission =
        Admission::with_residency(32 * 65_536, budget.clone()).with_flood_scratch_limit(0);
    let error = session
        .preparation()
        .unwrap()
        .apply_with_admission(&batch, admission.clone(), || Ok(()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(error.message, "flood-fill scratch budget exceeded");
    assert!(error.allocation_denial().is_none());
    assert_eq!(budget.usage().peak, 0);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.history_view(), history);
}
#[test]
fn flood_observes_prior_staged_walls_without_crossing_them() {
    let doc = new_document(7, 3, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let wall = Rgba([7, 9, 11, 255]);
    let fill = Rgba([17, 88, 156, 128]);
    let batch = batch(
        &doc,
        vec![
            Op::FillRect {
                layer: LayerRef::Id(id),
                rect: Rect::new(3, 0, 1, 3).unwrap(),
                color: wall,
                mode: Mode::Replace,
            },
            Op::FloodFill {
                layer: LayerRef::Id(id),
                at: (0, 0),
                color: fill,
                mode: Mode::Replace,
                tolerance: 0,
            },
        ],
    );
    let staged = execute(&doc, Selection::Inactive, &batch).unwrap();
    for y in 0..3 {
        for x in 0..7 {
            assert_eq!(
                staged.doc.layers[&id].raster.get(x, y),
                if x < 3 {
                    fill
                } else if x == 3 {
                    wall
                } else {
                    Rgba([31, 63, 127, 0])
                }
            );
            assert_eq!(doc.layers[&id].raster.get(x, y), Rgba([31, 63, 127, 0]));
        }
    }
}
#[test]
fn local_span_replay_preserves_selection_and_negative_layer_origin() {
    let mut doc = new_document(10, 8, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().origin = (-2, 1);
    let fill = Rgba([17, 88, 156, 128]);
    let batch = batch(
        &doc,
        vec![Op::FloodFill {
            layer: LayerRef::Id(id),
            at: (2, 2),
            color: fill,
            mode: Mode::SourceOver,
            tolerance: 0,
        }],
    );
    let selection = Selection::Rect(Rect::new(1, 2, 3, 2).unwrap());
    let staged = execute(&doc, selection.clone(), &batch).unwrap();
    assert_eq!(staged.selection, selection);
    for y in 0..8 {
        for x in 0..10 {
            assert_eq!(
                staged.doc.layers[&id].raster.get(x, y),
                if (3..6).contains(&x) && (1..3).contains(&y) {
                    fill
                } else {
                    Rgba([31, 63, 127, 0])
                }
            );
            assert_eq!(doc.layers[&id].raster.get(x, y), Rgba([31, 63, 127, 0]));
        }
    }
}
