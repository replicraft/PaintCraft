use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{ClassicVariant, Settings};

#[test]
fn classic_starts_at_window_fit_and_layer_toggles_settle_without_discard_warnings() {
    for ppp in [1.0, 2.0] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(1360.0, 900.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(|ui, app: &mut App| app.ui(ui), App::new(None));
        h.run();
        let fitted = h.state().canvas_rect().unwrap().size();
        let doc = h.state().editor().document_snapshot();
        h.get_by_label("Zoom to window").click();
        h.run();
        assert!((h.state().canvas_rect().unwrap().size() - fitted).length() < 1.0);
        for _ in 0..6 {
            h.get_by_role_and_label(Role::Button, "Layers").click();
            h.run();
            for _ in 0..6 {
                h.step();
                assert!(
                    h.output()
                        .platform_output
                        .request_discard_reasons
                        .is_empty()
                );
                assert!(!h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    Shape::Text(text) if text.galley.job.text.contains("PERF WARNING"))));
            }
        }
        assert!(h.state().editor().document().same_semantics(&doc));
    }
}

#[test]
fn classic_zoom_reuses_the_small_canvas_texture_without_publishing() {
    let mut h = harness();
    let image_id = |h: &Harness<App>| {
        let canvas = h.state().canvas_rect().unwrap();
        h.output()
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                Shape::Mesh(mesh)
                    if mesh.vertices.len() == 4
                        && {
                            let mut bounds = eframe::egui::Rect::NOTHING;
                            for vertex in &mesh.vertices {
                                bounds.extend_with(vertex.pos);
                            }
                            bounds == canvas
                        }
                        && mesh.texture_id != eframe::egui::TextureId::default() =>
                {
                    Some(mesh.texture_id)
                }
                _ => None,
            })
            .expect("canvas texture")
    };
    let texture = image_id(&h);
    let source = h.state().editor().document_snapshot();
    for _ in 0..4 {
        h.get_by_label("Zoom out").click();
        h.run();
        assert_eq!(image_id(&h), texture);
        assert_eq!(h.state().rendered_size(), Some([1000, 486]));
    }
    for _ in 0..5 {
        h.get_by_label("Zoom in").click();
        h.run();
        assert_eq!(image_id(&h), texture);
    }
    assert!(h.state().editor().document().same_semantics(&source));
    assert!(!h.state().editor().is_dirty());
}

#[test]
fn selection_delete_erases_only_selected_pixels_and_undo_restores_them() {
    for variant in ClassicVariant::ALL {
        let dir = scratch(&format!("selection-delete-{}", variant.id()));
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
        drag(&mut h, (180.0, 50.0), (230.0, 50.0));
        let before = h.state().editor().document_snapshot();
        h.get_by_role_and_label(Role::Button, "Select").click();
        h.run();
        drag(&mut h, (190.0, 45.0), (210.0, 55.0));
        let selection = h.state().editor().selection();
        let publications_before = publications(&h);
        h.key_press(Key::Delete);
        h.run();
        assert_eq!(publications(&h), publications_before + 1);
        assert_eq!(top_pixel(&h, 200, 50), Rgba([255; 4]));
        assert_eq!(top_pixel(&h, 180, 50), Rgba([0, 0, 0, 255]));
        assert_eq!(h.state().editor().selection(), selection);
        h.event(key(Key::Z, Modifiers::COMMAND));
        h.run();
        assert!(h.state().editor().document().same_semantics(&before));
        assert_eq!(h.state().editor().selection(), selection);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn magnified_paint_previews_remain_regional_then_restore_the_reusable_idle_texture() {
    let mut h = harness();
    h.get_by_label("Brush").click();
    h.run();
    for _ in 0..3 {
        h.get_by_label("Zoom in").click();
        h.run();
    }
    let point = h.state().canvas_viewport().unwrap().center();
    let canvas = h.state().canvas_rect().unwrap();
    let scale = canvas.width() / 1000.0;
    let local = (point - canvas.min) / scale;
    let pixel = (local.x.floor() as i64, local.y.floor() as i64);
    h.event(Event::PointerMoved(point));
    h.event(press(point));
    h.run();
    assert!(h.state().rendered_region().unwrap().area() < 1000 * 486);
    let preview = displayed_pixel(&h, pixel.0, pixel.1);
    h.event(release(point));
    h.run();
    assert_eq!(h.state().rendered_size(), Some([1000, 486]));
    assert_eq!(displayed_pixel(&h, pixel.0, pixel.1), preview);
    assert_eq!(top_pixel(&h, pixel.0, pixel.1), Rgba([0, 0, 0, 255]));
}
