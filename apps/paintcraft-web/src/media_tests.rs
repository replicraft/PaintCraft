use crate::editor::Editor;
use paintcraft_core::types::{Rgba, brush::BrushMedia};
use paintcraft_tools::{Action, PointerButton, Tool};
#[test]
fn browser_preview_and_release_share_every_brush_and_undo_redo_restore_exact_pixels() {
    for media in BrushMedia::ALL {
        let mut editor = Editor::new(64, 48).unwrap();
        let before = paintcraft_render::render_flat(editor.session.document()).unwrap();
        editor.tools.tool = Tool::Brush;
        editor.tools.options.media = media;
        editor.tools.options.size = 16;
        editor.tools.color = Rgba([0, 120, 212, 128]);
        editor.tools.pointer_down_subpixel(
            editor.target.unwrap(),
            (10 * 256 + 40, 20 * 256 + 70),
            PointerButton::Primary,
        );
        editor
            .tools
            .pointer_move_subpixel((49 * 256 + 30, 30 * 256 + 60));
        let preview =
            paintcraft_render::render_flat(&editor.display_document().unwrap().unwrap()).unwrap();
        assert_eq!(
            before,
            paintcraft_render::render_flat(editor.session.document()).unwrap()
        );
        let (_, action) = editor.tools.pointer_up().unwrap();
        editor.apply(action).unwrap();
        assert_eq!(
            preview,
            paintcraft_render::render_flat(editor.session.document()).unwrap(),
            "{media:?}"
        );
        editor.apply(Action::Undo).unwrap();
        assert_eq!(
            before,
            paintcraft_render::render_flat(editor.session.document()).unwrap()
        );
        editor.apply(Action::Redo).unwrap();
        assert_eq!(
            preview,
            paintcraft_render::render_flat(editor.session.document()).unwrap()
        );
    }
}
