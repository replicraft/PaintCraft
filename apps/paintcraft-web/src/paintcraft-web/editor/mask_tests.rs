//! Browser previews and history must preserve the selected field, not its bounds.
use super::*;
use paintcraft_tools::Tool;

const BASE: Rgba = Rgba([21, 45, 69, 255]);
const PAINT: Rgba = Rgba([211, 37, 83, 255]);

fn polygon() -> Value {
    Value::object([
        ("op", Value::str("selection.set_polygon")),
        (
            "points",
            Value::Array(
                [(1, 1), (7, 1), (7, 3), (3, 3), (3, 7), (1, 7)]
                    .into_iter()
                    .map(|(x, y)| Value::object([("x", Value::Int(x)), ("y", Value::Int(y))]))
                    .collect(),
            ),
        ),
    ])
}
fn selected(x: i64, y: i64, inverted: bool) -> bool {
    let inside =
        (1..7).contains(&x) && (1..3).contains(&y) || (1..3).contains(&x) && (3..7).contains(&y);
    inside != inverted
}
fn check_pixels(document: &Document, inverted: bool, selected_color: Rgba) {
    let layer = &document.layers[&document.roots[0]];
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(
                layer.raster.get(x, y),
                if selected(x, y, inverted) {
                    selected_color
                } else {
                    BASE
                },
                "pixel {x},{y}, inverted={inverted}"
            );
        }
    }
}

#[test]
fn mask_brush_preview_commit_delete_and_undo_match_explicit_pixels() {
    for inverted in [false, true] {
        let mut editor = Editor::new(8, 8).unwrap();
        editor.apply(Action::ClearCanvas { color: BASE }).unwrap();
        let mut operations = vec![polygon()];
        if inverted {
            operations.push(Value::object([("op", Value::str("selection.invert"))]));
        }
        editor.commit(operations).unwrap();
        let selection = editor.session.selection();
        assert!(matches!(selection, execute::Selection::Mask(_)));
        let before = editor.session.snapshot();
        let ticket = editor.session.ticket();
        let depth = editor.session.undo_depth();
        editor.tools.tool = Tool::Brush;
        editor.tools.options.round = false;
        editor.tools.options.size = 20;
        editor.tools.color = PAINT;
        editor.tools.pointer_down(editor.target.unwrap(), (4, 4));
        let preview = editor.display_document().unwrap().unwrap();
        check_pixels(&preview, inverted, PAINT);
        assert_eq!(editor.session.ticket(), ticket);
        assert_eq!(editor.session.undo_depth(), depth);
        assert!(std::sync::Arc::ptr_eq(&before, &editor.session.snapshot()));
        let (_, action) = editor.tools.pointer_up().unwrap();
        editor.apply(action).unwrap();
        check_pixels(editor.session.document(), inverted, PAINT);
        assert!(editor.session.document().same_semantics(&preview));
        assert_eq!(editor.session.selection(), selection);
        assert_eq!(editor.session.undo_depth(), depth + 1);
        editor.apply(Action::Undo).unwrap();
        assert!(editor.session.document().same_semantics(&before));
        assert_eq!(editor.session.selection(), selection);
        editor.apply(Action::Redo).unwrap();
        check_pixels(editor.session.document(), inverted, PAINT);
        editor
            .apply(Action::EraseSelection { color: WHITE })
            .unwrap();
        check_pixels(editor.session.document(), inverted, WHITE);
        assert_eq!(editor.session.selection(), selection);
        editor.apply(Action::Undo).unwrap();
        check_pixels(editor.session.document(), inverted, PAINT);
        assert_eq!(editor.session.selection(), selection);
    }
}
