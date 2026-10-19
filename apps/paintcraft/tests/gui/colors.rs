use super::studio::studio_harness;
use super::*;
use eframe::egui::Role;

fn hex(h: &mut Harness<App>, value: &str) {
    h.get_by_label("Hex RGBA").focus();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    h.run();
    h.get_by_label("Hex RGBA").type_text(value);
    h.run();
}
fn click_wheel(h: &mut Harness<App>, fraction: Vec2, button: PointerButton) {
    let rect = h.get_by_label("Color wheel").rect();
    let p = rect.center() + fraction * (rect.width() / 2.0 - 4.0);
    h.input_mut().events.extend([
        Event::PointerMoved(p),
        Event::PointerButton {
            pos: p,
            button,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: p,
            button,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.run();
}

#[test]
fn swap_and_palette_use_selected_swatch_once_in_multipass_frames() {
    let (mut h, dir) = studio_harness(true);
    let before = publications(&h);
    h.get_by_label("Swap colors").click();
    h.run();
    assert_eq!(h.state().editor().color, Rgba([255; 4]));
    assert_eq!(h.state().editor().secondary, Rgba([0, 0, 0, 255]));
    h.get_by_label("Secondary").click();
    h.run();
    h.get_by_label("Color #FF0000").click();
    h.run();
    assert_eq!(h.state().editor().secondary, Rgba([255, 0, 0, 255]));
    assert_eq!(h.state().editor().color, Rgba([255; 4]));
    h.get_by_label("Primary").click();
    h.run();
    h.get_by_label("Color #00FFFF").click_secondary();
    h.run();
    assert_eq!(h.state().editor().secondary, Rgba([0, 255, 255, 255]));
    assert_eq!(publications(&h), before);
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rgba_and_hex_are_exact_including_zero_alpha_and_invalid_text_is_local() {
    let (mut h, dir) = studio_harness(true);
    let snapshot = h.state().editor().document_snapshot();
    h.get_by_label("More >>").click();
    h.run();
    assert!(
        h.get_by_role_and_label(Role::Window, "Colors")
            .rect()
            .width()
            <= 350.0
    );
    hex(&mut h, "#12345600");
    assert_eq!(h.state().editor().color, Rgba([0x12, 0x34, 0x56, 0]));
    h.run_steps(3);
    assert_eq!(h.state().editor().color, Rgba([0x12, 0x34, 0x56, 0]));
    hex(&mut h, "#bad");
    assert_eq!(h.state().editor().color, Rgba([0x12, 0x34, 0x56, 0]));
    h.get_by_role_and_label(Role::Slider, "Alpha")
        .set_value_accesskit(93.0);
    h.run();
    assert_eq!(h.state().editor().color, Rgba([0x12, 0x34, 0x56, 93]));
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn wheel_edits_real_colors_without_painting_and_right_click_keeps_secondary_alpha() {
    let (mut h, dir) = studio_harness(true);
    h.get_by_label("More >>").click();
    h.run();
    hex(&mut h, "#01020325");
    h.get_by_label("Secondary").click();
    h.run();
    hex(&mut h, "#0405065D");
    h.get_by_label("Primary").click();
    h.run();
    let before = publications(&h);
    click_wheel(&mut h, Vec2::new(0.99, 0.0), PointerButton::Secondary);
    let secondary = h.state().editor().secondary;
    assert_eq!(secondary.0[3], 93);
    assert!(secondary.0[0] > 250 && secondary.0[1] < 40 && secondary.0[2] < 40);
    assert_eq!(h.state().editor().color, Rgba([1, 2, 3, 37]));
    h.get_by_label("Primary").click();
    h.run();
    click_wheel(&mut h, Vec2::ZERO, PointerButton::Primary);
    assert_eq!(h.state().editor().color, Rgba([255, 255, 255, 37]));
    assert_eq!(publications(&h), before);
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn color_preferences_remain_independent_between_document_tabs() {
    let (mut h, dir) = studio_harness(false);
    h.get_by_label("Color #FF0000").click();
    h.run();
    h.get_by_label("New").click();
    h.run();
    assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
    h.get_by_label("Color #00FFFF").click();
    h.run();
    h.get_by_label("Document 1: Untitled 1").click();
    h.run();
    assert_eq!(h.state().editor().color, Rgba([255, 0, 0, 255]));
    h.get_by_label("Document 2: Untitled 2").click();
    h.run();
    assert_eq!(h.state().editor().color, Rgba([0, 255, 255, 255]));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn chosen_rgba_reaches_the_paint_tool_and_undo_restores_the_pixels() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    h.get_by_label("More >>").click();
    h.run();
    hex(&mut h, "#1234565D");
    h.get_by_label("Less <<").click();
    h.run();
    let before = publications(&h);
    drag(&mut h, (180.0, 30.0), (240.0, 30.0));
    assert_eq!(top_pixel(&h, 200, 30), Rgba([0x12, 0x34, 0x56, 93]));
    assert_eq!(publications(&h), before + 1);
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(top_pixel(&h, 200, 30), Rgba::TRANSPARENT);
    assert_eq!(h.state().editor().color, Rgba([0x12, 0x34, 0x56, 93]));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_studio_colors_for_review() {
    let (mut h, dir) = studio_harness(false);
    h.get_by_label("More >>").click();
    h.run();
    hex(&mut h, "#358BCB9F");
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-studio-colors.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
