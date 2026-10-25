use super::*;
use eframe::egui::Role;

#[test]
fn studio_menu_and_shortcut_flatten_once_and_undo_restores_the_stack() {
    for multiple in [false, true] {
        let (mut h, dir) = super::studio::studio_harness(multiple);
        h.set_size(Vec2::new(1480.0, 960.0));
        h.run();
        with_layer(&mut h);
        drag(&mut h, (180.0, 10.0), (240.0, 10.0));
        let source = h.state().editor().document_snapshot();
        assert!(source.roots.len() > 1);
        let pixels = paintcraft_render::render_flat(&source).unwrap();
        let depth = h.state().editor().undo_depth();
        let before = publications(&h);
        h.get_by_role_and_label(Role::Button, "Image").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "Image")
            .get_by_label_contains("Flatten image")
            .click();
        h.run();
        assert_eq!(publications(&h), before + 1);
        assert_eq!(h.state().editor().document().roots.len(), 1);
        assert_eq!(h.state().editor().undo_depth(), depth + 1);
        assert_eq!(
            paintcraft_render::render_flat(h.state().editor().document()).unwrap(),
            pixels
        );
        h.event(key(Key::Z, Modifiers::COMMAND));
        h.event(key_up(Key::Z, Modifiers::COMMAND));
        h.run();
        assert!(h.state().editor().document().same_semantics(&source));
        h.event(key(Key::F, Modifiers::COMMAND | Modifiers::SHIFT));
        h.event(key_up(Key::F, Modifiers::COMMAND | Modifiers::SHIFT));
        h.run();
        assert_eq!(h.state().editor().document().roots.len(), 1);
        assert_eq!(h.state().editor().undo_depth(), depth + 1);
        let one = publications(&h);
        h.event(key(Key::F, Modifiers::COMMAND | Modifiers::SHIFT));
        h.event(key_up(Key::F, Modifiers::COMMAND | Modifiers::SHIFT));
        h.run();
        assert_eq!(publications(&h), one, "single layer disables flatten");
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
