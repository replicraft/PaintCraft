use super::studio::studio_harness;
use super::*;
use eframe::egui::Role;

fn view_action(h: &mut Harness<App>, label: &str) {
    h.get_by_role_and_label(Role::Button, "View").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "View")
        .get_by_label_contains(label)
        .click();
    h.run();
}

#[test]
fn zoom_shortcuts_menu_and_status_presets_share_mapping_without_history_changes() {
    let (mut h, dir) = studio_harness(true);
    let before = publications(&h);
    let rendered = paintcraft_render::render_flat(h.state().editor().document()).unwrap();
    h.event(key(Key::Minus, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 160.0);
    view_action(&mut h, "Zoom in");
    assert_eq!(h.state().canvas_rect().unwrap().width(), 320.0);
    h.event(key(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 640.0);
    h.get_by_label("4×").click();
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 1280.0);
    view_action(&mut h, "Pixel grid");
    assert!(paintcraft_render::render_flat(h.state().editor().document()).unwrap() == rendered);
    assert!(
        h.state().rendered()
            == paintcraft_render::render_region(
                h.state().editor().document(),
                h.state().rendered_region().unwrap()
            )
            .unwrap()
    );
    h.event(key(Key::Num0, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 320.0);
    assert_eq!(publications(&h), before);
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fit_selection_magnifies_and_scrolls_to_the_real_selected_rectangle() {
    let (mut h, dir) = studio_harness(true);
    h.get_by_label("Select").click();
    h.run();
    drag(&mut h, (180.0, 20.0), (220.0, 50.0));
    let selection = h.state().editor().selection();
    let before = publications(&h);
    h.event(key(Key::B, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    assert!(h.state().canvas_rect().unwrap().width() > 320.0);
    let canvas = h.state().canvas_rect().unwrap();
    let center = canvas.min + Vec2::new(200.5, 35.5) * (canvas.width() / 320.0);
    assert!(
        (200.0..900.0).contains(&center.x) && (200.0..850.0).contains(&center.y),
        "selection center {center:?} must be visible after fitting"
    );
    assert_eq!(h.state().editor().selection(), selection);
    assert_eq!(publications(&h), before);
    view_action(&mut h, "Zoom to window");
    let rect = h.state().canvas_rect().unwrap();
    assert!(rect.width() > 320.0 && rect.width() < 1100.0);
    assert_eq!(publications(&h), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fractional_zoom_paints_the_mapped_document_pixel_and_actual_size_restores_logical_pixel_scale() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    h.get_by_label("Pencil").click();
    h.run();
    h.event(key(Key::Minus, Modifiers::COMMAND));
    h.run();
    let canvas = h.state().canvas_rect().unwrap();
    let pos = canvas.min + Vec2::new(180.5, 30.5) * (canvas.width() / 320.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(pos), press(pos), release(pos)]);
    h.run();
    assert_eq!(top_pixel(&h, 180, 30), Rgba([0, 0, 0, 255]));
    assert_eq!(top_pixel(&h, 181, 30), Rgba::TRANSPARENT);
    h.event(key(Key::Num0, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 320.0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fitting_large_documents_and_actual_size_work_at_multiple_display_scales() {
    use paintcraft::settings::{Experience, Settings};
    for ppp in [1.0, 2.0] {
        let dir = scratch(&format!("view-fit-{}", uuid::Uuid::new_v4()));
        Settings {
            experience: Experience::Studio,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let doc = paintcraft::controller::Editor::new_document(1600, 1000).unwrap();
        let file = dir.join("large.pcnd");
        std::fs::write(
            &file,
            paintcraft_formats::pcnd::write(doc.document()).unwrap(),
        )
        .unwrap();
        let mut h = harness_with(App::new(Some(file)).with_settings(dir.clone()), ppp, true);
        let before = publications(&h);
        h.event(key(Key::B, Modifiers::COMMAND));
        h.run();
        let fitted = h.state().canvas_rect().unwrap();
        assert!(fitted.width() < 1100.0 && fitted.height() < 750.0);
        assert_eq!(publications(&h), before);
        h.event(key(Key::Num0, Modifiers::COMMAND));
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width(), 1600.0);
        assert!(!h.state().editor().is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_studio_pixel_grid_for_review() {
    let (mut h, dir) = studio_harness(false);
    h.get_by_label("8×").click();
    h.run();
    view_action(&mut h, "Pixel grid");
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-studio-pixel-grid.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn large_canvas_renders_only_visible_pixels_at_each_dpi_and_zoom() {
    use paintcraft_formats::pcnd;
    for ppp in [1.0, 1.5, 2.0] {
        let dir = scratch("viewport-large");
        std::fs::create_dir_all(&dir).unwrap();
        paintcraft::settings::Settings {
            experience: paintcraft::settings::Experience::Studio,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let file = dir.join("large.pcnd");
        let doc = paintcraft_document::new_document(
            16384,
            16384,
            Some(Rgba([17, 53, 91, 127])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        std::fs::write(&file, pcnd::write(&doc).unwrap()).unwrap();
        // Native backends report their device limit. The default headless egui
        // limit is only 2048, below this test's 2200 physical-pixel viewport.
        let mut h = Harness::builder()
            .with_size(Vec2::new(1100.0, 900.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, app: &mut App| {
                    ui.ctx().input_mut(|input| input.max_texture_side = 8192);
                    app.ui(ui);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx().request_discard("viewport test: second pass");
                    }
                },
                App::new(Some(file)).with_settings(dir.clone()),
            );
        h.run();
        let before = publications(&h);
        for step in 0..3 {
            let region = h.state().rendered_region().unwrap();
            assert!(region.area() < 4_000_000);
            assert_eq!(h.state().rendered().len(), region.area() as usize * 4);
            assert!(
                h.state().rendered()
                    == paintcraft_render::render_region(h.state().editor().document(), region)
                        .unwrap()
            );
            let rect = h.state().canvas_rect().unwrap();
            let visible = rect.intersect(h.state().canvas_viewport().unwrap());
            let view = paintcraft_ui::view::View::with_scale(
                (rect.min.x, rect.min.y),
                2.0_f32.powi(step) * ppp,
                ppp,
            );
            assert_eq!(
                Some(region),
                view.visible_region(
                    (visible.min.x, visible.min.y),
                    (visible.max.x, visible.max.y),
                    (16384, 16384)
                )
            );
            h.event(key(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
            h.run();
            h.event(key_up(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
            h.run();
        }
        assert_eq!(publications(&h), before);
        assert!(!h.state().editor().is_dirty());
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn fitting_a_million_square_canvas_uses_coarse_display_without_changing_pixels_or_history() {
    let dir = scratch("million-display");
    paintcraft::settings::Settings {
        experience: paintcraft::settings::Experience::Studio,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let doc = paintcraft_document::new_document(
        1_048_576,
        1_048_576,
        Some(Rgba([17, 37, 53, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let file = dir.join("million.pcnd");
    std::fs::write(&file, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    let mut h = harness_with(App::new(Some(file)).with_settings(dir.clone()), 1.0, true);
    let before = publications(&h);
    h.event(key(Key::B, Modifiers::COMMAND));
    h.run();
    let step = h.state().rendered_sample_step().unwrap();
    assert!(step >= 1024);
    let size = h.state().rendered_size().unwrap();
    assert!(size[0] <= 2048 && size[1] <= 2048);
    assert_eq!(
        h.state().rendered().len(),
        size[0] as usize * size[1] as usize * 4
    );
    assert_eq!(h.state().rendered_region(), Some(doc.canvas_rect()));
    assert!(
        h.state()
            .rendered()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [17, 37, 53, 127])
    );
    assert_eq!(publications(&h), before);
    assert!(!h.state().editor().is_dirty());
    assert!(h.state().editor().document().same_semantics(&doc));
    h.event(key(Key::Num0, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().rendered_sample_step(), Some(1));
    assert_eq!(publications(&h), before);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn coarse_partial_edge_texels_keep_their_footprint_and_clip_at_the_canvas() {
    let dir = scratch("partial-display-edge");
    paintcraft::settings::Settings {
        experience: paintcraft::settings::Experience::Studio,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let doc = paintcraft_document::new_document(
        9,
        5,
        Some(Rgba([17, 37, 53, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let file = dir.join("edge.pcnd");
    std::fs::write(&file, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    let mut h = harness_with(App::new(Some(file)).with_settings(dir.clone()), 1.0, true);
    h.event(key(Key::Minus, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().rendered_sample_step(), Some(2));
    assert_eq!(h.state().rendered_size(), Some([5, 3]));
    let canvas = h.state().canvas_rect().unwrap();
    assert_eq!(canvas.size(), Vec2::new(4.5, 2.5));
    let footprint = eframe::egui::Rect::from_min_size(canvas.min, Vec2::new(5.0, 3.0));
    let image = h
        .output()
        .shapes
        .iter()
        .find_map(|shape| {
            let eframe::egui::epaint::Shape::Mesh(mesh) = &shape.shape else {
                return None;
            };
            if mesh.vertices.len() != 4 {
                return None;
            }
            let mut bounds = eframe::egui::Rect::NOTHING;
            for vertex in &mesh.vertices {
                bounds.extend_with(vertex.pos);
            }
            (bounds == footprint).then_some((shape.clip_rect, mesh))
        })
        .expect("display mesh retains the padded texel footprint");
    assert_eq!(image.0, canvas);
    assert!(
        image
            .1
            .vertices
            .iter()
            .all(|v| [0.0, 1.0].contains(&v.uv.x) && [0.0, 1.0].contains(&v.uv.y))
    );
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
