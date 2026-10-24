use paintcraft_core::types::Rect;
use paintcraft_tools::{Tool, ToolState};
use uuid::Uuid;

#[test]
fn dimensions_follow_both_drag_directions_clip_and_clear_on_cancel_or_release() {
    let mut tools = ToolState::default();
    tools.tool = Tool::Select;
    let canvas = Rect::new(0, 0, 100, 80).unwrap();
    assert_eq!(tools.selection_preview_size(canvas), None);
    tools.pointer_down(Uuid::nil(), (20, 30));
    assert_eq!(tools.selection_preview_size(canvas), Some((1, 1)));
    tools.pointer_move((45, 55));
    assert_eq!(tools.selection_preview_size(canvas), Some((26, 26)));
    tools.pointer_move((-10, 12));
    assert_eq!(tools.selection_preview_size(canvas), Some((21, 19)));
    tools.pointer_up().unwrap();
    assert_eq!(tools.selection_preview_size(canvas), None);
    tools.pointer_down(Uuid::nil(), (150, 100));
    tools.pointer_move((160, 120));
    assert_eq!(tools.selection_preview_size(canvas), Some((0, 0)));
    tools.cancel_gesture();
    assert_eq!(tools.selection_preview_size(canvas), None);
}
