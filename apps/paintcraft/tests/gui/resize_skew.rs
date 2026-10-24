use super::*;
use eframe::egui::{Role, accesskit::Toggled};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn value(h: &Harness<App>, label: &str) -> f64 {
    h.get_by_role_and_label(Role::SpinButton, label)
        .accesskit_node()
        .numeric_value()
        .unwrap()
}

fn set(h: &mut Harness<App>, label: &str, number: f64) {
    h.get_by_role_and_label(Role::SpinButton, label)
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::SpinButton, label)
        .set_value_accesskit(number);
    h.run();
}

fn choose_unit(h: &mut Harness<App>, label: &str) {
    h.get_by_role_and_label(Role::RadioButton, label)
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::RadioButton, label).click();
    h.run();
}

#[test]
fn resize_skew_preview_stages_units_link_and_angles_without_publishing() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("resize-skew-draft");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        with_layer(&mut h);
        h.get_by_role_and_label(Role::Button, "Layers").click();
        h.run();
        h.set_size(size);
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        h.get_by_role_and_label(Role::Button, "Resize and skew")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Resize and skew")
            .click();
        h.run();
        assert_eq!(value(&h, "Resize horizontal"), 100.0);
        assert_eq!(value(&h, "Resize vertical"), 100.0);
        assert!(
            h.get_by_role_and_label(Role::Button, "OK")
                .accesskit_node()
                .is_disabled()
        );
        set(&mut h, "Resize horizontal", 50.0);
        assert_eq!(value(&h, "Resize vertical"), 50.0);
        choose_unit(&mut h, "Pixels");
        assert_eq!(value(&h, "Resize horizontal"), 500.0);
        assert_eq!(value(&h, "Resize vertical"), 243.0);
        set(&mut h, "Resize horizontal", 1.0);
        assert_eq!(value(&h, "Resize horizontal"), 1.0);
        assert_eq!(value(&h, "Resize vertical"), 1.0);
        choose_unit(&mut h, "Percentage");
        assert!((value(&h, "Resize horizontal") - 0.1).abs() < 0.000001);
        assert!((value(&h, "Resize vertical") - 0.1).abs() < 0.000001);
        choose_unit(&mut h, "Pixels");
        assert_eq!(value(&h, "Resize horizontal"), 1.0);
        assert_eq!(value(&h, "Resize vertical"), 1.0);
        set(&mut h, "Resize horizontal", 500.0);

        h.get_by_role_and_label(Role::Button, "Maintain aspect ratio")
            .click();
        h.run();
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "Maintain aspect ratio")
                .accesskit_node()
                .toggled(),
            Some(Toggled::False)
        );
        set(&mut h, "Resize horizontal", 400.0);
        assert_eq!(value(&h, "Resize vertical"), 243.0);
        h.get_by_role_and_label(Role::Button, "Maintain aspect ratio")
            .click();
        h.run();
        set(&mut h, "Resize horizontal", 600.0);
        assert_eq!(value(&h, "Resize vertical"), 292.0);
        choose_unit(&mut h, "Percentage");
        assert_eq!(value(&h, "Resize horizontal"), 60.0);
        assert_eq!(value(&h, "Resize vertical"), 60.0);
        set(&mut h, "Skew horizontal", 15.0);
        set(&mut h, "Skew vertical", -20.0);
        assert_eq!(value(&h, "Skew horizontal"), 15.0);
        assert_eq!(value(&h, "Skew vertical"), -20.0);
        let footer = h.get_by_role_and_label(Role::Button, "Cancel").rect();
        assert!(footer.min.y >= 0.0 && footer.max.y <= size.y);
        h.get_by_role_and_label(Role::Button, "OK").click();
        h.run();
        assert!(h.query_by_role_and_label(Role::Button, "Cancel").is_some());
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::Z]);
        h.run();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        h.get_by_role_and_label(Role::Button, "Cancel").click();
        h.run();
        assert!(h.query_by_role_and_label(Role::Button, "OK").is_none());
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn resize_skew_preview_close_escape_and_reopening_keep_document_and_preferences() {
    let dir = scratch("resize-skew-close");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 2.0, true);
    h.set_size(Vec2::new(360.0, 260.0));
    h.run();
    h.ctx.style_mut_of(h.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    let saved = std::fs::read(dir.join("settings.json")).unwrap();
    for keyboard in [false, true] {
        h.get_by_role_and_label(Role::Button, "Resize and skew")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Resize and skew")
            .hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, "Resize and skew")
                .count(),
            1
        );
        h.get_by_role_and_label(Role::Button, "Resize and skew")
            .click();
        h.run();
        assert_eq!(value(&h, "Resize horizontal"), 100.0);
        let header = h
            .get_by_role_and_label(Role::Button, "Close resize and skew")
            .rect();
        let footer = h.get_by_role_and_label(Role::Button, "Cancel").rect();
        assert_eq!(header.size(), Vec2::splat(40.0));
        assert!(header.min.y >= 0.0 && footer.max.y <= 260.0);
        let notice = h.get_by_role_and_label(Role::Label, "Preview only").rect();
        assert!(notice.min.y >= 0.0 && notice.max.y < footer.min.y);
        set(&mut h, "Skew vertical", 30.0);
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "Close resize and skew")
                .rect(),
            header
        );
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "Cancel").rect(),
            footer
        );
        if keyboard {
            h.key_press(Key::Escape);
        } else {
            h.get_by_role_and_label(Role::Button, "Close resize and skew")
                .hover();
            h.run();
            assert_eq!(
                h.query_all_by_role_and_label(Role::Label, "Close resize and skew")
                    .count(),
                1
            );
            h.get_by_role_and_label(Role::Button, "Close resize and skew")
                .click();
        }
        h.run();
        assert!(h.query_by_role_and_label(Role::Button, "OK").is_none());
    }
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::W]);
    h.run();
    assert_eq!(h.state().document_count(), 0);
    assert!(
        h.get_by_role_and_label(Role::Button, "Resize and skew")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_role_and_label(Role::Button, "Resize and skew")
        .click();
    h.run();
    assert!(h.query_by_role_and_label(Role::Button, "OK").is_none());
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "Explicit Wgpu review of the Resize and skew presentation preview"]
fn export_resize_skew_preview() {
    let output = std::env::temp_dir().join("paintcraft-resize-skew-preview");
    std::fs::create_dir_all(&output).unwrap();
    for (theme, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
        let dir = scratch("resize-skew-gpu");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        for (name, size) in [
            ("desktop", Vec2::new(1360.0, 900.0)),
            ("compact", Vec2::new(360.0, 360.0)),
        ] {
            h.set_size(size);
            h.run();
            h.get_by_role_and_label(Role::Button, "Resize and skew")
                .scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, "Resize and skew")
                .click();
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}-percentage.png")))
                .unwrap();
            h.get_by_role_and_label(Role::RadioButton, "Pixels").click();
            h.run();
            set(&mut h, "Resize horizontal", 500.0);
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}-pixels.png")))
                .unwrap();
            set(&mut h, "Skew vertical", 15.0);
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}-skew.png")))
                .unwrap();
            h.get_by_role_and_label(Role::Button, "Cancel").click();
            h.run();
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
