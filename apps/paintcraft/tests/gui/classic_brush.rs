use super::*;
use paintcraft::settings::{ClassicVariant, Settings};

#[test]
fn classic_round_brush_retains_subpixel_motion_and_antialiasing_after_release() {
    for variant in ClassicVariant::ALL {
        for ppp in [1.0, 2.0] {
            let dir = scratch(&format!("brush-{variant:?}-{ppp}"));
            Settings {
                classic_variant: Some(variant),
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
            with_layer(&mut h);
            h.get_by_label("Brush").click();
            h.run();
            let base = h.state().editor().document_snapshot();
            let depth = h.state().editor().undo_depth();
            let origin = h.state().canvas_rect().unwrap().min;
            let a = origin + Vec2::new(80.125, 60.75);
            let b = origin + Vec2::new(80.75, 60.75);
            h.input_mut()
                .events
                .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
            h.run();
            let Some(paintcraft::controller::Preview::Path(paintcraft_tools::Stroke {
                points,
                point_scale,
                ..
            })) = h.state().editor().preview()
            else {
                panic!("brush preview");
            };
            assert_eq!(point_scale, 256);
            assert_eq!(
                points,
                vec![(20512, 15552), (20672, 15552)],
                "motion inside a pixel must survive"
            );
            let preview = h.state().rendered().to_vec();
            let preview_region = h.state().rendered_region().unwrap();
            assert!(h.state().editor().document().same_semantics(&base));
            h.event(release(b));
            h.run();
            assert_eq!(
                paintcraft_render::render_region(h.state().editor().document(), preview_region)
                    .unwrap(),
                preview,
            );
            assert_eq!(
                h.state().rendered(),
                paintcraft_render::render_region(
                    h.state().editor().document(),
                    h.state().rendered_region().unwrap()
                )
                .unwrap(),
            );
            assert_eq!(h.state().editor().undo_depth(), depth + 1);
            assert!((57..65).any(|y| (77..85).any(|x| {
                let a = top_pixel(&h, x, y).a();
                a > 0 && a < 255
            })));
            h.get_by_label("Undo").click();
            h.run();
            assert!(h.state().editor().document().same_semantics(&base));
            h.get_by_label("Redo").click();
            h.run();
            assert_eq!(
                paintcraft_render::render_region(h.state().editor().document(), preview_region)
                    .unwrap(),
                preview,
            );
            assert_eq!(
                h.state().rendered(),
                paintcraft_render::render_region(
                    h.state().editor().document(),
                    h.state().rendered_region().unwrap()
                )
                .unwrap(),
            );
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn brush_opacity_previews_release_exactly_zero_is_noop_and_eraser_still_erases() {
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        with_layer(&mut h);
        let original = h.state().editor().document_snapshot();
        h.get_by_label("Brush").click();
        h.run();
        h.get_by_role_and_label(eframe::egui::Role::Slider, "Brush opacity")
            .set_value_accesskit(50.0);
        h.run();
        assert_eq!(h.state().editor().options.brush_opacity, 50);
        assert!(h.state().editor().document().same_semantics(&original));
        let origin = h.state().canvas_rect().unwrap().min;
        let a = origin + Vec2::new(50.125, 70.75);
        let b = origin + Vec2::new(80.75, 70.125);
        h.input_mut().events.extend([
            Event::PointerMoved(a),
            press(a),
            Event::PointerMoved(b),
            Event::PointerMoved(a),
        ]);
        h.run();
        let preview = h.state().rendered().to_vec();
        let preview_region = h.state().rendered_region().unwrap();
        assert!(h.state().editor().document().same_semantics(&original));
        h.event(release(a));
        h.run();
        assert_eq!(
            paintcraft_render::render_region(h.state().editor().document(), preview_region)
                .unwrap(),
            preview,
        );
        assert_eq!(
            h.state().rendered(),
            paintcraft_render::render_region(
                h.state().editor().document(),
                h.state().rendered_region().unwrap()
            )
            .unwrap(),
        );
        assert_eq!(top_pixel(&h, 65, 70), Rgba([0, 0, 0, 128]));
        assert!((67..74).any(|y| (47..85).any(|x| {
            let alpha = top_pixel(&h, x, y).a();
            alpha > 0 && alpha < 128
        })));
        assert!((67..74).all(|y| (47..85).all(|x| top_pixel(&h, x, y).a() <= 128)));
        let painted = h.state().editor().document_snapshot();
        let depth = h.state().editor().undo_depth();
        let key = h.state().editor().render_key();
        h.get_by_role_and_label(eframe::egui::Role::Slider, "Brush opacity")
            .set_value_accesskit(0.0);
        h.run();
        h.input_mut().events.extend([
            Event::PointerMoved(a),
            press(a),
            Event::PointerMoved(b),
            release(b),
        ]);
        h.run();
        assert!(h.state().editor().document().same_semantics(&painted));
        assert_eq!(h.state().editor().undo_depth(), depth);
        assert_eq!(h.state().editor().render_key(), key);
        h.get_by_label("Eraser").click();
        h.run();
        h.input_mut().events.extend([
            Event::PointerMoved(a),
            press(a),
            Event::PointerMoved(b),
            release(b),
        ]);
        h.run();
        assert_eq!(top_pixel(&h, 65, 70), Rgba([255, 255, 255, 255]));
        h.get_by_label("Undo").click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&painted));
        h.get_by_label("Undo").click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&original));
        h.get_by_label("Redo").click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&painted));
        let encoded = paintcraft_formats::pcnd::write(&painted).unwrap();
        assert!(
            paintcraft_formats::pcnd::read(&encoded)
                .unwrap()
                .same_semantics(&painted)
        );
    }
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_classic_brush_opacity_for_review() {
    let mut h = harness_with(App::new(None), 1.0, false);
    with_layer(&mut h);
    h.get_by_label("Brush").click();
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::Slider, "Brush opacity")
        .set_value_accesskit(50.0);
    h.run();
    drag(&mut h, (40.0, 70.0), (220.0, 130.0));
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-classic-brush-opacity.png")
        .unwrap();
}
