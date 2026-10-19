use super::studio::studio_harness;
use super::*;
use eframe::egui::Role;

fn menu_action(h: &mut Harness<App>, label: &str) {
    h.get_by_role_and_label(Role::Button, "Layers").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Layers")
        .get_by_label_contains(label)
        .click();
    h.run();
}

#[test]
fn duplicate_from_menu_selects_an_independent_copy_with_one_history_entry() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    drag(&mut h, (180.0, 10.0), (240.0, 10.0));
    let source = h.state().editor().target().unwrap();
    let original = h.state().editor().document().layers[&source].clone();
    let before = publications(&h);
    menu_action(&mut h, "Duplicate layer");
    let copy = h.state().editor().target().unwrap();
    assert_ne!(copy, source);
    assert_eq!(publications(&h), before + 1);
    assert_eq!(h.state().editor().document().layers[&source], original);
    assert_eq!(
        h.state().editor().document().layers[&copy].raster,
        original.raster
    );
    assert_eq!(
        h.state()
            .editor()
            .history_view()
            .entries
            .last()
            .unwrap()
            .label
            .text(),
        "Duplicate layer"
    );
    drag(&mut h, (180.0, 30.0), (240.0, 30.0));
    assert_eq!(h.state().editor().document().layers[&source], original);
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(
        h.state().editor().document().layers[&copy].raster,
        original.raster
    );
    h.get_by_label("Undo").click();
    h.run();
    assert!(!h.state().editor().document().layers.contains_key(&copy));
    h.get_by_label("Redo").click();
    h.run();
    assert_eq!(
        h.state().editor().document().layers[&copy].raster,
        original.raster
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn layer_shortcuts_navigate_without_history_then_toggle_duplicate_and_reorder() {
    let (mut h, dir) = studio_harness(true);
    let bottom = h.state().editor().target().unwrap();
    h.event(key(Key::N, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    assert_eq!(h.state().document_count(), 1);
    let top = h.state().editor().target().unwrap();
    assert_ne!(top, bottom);
    let before = publications(&h);
    h.event(key(Key::PageDown, Modifiers::ALT));
    h.run();
    assert_eq!(h.state().editor().target(), Some(bottom));
    assert_eq!(publications(&h), before);
    h.event(key(
        Key::Comma,
        if cfg!(target_os = "macos") {
            Modifiers::COMMAND | Modifiers::SHIFT
        } else {
            Modifiers::COMMAND
        },
    ));
    h.run();
    assert!(!h.state().editor().document().layers[&bottom].visible);
    menu_action(&mut h, "Move layer to top");
    assert_eq!(h.state().editor().document().roots, vec![top, bottom]);
    assert_eq!(h.state().editor().target(), Some(bottom));
    h.event(key(Key::D, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    let copy = h.state().editor().target().unwrap();
    assert_ne!(copy, bottom);
    assert!(!h.state().editor().document().layers[&copy].visible);
    h.event(key(Key::Delete, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    assert!(!h.state().editor().document().layers.contains_key(&copy));
    assert_eq!(publications(&h), before + 4);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn deleting_the_last_layer_disables_layer_specific_commands_but_add_still_works() {
    let (mut h, dir) = studio_harness(false);
    menu_action(&mut h, "Delete layer");
    assert_eq!(h.state().editor().target(), None);
    let before = publications(&h);
    h.event(key(Key::D, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    assert_eq!(publications(&h), before);
    menu_action(&mut h, "Add new layer");
    assert!(h.state().editor().target().is_some());
    assert_eq!(h.state().editor().layers().len(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
