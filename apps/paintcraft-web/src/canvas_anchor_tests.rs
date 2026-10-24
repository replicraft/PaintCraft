use crate::editor::Editor;
use paintcraft_core::types::{ErrorCode, Rect, canvas::Anchor};
use paintcraft_editor::execute::Selection;
use paintcraft_tools::{Action, Tool};
#[test]
fn browser_anchored_resize_preserves_layer_bounds_and_undo_restores_selection() {
    for (index, anchor) in Anchor::ALL.into_iter().enumerate() {
        let mut editor = Editor::new(8, 7).unwrap();
        editor
            .apply(Action::Select {
                corners: Some(((1, 1), (5, 4))),
            })
            .unwrap();
        let source = editor.session.snapshot();
        let selection = editor.session.selection();
        editor
            .apply(Action::ResizeCanvasAnchored {
                width: 5,
                height: 4,
                anchor,
            })
            .unwrap();
        assert_eq!(editor.session.document().canvas, (5, 4));
        let shifts = [0, -2, -3];
        let delta = (shifts[index % 3], shifts[index / 3]);
        for id in &source.roots {
            let layer = &editor.session.document().layers[id];
            assert_eq!(layer.origin, delta);
            assert_eq!(layer.raster.bounds(), source.layers[id].raster.bounds());
        }
        let expected = Rect::new(1 + delta.0, 1 + delta.1, 5, 4)
            .unwrap()
            .intersect(&editor.session.document().canvas_rect())
            .map_or(Selection::Empty, Selection::Rect);
        assert_eq!(editor.session.selection(), expected);
        editor.apply(Action::Undo).unwrap();
        assert!(editor.session.document().same_semantics(&source));
        assert_eq!(editor.session.selection(), selection);
    }
}
#[test]
fn browser_noop_and_rejected_resize_preserve_the_unpublished_text_draft() {
    let mut editor = Editor::new(32, 24).unwrap();
    editor.tools.tool = Tool::Text;
    editor.tools.begin_text(editor.target.unwrap(), (3, 3));
    editor.tools.type_text("Keep draft");
    assert_eq!(editor.tools.text_content(), Some("Keep draft"));
    let source = editor.session.snapshot();
    let ticket = editor.session.ticket();
    for (size, code) in [
        ((32, 24), None),
        ((0, 20), Some(ErrorCode::InputSchema)),
        ((4096, 4096), Some(ErrorCode::ResourceLimit)),
    ] {
        let result = editor.apply(Action::ResizeCanvasAnchored {
            width: size.0,
            height: size.1,
            anchor: Anchor::Center,
        });
        assert_eq!(result.err().map(|error| error.code), code);
        assert_eq!(editor.tools.text_content(), Some("Keep draft"));
        assert_eq!(editor.session.ticket(), ticket);
        assert!(editor.session.document().same_semantics(&source));
    }
}

#[test]
fn browser_selection_delete_clears_only_the_selected_pixels_and_undo_restores_them() {
    use paintcraft_core::types::Rgba;
    let mut editor = Editor::new(32, 24).unwrap();
    let layer = editor.target.unwrap();
    editor
        .apply(Action::Shape {
            layer,
            tool: Tool::Rect,
            from: (0, 0),
            to: (31, 23),
            stroke: None,
            fill: Some(Rgba([12, 34, 56, 255])),
        })
        .unwrap();
    editor
        .apply(Action::Select {
            corners: Some(((5, 5), (10, 10))),
        })
        .unwrap();
    let source = editor.session.snapshot();
    let selection = editor.session.selection();
    editor
        .apply(Action::EraseSelection {
            color: Rgba([255; 4]),
        })
        .unwrap();
    assert_eq!(
        editor.session.document().layers[&layer].raster.get(7, 7),
        Rgba([255; 4])
    );
    assert_eq!(
        editor.session.document().layers[&layer].raster.get(4, 7),
        Rgba([12, 34, 56, 255])
    );
    editor.apply(Action::Undo).unwrap();
    assert!(editor.session.document().same_semantics(&source));
    assert_eq!(editor.session.selection(), selection);
}
