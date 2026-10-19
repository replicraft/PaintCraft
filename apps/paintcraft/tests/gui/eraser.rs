use super::*;

fn erase_and_check(h: &mut Harness<App>, expected: Rgba) {
    h.get_by_role_and_label(eframe::egui::Role::Button, "Pencil")
        .click();
    h.run();
    drag(h, (40.0, 40.0), (60.0, 40.0));
    let painted = h.state().editor().document_snapshot();
    let depth = h.state().editor().undo_depth();
    h.get_by_role_and_label(eframe::egui::Role::Button, "Eraser")
        .click();
    h.run();
    let a = at(h, 40.0, 40.0);
    let b = at(h, 60.0, 40.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
    h.run();
    let preview = h.state().rendered().to_vec();
    assert!(h.state().editor().document().same_semantics(&painted));
    h.event(release(b));
    h.run();
    assert_eq!(h.state().rendered(), preview);
    assert_eq!(top_pixel(h, 50, 40), expected);
    assert_eq!(h.state().editor().undo_depth(), depth + 1);
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&painted));
    h.get_by_label("Redo").click();
    h.run();
    assert_eq!(top_pixel(h, 50, 40), expected);
}

#[test]
fn classic_eraser_restores_white_on_the_original_canvas() {
    erase_and_check(&mut harness(), Rgba([255, 255, 255, 255]));
}

#[test]
fn classic_eraser_restores_a_custom_canvas_ground_on_new_layers() {
    let dir = scratch("eraser-custom-ground");
    let ground = Rgba([32, 96, 160, 255]);
    let doc =
        paintcraft_document::new_document(1000, 486, Some(ground), 0, paintcraft_document::POLICY)
            .unwrap();
    let path = dir.join("ground.pcnd");
    std::fs::write(&path, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    let mut h = harness_with(App::new(Some(path)), 1.0, true);
    with_layer(&mut h);
    erase_and_check(&mut h, ground);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn studio_eraser_keeps_transparent_replacement() {
    let (mut h, dir) = super::studio::studio_harness(true);
    erase_and_check(&mut h, Rgba::TRANSPARENT);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn classic_eraser_uses_white_for_a_transparent_import() {
    let dir = scratch("eraser-transparent-import");
    let path = dir.join("transparent.png");
    let png = paintcraft_formats::png::encode_rgba8(1000, 486, &vec![0; 1000 * 486 * 4]).unwrap();
    std::fs::write(&path, png).unwrap();
    let mut h = harness_with(App::new(Some(path)), 1.0, true);
    erase_and_check(&mut h, Rgba([255, 255, 255, 255]));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
