use super::*;
use paintcraft_input::PointerButton;
use paintcraft_tools::{ShapeStyle, Tool};

#[test]
fn preview_release_pixels_match_without_publishing_and_undo_redo_restore() {
    for tool in [
        Tool::Pencil,
        Tool::Brush,
        Tool::Eraser,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
    ] {
        let mut editor = Editor::new(32, 24).unwrap();
        editor.tools.tool = tool;
        editor.tools.options.size = 5;
        editor.tools.options.style = ShapeStyle::Both;
        editor.tools.options.eraser_color = WHITE;
        editor.tools.color = Rgba([30, 80, 130, 128]);
        if tool == Tool::Eraser {
            editor
                .apply(Action::Fill {
                    layer: editor.target.unwrap(),
                    at: (0, 0),
                    color: Rgba([0, 0, 0, 255]),
                    tolerance: 0,
                })
                .unwrap();
        }
        let before = editor.session.ticket();
        let depth = editor.session.undo_depth();
        let base = paintcraft_render::render_flat(editor.session.document()).unwrap();
        editor.tools.pointer_down_subpixel(
            editor.target.unwrap(),
            (4 * 256 + 64, 5 * 256 + 96),
            PointerButton::Primary,
        );
        editor
            .tools
            .pointer_move_subpixel((19 * 256 + 192, 17 * 256 + 64));
        let preview =
            paintcraft_render::render_flat(&editor.display_document().unwrap().unwrap()).unwrap();
        assert_eq!(editor.session.ticket(), before);
        assert_eq!(editor.session.undo_depth(), depth);
        let (_, action) = editor.tools.pointer_up().unwrap();
        editor.apply(action).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            preview,
            "{tool:?}"
        );
        assert_ne!(preview, base, "{tool:?}");
        editor.apply(Action::Undo).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            base
        );
        editor.apply(Action::Redo).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            preview
        );
    }
}

#[test]
fn eraser_restores_white_and_clear_is_undoable() {
    let mut e = Editor::new(8, 8).unwrap();
    e.apply(Action::Fill {
        layer: e.target.unwrap(),
        at: (0, 0),
        color: Rgba([0, 0, 0, 255]),
        tolerance: 0,
    })
    .unwrap();
    e.tools.tool = Tool::Eraser;
    e.tools.options.eraser_color = WHITE;
    e.tools.pointer_down(e.target.unwrap(), (3, 3));
    let (_, action) = e.tools.pointer_up().unwrap();
    e.apply(action).unwrap();
    assert_eq!(
        e.session.document().layers[&e.target.unwrap()]
            .raster
            .get(3, 3),
        WHITE
    );
    assert_eq!(
        e.session.document().layers[&e.target.unwrap()]
            .raster
            .get(7, 7),
        Rgba([0, 0, 0, 255])
    );
    let before = paintcraft_render::render_flat(e.session.document()).unwrap();
    e.apply(Action::ClearCanvas { color: WHITE }).unwrap();
    assert_eq!(
        paintcraft_render::render_flat(e.session.document()).unwrap(),
        vec![255; 8 * 8 * 4]
    );
    e.apply(Action::Undo).unwrap();
    assert_eq!(
        paintcraft_render::render_flat(e.session.document()).unwrap(),
        before
    );
}

#[test]
fn denied_release_and_cancel_do_not_publish() {
    let mut e = Editor::new(16, 16).unwrap();
    let before = e.session.ticket();
    e.tools.pointer_down(e.target.unwrap(), (3, 3));
    assert!(e.display_document().unwrap().is_some());
    e.cancel();
    assert!(e.display_document().unwrap().is_none());
    assert_eq!(e.session.ticket(), before);
    e.tools.pointer_down(e.target.unwrap(), (3, 3));
    let (_, action) = e.tools.pointer_up().unwrap();
    e.session.set_staged_pixel_budget(0);
    assert!(e.apply(action).is_err());
    assert_eq!(e.session.ticket(), before);
    assert_eq!(e.session.undo_depth(), 0);
    assert!(Editor::new(4096, 4096).is_err());
}

#[test]
fn selection_clips_paint_and_clear_ignores_selection() {
    let mut e = Editor::new(8, 8).unwrap();
    e.apply(Action::Select {
        corners: Some(((1, 1), (2, 2))),
    })
    .unwrap();
    e.apply(Action::Fill {
        layer: e.target.unwrap(),
        at: (1, 1),
        color: Rgba([0, 0, 0, 255]),
        tolerance: 0,
    })
    .unwrap();
    let pixels = paintcraft_render::render_flat(e.session.document()).unwrap();
    assert_eq!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| **pixel == [0, 0, 0, 255])
            .count(),
        4
    );
    e.apply(Action::ClearCanvas { color: WHITE }).unwrap();
    assert_eq!(e.session.selection(), execute::Selection::Inactive);
    assert_eq!(
        paintcraft_render::render_flat(e.session.document()).unwrap(),
        vec![255; 8 * 8 * 4]
    );
}

#[test]
fn resize_keeps_cropped_pixels_extends_white_ground_and_restores_history() {
    let mut e = Editor::new(32, 24).unwrap();
    let layer = e.target.unwrap();
    e.apply(Action::Fill {
        layer,
        at: (0, 0),
        color: Rgba([10, 20, 30, 255]),
        tolerance: 0,
    })
    .unwrap();
    e.apply(Action::ResizeCanvas {
        width: 10,
        height: 10,
    })
    .unwrap();
    assert_eq!(
        e.session.document().layers[&layer].raster.get(20, 17),
        Rgba([10, 20, 30, 255])
    );
    e.apply(Action::ResizeCanvas {
        width: 40,
        height: 30,
    })
    .unwrap();
    assert_eq!(e.session.document().canvas, (40, 30));
    assert_eq!(
        e.session.document().layers[&layer].raster.get(20, 17),
        Rgba([10, 20, 30, 255])
    );
    assert_eq!(
        e.session.document().layers[&layer].raster.get(39, 29),
        WHITE
    );
    e.apply(Action::Undo).unwrap();
    assert_eq!(e.session.document().canvas, (10, 10));
    e.apply(Action::Redo).unwrap();
    assert_eq!(e.session.document().canvas, (40, 30));
}

#[test]
fn invalid_resize_and_unchanged_cropped_layer_leave_session_unchanged() {
    let mut doc = new_document(2, 2, Some(WHITE), 0, POLICY).unwrap();
    doc.canvas = (32, 24);
    let mut e = Editor::from_document(doc).unwrap();
    let before = e.session.snapshot();
    let ticket = e.session.ticket();
    e.apply(Action::ResizeCanvas {
        width: 32,
        height: 24,
    })
    .unwrap();
    assert!(
        e.apply(Action::ResizeCanvas {
            width: 4096,
            height: 4096
        })
        .is_err()
    );
    assert!(
        e.apply(Action::ResizeCanvas {
            width: 0,
            height: 1
        })
        .is_err()
    );
    assert_eq!(e.session.ticket(), ticket);
    assert_eq!(e.session.undo_depth(), 0);
    assert!(e.session.document().same_semantics(&before));
}
