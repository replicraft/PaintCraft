use crate::WebApp;
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};

fn harness() -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_max_steps(12)
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h
}
fn begin(h: &mut Harness<WebApp>, text: &str) {
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    let p = h.state().canvas.rect.unwrap().min + Vec2::new(20.0, 20.0);
    h.event(Event::PointerMoved(p));
    h.event(Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.event(Event::Text(text.into()));
    h.event(Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some(text));
}

#[test]
fn canvas_text_coalesced_input_previews_and_places_one_undoable_edit() {
    let mut h = harness();
    let ticket = h.state().editor.session.ticket();
    begin(&mut h, "Paint café Ω");
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(h.state().unsaved());
    assert!(h.output().platform_output.ime.is_some());
    let copy = h.state().editor.copy_snapshot().unwrap();
    h.get_by_label("Place text").click();
    h.run();
    assert!(!h.state().editor.tools.is_editing_text());
    assert_eq!(h.state().editor.session.undo_depth(), 1);
    assert_eq!(h.state().editor.session.document().state_id, copy.state_id);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().editor.session.ticket().state_id, ticket.state_id);
    assert!(!h.state().unsaved());
}

#[test]
fn text_selection_replacement_styles_and_backspace_do_not_publish() {
    let mut h = harness();
    begin(&mut h, "Original");
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.event(Event::Text("Café".into()));
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some("Café"));
    h.key_press(Key::Backspace);
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some("Caf"));
    h.key_press_modifiers(Modifiers::COMMAND, Key::B);
    h.run();
    assert!(h.state().text.style.bold);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().editor.tools.is_editing_text());
    assert!(!h.state().unsaved());
}

#[test]
fn new_discard_prompt_and_color_dialog_preserve_unpublished_text_on_cancel() {
    let mut h = harness();
    begin(&mut h, "Keep this draft");
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    assert!(h.query_by_label("Discard and continue").is_some());
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(
        h.state().editor.tools.text_content(),
        Some("Keep this draft")
    );
    h.get_by_label("Edit colors…").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(
        h.state().editor.tools.text_content(),
        Some("Keep this draft")
    );
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    h.key_press_modifiers(Modifiers::COMMAND, Key::N);
    h.run();
    h.get_by_label("Discard and continue").click();
    h.run();
    assert!(!h.state().editor.tools.is_editing_text());
    assert!(!h.state().unsaved());
}

#[test]
fn font_popup_typing_and_escape_do_not_edit_or_cancel_canvas_text() {
    let mut h = harness();
    begin(&mut h, "Canvas text");
    h.get_by_label("Font family").click();
    h.run();
    h.get_by_label("Search fonts").type_text("Selawik");
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some("Canvas text"));
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().editor.tools.is_editing_text());
    assert_eq!(h.state().editor.tools.text_content(), Some("Canvas text"));
}

#[test]
fn ime_commit_is_inserted_once_and_download_does_not_discard_the_draft() {
    let mut h = harness();
    begin(&mut h, "Caf");
    h.event(Event::Ime(egui::ImeEvent::Preedit {
        text: "é".into(),
        active_range_chars: None,
    }));
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some("Caf"));
    h.event(Event::Ime(egui::ImeEvent::Commit("é".into())));
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some("Café"));
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some("Café"));
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    assert!(h.state().unsaved());
}

#[test]
fn focus_loss_and_pointer_gone_keep_text_while_escape_cancels() {
    let mut h = harness();
    begin(&mut h, "Do not lose this");
    h.event(Event::PointerGone);
    h.run();
    assert_eq!(
        h.state().editor.tools.text_content(),
        Some("Do not lose this")
    );
    h.event(Event::WindowFocused(false));
    h.run();
    assert_eq!(
        h.state().editor.tools.text_content(),
        Some("Do not lose this")
    );
    h.event(Event::WindowFocused(true));
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().unsaved());
}

#[test]
fn text_and_place_in_one_frame_capture_the_last_characters_in_order() {
    let mut h = harness();
    begin(&mut h, "Prefix");
    h.state_mut().editor.tools.type_text(" Last");
    let expected = h.state().editor.copy_snapshot().unwrap();
    for _ in 0..5 {
        h.state_mut().editor.tools.backspace();
    }
    h.event(Event::Text(" Last".into()));
    h.event(Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    });
    h.run();
    assert!(!h.state().editor.tools.is_editing_text());
    assert_eq!(
        h.state().editor.session.document().state_id,
        expected.state_id
    );
}

#[test]
fn invalid_preview_and_denied_place_do_not_erase_the_draft() {
    let mut h = harness();
    begin(&mut h, "Recoverable draft");
    h.state_mut().editor.session.set_staged_pixel_budget(0);
    h.get_by_label("Place text").click();
    h.run();
    assert_eq!(
        h.state().editor.tools.text_content(),
        Some("Recoverable draft")
    );
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    h.state_mut().text.style.family = "x".repeat(257);
    h.run();
    assert_eq!(
        h.state().editor.tools.text_content(),
        Some("Recoverable draft")
    );
    assert!(h.state().unsaved());
}
