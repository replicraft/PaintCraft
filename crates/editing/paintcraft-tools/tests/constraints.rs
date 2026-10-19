use paintcraft_tools::{Action, Preview, Tool, ToolState};
use uuid::Uuid;

#[test]
fn shift_keeps_shape_dimensions_equal_in_all_drag_directions_and_release_matches() {
    for tool in [Tool::Rect, Tool::Ellipse] {
        for (dx, dy) in [(30, 10), (-30, 10), (10, -30), (-10, -30), (0, 30)] {
            let mut tools = ToolState::default();
            tools.tool = tool;
            tools.pointer_down(Uuid::nil(), (50, 50));
            tools.pointer_move((50 + dx, 50 + dy));
            tools.constrain_shape(true);
            let Some(Preview::Shape { from, to, .. }) = tools.preview() else {
                panic!("shape")
            };
            assert_eq!((to.0 - from.0).abs(), 30);
            assert_eq!((to.1 - from.1).abs(), 30);
            let (
                _,
                Action::Shape {
                    from: start,
                    to: end,
                    ..
                },
            ) = tools.pointer_up().unwrap()
            else {
                panic!("shape")
            };
            assert_eq!((start, end), (from, to));
        }
    }
}

#[test]
fn releasing_shift_restores_raw_shape_geometry_without_moving_pointer() {
    let mut tools = ToolState::default();
    tools.tool = Tool::Rect;
    tools.pointer_down(Uuid::nil(), (50, 50));
    tools.pointer_move((80, 60));
    tools.constrain_shape(true);
    tools.constrain_shape(false);
    let (_, Action::Shape { to, .. }) = tools.pointer_up().unwrap() else {
        panic!("shape")
    };
    assert_eq!(to, (80, 60));
}

#[test]
fn shift_snaps_lines_to_horizontal_vertical_and_diagonal() {
    for (point, expected) in [
        ((80, 55), (80, 50)),
        ((55, 80), (50, 80)),
        ((80, 70), (80, 80)),
    ] {
        let mut tools = ToolState::default();
        tools.tool = Tool::Line;
        tools.pointer_down(Uuid::nil(), (50, 50));
        tools.pointer_move(point);
        tools.constrain_shape(true);
        let (_, Action::Shape { to, .. }) = tools.pointer_up().unwrap() else {
            panic!("line")
        };
        assert_eq!(to, expected);
    }
}
