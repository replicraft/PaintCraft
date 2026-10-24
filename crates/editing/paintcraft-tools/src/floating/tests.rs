use super::*;
use crate::{Tool, ToolState};
fn options() -> FloatingOptions {
    FloatingOptions {
        layer: Uuid::from_u128(12),
        selection: Selection::Rect(Rect::new(-2, -2, 8, 8).unwrap()),
        bounds: Rect::new(-2, -2, 8, 8).unwrap(),
        clear: Rgba([9, 8, 7, 255]),
        transparent_key: Some(Rgba([1, 2, 3, 4])),
        token: 85,
        commit_on_release: true,
    }
}
#[test]
fn absolute_motion_return_to_origin_and_matching_release_preserve_captured_options() {
    let mut captured = options();
    let mut drag = FloatingSelection::begin(captured.clone(), (0, 0), PointerButton::Primary)
        .unwrap()
        .unwrap();
    let revision = drag.revision();
    assert!(drag.preview().is_none());
    drag.update((4, 3)).unwrap();
    assert_ne!(drag.revision(), revision);
    let revision = drag.revision();
    drag.update((4, 3)).unwrap();
    assert_eq!(
        drag.revision(),
        revision,
        "stationary samples do not invalidate previews"
    );
    drag.update((1, 2)).unwrap();
    assert_eq!(drag.offset(), (1, 2), "positions never accumulate");
    assert!(
        drag.release((i64::MAX, i64::MAX), PointerButton::Secondary)
            .unwrap()
            .is_none()
    );
    assert!(drag.is_active());
    captured.clear = Rgba::TRANSPARENT;
    captured.transparent_key = None;
    assert_eq!(
        drag.release((5, 6), PointerButton::Primary).unwrap(),
        Some((
            85,
            Action::MoveSelection {
                layer: captured.layer,
                offset: (5, 6),
                clear: Rgba([9, 8, 7, 255]),
                transparent_key: Some(Rgba([1, 2, 3, 4])),
            }
        ))
    );
    assert!(drag.preview().is_none());
    assert!(
        drag.release((9, 9), PointerButton::Primary)
            .unwrap()
            .is_none()
    );
    let mut drag = FloatingSelection::begin(options(), (0, 0), PointerButton::Primary)
        .unwrap()
        .unwrap();
    drag.update((4, 3)).unwrap();
    drag.update((0, 0)).unwrap();
    assert!(
        drag.release((0, 0), PointerButton::Primary)
            .unwrap()
            .is_none()
    );
}
#[test]
fn subpixels_floor_negatives_and_policy_edges_clamp_or_cancel_without_partial_actions() {
    let mut drag = FloatingSelection::begin_subpixel(options(), (-1, -1), PointerButton::Primary)
        .unwrap()
        .unwrap();
    drag.update_subpixel((255, -257)).unwrap();
    assert_eq!(drag.offset(), (1, -1));
    assert_eq!(
        drag.release_subpixel((256, 0), PointerButton::Primary)
            .unwrap()
            .unwrap()
            .1,
        Action::MoveSelection {
            layer: options().layer,
            offset: (2, 1),
            clear: options().clear,
            transparent_key: options().transparent_key
        }
    );
    let mut edge = options();
    edge.bounds = Rect::new(COORD_ABS_MAX - 8, 0, 8, 8).unwrap();
    edge.selection = Selection::Rect(edge.bounds);
    let mut drag =
        FloatingSelection::begin(edge.clone(), (COORD_ABS_MAX - 4, 2), PointerButton::Primary)
            .unwrap()
            .unwrap();
    drag.update((COORD_ABS_MAX, 4)).unwrap();
    assert_eq!(drag.offset(), (0, 2));
    assert_eq!(drag.bounds().x1, COORD_ABS_MAX);
    assert!(drag.update((i64::MAX, 0)).is_err());
    assert!(!drag.is_active());
    assert!(drag.preview().is_none());
    assert!(
        drag.release((0, 0), PointerButton::Primary)
            .unwrap()
            .is_none()
    );
}
#[test]
fn actual_mask_holes_empty_and_foreign_buttons_do_not_grab_a_bounding_rectangle() {
    let bounds = Rect::new(0, 0, 8, 8).unwrap();
    let mut captured = options();
    captured.bounds = bounds;
    let budget = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
    captured.selection = Selection::Mask(
        paintcraft_raster::selection::BinaryMask::polygon(
            bounds,
            &[(0, 0), (8, 0), (8, 2), (2, 2), (2, 8), (0, 8)],
            &budget,
            paintcraft_raster::selection::MaskLimits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    );
    assert!(
        FloatingSelection::begin(captured.clone(), (6, 6), PointerButton::Primary)
            .unwrap()
            .is_none()
    );
    assert!(
        FloatingSelection::begin(captured.clone(), (1, 1), PointerButton::Secondary)
            .unwrap()
            .is_none()
    );
    let drag = FloatingSelection::begin(captured.clone(), (1, 6), PointerButton::Primary)
        .unwrap()
        .unwrap();
    assert_eq!(drag.source_selection(), &captured.selection);
    for selection in [
        Selection::Inactive,
        Selection::Empty,
        Selection::Mask(paintcraft_raster::selection::BinaryMask::uniform(bounds, false).unwrap()),
    ] {
        captured.selection = selection;
        assert!(
            FloatingSelection::begin(captured.clone(), (1, 1), PointerButton::Primary)
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn toolstate_routes_fixed_capacity_motion_preview_tokens_release_and_cancellation() {
    let mut tools = ToolState::default();
    tools.choose_tool(Tool::Select);
    assert!(
        tools
            .begin_selection_move(options(), (0, 0), PointerButton::Primary)
            .unwrap()
    );
    let token = tools.gesture_token().unwrap();
    assert!(token >= 1 << 63);
    assert_eq!(tools.preview_token(), Some(token));
    assert_eq!(tools.gesture_button(), Some(PointerButton::Primary));
    assert!(tools.gesture_in_progress() && tools.pointer_is_down());
    assert!(!tools.gesture_is_path() && !tools.gesture_is_lasso());
    let revision = tools.preview_revision();
    tools
        .try_pointer_move_subpixel((2 * 256 + 20, 3 * 256 + 40), 0, 0)
        .unwrap();
    assert_ne!(revision, tools.preview_revision());
    assert_eq!(tools.gesture_point_capacity(), 0);
    assert_eq!(
        tools.preview(),
        Some(Preview::MoveSelection {
            layer: options().layer,
            offset: (2, 3),
            clear: options().clear,
            transparent_key: options().transparent_key
        })
    );
    tools.options.transparent_selection = false;
    tools.secondary = Rgba([100; 4]);
    assert!(tools.pointer_up_button(PointerButton::Secondary).is_none());
    let action = tools
        .try_pointer_up_at((4, 5), PointerButton::Primary, 0, 0)
        .unwrap()
        .unwrap();
    assert_eq!(action.0, token);
    assert_eq!(
        action.1,
        Action::MoveSelection {
            layer: options().layer,
            offset: (4, 5),
            clear: options().clear,
            transparent_key: options().transparent_key
        }
    );
    assert!(!tools.gesture_in_progress());
    assert!(tools.preview().is_none());
    for tool in [Tool::Select, Tool::Pencil] {
        tools.choose_tool(Tool::Select);
        assert!(
            tools
                .begin_selection_move(options(), (0, 0), PointerButton::Primary)
                .unwrap()
        );
        tools.pointer_move((2, 3));
        tools.choose_tool(tool);
        assert!(!tools.gesture_in_progress());
        assert!(tools.preview().is_none());
        assert!(tools.pointer_up().is_none());
    }
    tools.choose_tool(Tool::Select);
    tools
        .begin_selection_move(options(), (0, 0), PointerButton::Primary)
        .unwrap();
    tools.cancel_gesture();
    assert!(!tools.gesture_in_progress());
    tools
        .begin_selection_move(options(), (0, 0), PointerButton::Primary)
        .unwrap();
    assert!(tools.try_pointer_move((i64::MAX, 0), 0, 0).is_err());
    assert!(!tools.gesture_in_progress());
    assert!(tools.pointer_up().is_none());
}
#[test]
fn command_describes_captured_rgb_key_and_checked_offset() {
    let opts = options();
    let value =
        crate::commands::move_selection(opts.layer, (3, -2), opts.clear, opts.transparent_key)
            .unwrap();
    let expected = paintcraft_core::types::json::parse(format!(
        "{{\"op\":\"raster.move_selection\",\"layer_ref\":{{\"id\":\"{}\"}},\"offset\":{{\"x\":3,\"y\":-2}},\"clear\":\"#090807FF\",\"transparent_key\":\"#01020304\"}}",
        opts.layer
    ).as_bytes());
    assert_eq!(value, expected.unwrap());
    assert!(crate::commands::move_selection(opts.layer, (i64::MAX, 0), opts.clear, None).is_err());
    assert!(!ToolState::default().options.transparent_selection);
}

#[test]
fn deferred_draft_regrabs_original_samples_with_stable_token_then_finishes_once() {
    let mut tools = ToolState::default();
    tools.choose_tool(Tool::Select);
    let mut captured = options();
    captured.commit_on_release = false;
    assert!(
        tools
            .begin_selection_move(captured.clone(), (0, 0), PointerButton::Primary)
            .unwrap()
    );
    let token = tools.gesture_token().unwrap();
    tools.pointer_move((4, 3));
    assert!(tools.pointer_up().is_none());
    assert!(tools.gesture_in_progress());
    assert!(!tools.pointer_is_down());
    assert!(tools.preview().is_some());
    assert_eq!(tools.preview_token(), Some(token));
    tools.pointer_move((400, 400));
    assert_eq!(tools.floating_selection().unwrap().offset(), (4, 3));
    let mut replacement = options();
    replacement.clear = Rgba::TRANSPARENT;
    replacement.transparent_key = None;
    assert!(
        !tools
            .begin_selection_move(replacement.clone(), (100, 100), PointerButton::Primary)
            .unwrap()
    );
    assert!(
        tools
            .begin_selection_move(replacement, (4, 3), PointerButton::Primary)
            .unwrap()
    );
    assert!(tools.pointer_is_down());
    assert_eq!(tools.gesture_token(), Some(token));
    tools.pointer_move((6, 7));
    assert!(tools.pointer_up().is_none());
    assert_eq!(tools.floating_selection().unwrap().offset(), (6, 7));
    assert_eq!(
        tools.finish_selection_move(),
        Some((
            token,
            Action::MoveSelection {
                layer: captured.layer,
                offset: (6, 7),
                clear: captured.clear,
                transparent_key: captured.transparent_key,
            }
        ))
    );
    assert!(tools.finish_selection_move().is_none());
    assert!(!tools.gesture_in_progress());
    tools
        .begin_selection_move(captured.clone(), (0, 0), PointerButton::Primary)
        .unwrap();
    tools.pointer_move((4, 3));
    tools.pointer_up();
    tools
        .begin_selection_move(captured, (4, 3), PointerButton::Primary)
        .unwrap();
    tools.pointer_move((0, 0));
    tools.pointer_up();
    assert_eq!(tools.floating_selection().unwrap().offset(), (0, 0));
    assert!(tools.preview().is_none());
    assert!(tools.finish_selection_move().is_none());
}
