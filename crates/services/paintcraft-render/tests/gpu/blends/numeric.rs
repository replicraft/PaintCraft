use super::*;
#[test]
fn exact_public_blends_straight_region_display_and_mask_opacity_literals() {
    let Some(gpu) = gpu() else { return };
    for row in oracle().get("vectors").unwrap().as_array().unwrap() {
        let mut d = scene(
            1,
            rgba(row.get("back").unwrap()),
            &bytes(row.get("front").unwrap()),
            mode(row.get("mode").unwrap()),
        );
        let opacity = row.get("opacity").unwrap().as_i64().unwrap() as u8;
        let coverage = row.get("mask").unwrap().as_i64().unwrap() as u8;
        top(&mut d).opacity = opacity;
        if coverage != 255 {
            top(&mut d).mask = Some(mask(d.canvas_rect(), coverage));
        }
        let before = d.clone();
        admitted(|| {
            assert_eq!(
                gpu.render_cancellable(&d, || Ok(()))?,
                bytes(row.get("straight").unwrap()),
                "{}",
                row.get("name").unwrap().as_str().unwrap()
            );
            let region = gpu.render_region_texture(&d, d.canvas_rect(), || Ok(()))?;
            assert_eq!(
                region.readback(|| Ok(()))?,
                bytes(row.get("display").unwrap()),
                "{}",
                row.get("name").unwrap().as_str().unwrap()
            );
            let request = DisplayRequest::new(&d, d.canvas_rect(), 0)?;
            assert_eq!(
                gpu.render_display_texture(&d, request, || Ok(()))?
                    .readback(|| Ok(()))?,
                bytes(row.get("display").unwrap()),
                "{}",
                row.get("name").unwrap().as_str().unwrap()
            );
            Ok(())
        });
        assert!(d.same_semantics(&before));
    }
}
#[test]
fn mode_only_and_reset_reuse_cacheable_mask_source_uploads_and_recompose() {
    let Some(gpu) = gpu() else { return };
    let rows = oracle();
    let row = rows
        .get("vectors")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v.get("name").and_then(Value::as_str) == Some("mask127-255-multiply"))
        .unwrap();
    let mut d = scene(
        1,
        rgba(row.get("back").unwrap()),
        &bytes(row.get("front").unwrap()),
        Mode::Multiply,
    );
    top(&mut d).mask = Some(mask(d.canvas_rect(), 127));
    top(&mut d).opacity = 128;
    let state = d.state_id;
    admitted(|| {
        let request = DisplayRequest::new(&d, d.canvas_rect(), 0)?;
        let first = gpu.render_display_texture(&d, request, || Ok(()))?;
        assert_eq!(
            first.readback(|| Ok(()))?,
            bytes(row.get("display").unwrap())
        );
        for blend in [Mode::Screen, Mode::Multiply] {
            top(&mut d).blend_mode = blend;
            let name = if blend == Mode::Screen {
                "mask127-255-screen"
            } else {
                "mask127-255-multiply"
            };
            let expected = rows
                .get("vectors")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v.get("name").and_then(Value::as_str) == Some(name))
                .unwrap();
            let hit = gpu.render_display_texture(&d, request, || Ok(()))?;
            assert_eq!(hit.stats.uploaded_bytes, 0);
            assert!(hit.stats.source_hits > 0);
            assert_eq!(
                hit.readback(|| Ok(()))?,
                bytes(expected.get("display").unwrap())
            );
        }
        Ok(())
    });
    assert_eq!(d.state_id, state);
}
#[test]
fn box_prefix_before_own_mask_native_blend_before_lod_and_partial_edge() {
    use paintcraft_core::types::filters::{BoxBlurV1, Descriptor};
    let Some(gpu) = gpu() else { return };
    for row in oracle().get("spatial").unwrap().as_array().unwrap() {
        let mut d = scene(
            2,
            rgba(row.get("back").unwrap()),
            &bytes(row.get("raw").unwrap()),
            mode(row.get("mode").unwrap()),
        );
        top(&mut d).opacity = 128;
        top(&mut d).mask = Some(mask(Rect::new(0, 0, 1, 1).unwrap(), 127));
        top(&mut d).filters = FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["blend-box"]),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        )])
        .unwrap();
        admitted(|| {
            assert_eq!(gpu.render(&d)?, bytes(row.get("straight").unwrap()));
            let request = DisplayRequest::new(&d, d.canvas_rect(), 1)?;
            let result = gpu.render_display_texture(&d, request, || Ok(()))?;
            assert_eq!(
                result.readback(|| Ok(()))?,
                bytes(row.get("lod1_display").unwrap())
            );
            // Spatial sources deliberately keep the established uncached gather.
            assert!(result.stats.uploaded_bytes > 0);
            let roi = Rect::new(1, 0, 1, 1).unwrap();
            assert_eq!(
                gpu.render_region_texture(&d, roi, || Ok(()))?
                    .readback(|| Ok(()))?,
                bytes(row.get("display").unwrap())[4..]
            );
            Ok(())
        });
    }
    for row in oracle().get("edge").unwrap().as_array().unwrap() {
        let mut d = scene(
            3,
            rgba(row.get("back").unwrap()),
            &bytes(row.get("raw").unwrap()),
            mode(row.get("mode").unwrap()),
        );
        top(&mut d).mask = Some(mask(d.canvas_rect(), 127));
        top(&mut d).opacity = 128;
        admitted(|| {
            assert_eq!(gpu.render(&d)?, bytes(row.get("native").unwrap()));
            let request = DisplayRequest::new(&d, d.canvas_rect(), 1)?;
            assert_eq!(
                gpu.render_display_texture(&d, request, || Ok(()))?
                    .readback(|| Ok(()))?,
                bytes(row.get("lod1_display").unwrap())
            );
            Ok(())
        });
    }
}

#[test]
fn mixed_modes_follow_root_order_and_reversed_order_has_independent_pixels() {
    let Some(gpu) = gpu() else { return };
    let data: Value = json::parse(include_bytes!(
        "../../../../../foundation/paintcraft-core/tests/layer-blend-fixtures/oracle.json"
    ))
    .unwrap();
    let order = data.get("order").unwrap();
    let mut d = new_document(1, 1, Some(rgba(order.get("backdrop").unwrap())), 0, POLICY).unwrap();
    for (i, row) in order
        .get("layers")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let id = deterministic_id(&["ordered-gpu-blend", &i.to_string()]);
        let mut l = d.layers[&d.roots[0]].clone();
        l.id = id;
        l.raster = RasterLayer::new(1, 1, rgba(row.get("rgba").unwrap()));
        l.blend_mode = mode(row.get("mode").unwrap());
        l.opacity = row.get("opacity").unwrap().as_i64().unwrap() as u8;
        d.roots.push(id);
        d.layers.insert(id, l);
    }
    admitted(|| {
        for (reverse, row) in oracle()
            .get("ordered")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            if reverse == 1 {
                d.roots.swap(1, 2);
            }
            assert_eq!(gpu.render(&d)?, bytes(row.get("straight").unwrap()));
            let req = DisplayRequest::new(&d, d.canvas_rect(), 0)?;
            assert_eq!(
                gpu.render_display_texture(&d, req, || Ok(()))?
                    .readback(|| Ok(()))?,
                bytes(row.get("display").unwrap())
            );
        }
        Ok(())
    });
}
