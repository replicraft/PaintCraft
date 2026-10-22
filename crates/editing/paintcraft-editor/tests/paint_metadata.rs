use paintcraft_commands::{Batch, LayerRef, Mode, Op, ShapeKind};
use paintcraft_core::types::{ErrorCode, Rect, Rgba, resources::SharedBudget, text::MaskSpan};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::{
    execute::{Selection, execute_with_admission},
    session::Session,
};
use paintcraft_raster::{Admission, RasterLayer};
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn batch(doc: &paintcraft_document::Document, operations: Vec<Op>) -> Batch {
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

#[test]
fn all_painting_families_use_the_admitted_metadata_policy() {
    let doc = new_document(300, 32, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let layer = LayerRef::Id(doc.roots[0]);
    let color = Rgba([17, 88, 156, 128]);
    let mut operations = vec![
        Op::FillRect {
            layer: layer.clone(),
            rect: Rect::new(0, 0, 300, 16).unwrap(),
            color,
            mode: Mode::Replace,
        },
        Op::PencilPath {
            layer: layer.clone(),
            points: vec![(0, 8), (299, 8)],
            color,
            mode: Mode::Replace,
        },
        Op::FloodFill {
            layer: layer.clone(),
            at: (0, 0),
            color,
            mode: Mode::Replace,
            tolerance: 0,
        },
        Op::Shape {
            layer: layer.clone(),
            kind: ShapeKind::Rect,
            from: (0, 0),
            to: (299, 15),
            fill: Some(color),
            stroke: Some((color, 1)),
            mode: Mode::SourceOver,
        },
        Op::MaskSpans {
            layer: layer.clone(),
            color,
            spans: vec![MaskSpan {
                y: 8,
                x0: 0,
                x1: 299,
                coverage: 128,
            }],
        },
        Op::Text {
            layer: layer.clone(),
            at: (250, 0),
            text: "AAAA".into(),
            scale: 1,
            color,
            mode: Mode::Replace,
        },
    ];
    for smooth in [false, true] {
        operations.push(Op::StrokePath {
            layer: layer.clone(),
            points: vec![(1, 8), (298, 8)],
            color,
            mode: Mode::Replace,
            size: 5,
            round: true,
            smooth,
            point_scale: 1,
        });
    }
    for op in operations {
        let budget = SharedBudget::new(2 * BYTES);
        let batch = batch(&doc, vec![op]);
        let error = execute_with_admission(
            &doc,
            Selection::Inactive,
            &batch,
            Admission::with_residency(2 * 65_536, budget.clone()).with_paint_metadata_limit(0),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(error.operation_index, Some(0));
        assert_eq!(error.message, "paint plan metadata budget exceeded");
        assert!(error.allocation_denial().is_none());
        assert_eq!(budget.usage().peak, 0);
        assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
    }
}

#[test]
fn late_metadata_failure_releases_private_prefix_and_preserves_session_history() {
    let doc = new_document(300, 300, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
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
            Op::Shape {
                layer: LayerRef::Id(id),
                kind: ShapeKind::Ellipse,
                from: (0, 0),
                to: (299, 299),
                stroke: None,
                fill: Some(Rgba([17, 88, 156, 128])),
                mode: Mode::SourceOver,
            },
        ],
    );
    let session = Session::new(doc);
    let ticket = session.ticket();
    let snapshot = session.snapshot();
    let history = session.history_view();
    let budget = SharedBudget::new(4 * BYTES);
    let error = session
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch,
            Admission::with_residency(4 * 65_536, budget.clone()).with_paint_metadata_limit(2048),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(error.message, "paint plan metadata budget exceeded");
    assert!(error.allocation_denial().is_none());
    assert_eq!(budget.usage().peak, 0);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(session.ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(&session.snapshot(), &snapshot));
    assert_eq!(session.history_view(), history);
    assert_eq!(
        session.document().layers[&id].raster.get(0, 0),
        Rgba([31, 63, 127, 0])
    );
}
