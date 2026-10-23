use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn classic_top_actions_and_settings_remain_reachable_in_narrow_windows() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(700.0, 500.0), 2.0, Appearance::Dark),
        (Vec2::new(480.0, 500.0), 1.0, Appearance::Light),
        (Vec2::new(360.0, 500.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("compact-classic-chrome");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, app: &mut App| {
                    app.ui(ui);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx().request_discard("test: repeated top chrome layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for label in [
            "File",
            "Edit",
            "View",
            "Save",
            "Share",
            "Undo",
            "Redo",
            "Clear Canvas",
            "Interface",
            "Settings…",
        ] {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            assert!(
                rect.min.x >= 0.0
                    && rect.max.x <= size.x
                    && rect.min.y >= 0.0
                    && rect.max.y < h.state().canvas_viewport().unwrap().top(),
                "{label} outside {size:?}: {rect:?}"
            );
        }
        let labels = [
            "File",
            "Edit",
            "View",
            "Save",
            "Share",
            "Undo",
            "Redo",
            "Clear Canvas",
            "Interface",
            "Settings…",
        ];
        for (index, left) in labels.iter().enumerate() {
            for right in &labels[index + 1..] {
                let a = h.get_by_role_and_label(Role::Button, left).rect();
                let b = h.get_by_role_and_label(Role::Button, right).rect();
                assert!(
                    !a.intersects(b),
                    "{left} and {right} overlap at {size:?}: {a:?}, {b:?}"
                );
            }
        }
        let settings = h.get_by_label("Settings…").rect();
        let changer = h.get_by_role_and_label(Role::Button, "Interface").rect();
        assert!(changer.right() <= settings.left());
        assert!((changer.center().y - settings.center().y).abs() < 2.0);
        for (left, right) in [("File", "Edit"), ("Edit", "View")] {
            assert!(
                h.get_by_label(right).rect().left() - h.get_by_label(left).rect().right() >= 12.0
            );
        }
        let save = h.get_by_label("Save").rect();
        let file = h.get_by_role_and_label(Role::Button, "File").rect();
        if size.x < 600.0 {
            assert!(save.top() >= file.bottom());
        } else {
            assert!((save.center().y - file.center().y).abs() < 2.0);
        }
        h.run_steps(8);
        assert_eq!(h.get_by_label("Settings…").rect(), settings);
        assert_eq!(h.get_by_label("Save").rect(), save);
        h.get_by_label("Settings…").click();
        h.run();
        assert!(h.query_by_label("Appearance").is_some());
        h.get_by_label("Cancel").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "Interface").click();
        h.run();
        h.get_by_label("Change Classic skin…").click();
        h.run();
        assert!(h.state().picker_open());
        h.get_by_label("Cancel").click();
        h.run();
        assert!(!h.state().picker_open());
        if size.x < 600.0 {
            h.get_by_role_and_label(Role::Button, "View").click();
            h.run();
            h.get_by_label("Automatically hide toolbar").scroll_to_me();
            h.run();
            assert!(
                h.get_by_role_and_label(Role::Button, "View")
                    .query_by_label_contains("Actual size")
                    .is_none(),
                "scrolling must not hover an off-screen Zoom entry"
            );
            let hide = h.get_by_label("Automatically hide toolbar").rect();
            assert!(
                hide.min.y >= 0.0 && hide.max.y <= size.y,
                "hide row outside {size:?}: {hide:?}"
            );
            h.get_by_label("Automatically hide toolbar").click();
            h.run();
            assert!(
                Settings::load(&dir).0.classic_toolbar_auto_hide,
                "hide action did not activate at {size:?}, row={hide:?}"
            );
            let point = h.state().canvas_viewport().unwrap().center();
            h.event(Event::PointerMoved(point));
            h.run();
            assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_none());
            assert_eq!(h.get_by_label("Save").rect(), save);
            assert_eq!(h.get_by_label("Settings…").rect(), settings);
            h.get_by_label("Save").hover();
            h.run();
            assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_some());
            assert_eq!(h.get_by_label("Save").rect(), save);
        }
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
