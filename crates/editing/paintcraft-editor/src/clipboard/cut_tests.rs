use super::*;
use paintcraft_commands::Op;
use paintcraft_core::types::{json::Value, policy::POLICY};
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::Cell;

fn fixture() -> (Session, Uuid) {
    let mut doc = paintcraft_document::new_document(5, 4, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.origin = (-1, 1);
    let pixels: Vec<_> = (0..12u8)
        .flat_map(|n| [10 + n, 90, 180, if n % 2 == 0 { 0 } else { 127 }])
        .collect();
    layer.raster = RasterLayer::from_rgba(
        4,
        3,
        Rgba([77, 88, 99, 0]),
        &pixels,
        &mut Admission::new(1 << 20),
    )
    .unwrap();
    (Session::new(doc), id)
}
fn batch(session: &Session, version: u8, ops: &str) -> Batch {
    Batch::from_json(format!(r#"{{"schema":"paintcraft.batch/v{version}","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{ops}]}}"#,session.ticket().state_id, session.ticket().publication_epoch).as_bytes()).unwrap()
}
fn select(session: &mut Session, kind: u8) {
    let ops = match kind {
        0 => return,
        1 => r#"{"op":"selection.set_rect","rect":{"x":1,"y":1,"width":2,"height":2}}"#,
        _ => {
            r#"{"op":"selection.set_polygon","points":[{"x":0,"y":1},{"x":3,"y":1},{"x":3,"y":2},{"x":1,"y":2},{"x":1,"y":4},{"x":0,"y":4}]}"#
        }
    };
    session.apply(&batch(session, 2, ops)).unwrap();
    if kind == 3 {
        session
            .apply(&batch(session, 2, r#"{"op":"selection.invert"}"#))
            .unwrap();
    }
}
fn selected(kind: u8, x: i64, y: i64) -> bool {
    match kind {
        0 => true,
        1 => (1..3).contains(&x) && (1..3).contains(&y),
        _ => ((y == 1 && (0..3).contains(&x)) || ((2..4).contains(&y) && x == 0)) != (kind == 3),
    }
}

#[test]
fn cut_exact_fields_preserve_offcanvas_hidden_rgb_metadata_and_one_step_history() {
    for kind in 0..4 {
        for clear in [Rgba::TRANSPARENT, Rgba([250, 249, 248, 255])] {
            let (mut session, id) = fixture();
            select(&mut session, kind);
            let snapshot = session.snapshot();
            let selection = session.selection();
            let depth = session.undo_depth();
            let source = CutSource::new(&session, id, clear).unwrap();
            let capture = capture(
                &snapshot,
                selection.clone(),
                id,
                &SharedBudget::new(4096),
                &|| Ok(()),
            )
            .unwrap()
            .unwrap();
            let removal = source.removal_batch(&session, Some(id)).unwrap();
            assert!(matches!(removal.operations[0], Op::CutSelection { .. }));
            let receipt = session.apply(&removal).unwrap();
            assert!(receipt.committed);
            assert_eq!(session.undo_depth(), depth + 1);
            assert_eq!(
                session.history_view().entries.last().unwrap().label.text(),
                "Cut"
            );
            assert_eq!(session.selection(), selection);
            let layer = &session.document().layers[&id];
            assert_eq!(layer.origin, (-1, 1));
            assert_eq!(
                layer.raster.default_pixel(),
                snapshot.layers[&id].raster.default_pixel()
            );
            for y in 0..3 {
                for x in 0..4 {
                    let (dx, dy) = (x - 1, y + 1);
                    let expected = if dx >= 0 && selected(kind, dx, dy) {
                        clear
                    } else {
                        snapshot.layers[&id].raster.get(x, y)
                    };
                    assert_eq!(
                        layer.raster.get(x, y),
                        expected,
                        "kind={kind} local={x},{y}"
                    );
                }
            }
            // Source clipboard bytes retain hidden RGB/partial alpha after removal.
            let original = capture.image.rgba().to_vec();
            session.undo().unwrap().unwrap();
            assert!(session.document().same_semantics(&snapshot));
            assert_eq!(session.selection(), selection);
            session.redo().unwrap().unwrap();
            assert_eq!(capture.image.rgba(), original);
        }
    }
}

#[test]
fn source_guard_rejects_foreign_owner_changed_target_selection_and_undo_redo_epochs() {
    let (mut session, id) = fixture();
    let source = CutSource::new(&session, id, Rgba::TRANSPARENT).unwrap();
    assert_eq!(
        source.removal_batch(&session, None).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let foreign = Session::new((*session.snapshot()).clone());
    assert_eq!(
        source.removal_batch(&foreign, Some(id)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    select(&mut session, 1);
    assert_eq!(
        source.removal_batch(&session, Some(id)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    session.undo().unwrap().unwrap();
    assert_eq!(
        source.removal_batch(&session, Some(id)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(CutSource::new(&session, Uuid::new_v4(), Rgba::TRANSPARENT).is_err());
}

#[test]
fn compiled_cut_in_mixed_batch_and_late_failure_are_atomic() {
    let (mut session, id) = fixture();
    let before = session.snapshot();
    let guard = CutSource::new(&session, id, Rgba::TRANSPARENT).unwrap();
    let mut value = guard.removal_batch(&session, Some(id)).unwrap();
    // The same compiler must preserve prior writes rather than replaying an old source.
    value.operations.insert(
        0,
        Op::FillRect {
            layer: paintcraft_commands::LayerRef::Id(id),
            rect: session.document().canvas_rect(),
            color: Rgba([200, 100, 50, 255]),
            mode: paintcraft_commands::Mode::Replace,
        },
    );
    value.operations.push(Op::LayerDelete {
        layer: paintcraft_commands::LayerRef::Id(Uuid::new_v4()),
    });
    assert!(session.apply(&value).is_err());
    assert!(session.document().same_semantics(&before));
    assert_eq!(session.undo_depth(), 0);
    value.operations.pop();
    session.apply(&value).unwrap();
    assert_eq!(
        session.document().layers[&id].raster.get(1, 0),
        Rgba::TRANSPARENT
    );
    session.undo().unwrap().unwrap();
    assert!(session.document().same_semantics(&before));
}

#[test]
fn cut_cancellation_at_every_checkpoint_never_publishes_a_private_prefix() {
    let (mut session, id) = fixture();
    select(&mut session, 2);
    let source = CutSource::new(&session, id, Rgba::TRANSPARENT).unwrap();
    let removal = source.removal_batch(&session, Some(id)).unwrap();
    let before = session.snapshot();
    let ticket = session.ticket();
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    drop(
        session
            .preparation()
            .unwrap()
            .apply(&removal, count)
            .unwrap(),
    );
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let check = || {
            current.set(current.get() + 1);
            if current.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            session
                .preparation()
                .unwrap()
                .apply(&removal, check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.ticket(), ticket);
    }
    session.set_staged_pixel_budget(0);
    let depth = session.undo_depth();
    assert!(session.apply(&removal).is_err());
    assert_eq!(session.undo_depth(), depth);
    assert!(session.document().same_semantics(&before));
}

#[test]
fn cut_grammar_is_v2_only_and_rejects_unknown_fields() {
    let (session, id) = fixture();
    let source = CutSource::new(&session, id, Rgba::TRANSPARENT).unwrap();
    let valid = source.removal_batch(&session, Some(id)).unwrap();
    let mut operation = Value::object([
        ("op", Value::str("raster.cut_selection")),
        (
            "layer_ref",
            Value::object([("id", Value::str(id.to_string()))]),
        ),
        ("clear", Value::str("#00000000")),
    ]);
    let mut wire = Value::object([
        ("schema", Value::str("paintcraft.batch/v1")),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(POLICY)),
        (
            "base",
            Value::object([
                (
                    "state_id",
                    Value::str(session.ticket().state_id.to_string()),
                ),
                (
                    "publication_epoch",
                    Value::str(session.ticket().publication_epoch.to_string()),
                ),
            ]),
        ),
        ("operations", Value::Array(vec![operation.clone()])),
    ]);
    assert_eq!(
        Batch::from_value(&wire).unwrap_err().code,
        ErrorCode::InputSchema
    );
    let Value::Object(fields) = &mut operation else {
        unreachable!()
    };
    fields.insert("clipboard_acknowledged".into(), Value::Bool(true));
    let Value::Object(fields) = &mut wire else {
        unreachable!()
    };
    fields.insert("schema".into(), Value::str("paintcraft.batch/v2"));
    fields.insert("operations".into(), Value::Array(vec![operation]));
    assert_eq!(
        Batch::from_value(&wire).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert_eq!(valid.operations.len(), 1);
}
