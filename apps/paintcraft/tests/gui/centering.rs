use super::*;
use paintcraft::settings::ClassicVariant;

#[test]
fn every_classic_variant_centers_small_canvases_at_each_display_scale() {
    for variant in ClassicVariant::ALL {
        for ppp in [1.0, 2.0] {
            let dir = scratch(&format!("classic-center-{}", uuid::Uuid::new_v4()));
            paintcraft::settings::Settings {
                classic_variant: Some(variant),
                canvas: paintcraft::settings::CanvasDefaults {
                    configured: (320, 320),
                    ..Default::default()
                },
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
            let canvas = h.state().canvas_rect().unwrap();
            let viewport = h.state().canvas_viewport().unwrap();
            assert!(
                (canvas.center() - viewport.center()).length() < 2.0,
                "{variant:?} at {ppp}: {canvas:?} within {viewport:?}"
            );
            assert_eq!(canvas.width() * ppp, 320.0);
            let before = publications(&h);
            let point = canvas.min + Vec2::new(20.5, 30.5) / ppp;
            h.input_mut()
                .events
                .extend([Event::PointerMoved(point), press(point), release(point)]);
            h.run();
            assert_eq!(top_pixel(&h, 20, 30), Rgba([0, 0, 0, 255]));
            assert_eq!(publications(&h), before + 1);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
