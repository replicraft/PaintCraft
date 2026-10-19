use super::*;

fn enter_percent(h: &mut Harness<App>, text: &str) {
    h.get_by_label("Zoom percentage").click();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    h.run();
    h.get_by_label("Zoom percentage").type_text(text);
    h.run();
    h.key_press(Key::Enter);
    h.run();
}

#[test]
fn classic_percentage_presets_fit_and_shortcuts_preserve_document_and_ui_scale() {
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        let before = h.state().editor().document_snapshot();
        let depth = h.state().editor().undo_depth();
        enter_percent(&mut h, "73%");
        assert!((h.state().canvas_rect().unwrap().width() * ppp - 730.0).abs() < 0.1);
        enter_percent(&mut h, "oops");
        assert!((h.state().canvas_rect().unwrap().width() * ppp - 730.0).abs() < 0.1);
        h.get_by_label("Zoom presets").click();
        h.run();
        h.get_by_label("12.5%").click();
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width() * ppp, 125.0);
        h.event(key(Key::Num0, Modifiers::COMMAND));
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width() * ppp, 1000.0);
        h.get_by_label("Zoom to window").click();
        h.run();
        let canvas = h.state().canvas_rect().unwrap();
        let viewport = h.state().canvas_viewport().unwrap();
        assert!(canvas.width() <= viewport.width() && canvas.height() <= viewport.height());
        assert!(h.state().editor().document().same_semantics(&before));
        assert_eq!(h.state().editor().undo_depth(), depth);
        assert!(!h.state().editor().is_dirty());
    }
}
