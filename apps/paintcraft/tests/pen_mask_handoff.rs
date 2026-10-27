//! Headless shared pen preparation proof; no native driver or GUI qualification.
use paintcraft_commands::Batch;
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    json::{self, Value},
    policy::{MaskIntent, MaskMode},
    resources::SharedBudget,
};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_input::{Modifiers, tablet::*};
use paintcraft_tools::pen_stroke::{PenStroke, Style};
fn sample(sequence: u64, phase: Phase, at: (f64, f64), pressure: Option<f32>) -> Sample {
    Sample {
        identity: Identity {
            device: 7,
            pointer: 9,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 3,
            micros: sequence * 100,
            clock: Clock::HostReceipt,
        },
        phase,
        position: at,
        transform: Transform {
            owner: 4,
            epoch: 5,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        },
        buttons: Buttons(Buttons::TIP),
        modifiers: Modifiers::default(),
        pressure,
        tilt: Some(Tilt::Degrees(5.0, 10.0)),
        twist_degrees: Some(12.0),
        contact: None,
        provenance: Provenance::Actual,
    }
}
fn batch(session: &Session, operations: Vec<Value>) -> Batch {
    Batch::from_json(&json::canonical(&Value::object([
        ("schema", Value::str("paintcraft.batch/v2")),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(POLICY)),
        (
            "base",
            Value::object([
                (
                    "state_id",
                    Value::str(session.document().state_id.to_string()),
                ),
                (
                    "publication_epoch",
                    Value::str(session.ticket().publication_epoch.to_string()),
                ),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ])))
    .unwrap()
}
#[test]
fn captured_pressure_masks_preview_publish_and_history_have_identical_pixels_and_one_edit() {
    for (mode, intent, color) in [
        (
            MaskMode::SourceOver,
            MaskIntent::Brush,
            Rgba([31, 79, 191, 128]),
        ),
        (
            MaskMode::SourceOver,
            MaskIntent::Pencil,
            Rgba([31, 79, 191, 128]),
        ),
        (MaskMode::Replace, MaskIntent::Erase, Rgba([255; 4])),
        (
            MaskMode::Replace,
            MaskIntent::Brush,
            Rgba([31, 79, 191, 128]),
        ),
        (
            MaskMode::DestinationOut,
            MaskIntent::Erase,
            Rgba([0, 0, 0, 255]),
        ),
    ] {
        let ground = Rgba([231, 219, 173, 255]);
        let before = new_document(40, 24, Some(ground), 0, POLICY).unwrap();
        let id = before.roots[0];
        let mut session = Session::new(before.clone());
        let budget = SharedBudget::new(16 << 20);
        let mut stroke = PenStroke::begin(
            Style {
                layer: id,
                diameter: 12,
                color,
                mode,
                intent,
            },
            sample(1, Phase::Down, (8.0, 8.0), Some(0.25)),
            &budget,
        )
        .unwrap();
        stroke
            .append(sample(2, Phase::Move, (22.0, 12.0), Some(0.9)))
            .unwrap();
        stroke
            .append(sample(3, Phase::Up, (29.0, 13.0), Some(0.0)))
            .unwrap();
        let captured = stroke.clone();
        let resolved = stroke
            .resolve(Rect::new(0, 0, 40, 24).unwrap(), &budget, &|| Ok(()))
            .unwrap();
        let release = captured
            .resolve(Rect::new(0, 0, 40, 24).unwrap(), &budget, &|| Ok(()))
            .unwrap();
        assert_eq!(
            resolved.operations(&|| Ok(())).unwrap(),
            release.operations(&|| Ok(())).unwrap()
        );
        let b = batch(&session, resolved.operations(&|| Ok(())).unwrap());
        let prepared = session
            .preparation()
            .unwrap()
            .apply_pen_mask_stroke(&b, resolved.style().intent, || Ok(()))
            .unwrap();
        let preview = prepared.document_snapshot();
        assert!(session.snapshot().same_semantics(&before));
        assert_eq!(session.undo_depth(), 0);
        assert!(budget.usage().reserved > 0);
        for span in resolved.spans() {
            for x in span.x0..=span.x1 {
                let pixel = preview.layers[&id].raster.get(x, span.y);
                let alpha = ((u32::from(color.a()) * u32::from(span.coverage) + 127) / 255) as u8;
                let expected = match mode {
                    MaskMode::SourceOver => {
                        let mut source = color;
                        source.0[3] = alpha;
                        source.source_over(ground)
                    }
                    MaskMode::Replace => {
                        let mut pixel = ground;
                        for ((dst, src), original) in pixel.0.iter_mut().zip(color.0).zip(ground.0)
                        {
                            *dst = ((u32::from(src) * u32::from(span.coverage)
                                + u32::from(original) * (255 - u32::from(span.coverage))
                                + 127)
                                / 255) as u8;
                        }
                        pixel
                    }
                    MaskMode::DestinationOut => {
                        Rgba([ground.0[0], ground.0[1], ground.0[2], 255 - alpha])
                    }
                };
                assert_eq!(pixel, expected, "mode {mode:?}, at ({x},{})", span.y);
            }
        }
        session.publish_prepared(prepared, || Ok(())).unwrap();
        assert!(session.snapshot().same_semantics(&preview));
        assert_eq!(session.undo_depth(), 1);
        let label = match intent {
            MaskIntent::Brush => "Brush",
            MaskIntent::Pencil => "Pencil",
            MaskIntent::Erase => "Erase",
        };
        assert_eq!(
            session.history_view().entries.last().unwrap().label.text(),
            label
        );
        session.undo().unwrap().unwrap();
        assert!(session.snapshot().same_semantics(&before));
        session.redo().unwrap().unwrap();
        assert!(session.snapshot().same_semantics(&preview));
        drop((resolved, release, captured, stroke));
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn cancelled_publication_drops_the_prepared_mask_and_preserves_session_history() {
    let before = new_document(24, 24, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let mut session = Session::new(before.clone());
    let history = session.history_view();
    let budget = SharedBudget::new(16 << 20);
    let stroke = PenStroke::begin(
        Style {
            layer: before.roots[0],
            diameter: 12,
            color: Rgba([0, 0, 0, 255]),
            mode: MaskMode::DestinationOut,
            intent: MaskIntent::Erase,
        },
        sample(1, Phase::Down, (8.0, 8.0), None),
        &budget,
    )
    .unwrap();
    let resolved = stroke
        .resolve(Rect::new(0, 0, 24, 24).unwrap(), &budget, &|| Ok(()))
        .unwrap();
    let b = batch(&session, resolved.operations(&|| Ok(())).unwrap());
    let prepared = session
        .preparation()
        .unwrap()
        .apply_pen_mask_stroke(&b, resolved.style().intent, || Ok(()))
        .unwrap();
    let error = session
        .publish_prepared(prepared, || {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel pen publication",
            ))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(session.snapshot().same_semantics(&before));
    assert_eq!(session.history_view(), history);
    let prepared = session
        .preparation()
        .unwrap()
        .apply_pen_mask_stroke(&b, resolved.style().intent, || Ok(()))
        .unwrap();
    let mut foreign = Session::new(before.clone());
    let foreign_history = foreign.history_view();
    assert_eq!(
        foreign
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert!(foreign.snapshot().same_semantics(&before));
    assert_eq!(foreign.history_view(), foreign_history);
    let prepared = session
        .preparation()
        .unwrap()
        .apply_pen_mask_stroke(&b, resolved.style().intent, || Ok(()))
        .unwrap();
    session.apply(&b).unwrap();
    let newer = session.snapshot();
    let newer_history = session.history_view();
    assert_eq!(
        session
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert!(session.snapshot().same_semantics(&newer));
    assert_eq!(session.history_view(), newer_history);
    drop((stroke, resolved));
    assert_eq!(budget.usage().reserved, 0);
}
