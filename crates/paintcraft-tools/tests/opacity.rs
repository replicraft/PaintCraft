use paintcraft_core::types::Rgba;
use paintcraft_tools::{Action, BlendMode, PointerButton, Preview, Tool, ToolState};
use uuid::Uuid;

#[test]
fn opacity_and_button_color_are_captured_once_and_match_preview() {
    let mut tools = ToolState::default();
    tools.tool = Tool::Brush;
    tools.secondary = Rgba([12, 34, 56, 128]);
    tools.options.brush_opacity = 50;
    tools.pointer_down_subpixel(Uuid::nil(), (100, 200), PointerButton::Secondary);
    tools.options.brush_opacity = 0;
    tools.secondary = Rgba([99, 99, 99, 255]);
    tools.pointer_move_subpixel((300, 400));
    let Some(Preview::Path(preview)) = tools.preview() else {
        panic!("preview");
    };
    assert_eq!(preview.color, Rgba([12, 34, 56, 64]));
    assert_eq!(preview.mode, BlendMode::SourceOver);
    let (_, Action::Stroke(released)) = tools.pointer_up_button(PointerButton::Secondary).unwrap()
    else {
        panic!("stroke");
    };
    assert_eq!(preview, released);
}

#[test]
fn transparent_paint_is_source_over_and_only_eraser_replaces_pixels() {
    for (tool, alpha, mode) in [
        (Tool::Brush, 0, BlendMode::SourceOver),
        (Tool::Pencil, 128, BlendMode::SourceOver),
        (Tool::Eraser, 0, BlendMode::Replace),
    ] {
        let mut tools = ToolState::default();
        tools.tool = tool;
        tools.color = Rgba([12, 34, 56, 128]);
        tools.options.brush_opacity = 0;
        tools.pointer_down(Uuid::nil(), (2, 3));
        let (_, Action::Stroke(stroke)) = tools.pointer_up().unwrap() else {
            panic!("stroke");
        };
        assert_eq!(stroke.color.a(), alpha);
        assert_eq!(stroke.mode, mode);
    }
}
