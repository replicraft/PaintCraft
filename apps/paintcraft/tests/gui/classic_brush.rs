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
            let a = origin + Vec2::new(80.125, 60.75) / ppp;
            let b = origin + Vec2::new(80.75, 60.75) / ppp;
            h.input_mut()
                .events
                .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
            h.run();
            let Some(paintcraft::controller::Preview::Path {
                points,
                point_scale,
                ..
            }) = h.state().editor().preview()
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
            assert!(h.state().editor().document().same_semantics(&base));
            h.event(release(b));
            h.run();
            assert_eq!(h.state().rendered(), preview);
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
            assert_eq!(h.state().rendered(), preview);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
