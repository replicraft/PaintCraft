use paintcraft_tools::{PointerButton, Tool, ToolState};
use uuid::Uuid;

#[test]
fn magnifier_never_starts_a_paint_gesture_or_produces_an_edit() {
    let mut tools = ToolState::default();
    tools.choose_tool(Tool::Magnifier);
    for button in [PointerButton::Primary, PointerButton::Secondary] {
        tools.pointer_down_subpixel(Uuid::nil(), (128, 384), button);
        tools.pointer_move_subpixel((2048, 4096));
        assert!(!tools.gesture_in_progress());
        assert!(tools.preview().is_none());
        assert!(tools.pointer_up_button(button).is_none());
    }
    let mut ids: Vec<_> = Tool::ALL.into_iter().map(Tool::action_id).collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count);
    assert_eq!(Tool::Magnifier.action_id(), 30);
}
