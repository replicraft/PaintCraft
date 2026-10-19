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
