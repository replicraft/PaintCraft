use crate::WebApp;
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_tools::Tool;

fn harness() -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h
}
fn shortcut(h: &mut Harness<WebApp>, modifiers: Modifiers, key: Key) {
    h.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    h.step();
    h.event(Event::Key {
        key,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers,
    });
    h.step();
}
fn button(h: &mut Harness<WebApp>, pressed: bool) {
    let pos = h.state().canvas.rect.unwrap().center();
    h.event(Event::PointerMoved(pos));
    h.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
    h.step();
}

#[test]
fn canvas_publishes_once_on_release_and_keyboard_undo_redo_works() {
    let mut h = harness();
    h.state_mut().controls.tool = Tool::Brush;
    let before = h.state().editor.session.ticket();
    button(&mut h, true);
    assert!(h.state().editor.tools.gesture_in_progress());
    assert_eq!(h.state().editor.session.ticket(), before);
    button(&mut h, false);
    assert_eq!(h.state().editor.session.undo_depth(), 1);
    let painted = h.state().editor.session.ticket().state_id;
    assert_ne!(painted, before.state_id);
    shortcut(&mut h, Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().editor.session.ticket().state_id, before.state_id);
    shortcut(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    h.run();
    assert_eq!(h.state().editor.session.ticket().state_id, painted);
}

#[test]
fn new_requires_discard_and_cancel_keeps_the_document() {
    let mut h = harness();
    button(&mut h, true);
    button(&mut h, false);
    let before = h.state().editor.session.ticket();
    shortcut(&mut h, Modifiers::COMMAND, Key::N);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().editor.session.ticket(), before);
    shortcut(&mut h, Modifiers::COMMAND, Key::N);
    h.run();
    h.get_by_label("Discard and create new").click();
    h.run();
    assert!(!h.state().editor.session.is_dirty());
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    assert_ne!(h.state().editor.session.ticket().state_id, before.state_id);
}

#[test]
fn escape_and_pointer_loss_cancel_without_history() {
    let mut h = harness();
    let before = h.state().editor.session.ticket();
    button(&mut h, true);
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().editor.tools.gesture_in_progress());
    button(&mut h, false);
    button(&mut h, true);
    h.event(Event::PointerGone);
    h.step();
    assert!(!h.state().editor.tools.gesture_in_progress());
    assert_eq!(h.state().editor.session.ticket(), before);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}

#[test]
fn layers_add_delete_and_undo_retarget_paint_safely() {
    let mut h = harness();
    h.get_by_role_and_label(egui::Role::Button, "Layers")
        .click();
    h.run();
    h.get_by_label("Add layer").click();
    h.run();
    let target = h.state().editor.target.unwrap();
    assert_eq!(h.state().editor.session.document().roots.len(), 2);
    h.get_by_label("Delete layer").click();
    h.run();
    assert_ne!(h.state().editor.target, Some(target));
    shortcut(&mut h, Modifiers::COMMAND, Key::Z);
    h.run();
    assert!(
        h.state()
            .editor
            .session
            .document()
            .layers
            .contains_key(&target)
    );
    button(&mut h, true);
    button(&mut h, false);
    assert!(h.state().editor.session.is_dirty());
}
