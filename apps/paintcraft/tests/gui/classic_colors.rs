use super::*;
use paintcraft::settings::{ClassicVariant, Settings};

fn classic(ppp: f32, two_passes: bool) -> (Harness<'static, App>, PathBuf) {
    let dir = scratch(&format!("classic-colors-{}", uuid::Uuid::new_v4()));
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    (
        harness_with(App::new(None).with_settings(dir.clone()), ppp, two_passes),
        dir,
    )
}

fn hex(h: &mut Harness<App>, value: &str) {
    h.get_by_label("Hex RGB").focus();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    h.run();
    h.get_by_label("Hex RGB").type_text(value);
    h.run();
}

#[test]
fn color_dialog_stages_exact_rgb_custom_slots_and_cancel_without_document_history() {
    let (mut h, dir) = classic(1.0, true);
    with_layer(&mut h);
    let original = h.state().editor().document_snapshot();
    let depth = h.state().editor().undo_depth();
    let saved = std::fs::read(dir.join("settings.json")).unwrap();
    h.get_by_label("Edit colors…").click();
    h.run();
    hex(&mut h, "#2080ff");
    h.get_by_label("Custom color 28").click();
    h.run();
    h.get_by_label("Add custom color").click();
    h.run();
    assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
    assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
    h.event(key(Key::N, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().document_count(), 1, "the modal owns input");
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(Settings::load(&dir).0.custom_colors, [None; 28]);
    assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
    h.get_by_label("Edit colors…").click();
    h.run();
    hex(&mut h, "#2080FF");
    h.get_by_label("Custom color 28").click();
    h.run();
    h.get_by_label("Add custom color").click();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().editor().color, Rgba([32, 128, 255, 255]));
    let palette = Settings::load(&dir).0.custom_colors;
    assert_eq!(palette[27], Some(Rgba([32, 128, 255, 255])));
    assert_eq!(
        palette.iter().flatten().count(),
        1,
        "a repeated layout pass cannot add twice"
    );
    assert_eq!(h.state().editor().undo_depth(), depth);
    assert!(h.state().editor().document().same_semantics(&original));
    let mut restarted = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    restarted.get_by_label("Edit colors…").click();
    restarted.run();
    restarted.get_by_label("Custom color 28").click();
    restarted.run();
    restarted.get_by_label("OK").click();
    restarted.run();
    assert_eq!(restarted.state().editor().color, Rgba([32, 128, 255, 255]));
    assert!(!restarted.state().editor().is_dirty());
    // The adopted preference reaches the real pencil path and its own undo step.
    let point = at(&h, 30.0, 40.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(point), press(point), release(point)]);
    h.run();
    assert_eq!(top_pixel(&h, 30, 40), Rgba([32, 128, 255, 255]));
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&original));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_hex_cannot_accept_and_secondary_color_has_its_own_staged_target() {
    let (mut h, dir) = classic(1.0, true);
    let before = h.state().editor().document_snapshot();
    h.get_by_label("Fill color").click();
    h.run();
    hex(&mut h, "#12");
    h.get_by_label("OK").click();
    h.run();
    let _ = h.get_by_label("Hex RGB"); // Incomplete text keeps the dialog open.
    assert_eq!(h.state().editor().secondary, Rgba([255; 4]));
    hex(&mut h, "#123456");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().editor().secondary, Rgba([18, 52, 86, 255]));
    assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
    h.get_by_label("Pencil color").click();
    h.run();
    hex(&mut h, "#ABCDEF");
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
    assert!(h.state().editor().document().same_semantics(&before));
    assert_eq!(h.state().editor().undo_depth(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hsv_picker_and_hsl_numeric_fields_edit_the_actual_color_at_each_display_scale() {
    for ppp in [1.0, 2.0] {
        let (mut h, dir) = classic(ppp, true);
        h.get_by_label("Edit colors…").click();
        h.run();
        let square = h.get_by_label("Hue and saturation").rect();
        let point = Pos2::new(square.left() + square.width() / 3.0, square.top());
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point), release(point)]);
        h.run();
        let bar = h.get_by_label("Brightness").rect();
        let point = Pos2::new(bar.center().x, bar.top());
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point), release(point)]);
        h.run();
        h.get_by_label("OK").click();
        h.run();
        let color = h.state().editor().color.0;
        assert!(
            color[0] <= 1 && color[1] == 255 && color[2] <= 1,
            "{color:?}"
        );
        h.get_by_label("Edit colors…").click();
        h.run();
        hex(&mut h, "#000000");
        h.get_by_label("Color model").click();
        h.run();
        h.get_by_label("HSL").click();
        h.run();
        for (label, value) in [("Hue", 240.0), ("Saturation", 100.0), ("Lightness", 50.0)] {
            h.get_by_label(label).set_value_accesskit(value);
            h.run();
        }
        h.get_by_label("OK").click();
        h.run();
        assert_eq!(h.state().editor().color, Rgba([0, 0, 255, 255]));
        assert!(!h.state().editor().is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_classic_edit_colors_for_review() {
    let (mut h, dir) = classic(1.0, false);
    h.get_by_label("Edit colors…").click();
    h.run();
    hex(&mut h, "#2080FF");
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-classic-edit-colors.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn responsive_color_dialog_keeps_controls_and_footer_reachable_and_cancel_discards_drafts() {
    use eframe::egui::Role;
    use paintcraft::settings::Appearance;
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(800.0, 600.0), 1.0, Appearance::Dark),
        (Vec2::new(480.0, 500.0), 2.0, Appearance::Light),
        (Vec2::new(360.0, 500.0), 1.0, Appearance::Dark),
        (Vec2::new(480.0, 360.0), 1.0, Appearance::Dark),
    ] {
        let dir = scratch("responsive-colors");
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
                        ui.ctx()
                            .request_discard("test: repeated color-dialog layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_label("Edit colors…").scroll_to_me();
        h.run();
        h.get_by_label("Edit colors…").click();
        h.run();
        let footer = h.get_by_label("OK").rect();
        h.run_steps(8);
        assert_eq!(
            h.get_by_label("OK").rect(),
            footer,
            "dialog must settle at {size:?}"
        );
        for label in ["OK", "Cancel"] {
            let rect = h.get_by_label(label).rect();
            assert!(
                rect.min.x >= 0.0
                    && rect.min.y >= 0.0
                    && rect.max.x <= size.x
                    && rect.max.y <= size.y,
                "{label}: {rect:?} outside {size:?}"
            );
        }
        for label in [
            "Hue and saturation",
            "Brightness",
            "Hex RGB",
            "Red",
            "Custom color 28",
        ] {
            let role = match label {
                "Hue and saturation" => Role::ColorWell,
                "Brightness" => Role::Slider,
                "Hex RGB" => Role::TextInput,
                "Red" => Role::SpinButton,
                _ => Role::Button,
            };
            h.get_by_role_and_label(role, label).scroll_to_me();
            h.run();
            let rect = h.get_by_role_and_label(role, label).rect();
            assert!(
                rect.min.x >= 0.0 && rect.max.x <= size.x,
                "{label}: {rect:?}"
            );
            assert!(
                rect.max.y >= 0.0 && rect.min.y < footer.min.y,
                "{label} must scroll above the footer"
            );
            assert_eq!(h.get_by_label("OK").rect(), footer);
        }
        h.get_by_label("Hex RGB").scroll_to_me();
        h.run();
        hex(&mut h, "#2080FF");
        h.get_by_label("Custom color 28").scroll_to_me();
        h.run();
        h.get_by_label("Custom color 28").click();
        h.run();
        h.get_by_label("Add custom color").scroll_to_me();
        h.run();
        h.get_by_label("Add custom color").click();
        h.run();
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn narrow_color_dialog_accepts_rgb_and_custom_palette_changes_without_document_history() {
    let dir = scratch("narrow-color-accept");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = Harness::builder()
        .with_size(Vec2::new(480.0, 500.0))
        .build_ui_state(
            |ui, app: &mut App| app.ui(ui),
            App::new(None).with_settings(dir.clone()),
        );
    h.run();
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    h.get_by_label("Edit colors…").scroll_to_me();
    h.run();
    h.get_by_label("Edit colors…").click();
    h.run();
    h.get_by_label("Hex RGB").scroll_to_me();
    h.run();
    hex(&mut h, "#2080FF");
    h.get_by_label("Custom color 28").scroll_to_me();
    h.run();
    h.get_by_label("Custom color 28").click();
    h.run();
    h.get_by_label("Add custom color").scroll_to_me();
    h.run();
    h.get_by_label("Add custom color").click();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().editor().color, Rgba([32, 128, 255, 255]));
    assert_eq!(
        Settings::load(&dir).0.custom_colors[27],
        Some(Rgba([32, 128, 255, 255]))
    );
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(!h.state().editor().is_dirty());
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
