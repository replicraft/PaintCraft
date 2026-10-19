use super::*;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
#[test]
#[ignore = "requires GPU; exports real font text and updated Classic controls"]
fn render_font_text_for_review() {
    let dir = scratch("font-text-preview");
    let output = std::env::temp_dir().join("paintcraft-font-previews");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, name) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.get_by_label("Text").click();
        h.run();
        h.get_by_label("Font size").set_value_accesskit(36.0);
        h.run();
        let p = at(&h, 60.0, 65.0);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(p), press(p), release(p)]);
        h.run();
        h.event(Event::Text("PaintCraft — Selawik".into()));
        h.run();
        h.get_by_label("Bold").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{name}-font-text.png")))
            .unwrap();
        h.event(key(Key::Enter, Modifiers::COMMAND));
        h.run();
        assert!(h.state().editor().is_dirty());
        h.get_by_label("Pencil").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{name}-pencil-size.png")))
            .unwrap();
    }
    std::fs::remove_dir_all(dir).unwrap();
}
