use super::studio::studio_harness;
use super::*;

fn edit_name(h: &mut Harness<App>, name: &str) {
    h.get_by_label("Name").focus();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    h.run();
    h.get_by_label("Name").type_text(name);
    h.run();
}

#[test]
fn properties_apply_is_one_transaction_with_exact_undo_redo_and_save_reopen() {
    let (mut h, dir) = studio_harness(true);
    let source = h.state().editor().target().unwrap();
    let original = h.state().editor().document_snapshot();
    let before = publications(&h);
    h.key_press(Key::F4);
    h.run();
    edit_name(&mut h, "Sky");
    h.get_by_label("Visible").click();
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::Slider, "Layer opacity")
        .set_value_accesskit(93.0);
    h.run();
    assert!(h.state().editor().document().same_semantics(&original));
    assert_eq!(publications(&h), before);
    h.get_by_label("Apply").click();
    h.run();
    let layer = &h.state().editor().document().layers[&source];
    assert_eq!(
        (&*layer.name, layer.visible, layer.opacity),
        ("Sky", false, 93)
    );
    assert_eq!(publications(&h), before + 1);
    assert_eq!(
        h.state()
            .editor()
            .history_view()
            .entries
            .last()
            .unwrap()
            .label
            .text(),
        "Layer properties"
    );
    let applied = h.state().editor().document_snapshot();
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&original));
    h.get_by_label("Redo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&applied));
    let file = dir.join("properties.pcnd");
    h.state_mut().set_path_text(&file.to_string_lossy());
    h.get_by_label("Save").click();
    h.run();
    let reopened = paintcraft::controller::Editor::open(&file).unwrap();
    assert!(reopened.document().same_semantics(&applied));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancel_and_escape_drop_staged_properties_and_modal_blocks_new_documents() {
    for escape in [false, true] {
        let (mut h, dir) = studio_harness(true);
        let original = h.state().editor().document_snapshot();
        let before = publications(&h);
        h.get_by_label("Layer properties…").click();
        h.run();
        edit_name(&mut h, "Discarded");
        h.get_by_label("Visible").click();
        h.run();
        h.event(key(Key::N, Modifiers::COMMAND));
        h.run();
        assert_eq!(h.state().document_count(), 1);
        if escape {
            h.key_press(Key::Escape);
        } else {
            h.get_by_label("Cancel").click();
        }
        h.run();
        assert!(h.query_by_label("Apply").is_none());
        assert!(h.state().editor().document().same_semantics(&original));
        assert_eq!(publications(&h), before);
        assert!(!h.state().editor().is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn unchanged_properties_do_not_create_history_and_invalid_names_cannot_apply() {
    let (mut h, dir) = studio_harness(false);
    let before = publications(&h);
    h.key_press(Key::F4);
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(publications(&h), before);
    assert!(!h.state().editor().is_dirty());
    h.key_press(Key::F4);
    h.run();
    edit_name(&mut h, &"é".repeat(1025));
    h.get_by_label("Apply").click();
    h.run();
    assert!(h.query_by_label("Cancel").is_some());
    assert_eq!(publications(&h), before);
    h.get_by_label("Cancel").click();
    h.run();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_layer_properties_for_review() {
    let (mut h, dir) = studio_harness(false);
    h.key_press(Key::F4);
    h.run();
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-layer-properties.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
