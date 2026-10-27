use paintcraft_commands::{Batch, LayerRef, Mode, Op, ShapeKind};
use paintcraft_core::types::{Rect, Rgba, resources::SharedBudget, text::MaskSpan};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::execute::{Selection, execute_with_admission};
use paintcraft_raster::{Admission, RasterLayer};
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
#[test]
fn painting_families_admit_their_operation_before_allocating_a_tile_prefix() {
    let doc = new_document(300, 16, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let layer = LayerRef::Id(id);
    let color = Rgba([1, 2, 3, 255]);
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
            stroke: Some((Rgba([17, 88, 156, 255]), 1)),
            mode: Mode::SourceOver,
        },
        Op::MaskSpans {
            layer: layer.clone(),
            color,
            mode: paintcraft_commands::MaskMode::SourceOver,
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
            media: Default::default(),
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
        let batch = Batch {
            version: 2,
            policy: POLICY.into(),
            base_state: doc.state_id,
            base_epoch: 0,
            seed: None,
            digest: "0".repeat(64),
            operations: vec![op],
        };
        let budget = SharedBudget::new(2 * BYTES);
        let blocker = budget.reserve(BYTES).unwrap();
        let error = execute_with_admission(
            &doc,
            Selection::Inactive,
            &batch,
            Admission::with_residency(2 * 65_536, budget.clone()),
        )
        .unwrap_err();
        assert_eq!(error.operation_index, Some(0));
        assert_eq!(
            error.allocation_denial().unwrap().required_bytes(),
            2 * BYTES,
            "{:?}",
            batch.operations[0]
        );
        assert_eq!(budget.usage().peak, BYTES); // The first raster tile was never admitted.
        assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
        drop(blocker);
        let staged = execute_with_admission(
            &doc,
            Selection::Inactive,
            &batch,
            Admission::with_residency(2 * 65_536, budget.clone()),
        )
        .unwrap();
        assert_eq!(staged.doc.layers[&id].raster.resident_tiles(), 2);
        assert_eq!(budget.usage().reserved, 2 * BYTES);
        drop(staged);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn cancelled_smooth_preparation_preserves_owner_and_releases_decoded_capacity() {
    use paintcraft_core::types::{Error, ErrorCode};
    use paintcraft_editor::session::Session;
    use std::{cell::Cell, sync::Arc};
    let doc = new_document(300, 300, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let session = Session::new(doc);
    let snapshot = session.snapshot();
    let history = session.history_view();
    let ticket = session.ticket();
    let batch = Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: snapshot.state_id,
        base_epoch: 0,
        seed: None,
        digest: "0".repeat(64),
        operations: vec![Op::StrokePath {
            media: Default::default(),
            layer: LayerRef::Id(snapshot.roots[0]),
            points: vec![(8, 8), (290, 290)],
            color: Rgba([17, 88, 156, 128]),
            mode: Mode::SourceOver,
            size: 32,
            round: true,
            smooth: true,
            point_scale: 1,
        }],
    };
    let budget = SharedBudget::new(4 * BYTES);
    let calls = Cell::new(0);
    let success = session
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch,
            Admission::with_residency(4 * 65_536, budget.clone()),
            || {
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap();
    drop(success);
    assert_eq!(budget.usage().reserved, 0);
    // Check throughout geometry, descriptors, tile allocation/replay and history.
    for stop in [5, 10, 20, 40, calls.get()] {
        assert!(stop <= calls.get());
        let seen = Cell::new(0);
        let result = session.preparation().unwrap().apply_with_admission(
            &batch,
            Admission::with_residency(4 * 65_536, budget.clone()),
            || {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel smooth edit",
                    ))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(seen.get(), stop);
        assert!(Arc::ptr_eq(&snapshot, &session.snapshot()));
        assert_eq!(session.ticket(), ticket);
        assert_eq!(session.history_view(), history);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn incremental_preview_geometry_never_reuses_blended_pixels_or_skips_current_selection() {
    use paintcraft_editor::execute::{execute, execute_with_geometry_cache};
    use paintcraft_raster::{Selection, smooth::PathCache};
    let mut doc = new_document(32, 32, Some(Rgba([200, 100, 50, 127])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().origin = (-2, 1);
    let points = [(0, 3), (15, 19), (0, 3), (7, 20)];
    let mut cache = PathCache::default();
    for mode in [Mode::SourceOver, Mode::Replace] {
        for selection in [
            Selection::Inactive,
            Selection::Rect(Rect::new(2, 3, 9, 13).unwrap()),
            Selection::Empty,
        ] {
            for n in 1..=points.len() {
                let batch = Batch {
                    version: 2,
                    policy: POLICY.into(),
                    base_state: doc.state_id,
                    base_epoch: 0,
                    seed: None,
                    digest: "0".repeat(64),
                    operations: vec![Op::StrokePath {
                        media: Default::default(),
                        layer: LayerRef::Id(id),
                        points: points[..n].to_vec(),
                        color: Rgba([17, 88, 156, 128]),
                        mode,
                        size: 5,
                        round: true,
                        smooth: true,
                        point_scale: 1,
                    }],
                };
                let actual = execute_with_geometry_cache(
                    &doc,
                    selection.clone(),
                    &batch,
                    Admission::new(65_536),
                    Some(&mut cache),
                    &|| Ok(()),
                )
                .unwrap();
                let expected = execute(&doc, selection.clone(), &batch).unwrap();
                for y in 0..32 {
                    for x in 0..32 {
                        assert_eq!(
                            actual.doc.layers[&id].raster.get(x, y),
                            expected.doc.layers[&id].raster.get(x, y)
                        );
                    }
                }
                assert_eq!(doc.layers[&id].raster.get(4, 4), Rgba([200, 100, 50, 127]));
            }
        }
    }
}
