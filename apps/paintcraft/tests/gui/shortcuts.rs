use super::studio::studio_harness;
use super::*;
use eframe::egui;
use paintcraft_editor::execute::Selection;

#[test]
fn menu_and_keyboard_selection_actions_have_the_same_history_behavior() {
    let (mut h, dir) = studio_harness(true);
    let initial = publications(&h);
    h.event(key(Key::A, Modifiers::COMMAND));
    h.run();
    assert!(matches!(h.state().editor().selection(), Selection::Rect(_)));
    assert_eq!(publications(&h), initial + 1);
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label_contains("Deselect").click();
    h.run();
    assert_eq!(h.state().editor().selection(), Selection::Inactive);
    assert_eq!(publications(&h), initial + 2);
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label_contains("Select all").click();
    h.run();
    assert!(matches!(h.state().editor().selection(), Selection::Rect(_)));
    h.event(key(Key::D, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().editor().selection(), Selection::Inactive);
    assert_eq!(publications(&h), initial + 4);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn close_shortcut_and_menu_leave_empty_workspace_where_new_shortcut_still_works() {
    let (mut h, dir) = studio_harness(true);
    h.event(key(Key::W, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().document_count(), 0);
    h.event(key(Key::N, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().document_count(), 1);
    h.get_by_label("File").click();
    h.run();
    h.get_by_label("File")
        .get_by_label_contains("Close")
        .click();
    h.run();
    assert_eq!(h.state().document_count(), 0);
    h.event(key_up(Key::N, Modifiers::COMMAND));
    h.event(key(Key::N, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().document_count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_as_shortcut_never_falls_through_to_overwriting_the_bound_file() {
    let dir = scratch("shortcut-save-as");
    let file = dir.join("work.pcnd");
    let document = paintcraft_document::new_document(
        32,
        32,
        Some(Rgba([255; 4])),
        514,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let original = paintcraft_formats::pcnd::write(&document).unwrap();
    std::fs::write(&file, &original).unwrap();
    let mut h = harness_with(App::new(Some(file.clone())), 1.0, true);
    assert_eq!(h.state().editor().path(), Some(file.as_path()));
    with_layer(&mut h);
    // This harness has no native picker, so Save As can only be cancelled/no-op.
    h.event(key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert!(h.state().editor().is_dirty());
    h.event(key_up(Key::S, Modifiers::COMMAND | Modifiers::SHIFT));
    h.event(key(Key::S, Modifiers::COMMAND));
    h.run();
    assert!(file.exists());
    assert_ne!(std::fs::read(&file).unwrap(), original);
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn text_field_owns_undo_and_document_shortcuts() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    let history = h.state().editor().history_view().ticket;
    h.get_by_label("Layer name").click();
    h.run();
    h.event(key(Key::Z, Modifiers::COMMAND));
    h.event(key(Key::N, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().editor().history_view().ticket, history);
    assert_eq!(h.state().document_count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ime_composition_and_close_modal_prevent_unrelated_document_shortcuts() {
    let (mut h, dir) = studio_harness(false);
    h.event(Event::Ime(egui::ImeEvent::Preedit {
        text: "candidate".into(),
        active_range_chars: None,
    }));
    h.event(key(Key::N, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().document_count(), 1);
    h.event(Event::Ime(egui::ImeEvent::Commit("candidate".into())));
    h.event(key_up(Key::N, Modifiers::COMMAND));
    h.event(key(Key::N, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().document_count(), 2);
    with_layer(&mut h);
    h.event(key(Key::W, Modifiers::COMMAND));
    h.run();
    h.event(key_up(Key::N, Modifiers::COMMAND));
    h.event(key(Key::N, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().document_count(), 2);
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().document_count(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn coalesced_undo_then_redo_uses_updated_enabled_states_in_event_order() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    let snapshot = h.state().editor().document_snapshot();
    let before = publications(&h);
    h.event(key(Key::Z, Modifiers::COMMAND));
    h.event(key(Key::Y, Modifiers::COMMAND));
    h.run();
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(publications(&h), before + 2);
    std::fs::remove_dir_all(dir).unwrap();
}
