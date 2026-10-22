#[path = "../benches/support/sequential.rs"]
#[allow(dead_code)]
mod sequential;
use paintcraft_commands::{Anchor, Batch, LayerRef, Mode, Op, ShapeKind};
use paintcraft_core::types::{Rect, Rgba, resources::SharedBudget, text::MaskSpan};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::execute::{Selection, execute_with_admission};
use paintcraft_raster::{Admission, RasterLayer};
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
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
fn fill(layer: LayerRef, x: i64, color: Rgba) -> Op {
    Op::FillRect {
        layer,
        rect: rect(x, 0, 1, 1),
        color,
        mode: Mode::Replace,
    }
}
fn compare(doc: &Document, ops: Vec<Op>) {
    let batch = batch(doc, ops);
    let source = doc.clone();
    let planned_budget = SharedBudget::new(64 * BYTES);
    let reference_budget = SharedBudget::new(64 * BYTES);
    let planned = execute_with_admission(
        doc,
        Selection::Inactive,
        &batch,
        Admission::with_residency(64 * 65_536, planned_budget.clone()),
    )
    .unwrap();
    let reference = sequential::execute_with_admission(
        doc,
        Selection::Inactive,
        &batch,
        Admission::with_residency(64 * 65_536, reference_budget.clone()),
    )
    .unwrap();
    assert!(planned.doc.same_semantics(&reference.doc));
    assert_eq!(planned.selection, reference.selection);
    assert_eq!(planned.aliases, reference.aliases);
    assert_eq!(planned_budget.usage().peak, reference_budget.usage().peak);
    assert_eq!(
        planned_budget.usage().reserved,
        reference_budget.usage().reserved
    );
    assert!(doc.same_semantics(&source));
}

#[test]
fn structural_lineages_preserve_duplicates_expansion_deletion_clear_and_selection() {
    let doc = new_document(520, 8, Some(Rgba([13, 29, 47, 0])), 0, POLICY).unwrap();
    let original = LayerRef::Id(doc.roots[0]);
    let copy = LayerRef::Alias("copy".into());
    for deletion in [false, true] {
        let mut ops = vec![
            fill(original.clone(), 0, Rgba([255; 4])),
            Op::LayerDuplicate {
                layer: original.clone(),
                alias: "copy".into(),
                name: "Copy".into(),
            },
            fill(original.clone(), 0, Rgba([7, 8, 9, 255])),
            fill(copy.clone(), 256, Rgba([1, 2, 3, 0])),
            Op::LayerExpand {
                layer: copy.clone(),
                width: 600,
                height: 10,
            },
            Op::CanvasResize {
                width: 600,
                height: 10,
                anchor: Anchor::TopLeft,
            },
            Op::SelectionSetRect {
                rect: rect(250, 0, 300, 3),
            },
            Op::FloodFill {
                layer: copy.clone(),
                at: (300, 0),
                color: Rgba([20, 40, 80, 128]),
                mode: Mode::SourceOver,
                tolerance: 0,
            },
            Op::SelectionClear,
        ];
        if deletion {
            ops.push(Op::LayerDelete {
                layer: original.clone(),
            });
        }
        compare(&doc, ops);
    }
    compare(
        &doc,
        vec![
            fill(original.clone(), 0, Rgba([255; 4])),
            Op::CanvasClear {
                alias: "reset".into(),
                color: Rgba([13, 29, 47, 0]),
            },
            fill(LayerRef::Alias("reset".into()), 1, Rgba([9, 8, 7, 255])),
            Op::CanvasClear {
                alias: "again".into(),
                color: Rgba([13, 29, 47, 0]),
            },
        ],
    );
}

#[test]
fn whole_batch_peak_denial_allocates_no_decoded_prefix_even_after_prior_logical_paint() {
    let doc = new_document(257, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = LayerRef::Id(doc.roots[0]);
    let batch = batch(
        &doc,
        vec![
            fill(id.clone(), 0, Rgba([255; 4])),
            fill(id, 256, Rgba([255; 4])),
        ],
    );
    let budget = SharedBudget::new(BYTES);
    let error = execute_with_admission(
        &doc,
        Selection::Inactive,
        &batch,
        Admission::with_residency(2 * 65_536, budget.clone()),
    )
    .unwrap_err();
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(
        error.allocation_denial().unwrap().required_bytes(),
        2 * BYTES
    );
    assert_eq!(budget.usage().peak, 0);
    assert_eq!(budget.usage().reserved, 0);
    let budget = SharedBudget::new(2 * BYTES);
    assert!(
        execute_with_admission(
            &doc,
            Selection::Inactive,
            &batch,
            Admission::with_residency(2 * 65_536, budget.clone())
        )
        .is_ok()
    );
}

#[test]
fn deleted_versions_recycle_peak_capacity_without_resetting_cumulative_staged_work() {
    let doc = new_document(257, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let original = LayerRef::Id(doc.roots[0]);
    let copy = LayerRef::Alias("copy".into());
    let ops = vec![
        fill(original.clone(), 0, Rgba([255; 4])),
        Op::LayerDuplicate {
            layer: original.clone(),
            alias: "copy".into(),
            name: "Copy".into(),
        },
        fill(original.clone(), 0, Rgba([1, 2, 3, 255])),
        Op::LayerDelete { layer: original },
        fill(copy.clone(), 256, Rgba([255; 4])),
    ];
    let batch = batch(&doc, ops);
    let budget = SharedBudget::new(2 * BYTES);
    let result = execute_with_admission(
        &doc,
        Selection::Inactive,
        &batch,
        Admission::with_residency(3 * 65_536, budget.clone()),
    )
    .unwrap();
    assert_eq!(budget.usage().peak, 2 * BYTES);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    drop(result);
    assert_eq!(budget.usage().reserved, 0);
    let error = execute_with_admission(
        &doc,
        Selection::Inactive,
        &batch,
        Admission::with_residency(2 * 65_536, budget.clone()),
    )
    .unwrap_err();
    assert_eq!(error.operation_index, Some(4));
    assert!(error.allocation_denial().is_none());
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn all_painting_families_compile_once_against_prior_logical_results() {
    let doc = new_document(300, 16, Some(Rgba([13, 29, 47, 0])), 0, POLICY).unwrap();
    let id = LayerRef::Id(doc.roots[0]);
    let color = Rgba([100, 80, 60, 128]);
    let mut ops = vec![
        Op::FillRect {
            layer: id.clone(),
            rect: rect(255, 0, 1, 16),
            color: Rgba([255; 4]),
            mode: Mode::Replace,
        },
        Op::FloodFill {
            layer: id.clone(),
            at: (0, 0),
            color,
            mode: Mode::Replace,
            tolerance: 0,
        },
        Op::PencilPath {
            layer: id.clone(),
            points: vec![(0, 0), (299, 15)],
            color,
            mode: Mode::SourceOver,
        },
        Op::MaskSpans {
            layer: id.clone(),
            color,
            spans: vec![MaskSpan {
                y: 7,
                x0: 250,
                x1: 260,
                coverage: 127,
            }],
        },
        Op::Text {
            layer: id.clone(),
            at: (250, 0),
            text: "Ab".into(),
            scale: 1,
            color,
            mode: Mode::Replace,
        },
    ];
    for kind in [ShapeKind::Line, ShapeKind::Rect, ShapeKind::Ellipse] {
        ops.push(Op::Shape {
            layer: id.clone(),
            kind,
            from: (240, 0),
            to: (280, 15),
            stroke: Some((color, 2)),
            fill: Some(color),
            mode: Mode::SourceOver,
        });
    }
    for smooth in [false, true] {
        ops.push(Op::StrokePath {
            layer: id.clone(),
            points: vec![(250, 3), (260, 8), (270, 3)],
            color,
            mode: Mode::SourceOver,
            size: 3,
            round: true,
            smooth,
            point_scale: 1,
        });
    }
    compare(&doc, ops);
}

#[test]
fn deterministic_mixed_structural_sequences_match_the_frozen_executor_and_resource_peaks() {
    for seed in 0..64u64 {
        let doc = new_document(520, 8, Some(Rgba([13, 29, 47, 0])), seed, POLICY).unwrap();
        let mut live = vec![LayerRef::Id(doc.roots[0])];
        let mut ops = Vec::new();
        let mut state = seed + 1;
        for index in 0..32 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let layer = live[(state as usize >> 8) % live.len()].clone();
            let color = Rgba([
                state as u8,
                (state >> 8) as u8,
                (state >> 16) as u8,
                (state >> 24) as u8,
            ]);
            let op = match state % 10 {
                0 if live.len() < 4 => {
                    let alias = format!("duplicate{index}");
                    live.push(LayerRef::Alias(alias.clone()));
                    Op::LayerDuplicate {
                        layer,
                        alias,
                        name: "Copy".into(),
                    }
                }
                1 if live.len() > 1 => {
                    live.retain(|old| old != &layer);
                    Op::LayerDelete { layer }
                }
                2 => Op::LayerExpand {
                    layer,
                    width: 600,
                    height: 16,
                },
                3 => Op::CanvasResize {
                    width: 600,
                    height: 16,
                    anchor: Anchor::Center,
                },
                4 => Op::SelectionSetRect {
                    rect: rect((state % 500) as i64, 0, 20, 8),
                },
                5 => Op::SelectionClear,
                6 => Op::LayerSetProperties {
                    layer,
                    name: Some(format!("Layer {index}")),
                    visible: Some(state & 256 == 0),
                    opacity: Some((state >> 8) as u8),
                },
                7 => Op::FloodFill {
                    layer,
                    at: ((state % 520) as i64, 0),
                    color,
                    mode: Mode::Replace,
                    tolerance: 0,
                },
                8 => {
                    let alias = format!("clear{index}");
                    live = vec![LayerRef::Alias(alias.clone())];
                    Op::CanvasClear {
                        alias,
                        color: Rgba([13, 29, 47, 0]),
                    }
                }
                _ => fill(layer, (state % 520) as i64, color),
            };
            ops.push(op);
        }
        compare(&doc, ops);
    }
}

#[test]
fn cancelling_every_observed_compilation_reservation_and_replay_checkpoint_keeps_owner_history() {
    use paintcraft_core::types::{Error, ErrorCode};
    use paintcraft_editor::session::Session;
    use std::{cell::Cell, sync::Arc};
    let doc = new_document(8, 4, Some(Rgba([13, 29, 47, 0])), 0, POLICY).unwrap();
    let id = LayerRef::Id(doc.roots[0]);
    let batch = batch(
        &doc,
        vec![
            fill(id.clone(), 3, Rgba([255; 4])),
            Op::LayerDuplicate {
                layer: id.clone(),
                alias: "copy".into(),
                name: "Copy".into(),
            },
            Op::FloodFill {
                layer: LayerRef::Alias("copy".into()),
                at: (0, 0),
                color: Rgba([20, 40, 80, 128]),
                mode: Mode::SourceOver,
                tolerance: 0,
            },
            Op::LayerDelete { layer: id },
        ],
    );
    let session = Session::new(doc);
    let before = session.snapshot();
    let ticket = session.ticket();
    let history = session.history_view();
    let total = Cell::new(0);
    drop(
        session
            .preparation()
            .unwrap()
            .apply_with_admission(&batch, Admission::new(8 * 65_536), || {
                total.set(total.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    for stop in 1..=total.get() {
        let decoded = SharedBudget::new(8 * BYTES);
        let metadata = SharedBudget::new(1 << 20);
        let calls = Cell::new(0);
        let error = session
            .preparation()
            .unwrap()
            .apply_with_admission(
                &batch,
                Admission::with_residency(8 * 65_536, decoded.clone())
                    .with_paint_metadata_budget(metadata.clone()),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == stop {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel complete preparation",
                        ))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
        assert_eq!(decoded.usage().reserved, 0);
        assert_eq!(metadata.usage().reserved, 0);
        assert_eq!(session.ticket(), ticket);
        assert_eq!(session.history_view(), history);
        assert!(Arc::ptr_eq(&before, &session.snapshot()));
    }
}
