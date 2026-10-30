use super::*;
use eframe::egui::{Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_raster::{Admission, BlendMode, RasterLayer};

#[test]
fn printable_pixels_follow_order_visibility_and_offsets_with_exact_capture_bounds() {
    let mut doc = new_document(2, 1, Some(Rgba([255; 4])), 7, POLICY).unwrap();
    let base = doc.layers.get_mut(&doc.roots[0]).unwrap();
    base.raster
        .blend_pixel(
            0,
            0,
            Rgba([0, 0, 0, 255]),
            BlendMode::Replace,
            &mut Admission::new(65_536),
        )
        .unwrap();
    let mut top = base.clone();
    top.id = uuid::Uuid::from_u128(99);
    top.origin = (1, 0);
    top.raster = RasterLayer::new(1, 1, Rgba([255, 0, 0, 255]));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top.clone());
    top.id = uuid::Uuid::from_u128(100);
    top.visible = false;
    top.raster = RasterLayer::new(2, 1, Rgba([0, 255, 0, 255]));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    let png = capture(&doc).unwrap();
    let decoded = paintcraft_formats::image::decode_with_pixel_limit(&png, 2).unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 1));
    assert_eq!(decoded.rgba, [0, 0, 0, 255, 255, 0, 0, 255]);
    doc.canvas = (4096, 4096);
    assert!(capture(&doc).is_err());
}

fn harness() -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h
}

#[test]
fn printable_text_is_private_and_cancellation_keeps_the_draft_and_dirty_state() {
    let mut h = harness();
    let layer = h.state().editor.target.unwrap();
    h.state_mut().editor.tools.begin_text(layer, (10, 10));
    h.state_mut().editor.tools.type_text("Keep editable");
    let ticket = h.state().editor.session.ticket();
    let doc = h.state().editor.session.snapshot();
    let ctx = h.ctx.clone();
    h.state_mut().print_requested(&ctx);
    assert!(h.state().printing_pending());
    let (png, title) = h.state().printer.captured.as_ref().unwrap();
    assert_eq!(title, "Untitled - PaintCraft");
    let decoded =
        paintcraft_formats::image::decode_with_pixel_limit(png, crate::editor::MAX_PIXELS).unwrap();
    assert!(
        decoded
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255; 4])
    );
    let original = png.clone();
    h.state_mut().print_requested(&ctx);
    assert_eq!(h.state().printer.captured.as_ref().unwrap().0, original);
    h.run();
    h.get_by_role_and_label(Role::Button, "Cancel").click();
    h.run();
    assert!(!h.state().printing_pending());
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(h.state().editor.session.document().same_semantics(&doc));
    assert_eq!(h.state().editor.tools.text_content(), Some("Keep editable"));
    assert!(h.state().unsaved());
    assert_eq!(h.state().message, "Printing cancelled.");
    h.state_mut().test_start_preedit("変換中");
    h.state_mut().printer.captured = None;
    h.state_mut().print_requested(&ctx);
    assert!(!h.state().printing_pending());
    assert!(h.state().printer.captured.is_none());
    assert_eq!(h.state().text.preedit(), "変換中");
    assert_eq!(h.state().editor.session.ticket(), ticket);
}

#[cfg(target_os = "macos")]
#[test]
fn actual_print_shortcut_and_menu_capture_each_frontend_without_marking_saved() {
    use eframe::egui::{Key, Modifiers};
    use paintcraft_classic::ClassicVariant;
    use paintcraft_ui::actions::Frontend;
    for frontend in [Frontend::Classic, Frontend::Studio] {
        for variant in [ClassicVariant::V7, ClassicVariant::V10, ClassicVariant::V11] {
            let mut h = harness();
            h.state_mut().frontend = frontend;
            h.state_mut().variant = variant;
            h.run();
            let ticket = h.state().editor.session.ticket();
            h.key_press_modifiers(Modifiers::COMMAND, Key::P);
            h.run();
            assert!(h.state().printing_pending());
            assert!(h.state().printer.captured.is_some());
            *h.state().printer.outcome.borrow_mut() = Some("Print dialog closed.");
            h.run();
            assert!(!h.state().printing_pending());
            assert_eq!(h.state().editor.session.ticket(), ticket);
            assert!(!h.state().unsaved());
            h.get_by_role_and_label(Role::Button, "File").click();
            h.run();
            if variant != ClassicVariant::V11 && frontend == Frontend::Classic {
                h.get_by_role_and_label(Role::Button, "File")
                    .get_by_label("Print")
                    .click();
                h.run();
            } else if frontend == Frontend::Classic {
                h.get_by_role_and_label(Role::Button, "File")
                    .get_by_label("Print")
                    .hover();
                h.run();
            }
            h.get_by_role_and_label(Role::Button, "File")
                .get_by_label_contains("Print…")
                .click();
            h.run();
            assert!(
                h.state().printing_pending(),
                "{frontend:?} {variant:?}: {}",
                h.state().message
            );
            h.key_press(Key::Escape);
            h.run();
            assert!(!h.state().printing_pending());
            assert_eq!(h.state().editor.session.ticket(), ticket);
        }
    }
}
