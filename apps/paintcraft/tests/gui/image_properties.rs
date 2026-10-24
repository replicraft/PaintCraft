use super::*;
use eframe::egui::{Role, accesskit::Toggled};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn open(h: &mut Harness<App>, keyboard: bool) {
    if keyboard {
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::E]);
        h.run();
    } else {
        h.get_by_role_and_label(Role::Button, "File").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "File")
            .get_by_label_contains("Image properties")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "File")
            .get_by_label_contains("Image properties")
            .click();
        h.run();
    }
    assert!(
        h.query_by_role_and_label(Role::Button, "Close image properties")
            .is_some()
    );
}

fn dimension(h: &mut Harness<App>, label: &str, value: f64) {
    h.get_by_role_and_label(Role::SpinButton, label)
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::SpinButton, label)
        .set_value_accesskit(value);
    h.run();
}

#[test]
fn properties_menu_and_shortcut_resize_once_preserve_pixels_and_undo_redo() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.set_size(Vec2::new(1360.0, 900.0));
    h.run();
    drag(&mut h, (900.0, 400.0), (900.0, 400.0));
    assert_eq!(top_pixel(&h, 900, 400), Rgba([0, 0, 0, 255]));
    let snapshot = h.state().editor().document_snapshot();
    for keyboard in [false, true] {
        open(&mut h, keyboard);
        let before = publications(&h);
        dimension(&mut h, "Width", 640.0);
        dimension(&mut h, "Height", 480.0);
        h.get_by_role_and_label(Role::Button, "OK").click();
        h.run();
        assert_eq!(h.state().editor().document().canvas, (640, 480));
        assert_eq!(publications(&h), before + 1);
        assert_eq!(
            top_pixel(&h, 900, 400),
            Rgba([0, 0, 0, 255]),
            "canvas bounds preserve outside data"
        );
        h.get_by_role_and_label(Role::Button, "Undo").click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        h.get_by_role_and_label(Role::Button, "Redo").click();
        h.run();
        assert_eq!(h.state().editor().document().canvas, (640, 480));
        assert_eq!(top_pixel(&h, 900, 400), Rgba([0, 0, 0, 255]));
        h.get_by_role_and_label(Role::Button, "Undo").click();
        h.run();
    }
}

#[test]
fn properties_cancel_preserves_text_draft_history_and_preferences_and_gates_missing_metadata() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(360.0, 260.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("image-properties-cancel");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.get_by_role_and_label(Role::Button, "Text").click();
        h.run();
        drag(&mut h, (50.0, 50.0), (50.0, 50.0));
        h.event(Event::Text("Keep this editable".into()));
        h.run();
        h.set_size(size);
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        for close in ["Cancel", "Close image properties"] {
            open(&mut h, false);
            h.event(Event::Text("Ignore while modal".into()));
            h.key_press(Key::Backspace);
            h.run();
            assert!(
                matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text {text,..}) if text=="Keep this editable"),
                "dialog input must not reach the canvas text draft"
            );
            assert!(
                h.query_by_role_and_label(Role::Label, "Not available")
                    .is_some()
            );
            for label in ["Inches", "Centimeters", "Black and white"] {
                assert!(
                    h.get_by_role_and_label(Role::RadioButton, label)
                        .accesskit_node()
                        .is_disabled()
                );
            }
            assert_eq!(
                h.get_by_role_and_label(Role::RadioButton, "Pixels")
                    .accesskit_node()
                    .toggled(),
                Some(Toggled::True)
            );
            assert_eq!(
                h.get_by_role_and_label(Role::RadioButton, "Color")
                    .accesskit_node()
                    .toggled(),
                Some(Toggled::True)
            );
            let header = h
                .get_by_role_and_label(Role::Button, "Close image properties")
                .rect();
            let footer = h.get_by_role_and_label(Role::Button, "OK").rect();
            assert_eq!(header.size(), Vec2::splat(40.0));
            assert!(header.min.y >= 0.0 && footer.max.y <= size.y);
            dimension(&mut h, "Width", 777.0);
            dimension(&mut h, "Height", 512.0);
            h.get_by_role_and_label(Role::RadioButton, "Color")
                .scroll_to_me();
            h.run();
            assert_eq!(
                h.get_by_role_and_label(Role::Button, "Close image properties")
                    .rect(),
                header
            );
            assert_eq!(h.get_by_role_and_label(Role::Button, "OK").rect(), footer);
            h.get_by_role_and_label(Role::Button, close).click();
            h.run();
            assert!(h.query_by_role_and_label(Role::Button, "OK").is_none());
            assert!(h.state().editor().is_editing_text());
            assert!(
                matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text {text,..}) if text=="Keep this editable")
            );
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn properties_ok_without_changes_keeps_cropped_layers_and_the_file_clean() {
    let dir = scratch("image-properties-noop");
    let file = dir.join("cropped-layer.pcnd");
    let mut doc = paintcraft_document::new_document(
        32,
        16,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    // A valid native document can have a layer smaller than its canvas.
    doc.canvas = (1000, 486);
    std::fs::write(&file, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    let mut h = harness_with(App::new(Some(file.clone())), 1.0, true);
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    assert!(!h.state().editor().is_dirty());
    open(&mut h, true);
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run();
    assert!(
        h.state().editor().document().same_semantics(&snapshot),
        "unchanged properties must not expand cropped layers"
    );
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(!h.state().editor().is_dirty());
    assert_eq!(h.state().editor().path(), Some(file.as_path()));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn properties_long_file_name_stays_bounded_and_has_one_tooltip() {
    let dir = scratch("image-properties-name");
    let name = format!("{}.pcnd", "Long-PaintCraft-name-".repeat(10));
    let file = dir.join(&name);
    let doc = App::new(None).editor().document_snapshot();
    std::fs::write(&file, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    let mut h = harness_with(App::new(Some(file.clone())), 1.0, true);
    h.set_size(Vec2::new(360.0, 360.0));
    h.run();
    h.ctx.style_mut_of(h.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    open(&mut h, false);
    let label = h.get_by_role_and_label(Role::Label, &name);
    let rect = label.rect();
    assert!(rect.min.x >= 0.0 && rect.max.x <= 360.0);
    label.hover();
    h.run();
    assert_eq!(
        h.query_all_by_role_and_label(Role::Label, &name).count(),
        2,
        "one label and one full-name tooltip"
    );
    h.get_by_role_and_label(Role::Button, "Cancel").click();
    h.run();
    assert_eq!(h.state().editor().path(), Some(file.as_path()));
    assert!(!h.state().editor().is_dirty());
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "Explicit Wgpu review of Classic Image properties"]
fn export_image_properties() {
    let output = std::env::temp_dir().join("paintcraft-image-properties");
    std::fs::create_dir_all(&output).unwrap();
    for (theme, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
        let dir = scratch("image-properties-gpu");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        let doc = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (name, size) in [
            ("desktop", Vec2::new(1360.0, 900.0)),
            ("compact", Vec2::new(360.0, 360.0)),
        ] {
            h.set_size(size);
            h.run();
            open(&mut h, false);
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}-top.png")))
                .unwrap();
            dimension(&mut h, "Width", 777.0);
            dimension(&mut h, "Height", 512.0);
            h.get_by_role_and_label(Role::RadioButton, "Color")
                .scroll_to_me();
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}-staged.png")))
                .unwrap();
            h.get_by_role_and_label(Role::Button, "Cancel").click();
            h.run();
        }
        assert!(h.state().editor().document().same_semantics(&doc));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
