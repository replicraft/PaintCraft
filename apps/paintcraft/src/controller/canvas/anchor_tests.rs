use super::*;
use paintcraft_commands::{LayerRef, Op};
use paintcraft_core::types::{Rect, canvas::Anchor};
use paintcraft_editor::{execute::Selection, session::Session};

fn fixture() -> Editor {
    let doc = new_document(8, 7, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let mut session = Session::new(doc);
    let source = session.document();
    let batch = Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: source.state_id,
        base_epoch: 0,
        seed: None,
        digest: "7".repeat(64),
        operations: vec![
            Op::LayerAdd {
                alias: "offset".into(),
                name: "Offset".into(),
                width: 8,
                height: 7,
                origin: (-2, -1),
                default: Rgba([19, 31, 47, 0]),
            },
            Op::FillRect {
                layer: LayerRef::Alias("offset".into()),
                rect: Rect::new(1, 1, 1, 1).unwrap(),
                color: Rgba([77, 99, 111, 128]),
                mode: paintcraft_commands::Mode::Replace,
            },
        ],
    };
    session.apply(&batch).unwrap();
    let mut editor = Editor::with_document(session.document().clone(), None);
    editor.request(
        1,
        Action::Select {
            corners: Some(((1, 1), (5, 4))),
        },
    );
    editor.run_pending();
    editor.session.set_staged_pixel_budget(0);
    editor
}

#[test]
fn all_anchors_preserve_raw_layers_and_translate_selection_with_signed_floor_without_pixel_admission()
 {
    for (size, shifts) in [((5, 4), [0, -2, -3]), ((11, 10), [0, 1, 3])] {
        for (index, anchor) in Anchor::ALL.into_iter().enumerate() {
            let mut editor = fixture();
            let source = editor.document_snapshot();
            let selection = editor.selection();
            let depth = editor.undo_depth();
            editor.request(
                2,
                Action::ResizeCanvasAnchored {
                    width: size.0,
                    height: size.1,
                    anchor,
                },
            );
            assert_eq!(editor.run_pending(), 1);
            assert_eq!(editor.undo_depth(), depth + 1);
            let result = editor.document_snapshot();
            assert_eq!(result.canvas, size);
            assert_eq!(result.roots, source.roots);
            let delta = (shifts[index % 3], shifts[index / 3]);
            for id in &source.roots {
                let (a, b) = (&source.layers[id], &result.layers[id]);
                assert_eq!(b.origin, (a.origin.0 + delta.0, a.origin.1 + delta.1));
                assert_eq!(
                    (b.id, &b.name, b.visible, b.opacity),
                    (a.id, &a.name, a.visible, a.opacity)
                );
                assert_eq!(b.raster.bounds(), a.raster.bounds());
                assert_eq!(
                    b.raster.tile_identities().collect::<Vec<_>>(),
                    a.raster.tile_identities().collect::<Vec<_>>()
                );
                for y in 0..7 {
                    for x in 0..8 {
                        assert_eq!(b.raster.get(x, y), a.raster.get(x, y));
                    }
                }
            }
            let rect = Rect::new(1 + delta.0, 1 + delta.1, 5, 4).unwrap();
            let expected = rect
                .intersect(&result.canvas_rect())
                .map_or(Selection::Empty, Selection::Rect);
            assert_eq!(editor.selection(), expected, "{anchor:?}");
            editor.request(3, Action::Undo);
            editor.run_pending();
            assert!(editor.document().same_semantics(&source));
            assert_eq!(editor.selection(), selection);
            editor.request(4, Action::Redo);
            editor.run_pending();
            assert!(editor.document().same_semantics(&result));
            assert_eq!(editor.selection(), expected);
        }
    }
}
#[test]
fn unchanged_anchored_resize_and_invalid_dimensions_leave_history_and_raw_pixels_intact() {
    let mut editor = fixture();
    let source = editor.document_snapshot();
    let selection = editor.selection();
    let depth = editor.undo_depth();
    for (i, size) in [(8, 7), (0, 1), (1_048_577, 1)].into_iter().enumerate() {
        editor.request(
            i as u64 + 10,
            Action::ResizeCanvasAnchored {
                width: size.0,
                height: size.1,
                anchor: Anchor::Center,
            },
        );
        assert_eq!(editor.run_pending(), 0);
        assert_eq!(editor.undo_depth(), depth);
        assert_eq!(editor.selection(), selection);
        assert!(editor.document().same_semantics(&source));
    }
}
