use super::*;
use paintcraft::settings::{ClassicVariant, Settings};

#[test]
fn clear_button_routes_once_at_each_display_scale_and_restores_the_image_on_undo() {
    for ppp in [1.0, 2.0] {
        let dir = scratch("clear-canvas");
        Settings {
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.get_by_label("Pencil").click();
        h.run();
        let point = h.state().canvas_rect().unwrap().min + Vec2::splat(5.5);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point), release(point)]);
        h.run();
        let source = h.state().editor().document_snapshot();
        assert_eq!(top_pixel(&h, 5, 5), Rgba([0, 0, 0, 255]));
        let depth = h.state().editor().undo_depth();
        let before = publications(&h);
        h.get_by_role_and_label(eframe::egui::Role::Button, "Clear Canvas")
            .click();
        h.run();
        assert_eq!(top_pixel(&h, 5, 5), Rgba([255; 4]));
        assert_eq!(publications(&h), before + 1);
        assert_eq!(h.state().editor().undo_depth(), depth + 1);
        h.get_by_label("Undo").click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&source));
        // The Edit menu reaches the same action as the toolbar button.
        h.get_by_role_and_label(eframe::egui::Role::Button, "Edit")
            .click();
        h.run();
        let menu: Vec<_> = h
            .query_all_by_role_and_label(eframe::egui::Role::Button, "Clear Canvas")
            .collect();
        assert_eq!(menu.len(), 2);
        menu.last().unwrap().click();
        h.run();
        assert_eq!(top_pixel(&h, 5, 5), Rgba([255; 4]));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
