use paintcraft_core::types::{ErrorCode, json::Value, policy::COORD_ABS_MAX};
use paintcraft_tools::{Action, PointerButton, Preview, SelectionMode, Tool, ToolState, commands};
use uuid::Uuid;

fn lasso() -> ToolState {
    let mut tools = ToolState::default();
    tools.tool = Tool::Select;
    tools.options.selection_mode = SelectionMode::FreeForm;
    tools
}

#[test]
fn lasso_retains_integer_path_and_captured_mode_but_never_becomes_a_paint_path() {
    let mut tools = lasso();
    tools.pointer_down(Uuid::nil(), (-3, 2));
    let revision = tools.preview_revision();
    tools.pointer_move((-3, 2));
    assert_eq!(
        tools.preview_revision(),
        revision,
        "duplicate samples do not grow input"
    );
    tools.options.selection_mode = SelectionMode::Rectangle;
    tools.pointer_move((5, 2));
    tools.pointer_move((7, 6));
    tools.constrain_shape(true);
    assert!(tools.gesture_is_selection());
    assert!(tools.gesture_is_lasso());
    assert!(!tools.gesture_is_path());
    assert_eq!(tools.gesture_points(), &[(-3, 2), (5, 2), (7, 6)]);
    assert_eq!(
        tools.selection_preview_size(paintcraft_core::types::Rect::new(0, 0, 20, 20).unwrap()),
        None
    );
    assert_eq!(
        tools.preview(),
        Some(Preview::SelectionPath {
            points: vec![(-3, 2), (5, 2), (7, 6)]
        })
    );
    let (_, Action::SelectPolygon { points }) = tools
        .try_pointer_up_at((1, 8), PointerButton::Primary, 10, 10)
        .unwrap()
        .unwrap()
    else {
        panic!("polygon selection");
    };
    assert_eq!(points, [(-3, 2), (5, 2), (7, 6), (1, 8)]);
    assert!(!tools.gesture_in_progress());
    assert!(!tools.gesture_is_selection());
    assert_eq!(tools.preview(), None);
    assert_eq!(tools.pointer_up(), None, "release produces one intention");
}

#[test]
fn rectangle_keeps_existing_endpoints_when_preference_changes_during_drag() {
    let mut tools = ToolState::default();
    tools.tool = Tool::Select;
    tools.pointer_down(Uuid::nil(), (3, 4));
    tools.options.selection_mode = SelectionMode::FreeForm;
    tools.pointer_move((5, 7));
    tools.pointer_move((9, 8));
    assert!(tools.gesture_is_selection());
    assert!(!tools.gesture_is_lasso());
    assert_eq!(
        tools.preview(),
        Some(Preview::Marquee {
            from: (3, 4),
            to: (9, 8)
        })
    );
    let (_, action) = tools.pointer_up().unwrap();
    assert_eq!(
        action,
        Action::Select {
            corners: Some(((3, 4), (9, 8)))
        }
    );
}

#[test]
fn lasso_ignores_secondary_press_and_release_and_cancel_discards_every_point() {
    let mut tools = lasso();
    tools.pointer_down_button(Uuid::nil(), (0, 0), PointerButton::Secondary);
    assert!(!tools.pointer_is_down());
    tools.pointer_down(Uuid::nil(), (1, 1));
    tools.pointer_move((3, 1));
    assert_eq!(
        tools
            .try_pointer_up_at((9, 9), PointerButton::Secondary, 0, 0)
            .unwrap(),
        None
    );
    assert_eq!(tools.gesture_points(), &[(1, 1), (3, 1)]);
    tools.cancel_gesture();
    assert_eq!(tools.preview(), None);
    assert_eq!(tools.pointer_up(), None);
}

#[test]
fn fewer_than_three_distinct_vertices_never_replace_an_existing_selection() {
    for points in [
        vec![(1, 1)],
        vec![(1, 1), (4, 1)],
        vec![(1, 1), (4, 1), (1, 1), (4, 1)],
    ] {
        let mut tools = lasso();
        tools.pointer_down(Uuid::nil(), points[0]);
        for point in &points[1..] {
            tools.pointer_move(*point);
        }
        assert_eq!(tools.pointer_up(), None);
        assert!(!tools.gesture_in_progress());
    }
}

#[test]
fn fallible_admission_and_release_failures_cancel_instead_of_publishing_a_prefix() {
    for (point_limit, capacity_limit) in [(3, 100), (100, 3)] {
        let mut tools = lasso();
        tools.pointer_down(Uuid::nil(), (0, 0));
        tools
            .try_pointer_move((5, 0), point_limit, capacity_limit)
            .unwrap();
        tools
            .try_pointer_move((5, 5), point_limit, capacity_limit)
            .unwrap();
        let error = tools
            .try_pointer_up_at((0, 5), PointerButton::Primary, point_limit, capacity_limit)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(!tools.gesture_in_progress());
        assert_eq!(tools.pointer_up(), None);
        assert_eq!(tools.preview(), None);
    }
    let mut tools = lasso();
    tools.pointer_down(Uuid::nil(), (0, 0));
    tools.try_pointer_move((1, 0), 3, 3).unwrap();
    tools.try_pointer_move((1, 1), 3, 3).unwrap();
    // Releasing at an already admitted position needs no extra point capacity.
    assert!(
        tools
            .try_pointer_up_at((1, 1), PointerButton::Primary, 3, 3)
            .unwrap()
            .is_some()
    );
}

#[test]
fn schema_point_limit_never_silently_truncates_even_for_legacy_infallible_input() {
    let mut tools = lasso();
    tools.pointer_down(Uuid::nil(), (0, 0));
    for x in 1..65_536 {
        tools.pointer_move((x, x % 2));
    }
    assert_eq!(tools.gesture_points().len(), 65_536);
    assert!(tools.gesture_point_capacity() <= 65_536);
    tools.pointer_move((65_536, 0));
    assert!(!tools.gesture_in_progress());
    assert_eq!(tools.pointer_up(), None);
}

#[test]
fn lasso_subpixels_floor_to_document_pixels_and_invalid_coordinates_cancel() {
    let mut tools = lasso();
    tools.pointer_down_subpixel(Uuid::nil(), (-1, 257), PointerButton::Primary);
    tools.try_pointer_move_subpixel((511, 511), 10, 10).unwrap();
    tools
        .try_pointer_move_subpixel((1024, 768), 10, 10)
        .unwrap();
    assert_eq!(tools.gesture_points(), &[(-1, 1), (1, 1), (4, 3)]);
    let error = tools
        .try_pointer_move((COORD_ABS_MAX + 1, 0), 10, 10)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert_eq!(tools.pointer_up(), None);
    tools.pointer_down(Uuid::nil(), (i64::MIN, 0));
    assert!(!tools.pointer_is_down());
}

#[test]
fn polygon_and_invert_builders_have_no_target_or_raster_side_effect_fields() {
    let points = [(-1, 0), (5, 0), (2, 7)];
    let polygon = commands::selection_polygon(&points).unwrap();
    let Value::Object(fields) = polygon else {
        panic!("polygon object");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields["op"], Value::str("selection.set_polygon"));
    let Value::Array(items) = &fields["points"] else {
        panic!("points");
    };
    for (point, (x, y)) in items.iter().zip(points) {
        assert_eq!(point.get("x"), Some(&Value::Int(x)));
        assert_eq!(point.get("y"), Some(&Value::Int(y)));
    }
    assert_eq!(
        commands::invert_selection(),
        Value::object([("op", Value::str("selection.invert"))])
    );
    assert!(commands::selection_polygon(&points[..2]).is_err());
    assert!(commands::selection_polygon(&[(i64::MAX, 0); 3]).is_err());
}
