use super::*;
use paintcraft::settings::{Appearance, ClassicVariant, Experience, Settings};

#[test]
fn opacity_text_cancellation_preserves_layer_and_canvas_drafts() {
    for (experience, ppp, appearance) in [
        (Experience::Classic, 1.0, Appearance::Light),
        (Experience::Classic, 2.0, Appearance::Dark),
        (Experience::Studio, 1.0, Appearance::Light),
        (Experience::Studio, 2.0, Appearance::Dark),
    ] {
        let dir = scratch("layer-opacity-cancel");
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
        let opacity = h.state().editor().document().layers[&layer].opacity;
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let published = publications(&h);
        h.get_by_label("Opacity").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Opacity").type_text("72");
        h.run();
        assert_eq!(publications(&h), published);
        h.key_press(Key::Escape);
        h.run();
        assert_eq!(
            h.state().editor().document().layers[&layer].opacity,
            opacity
        );
        assert!(
            matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Keep me")
        );
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(publications(&h), published);
        // Cancel the preview separately before an ordinary accepted property edit.
        h.key_press(Key::Escape);
        h.run();
        assert!(!h.state().editor().is_editing_text());
        h.get_by_label("Opacity").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Opacity").type_text("93");
        h.run();
        assert_eq!(publications(&h), published);
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(h.state().editor().document().layers[&layer].opacity, 93);
        assert_eq!(publications(&h), published + 1);
        h.get_by_role_and_label(eframe::egui::Role::Button, "Undo")
            .click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        let before_drag = publications(&h);
        let start = h.get_by_label("Opacity").rect().center();
        let end = start - Vec2::new(24.0, 0.0);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(start), press(start)]);
        h.run();
        h.event(Event::PointerMoved(end));
        h.run();
        assert_eq!(publications(&h), before_drag);
        assert_eq!(
            h.state().editor().document().layers[&layer].opacity,
            opacity
        );
        h.event(release(end));
        h.run();
        assert!(h.state().editor().document().layers[&layer].opacity < opacity);
        assert_eq!(publications(&h), before_drag + 1);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
