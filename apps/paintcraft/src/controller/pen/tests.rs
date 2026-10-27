use super::*;
use paintcraft_input::{Modifiers, tablet::*};
fn sample(sequence: u64, phase: Phase, at: (f64, f64), pressure: Option<f32>) -> Sample {
    Sample {
        identity: Identity {
            device: 1,
            pointer: 2,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 1,
            micros: sequence * 100,
            clock: Clock::HostReceipt,
        },
        phase,
        position: at,
        transform: Transform {
            owner: 3,
            epoch: 4,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        },
        buttons: Buttons(Buttons::TIP),
        modifiers: Modifiers::default(),
        pressure,
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
fn editor() -> Editor {
    let mut editor = Editor::new_document(48, 32).unwrap();
    editor.select_tool(Tool::Brush);
    editor.options.size = 14;
    editor.color = Rgba([31, 79, 151, 191]);
    editor
}
#[test]
fn real_controller_pressure_changes_pixels_preview_equals_release_and_one_undo_item() {
    let mut outputs = Vec::new();
    for changing in [false, true] {
        let mut editor = editor();
        let budget = SharedBudget::new(64 << 20);
        let before = editor.document_snapshot();
        let depth = editor.undo_depth();
        assert!(
            editor
                .pen_sample(
                    sample(
                        1,
                        Phase::Down,
                        (7.0, 8.0),
                        Some(if changing { 0.2 } else { 1.0 })
                    ),
                    &budget
                )
                .unwrap()
        );
        editor
            .pen_sample(
                sample(
                    2,
                    Phase::Move,
                    (27.0, 17.0),
                    Some(if changing { 0.8 } else { 1.0 }),
                ),
                &budget,
            )
            .unwrap();
        let batch = editor.preview_batch().unwrap().unwrap();
        let preview =
            paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch)
                .unwrap()
                .doc;
        let pixels = paintcraft_render::render_flat(&preview).unwrap();
        editor.pointer_up();
        editor.run_pending();
        assert_eq!(editor.undo_depth(), depth);
        assert!(editor.tools.pen_in_progress());
        editor
            .pen_sample(
                sample(
                    3,
                    Phase::Up,
                    (27.0, 17.0),
                    Some(if changing { 0.8 } else { 1.0 }),
                ),
                &budget,
            )
            .unwrap();
        editor.run_pending();
        assert_eq!(
            paintcraft_render::render_flat(editor.document()).unwrap(),
            pixels
        );
        assert_eq!(editor.undo_depth(), depth + 1);
        assert!(
            editor
                .session
                .history_view()
                .entries
                .last()
                .unwrap()
                .label
                .text()
                == "Brush"
        );
        editor.request(55, Action::Undo);
        editor.run_pending();
        assert!(editor.document().same_semantics(&before));
        outputs.push(pixels);
    }
    assert_ne!(outputs[0], outputs[1]);
}
#[test]
fn source_cancel_and_stale_document_never_publish_a_pressure_prefix() {
    let budget = SharedBudget::new(64 << 20);
    let mut editor = editor();
    let before = editor.document_snapshot();
    let depth = editor.undo_depth();
    editor
        .pen_sample(sample(1, Phase::Down, (8.0, 8.0), Some(0.5)), &budget)
        .unwrap();
    editor
        .pen_sample(sample(2, Phase::Cancel, (8.0, 8.0), None), &budget)
        .unwrap();
    editor.run_pending();
    assert_eq!(editor.undo_depth(), depth);
    assert!(editor.document().same_semantics(&before));
    editor
        .pen_sample(sample(3, Phase::Down, (8.0, 8.0), Some(0.5)), &budget)
        .unwrap();
    let layer = editor.target().unwrap();
    let ticket = editor.session.ticket();
    let batch = strokes::captured_batch(
        ticket,
        vec![Value::object([
            ("op", Value::str("layer.set_properties")),
            ("layer_ref", layer_ref(layer)),
            ("name", Value::str("external edit")),
        ])],
    )
    .unwrap();
    editor.session.apply(&batch).unwrap();
    let newer = editor.document_snapshot();
    let history = editor.session.history_view();
    assert_eq!(
        editor
            .pen_sample(sample(4, Phase::Up, (20.0, 8.0), Some(1.0)), &budget)
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    editor.run_pending();
    assert!(editor.document().same_semantics(&newer));
    assert_eq!(editor.session.history_view(), history);
}
