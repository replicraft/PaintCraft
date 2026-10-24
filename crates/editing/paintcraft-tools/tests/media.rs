use paintcraft_core::types::brush::BrushMedia;
use paintcraft_tools::{Action, PointerButton, Preview, Tool, ToolState};
use uuid::Uuid;
#[test]
fn every_medium_is_captured_once_and_shared_between_preview_release_and_batch() {
    for media in BrushMedia::ALL {
        let mut tools = ToolState::default();
        tools.tool = Tool::Brush;
        tools.options.media = media;
        tools.pointer_down_subpixel(
            Uuid::nil(),
            (10 * 256 + 30, 20 * 256 + 80),
            PointerButton::Primary,
        );
        tools.options.media = BrushMedia::Plain;
        tools.pointer_move_subpixel((40 * 256 + 70, 22 * 256 + 20));
        let Some(Preview::Path(preview)) = tools.preview() else {
            panic!("path")
        };
        assert_eq!(preview.media, media);
        assert_eq!(preview.point_scale, 256);
        let (_, Action::Stroke(stroke)) = tools.pointer_up().unwrap() else {
            panic!("stroke")
        };
        assert_eq!(stroke, preview);
        let commands = paintcraft_tools::commands::stroke_operations(&stroke, &|| Ok(())).unwrap();
        assert_eq!(commands[0].get("brush").unwrap().as_str(), Some(media.id()));
    }
}
