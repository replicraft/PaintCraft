use super::*;
use paintcraft::settings::{Appearance, ClassicVariant, Experience, Settings};

#[test]
fn escape_discards_a_layer_name_draft_without_publishing_or_cancelling_canvas_text() {
    for (experience, ppp, appearance) in [
        (Experience::Classic, 1.0, Appearance::Light),
        (Experience::Classic, 2.0, Appearance::Dark),
        (Experience::Studio, 1.0, Appearance::Light),
        (Experience::Studio, 2.0, Appearance::Dark),
    ] {
        let dir = scratch("layer-rename-cancel");
        Settings {
            experience,
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        with_layer(&mut h);
        h.get_by_role_and_label(eframe::egui::Role::Button, "Text")
            .click();
        h.run();
        let point = h.state().canvas_rect().unwrap().center();
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point), release(point)]);
        h.run();
        h.event(Event::Text("Keep me".into()));
        h.run();
        assert!(h.state().editor().is_editing_text());
        let layer = h.state().editor().target().unwrap();
        let name = h.state().editor().document().layers[&layer]
            .name
            .to_string();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let published = publications(&h);
        h.get_by_label("Layer name").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Layer name")
            .type_text("Discard this rename");
        h.run();
        assert_eq!(publications(&h), published);
        h.key_press(Key::Escape);
        h.run();
        assert_eq!(&*h.state().editor().document().layers[&layer].name, name);
        assert_eq!(
            h.get_by_label("Layer name")
                .accesskit_node()
                .value()
                .as_deref(),
            Some(name.as_str())
        );
        assert!(
            matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Keep me")
        );
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(publications(&h), published);
        // After the property field releases focus, a separate Escape belongs to
        // the canvas. Cancel its preview before checking an ordinary rename.
        h.key_press(Key::Escape);
        h.run();
        assert!(!h.state().editor().is_editing_text());
        assert_eq!(publications(&h), published);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        h.get_by_label("Layer name").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Layer name").type_text("Sky");
        h.run();
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(&*h.state().editor().document().layers[&layer].name, "Sky");
        assert_eq!(publications(&h), published + 1);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
