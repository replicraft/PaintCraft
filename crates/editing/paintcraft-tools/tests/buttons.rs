use paintcraft_core::types::Rgba;
use paintcraft_tools::{Action, PointerButton as Button, Preview, ShapeStyle, Tool, ToolState};
use uuid::Uuid;

#[test]
fn secondary_color_is_captured_without_changing_preferences_and_requires_its_own_release() {
    let mut t = ToolState::default();
    t.secondary = Rgba([12, 34, 56, 93]);
    t.pointer_down_button(Uuid::nil(), (1, 2), Button::Secondary);
    t.color = Rgba([99, 88, 77, 255]);
    t.secondary = Rgba([66, 55, 44, 255]);
    assert!(matches!(
        t.preview(),
        Some(Preview::Path(paintcraft_tools::Stroke {
            point_scale: 1,
            color: Rgba([12, 34, 56, 93]),
            ..
        }))
    ));
    assert!(t.pointer_up().is_none());
    assert!(t.gesture_in_progress());
    let (_, Action::Stroke(paintcraft_tools::Stroke { color, .. })) =
        t.pointer_up_button(Button::Secondary).unwrap()
    else {
        panic!("stroke");
    };
    assert_eq!(color, Rgba([12, 34, 56, 93]));
    assert_eq!(t.color, Rgba([99, 88, 77, 255]));
    assert_eq!(t.secondary, Rgba([66, 55, 44, 255]));
    assert!(!t.gesture_in_progress());
}

#[test]
fn shape_colors_reverse_as_a_pair_and_fill_uses_secondary_at_the_seed() {
    let mut t = ToolState::default();
    t.tool = Tool::Rect;
    t.options.style = ShapeStyle::Both;
    t.color = Rgba([255, 0, 0, 255]);
    t.secondary = Rgba([0, 0, 255, 255]);
    t.pointer_down_button(Uuid::nil(), (1, 2), Button::Secondary);
    t.pointer_move((7, 8));
    let (_, Action::Shape { stroke, fill, .. }) = t.pointer_up_button(Button::Secondary).unwrap()
    else {
        panic!("shape");
    };
    assert_eq!(stroke.unwrap().0, Rgba([0, 0, 255, 255]));
    assert_eq!(fill, Some(Rgba([255, 0, 0, 255])));
    t.tool = Tool::Fill;
    t.pointer_down_button(Uuid::nil(), (4, 5), Button::Secondary);
    t.pointer_move((7, 8));
    let (_, Action::Fill { at, color, .. }) = t.pointer_up_button(Button::Secondary).unwrap()
    else {
        panic!("fill");
    };
    assert_eq!(at, (4, 5));
    assert_eq!(color, Rgba([0, 0, 255, 255]));
}

#[test]
fn unsupported_secondary_policies_and_foreign_button_presses_leave_the_owner_unchanged() {
    let mut t = ToolState::default();
    for tool in [Tool::Select, Tool::Eraser, Tool::Text] {
        t.tool = tool;
        t.pointer_down_button(Uuid::nil(), (1, 2), Button::Secondary);
        assert!(!t.gesture_in_progress());
    }
    t.tool = Tool::Pencil;
    t.pointer_down(Uuid::nil(), (1, 2));
    t.pointer_down_button(Uuid::new_v4(), (8, 9), Button::Secondary);
    assert!(t.pointer_up_button(Button::Secondary).is_none());
    assert_eq!(t.gesture_button(), Some(Button::Primary));
    assert_eq!(t.gesture_points(), &[(1, 2)]);
    t.cancel_gesture();
    assert_eq!(t.gesture_button(), None);
}
