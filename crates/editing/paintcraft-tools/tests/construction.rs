use paintcraft_core::types::Rgba;
use paintcraft_tools::{
    Action, Contour, PointerButton as Button, Preview, ShapeStyle, Tool, ToolState,
};
use uuid::Uuid;

fn click(t: &mut ToolState, p: (i64, i64)) -> Option<(u64, Action)> {
    t.pointer_down(Uuid::nil(), p);
    t.pointer_up()
}
fn shape(action: Option<(u64, Action)>) -> Contour {
    let Some((_, Action::Contour(shape))) = action else {
        panic!("completed contour")
    };
    shape
}

#[test]
fn polygon_retains_vertices_button_and_style_until_closed_once() {
    let mut t = ToolState::default();
    t.tool = Tool::Polygon;
    t.options.style = ShapeStyle::Both;
    t.color = Rgba([200, 10, 20, 128]);
    t.secondary = Rgba([10, 20, 200, 96]);
    t.pointer_down_button(Uuid::nil(), (10, 10), Button::Secondary);
    t.pointer_move((40, 10));
    assert!(t.pointer_up().is_none());
    assert_eq!(t.gesture_button(), Some(Button::Secondary));
    assert!(t.pointer_up_button(Button::Secondary).is_none());
    let token = t.preview_token();
    assert!(t.gesture_in_progress() && !t.pointer_is_down());
    t.options.style = ShapeStyle::Filled;
    t.options.size = 30;
    t.secondary = Rgba([0; 4]);
    t.pointer_down_button(Uuid::new_v4(), (40, 40), Button::Secondary);
    assert!(
        !t.pointer_is_down(),
        "a different target cannot take over a draft"
    );
    t.pointer_down_button(Uuid::nil(), (40, 40), Button::Secondary);
    assert!(t.pointer_up_button(Button::Secondary).is_none());
    t.pointer_down_button(Uuid::nil(), (10, 10), Button::Secondary);
    let Some(Preview::Contour(preview)) = t.preview() else {
        panic!("polygon preview")
    };
    let result = shape(t.pointer_up_button(Button::Secondary));
    assert_eq!(preview, result);
    assert_eq!(result.points, [(10, 10), (40, 10), (40, 40)]);
    assert_eq!(result.stroke, Some((Rgba([10, 20, 200, 96]), 4)));
    assert_eq!(result.fill, Some(Rgba([200, 10, 20, 128])));
    assert!(token.is_some() && !t.gesture_in_progress());
    assert!(t.pointer_up_button(Button::Secondary).is_none() && t.finish_shape().is_none());
}

#[test]
fn polygon_backspace_shift_enter_and_cancel_have_explicit_boundaries() {
    let mut t = ToolState::default();
    t.tool = Tool::Polygon;
    assert!(click(&mut t, (10, 10)).is_none());
    t.pointer_down(Uuid::nil(), (40, 15));
    t.constrain_shape(true);
    assert!(t.pointer_up().is_none());
    t.constrain_shape(false);
    assert!(click(&mut t, (40, 40)).is_none());
    t.remove_shape_vertex();
    assert!(t.finish_shape().is_none());
    assert!(click(&mut t, (20, 40)).is_none());
    let result = shape(t.finish_shape());
    assert_eq!(result.points, [(10, 10), (40, 10), (20, 40)]);
    click(&mut t, (10, 10));
    click(&mut t, (40, 10));
    t.cancel_gesture();
    assert!(!t.gesture_in_progress() && t.preview().is_none());
    assert!(t.pointer_up().is_none() && t.finish_shape().is_none());
}

#[test]
fn curve_uses_two_handles_and_preview_equals_final_open_cubic() {
    let mut t = ToolState::default();
    t.tool = Tool::Curve;
    t.options.style = ShapeStyle::Filled;
    t.pointer_down(Uuid::nil(), (0, 0));
    t.pointer_move((100, 0));
    assert!(t.pointer_up().is_none());
    t.pointer_down(Uuid::nil(), (0, 100));
    assert!(t.pointer_up().is_none());
    t.pointer_down(Uuid::nil(), (100, 100));
    let Some(Preview::Contour(preview)) = t.preview() else {
        panic!("curve preview")
    };
    let result = shape(t.pointer_up());
    assert_eq!(preview, result);
    assert_eq!(result.points.first(), Some(&(0, 0)));
    assert_eq!(result.points.last(), Some(&(100, 0)));
    assert!(
        result.points.contains(&(50, 75)),
        "cubic midpoint from independent Bernstein weights"
    );
    assert!(!result.closed && result.fill.is_none() && result.stroke.is_some());
    assert!(t.preview().is_none() && !t.gesture_in_progress());
}

#[test]
fn polygon_vertex_limit_closes_without_silently_dropping_a_vertex() {
    let mut t = ToolState::default();
    t.tool = Tool::Polygon;
    for i in 0..1023 {
        assert!(click(&mut t, (10 + i * 10, 10 + (i % 2) * 10)).is_none());
    }
    let result = shape(click(&mut t, (10 + 1023 * 10, 20)));
    assert_eq!(result.points.len(), 1024);
    assert!(result.closed && !t.gesture_in_progress());
}
