use paintcraft_core::types::{Error, ErrorCode, Rgba, picker::SampleSize};
use paintcraft_document::{Layer, POLICY, new_document};
use paintcraft_raster::{Admission, BlendMode, RasterLayer};
use paintcraft_render::{render_flat, sampling::sample_color};
fn layered() -> paintcraft_document::Document {
    let mut d = new_document(65, 53, Some(Rgba([20, 40, 60, 255])), 0, POLICY).unwrap();
    for (i, origin, visible, opacity) in [
        (1, (2, -1), true, 128),
        (2, (-1, 3), true, 173),
        (3, (0, 0), false, 255),
    ] {
        let mut r = RasterLayer::new(61, 49, Rgba([30 * i, 60, 90, 128]));
        let mut a = Admission::new(4 * 65536);
        for y in 0..49 {
            for x in 0..61 {
                r.blend_pixel(
                    x,
                    y,
                    Rgba([
                        x as u8 * 3,
                        y as u8 * 4,
                        10 * i,
                        if (x + y) % 3 == 0 { 0 } else { 200 },
                    ]),
                    BlendMode::Replace,
                    &mut a,
                )
                .unwrap();
            }
        }
        let id = paintcraft_core::types::ids::deterministic_id(&["sample", &i.to_string()]);
        d.roots.push(id);
        d.layers.insert(
            id,
            Layer {
                id,
                name: "Sample".into(),
                origin,
                visible,
                opacity,
                raster: r,
            },
        );
    }
    d
}
#[test]
fn image_neighborhoods_match_full_ordered_render_and_independent_channel_average() {
    let d = layered();
    let rendered = render_flat(&d).unwrap();
    for size in SampleSize::ALL {
        for at in [(0, 0), (64, 52), (32, 26), (4, 51)] {
            let radius = i64::from(size.side() / 2);
            let mut n = 0u64;
            let mut sum = [0u64; 4];
            for y in (at.1 - radius).max(0)..=(at.1 + radius).min(52) {
                for x in (at.0 - radius).max(0)..=(at.0 + radius).min(64) {
                    n += 1;
                    for c in 0..4 {
                        sum[c] += u64::from(rendered[(y as usize * 65 + x as usize) * 4 + c]);
                    }
                }
            }
            assert_eq!(
                sample_color(&d, None, at, size, || Ok(())).unwrap(),
                Rgba(sum.map(|s| ((s + n / 2) / n) as u8))
            );
        }
    }
}
#[test]
fn raw_layer_includes_hidden_rgb_ignores_opacity_visibility_and_bounds_default() {
    let d = layered();
    let l = &d.layers[d.roots.last().unwrap()];
    let raw = l.raster.get(0, 0);
    assert_eq!(raw.a(), 0);
    assert_ne!(raw, Rgba::TRANSPARENT);
    assert_eq!(
        sample_color(&d, Some(l), (0, 0), SampleSize::One, || Ok(())).unwrap(),
        raw
    );
    assert_eq!(
        sample_color(&d, Some(l), (64, 52), SampleSize::One, || Ok(())).unwrap(),
        Rgba::TRANSPARENT
    );
    assert_ne!(
        sample_color(&d, None, (0, 0), SampleSize::One, || Ok(())).unwrap(),
        raw
    );
    let d = new_document(1, 1, Some(Rgba([17, 33, 55, 0])), 0, POLICY).unwrap();
    assert_eq!(
        sample_color(&d, None, (0, 0), SampleSize::One, || Ok(())).unwrap(),
        Rgba::TRANSPARENT
    );
    assert_eq!(
        sample_color(
            &d,
            Some(&d.layers[&d.roots[0]]),
            (0, 0),
            SampleSize::FiftyOne,
            || Ok(())
        )
        .unwrap(),
        Rgba([17, 33, 55, 0])
    );
}
#[test]
fn huge_uniform_canvas_uses_a_bounded_region_and_rejects_bad_centers_or_cancellation() {
    let d = new_document(900_000, 800_000, Some(Rgba([17, 33, 55, 255])), 0, POLICY).unwrap();
    assert_eq!(
        sample_color(
            &d,
            None,
            (899_999, 799_999),
            SampleSize::FiftyOne,
            || Ok(())
        )
        .unwrap(),
        Rgba([17, 33, 55, 255])
    );
    for at in [(-1, 0), (900_000, 0), (i64::MAX, i64::MIN)] {
        assert_eq!(
            sample_color(&d, None, at, SampleSize::One, || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    let count = std::cell::Cell::new(0);
    let err = sample_color(&d, None, (500, 500), SampleSize::FiftyOne, || {
        count.set(count.get() + 1);
        if count.get() > 10 {
            Err(Error::new(ErrorCode::OperationCancelled, "test"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::OperationCancelled);
}
