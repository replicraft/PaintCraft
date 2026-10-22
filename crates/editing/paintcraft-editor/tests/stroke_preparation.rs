use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::{
    execute::{Selection, execute},
    session::Session,
};
use paintcraft_raster::{Admission, RasterLayer};
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
fn stroke(id: uuid::Uuid, round: bool) -> Op {
    Op::StrokePath {
        layer: LayerRef::Id(id),
        points: vec![(3, 8), (12, 8), (3, 8)],
        color: Rgba([17, 88, 156, 128]),
        mode: Mode::SourceOver,
        size: 5,
        round,
        smooth: false,
        point_scale: 1,
    }
}
#[test]
fn retraced_stamps_blend_each_pixel_once_under_the_existing_policy() {
    for round in [false, true] {
        let doc = new_document(20, 16, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
        let id = doc.roots[0];
        let staged = execute(
            &doc,
            Selection::Inactive,
            &batch(&doc, vec![stroke(id, round)]),
        )
        .unwrap();
        for y in 0..16 {
            for x in 0..20 {
                let covered = (3..=12).any(|cx| {
                    let (px, py) = (x - (cx - 2), y - 6);
                    (0..5).contains(&px)
                        && (0..5).contains(&py)
                        && (!round
                            || (2 * px - 4) * (2 * px - 4) + (2 * py - 4) * (2 * py - 4) <= 25)
                });
                assert_eq!(
                    staged.doc.layers[&id].raster.get(x, y),
                    if covered {
                        Rgba([17, 88, 156, 128])
                    } else {
                        Rgba([31, 63, 127, 0])
                    }
                );
            }
        }
    }
}
#[test]
fn fixed_stroke_limit_discards_private_prefix_without_a_decoded_retry() {
    let doc = new_document(20, 16, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let batch = batch(
        &doc,
        vec![
            Op::FillRect {
                layer: LayerRef::Id(id),
                rect: Rect::new(0, 0, 1, 1).unwrap(),
                color: Rgba([1, 2, 3, 255]),
                mode: Mode::Replace,
            },
            stroke(id, true),
        ],
    );
    let session = Session::new(doc);
    let snapshot = session.snapshot();
    let ticket = session.ticket();
    let history = session.history_view();
    let bytes = RasterLayer::TILE_BYTES as u64;
    let budget = SharedBudget::new(2 * bytes);
    let error = session
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch,
            Admission::with_residency(2 * 65_536, budget.clone()).with_stroke_scratch_limit(0),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(error.operation_index, Some(1));
    assert!(error.allocation_denial().is_none());
    assert_eq!(budget.usage().peak, bytes);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.history_view(), history);
    assert!(std::sync::Arc::ptr_eq(&snapshot, &session.snapshot()));
}
