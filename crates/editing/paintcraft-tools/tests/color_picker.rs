use paintcraft_core::types::{
    Rgba,
    picker::{PickerAfter, SampleSize, SampleSource},
};
use paintcraft_tools::{Action, ColorPick, PointerButton, Tool, ToolState};
fn release(s: &mut ToolState, button: PointerButton) -> ColorPick {
    let (_, Action::PickColor(p)) = s.pointer_up_button(button).unwrap() else {
        panic!("expected color sample")
    };
    p
}
#[test]
fn captures_button_options_previous_tool_and_last_subpixel_position_without_ink_preview() {
    let mut s = ToolState::default();
    s.activate_tool(Tool::Brush);
    s.activate_tool(Tool::ColorPicker);
    s.options.picker_source = SampleSource::Image;
    s.options.picker_size = SampleSize::ThirtyOne;
    s.options.picker_after = PickerAfter::Previous;
    let id = uuid::Uuid::new_v4();
    s.pointer_down_subpixel(id, (5 * 256 + 128, 6 * 256 + 128), PointerButton::Secondary);
    s.pointer_move_subpixel((7 * 256 + 255, 8 * 256));
    s.options.picker_source = SampleSource::Layer;
    s.options.picker_size = SampleSize::One;
    s.options.picker_after = PickerAfter::Stay;
    assert!(s.preview().is_none());
    assert!(s.pointer_up_button(PointerButton::Primary).is_none());
    let p = release(&mut s, PointerButton::Secondary);
    assert_eq!(
        (p.layer, p.at, p.source, p.size, p.return_to),
        (
            id,
            (7, 8),
            SampleSource::Image,
            SampleSize::ThirtyOne,
            Some(Tool::Brush)
        )
    );
    assert!(s.pointer_up_button(PointerButton::Secondary).is_none());
    let primary = s.color;
    assert!(s.apply_picked_color(&p, Rgba([17, 33, 55, 0])));
    assert_eq!(s.secondary, Rgba([17, 33, 55, 0]));
    assert_eq!(s.color, primary);
    assert_eq!(s.tool, Tool::Brush);
}
#[test]
fn cancelling_or_changing_tool_or_color_slot_prevents_late_result_and_does_not_restore_tool() {
    let mut s = ToolState::default();
    let id = uuid::Uuid::new_v4();
    s.activate_tool(Tool::ColorPicker);
    s.pointer_down(id, (1, 2));
    s.cancel_gesture();
    assert!(s.pointer_up().is_none());
    s.pointer_down(id, (1, 2));
    let p = release(&mut s, PointerButton::Primary);
    s.color = Rgba([70, 80, 90, 255]);
    assert!(!s.apply_picked_color(&p, Rgba([255; 4])));
    assert_eq!(s.color, Rgba([70, 80, 90, 255]));
    s.pointer_down(id, (1, 2));
    let p = release(&mut s, PointerButton::Primary);
    s.activate_tool(Tool::Pencil);
    s.activate_tool(Tool::ColorPicker);
    assert!(!s.apply_picked_color(&p, Rgba([255; 4])));
    s.options.picker_after = PickerAfter::Pencil;
    s.pointer_down(id, (1, 2));
    let p = release(&mut s, PointerButton::Primary);
    assert!(s.apply_picked_color(&p, Rgba([1, 2, 3, 128])));
    assert_eq!(s.tool, Tool::Pencil);
}

#[test]
fn reselecting_the_picker_invalidates_its_previous_request_but_layout_sync_does_not() {
    let mut s = ToolState::default();
    s.choose_tool(Tool::ColorPicker);
    let id = uuid::Uuid::new_v4();
    s.pointer_down(id, (1, 2));
    let p = release(&mut s, PointerButton::Primary);
    s.activate_tool(Tool::ColorPicker);
    assert!(s.can_apply_color_pick(&p));
    s.choose_tool(Tool::ColorPicker);
    assert!(!s.can_apply_color_pick(&p));
}
