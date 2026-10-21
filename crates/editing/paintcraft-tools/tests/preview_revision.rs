use paintcraft_core::types::text::TextOptions;
use paintcraft_tools::{PointerButton, Tool, ToolState};
use uuid::Uuid;

#[test]
fn rejected_moves_and_mismatched_release_preserve_revision_and_live_prefix() {
    let mut state = ToolState::default();
    state.pointer_down(Uuid::new_v4(), (1, 1));
    let revision = state.preview_revision();
    let preview = state.preview();
    state.try_pointer_move((1, 1), 1, 1).unwrap();
    assert!(state.try_pointer_move((2, 1), 1, 1).is_err());
    assert!(state.pointer_up_button(PointerButton::Secondary).is_none());
    assert_eq!(state.preview_revision(), revision);
    assert_eq!(state.preview(), preview);
    state.try_pointer_move((2, 1), 2, 2).unwrap();
    assert_ne!(state.preview_revision(), revision);
    let revision = state.preview_revision();
    state.pointer_up().unwrap();
    assert_ne!(state.preview_revision(), revision);
    assert!(state.preview().is_none());
}
#[test]
fn text_tracks_changes_without_invalidating_on_rejected_or_repeated_input() {
    let mut state = ToolState::default();
    state.set_text_style(TextOptions::default());
    state.begin_text(Uuid::new_v4(), (0, 0));
    let empty = state.preview_revision();
    state.backspace();
    assert_eq!(state.type_text("\0"), 1);
    state.set_text_style(TextOptions::default());
    assert_eq!(state.preview_revision(), empty);
    state.type_text("aé");
    let typed = state.preview_revision();
    assert_ne!(typed, empty);
    state.backspace();
    assert_ne!(state.preview_revision(), typed);
    let revision = state.preview_revision();
    state.commit_text().unwrap();
    assert_ne!(state.preview_revision(), revision);
    state.begin_text(Uuid::new_v4(), (0, 0));
    let revision = state.preview_revision();
    assert!(state.commit_text().is_none());
    assert_ne!(state.preview_revision(), revision);
    assert!(state.preview().is_none());
}
#[test]
fn every_pointer_preview_kind_tracks_endpoint_and_constraint_changes() {
    for tool in [
        Tool::Pencil,
        Tool::Brush,
        Tool::Eraser,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Select,
    ] {
        let mut state = ToolState::default();
        state.tool = tool;
        state.pointer_down(Uuid::new_v4(), (0, 0));
        let before = state.preview_revision();
        state.pointer_move((4, 2));
        assert_ne!(state.preview_revision(), before);
        let moved = state.preview_revision();
        state.pointer_move((4, 2));
        assert_eq!(state.preview_revision(), moved);
        state.constrain_shape(true);
        let constrained = state.preview_revision();
        state.constrain_shape(true);
        assert_eq!(state.preview_revision(), constrained);
        state.cancel_gesture();
        assert_ne!(state.preview_revision(), constrained);
        assert!(state.preview().is_none());
    }
}
