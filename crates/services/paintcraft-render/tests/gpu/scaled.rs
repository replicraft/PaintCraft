use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect};
use paintcraft_render::display::{DisplayRequest, render_display_cancellable};

fn renderer() -> Option<GpuRenderer> {
    renderer_edge(32)
}
fn renderer_edge(edge: u32) -> Option<GpuRenderer> {
    let adapter = adapter()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    Some(
        GpuRenderer::new(device, queue)
            .unwrap()
            .with_scaled_tile_edge(edge)
            .unwrap(),
    )
}
fn premultiplied(pixels: &[u8]) -> Vec<u8> {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = u32::from(p[3]);
            [
                ((u32::from(p[0]) * a + 127) / 255) as u8,
                ((u32::from(p[1]) * a + 127) / 255) as u8,
                ((u32::from(p[2]) * a + 127) / 255) as u8,
                p[3],
            ]
        })
        .collect()
}
fn fixture() -> Document {
    let mut doc = new_document(73, 51, None, 0, POLICY).unwrap();
    let mut admission = Admission::new(3 * 65536);
    for n in 0..3u32 {
        let bytes: Vec<_> = (0..79 * 57u32)
            .flat_map(|p| {
                [
                    (p * 17 + n) as u8,
                    (p / 79 * 7) as u8,
                    (p + n * 71) as u8,
                    [0, 1, 17, 128, 255][p as usize % 5],
                ]
            })
            .collect();
        let id = deterministic_id(&["gpu-scaled", &n.to_string()]);
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: "scaled offsets".into(),
                origin: (-3 + i64::from(n) * 7, -2 + i64::from(n) * 3),
                visible: true,
                opacity: 137 + n as u8 * 31,
                filters: Default::default(),
                raster: RasterLayer::from_rgba(
                    79,
                    57,
                    Rgba([47, 71, 97, 0]),
                    &bytes,
                    &mut admission,
                )
                .unwrap(),
            },
        );
    }
    doc
}
#[test]
fn scaled_gpu_matches_exact_cpu_boxes_offsets_partial_edges_levels_and_live_frames() {
    for edge in [32, 64] {
        compare_edge(edge);
    }
}
fn compare_edge(edge: u32) {
    let Some(gpu) = renderer_edge(edge) else {
        return;
    };
    let doc = fixture();
    let original = render_flat(&doc).unwrap();
    let mut retained = None;
    for level in [1, 2, 3, 5, 10] {
        for region in [
            doc.canvas_rect(),
            Rect::new(17, 13, 45, 33).unwrap(),
            Rect::new(72, 50, 1, 1).unwrap(),
        ] {
            let request = DisplayRequest::new(&doc, region, level).unwrap();
            let expected = premultiplied(
                &render_display_cancellable(&doc, request, || Ok(()))
                    .unwrap()
                    .pixels,
            );
            let frame = gpu
                .render_display_texture(&doc, request, || Ok(()))
                .unwrap();
            assert_eq!(frame.region(), request.region());
            assert_eq!(frame.level(), level);
            assert_eq!(frame.size(), request.size());
            assert_eq!(
                frame.readback(|| Ok(())).unwrap(),
                expected,
                "cold level {level} region {region:?}"
            );
            let warm = gpu
                .render_display_texture(&doc, request, || Ok(()))
                .unwrap();
            assert_eq!(warm.readback(|| Ok(())).unwrap(), expected);
            if let Some((old, bytes)) = &retained {
                let old: &paintcraft_render::gpu::GpuTextureFrame = old;
                assert_eq!(old.readback(|| Ok(())).unwrap(), *bytes);
            }
            retained = Some((frame, expected));
        }
    }
    assert_eq!(render_flat(&doc).unwrap(), original);
    parity(&gpu, &doc);
}

#[test]
fn wide_gpu_resolve_matches_analytic_million_square_composites_without_native_expansion() {
    let Some(gpu) = renderer() else { return };
    let mut cases = vec![
        (Rgba([17, 37, 53, 81]), Rgba([223, 71, 5, 137]), 173),
        (Rgba([255, 0, 17, 255]), Rgba([0, 255, 131, 255]), 255),
        (Rgba([3, 251, 97, 1]), Rgba([254, 1, 113, 1]), 127),
        (Rgba([255, 254, 253, 254]), Rgba([1, 2, 3, 127]), 137),
    ];
    for alpha in [0u8, 1, 2, 3, 17, 63, 64, 127, 128, 129, 193, 254, 255] {
        for color in [0u8, 1, 127, 128, 254, 255] {
            cases.push((
                Rgba([color, 255 - color, alpha ^ 0x5a, alpha]),
                Rgba([
                    255 - color,
                    color ^ 0xa5,
                    alpha.wrapping_mul(17),
                    255 - alpha,
                ]),
                alpha.wrapping_mul(73),
            ));
        }
    }
    for (width, height) in [(1_048_576, 1_048_576), (1_048_575, 1_048_573)] {
        for &(base, top, opacity) in &cases {
            let mut doc = new_document(width, height, Some(base), 0, POLICY).unwrap();
            let id = deterministic_id(&["gpu-wide-overlay"]);
            doc.roots.push(id);
            doc.layers.insert(
                id,
                Layer {
                    id,
                    name: "wide exact overlay".into(),
                    origin: (0, 0),
                    visible: true,
                    opacity,
                    filters: Default::default(),
                    raster: RasterLayer::new(width / 2, height / 4, top),
                },
            );
            let request = DisplayRequest::new(&doc, doc.canvas_rect(), 20).unwrap();
            let covered = top.with_opacity(opacity).source_over(base);
            let area = u128::from(width / 2) * u128::from(height / 4);
            let count = doc.canvas_rect().area();
            let sum_a = u128::from(base.a()) * (count - area) + u128::from(covered.a()) * area;
            let a = (sum_a + count / 2) / count;
            let pixel = if a == 0 {
                [0; 4]
            } else {
                let channel = |c| {
                    let sum = u128::from(base.0[c]) * u128::from(base.a()) * (count - area)
                        + u128::from(covered.0[c]) * u128::from(covered.a()) * area;
                    ((sum + sum_a / 2) / sum_a) as u8
                };
                [channel(0), channel(1), channel(2), a as u8]
            };
            let frame = gpu
                .render_display_texture(&doc, request, || Ok(()))
                .unwrap();
            assert_eq!(frame.stats.scaled_native_pixels, 0);
            assert_eq!(u128::from(frame.stats.scaled_uniform_pixels), count);
            assert_eq!(frame.readback(|| Ok(())).unwrap(), premultiplied(&pixel));
        }
    }
}

#[test]
fn scaled_cancellation_and_retired_epochs_never_return_or_reuse_partial_output() {
    use std::cell::Cell;
    let Some(gpu) = renderer() else { return };
    let doc = fixture();
    let request = DisplayRequest::new(&doc, Rect::new(0, 0, 8, 7).unwrap(), 2).unwrap();
    let expected = premultiplied(
        &render_display_cancellable(&doc, request, || Ok(()))
            .unwrap()
            .pixels,
    );
    drop(
        gpu.render_display_texture(&doc, request, || Ok(()))
            .unwrap(),
    );
    let calls = Cell::new(0);
    drop(
        gpu.render_display_texture(&doc, request, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    for fail in 1..=calls.get() {
        drop(
            gpu.render_display_texture(&doc, request, || Ok(()))
                .unwrap(),
        );
        let at = Cell::new(0);
        let result = gpu.render_display_texture(&doc, request, || {
            at.set(at.get() + 1);
            if at.get() == fail {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel GPU scaled display",
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(gpu.source_cache_usage().buffer_bytes, 0);
        assert_eq!(gpu.output_pool_usage(), 0);
        assert_eq!(
            gpu.render_display_texture(&doc, request, || Ok(()))
                .unwrap()
                .readback(|| Ok(()))
                .unwrap(),
            expected
        );
    }
    let frame = gpu
        .render_display_texture(&doc, request, || Ok(()))
        .unwrap();
    gpu.epoch().retire();
    assert!(
        gpu.render_display_texture(&doc, request, || Ok(()))
            .is_err()
    );
    assert!(frame.readback(|| Ok(())).is_err());
    assert_eq!(gpu.source_cache_usage().buffer_bytes, 0);
}

#[test]
fn dense_scaled_native_blocks_accumulate_carries_across_unaligned_source_splits() {
    for edge in [32, 64] {
        dense_carries(edge);
    }
}
fn dense_carries(edge: u32) {
    let Some(gpu) = renderer_edge(edge) else {
        return;
    };
    let mut doc = new_document(1029, 517, None, 0, POLICY).unwrap();
    let bytes: Vec<_> = (0..1029 * 517u32)
        .flat_map(|p| [255, 247 + (p % 9) as u8, 253 + (p % 3) as u8, 255])
        .collect();
    let id = deterministic_id(&["gpu-scaled-carry"]);
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "dense wide carries".into(),
            origin: (0, 0),
            visible: true,
            opacity: 255,
            filters: Default::default(),
            raster: RasterLayer::from_rgba(
                1029,
                517,
                Rgba::TRANSPARENT,
                &bytes,
                &mut Admission::new(15 * 65536),
            )
            .unwrap(),
        },
    );
    for level in [5, 9, 10, 20] {
        let request = DisplayRequest::new(&doc, doc.canvas_rect(), level).unwrap();
        let expected = premultiplied(
            &render_display_cancellable(&doc, request, || Ok(()))
                .unwrap()
                .pixels,
        );
        let frame = gpu
            .render_display_texture(&doc, request, || Ok(()))
            .unwrap();
        assert_eq!(frame.stats.scaled_native_pixels, 1029 * 517);
        assert_eq!(frame.stats.scaled_uniform_pixels, 0);
        assert_eq!(
            frame.readback(|| Ok(())).unwrap(),
            expected,
            "wide carry level {level}"
        );
    }
}
