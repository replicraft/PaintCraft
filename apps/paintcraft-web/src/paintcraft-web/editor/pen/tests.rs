use super::*;
use paintcraft_core::types::brush::BrushMedia;
use paintcraft_input::{Modifiers, PointerButton, tablet::*};
use paintcraft_tools::Tool;

mod media;

fn sample(sequence: u64, phase: Phase, pressure: Option<f32>) -> Sample {
    Sample {
        identity: Identity {
            device: 1,
            pointer: 2,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 1,
            micros: 100 * sequence,
            clock: Clock::BrowserPerformance,
        },
        phase,
        position: if sequence == 1 {
            (8.0, 9.0)
        } else {
            (35.0, 20.0)
        },
        transform: Transform {
            owner: 3,
            epoch: 4,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        },
        buttons: Buttons(if phase == Phase::Up { 0 } else { Buttons::TIP }),
        modifiers: Modifiers::default(),
        pressure,
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
fn setup() -> Editor {
    let mut editor = Editor::new(48, 32).unwrap();
    editor.tools.tool = Tool::Brush;
    editor.tools.color = Rgba([31, 79, 151, 191]);
    editor.tools.options.size = 14;
    editor
}

#[test]
fn changing_pressure_changes_actual_editor_pixels_and_preview_release_undo_are_exact() {
    let mut outputs = Vec::new();
    for changing in [false, true] {
        let budget = SharedBudget::new(64 << 20);
        let mut editor = setup();
        let before = paintcraft_render::render_flat(editor.session.document()).unwrap();
        let depth = editor.session.undo_depth();
        assert!(
            editor
                .pen_sample(
                    sample(1, Phase::Down, Some(if changing { 0.2 } else { 1.0 })),
                    &budget
                )
                .unwrap()
        );
        editor
            .pen_sample(
                sample(2, Phase::Move, Some(if changing { 0.8 } else { 1.0 })),
                &budget,
            )
            .unwrap();
        let preview =
            paintcraft_render::render_flat(&editor.display_document().unwrap().unwrap()).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            before
        );
        // A promoted mouse release cannot finish a classified pen capture.
        assert!(editor.tools.pointer_up().is_none());
        assert_eq!(editor.session.undo_depth(), depth);
        editor
            .pen_sample(
                sample(3, Phase::Up, Some(if changing { 0.8 } else { 1.0 })),
                &budget,
            )
            .unwrap();
        assert_eq!(
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            preview
        );
        assert_eq!(editor.session.undo_depth(), depth + 1);
        assert_eq!(
            editor
                .session
                .history_view()
                .entries
                .last()
                .unwrap()
                .label
                .text(),
            "Brush"
        );
        editor.apply(Action::Undo).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            before
        );
        editor.apply(Action::Redo).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            preview
        );
        outputs.push(preview);
    }
    assert_ne!(outputs[0], outputs[1]);
}

#[test]
fn pressure_not_applicable_preserves_all_nine_media_pixels_preview_and_replay() {
    for media in BrushMedia::ALL {
        let mut actual = setup();
        actual.tools.options.media = media;
        let mut reference = Editor::from_document(actual.session.document().clone()).unwrap();
        reference.tools.tool = actual.tools.tool;
        reference.tools.color = actual.tools.color;
        reference.tools.options = actual.tools.options;
        let budget = SharedBudget::new(64 << 20);
        assert!(
            !actual
                .pen_sample(sample(1, Phase::Down, None), &budget)
                .unwrap()
        );
        for editor in [&mut actual, &mut reference] {
            editor.tools.pointer_down_subpixel(
                editor.target.unwrap(),
                (8 * 256, 9 * 256),
                PointerButton::Primary,
            );
            editor.tools.pointer_move_subpixel((35 * 256, 20 * 256));
        }
        let preview =
            paintcraft_render::render_flat(&actual.display_document().unwrap().unwrap()).unwrap();
        let expected =
            paintcraft_render::render_flat(&reference.display_document().unwrap().unwrap())
                .unwrap();
        assert_eq!(preview, expected, "{media:?}");
        for editor in [&mut actual, &mut reference] {
            let (_, action) = editor.tools.pointer_up().unwrap();
            editor.apply(action).unwrap();
            assert_eq!(
                paintcraft_render::render_flat(editor.session.document()).unwrap(),
                expected,
                "{media:?}"
            );
        }
    }
}

#[test]
fn cancellation_and_external_publication_never_commit_private_prefix() {
    let budget = SharedBudget::new(64 << 20);
    let mut editor = setup();
    let before = editor.session.document().clone();
    editor
        .pen_sample(sample(1, Phase::Down, Some(0.5)), &budget)
        .unwrap();
    editor
        .pen_sample(sample(2, Phase::Cancel, None), &budget)
        .unwrap();
    assert!(editor.session.document().same_semantics(&before));
    assert_eq!(editor.session.undo_depth(), 0);
    editor
        .pen_sample(sample(3, Phase::Down, Some(0.5)), &budget)
        .unwrap();
    editor
        .commit(vec![Value::object([
            ("op", Value::str("layer.set_properties")),
            (
                "layer_ref",
                Value::object([("id", Value::str(editor.target.unwrap().to_string()))]),
            ),
            ("name", Value::str("External edit")),
        ])])
        .unwrap();
    let current = editor.session.document().clone();
    let history = editor.session.history_view();
    assert_eq!(
        editor
            .pen_sample(sample(4, Phase::Up, Some(1.0)), &budget)
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert!(editor.session.document().same_semantics(&current));
    assert_eq!(editor.session.history_view(), history);
}

#[test]
fn inverted_eraser_has_one_erase_transaction_and_preserves_surviving_straight_white() {
    for studio in [false, true] {
        let mut editor = setup();
        editor.tools.options.eraser_color = if studio {
            Rgba::TRANSPARENT
        } else {
            Rgba([231, 219, 207, 255])
        };
        let selected = editor.tools.tool;
        let budget = SharedBudget::new(64 << 20);
        for (sequence, phase) in [(1, Phase::Down), (2, Phase::Move)] {
            let mut s = sample(sequence, phase, Some(0.8));
            s.identity.kind = DeviceKind::Eraser;
            editor.pen_sample(s, &budget).unwrap();
        }
        let preview =
            paintcraft_render::render_flat(&editor.display_document().unwrap().unwrap()).unwrap();
        let mut up = sample(3, Phase::Up, Some(0.8));
        up.identity.kind = DeviceKind::Eraser;
        editor.pen_sample(up, &budget).unwrap();
        assert_eq!(editor.session.undo_depth(), 1);
        assert_eq!(
            editor
                .session
                .history_view()
                .entries
                .last()
                .unwrap()
                .label
                .text(),
            "Erase"
        );
        assert_eq!(editor.tools.tool, selected);
        assert_eq!(
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            preview
        );
        if studio {
            let edge = preview
                .as_chunks::<4>()
                .0
                .iter()
                .find(|p| p[3] > 0 && p[3] < 255)
                .expect("antialiased alpha edge");
            assert_eq!(&edge[..3], &[255, 255, 255]);
        } else {
            assert!(preview.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
            assert!(
                preview
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p == &[231, 219, 207, 255])
            );
        }
    }
}

#[test]
fn reopening_identical_document_has_a_distinct_input_owner() {
    let original = setup();
    let reopened = Editor::from_document(original.session.document().clone()).unwrap();
    assert_eq!(original.session.ticket(), reopened.session.ticket());
    assert_ne!(original.session.owner_id(), reopened.session.owner_id());
}
